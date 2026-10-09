//! The server's instruments (contracts/server-instruments.md, FR-010).
//!
//! The counters the server already keeps stay where they are, as atomics; the
//! observable instruments here only read them when a reader collects, so the
//! hot paths gain nothing. What has no atomic yet (GraphQL, HTTP, the pool
//! wait, world events, rolls) is a synchronous instrument in [`Recorders`].
//!
//! Every name and unit comes from `thunderforge_telemetry_policy::INSTRUMENTS`,
//! so a name the policy does not list cannot be built: `unit` panics in the
//! tests below before it could ship.
//!
//! [`register`] runs once, after the app installs the global meter provider.
//! Until then [`recorders`] is `None` and every call site records nothing.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use opentelemetry::KeyValue;
use opentelemetry::metrics::{Counter, Histogram, Meter};
use thunderforge_pg::DeliveryMetrics;
use thunderforge_telemetry_policy::INSTRUMENTS;

use crate::graphql::subscription_metrics as subs;

/// The delivery loop's counters. One per process: the listener takes this
/// rather than building its own, so the instruments read what it writes.
pub static DELIVERY: OnceLock<Arc<DeliveryMetrics>> = OnceLock::new();

/// Idle world channels the reaper dropped (R19).
pub static WORLD_CHANNELS_REAPED: AtomicU64 = AtomicU64::new(0);

/// The delivery counters, created on first use.
pub fn delivery() -> Arc<DeliveryMetrics> {
    DELIVERY.get_or_init(Default::default).clone()
}

/// The unit the policy gives `name`.
fn unit(name: &str) -> &'static str {
    INSTRUMENTS
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, _, u)| *u)
        .unwrap_or_else(|| panic!("{name} is not in the policy's INSTRUMENTS"))
}

/// The synchronous instruments, built once from the installed meter.
pub struct Recorders {
    pub world_events: Counter<u64>,
    pub world_event_record_failures: Counter<u64>,
    pub rolls: Counter<u64>,
    pub graphql_duration: Histogram<f64>,
    pub graphql_errors: Counter<u64>,
    pub http_duration: Histogram<f64>,
    pub pool_checkout_wait: Histogram<f64>,
    pub pool_checkout_timeouts: Counter<u64>,
}

impl Recorders {
    pub fn new(meter: &Meter) -> Self {
        let counter = |name: &'static str| meter.u64_counter(name).with_unit(unit(name)).build();
        let histogram =
            |name: &'static str| meter.f64_histogram(name).with_unit(unit(name)).build();
        Self {
            world_events: counter("thunderforge.world_events"),
            world_event_record_failures: counter("thunderforge.world_event.record_failures"),
            rolls: counter("thunderforge.rolls"),
            graphql_duration: histogram("thunderforge.graphql.operation.duration"),
            graphql_errors: counter("thunderforge.graphql.errors"),
            http_duration: histogram("thunderforge.http.server.duration"),
            pool_checkout_wait: histogram("thunderforge.db.pool.checkout_wait"),
            pool_checkout_timeouts: counter("thunderforge.db.pool.checkout_timeouts"),
        }
    }
}

static RECORDERS: OnceLock<Recorders> = OnceLock::new();

/// The installed recorders, or `None` before [`register`].
pub fn recorders() -> Option<&'static Recorders> {
    RECORDERS.get()
}

/// The observable instruments over the existing atomics. Their callbacks
/// live with `meter`'s provider; the handles can be dropped.
pub fn observe_atomics(meter: &Meter) {
    let counters: [(&'static str, fn() -> u64); 11] = [
        ("thunderforge.backplane.sent", || {
            delivery().sent.load(Ordering::Relaxed)
        }),
        ("thunderforge.backplane.dropped", || {
            delivery().dropped.load(Ordering::Relaxed)
        }),
        ("thunderforge.backplane.polls", || {
            delivery().polls.load(Ordering::Relaxed)
        }),
        ("thunderforge.backplane.errors", || {
            delivery().errors.load(Ordering::Relaxed)
        }),
        ("thunderforge.backplane.panics", || {
            delivery().panics.load(Ordering::Relaxed)
        }),
        ("thunderforge.backplane.timeouts", || {
            delivery().timeouts.load(Ordering::Relaxed)
        }),
        ("thunderforge.subscriptions.opened", || {
            subs::OPENED.load(Ordering::Relaxed)
        }),
        ("thunderforge.subscriptions.refused", || {
            subs::REFUSED.load(Ordering::Relaxed)
        }),
        ("thunderforge.subscriptions.delivered", || {
            subs::DELIVERED.load(Ordering::Relaxed)
        }),
        ("thunderforge.subscriptions.lagged", || {
            subs::LAGGED_EVENTS.load(Ordering::Relaxed)
        }),
        ("thunderforge.world_channels.reaped", || {
            WORLD_CHANNELS_REAPED.load(Ordering::Relaxed)
        }),
    ];
    for (name, read) in counters {
        meter
            .u64_observable_counter(name)
            .with_unit(unit(name))
            .with_callback(move |o| o.observe(read(), &[]))
            .build();
    }

    let gauges: [(&'static str, fn() -> i64); 2] = [
        ("thunderforge.backplane.cursor", || {
            delivery().cursor.load(Ordering::Relaxed)
        }),
        ("thunderforge.websocket.sockets_open", || {
            subs::SOCKETS_OPEN.load(Ordering::Relaxed)
        }),
    ];
    for (name, read) in gauges {
        meter
            .i64_observable_gauge(name)
            .with_unit(unit(name))
            .with_callback(move |o| o.observe(read(), &[]))
            .build();
    }
}

/// Builds every instrument on `meter` and keeps the recorders. Called once,
/// after the global provider is installed; a second call keeps the first.
pub fn register(meter: &Meter) {
    if RECORDERS.set(Recorders::new(meter)).is_ok() {
        observe_atomics(meter);
    }
}

/// `true` for the world events that are rolls (codes 36 and 37).
pub fn is_roll(code: i32) -> bool {
    code == crate::world_events::EVENT_CODE_ROLL_MADE
        || code == crate::world_events::EVENT_CODE_ROLL_REVEALED
}

/// Counts one `record_world_event` call: `ok` or a failure, and, for a roll,
/// the roll with its visibility read from the payload. Returns that
/// visibility, for the span.
pub fn count_world_event(
    r: &Recorders,
    code: i32,
    payload: Option<&serde_json::Value>,
    ok: bool,
) -> Option<&'static str> {
    let event = super::event_names::event_name(code);
    let attrs = [KeyValue::new("event", event)];
    if !ok {
        r.world_event_record_failures.add(1, &attrs);
        return None;
    }
    r.world_events.add(1, &attrs);
    if is_roll(code) {
        // Read as the server reads it: an unrecognised value is `gm_only`.
        let stored = payload
            .and_then(|p| p.get("visibility"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let visibility = crate::rolls::visibility::Visibility::parse(stored).as_str();
        r.rolls.add(
            1,
            &[
                KeyValue::new("event", event),
                KeyValue::new("visibility", visibility),
            ],
        );
        return Some(visibility);
    }
    None
}

#[cfg(test)]
pub(crate) mod testing {
    //! An in-memory meter, and its collected values by series and attributes.

    use opentelemetry::metrics::{Meter, MeterProvider};
    use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};
    use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};
    use std::collections::BTreeMap;

    pub struct TestMeter {
        provider: SdkMeterProvider,
        exporter: InMemoryMetricExporter,
        pub meter: Meter,
    }

    impl TestMeter {
        pub fn new() -> Self {
            let exporter = InMemoryMetricExporter::default();
            let provider = SdkMeterProvider::builder()
                .with_reader(PeriodicReader::builder(exporter.clone()).build())
                .build();
            let meter = provider.meter("thunderforge");
            Self {
                provider,
                exporter,
                meter,
            }
        }

        /// Every data point, keyed by `name{k=v,...}`: a sum's or gauge's
        /// value, or a histogram's count.
        pub fn collect(&self) -> BTreeMap<String, f64> {
            self.exporter.reset();
            self.provider.force_flush().expect("flush");
            let mut out = BTreeMap::new();
            let batches = self.exporter.get_finished_metrics().expect("metrics");
            let Some(last) = batches.last() else {
                return out;
            };
            for scope in last.scope_metrics() {
                for metric in scope.metrics() {
                    let name = metric.name();
                    let mut put = |attrs: Vec<String>, v: f64| {
                        let key = if attrs.is_empty() {
                            name.to_string()
                        } else {
                            format!("{name}{{{}}}", attrs.join(","))
                        };
                        out.insert(key, v);
                    };
                    macro_rules! points {
                        ($data:expr) => {
                            match $data {
                                MetricData::Sum(s) => {
                                    for p in s.data_points() {
                                        put(labels(p.attributes()), p.value() as f64)
                                    }
                                }
                                MetricData::Gauge(g) => {
                                    for p in g.data_points() {
                                        put(labels(p.attributes()), p.value() as f64)
                                    }
                                }
                                MetricData::Histogram(h) => {
                                    for p in h.data_points() {
                                        put(labels(p.attributes()), p.count() as f64)
                                    }
                                }
                                MetricData::ExponentialHistogram(_) => {}
                            }
                        };
                    }
                    match metric.data() {
                        AggregatedMetrics::U64(d) => points!(d),
                        AggregatedMetrics::I64(d) => points!(d),
                        AggregatedMetrics::F64(d) => points!(d),
                    }
                }
            }
            out
        }
    }

    /// Stops the reader's thread with the test, not at the binary's exit.
    impl Drop for TestMeter {
        fn drop(&mut self) {
            let _ = self.provider.shutdown();
        }
    }

    fn labels<'a>(attrs: impl Iterator<Item = &'a opentelemetry::KeyValue>) -> Vec<String> {
        let mut v: Vec<String> = attrs
            .map(|kv| format!("{}={}", kv.key, kv.value.as_str()))
            .collect();
        v.sort();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::testing::TestMeter;
    use super::*;
    use opentelemetry::KeyValue;
    use thunderforge_telemetry_policy::InstrumentKind;

    #[test]
    fn every_observable_series_is_reported() {
        let m = TestMeter::new();
        observe_atomics(&m.meter);
        let seen = m.collect();
        for (name, kind, _) in INSTRUMENTS {
            if matches!(
                kind,
                InstrumentKind::ObservableCounter | InstrumentKind::ObservableGauge
            ) && !name.starts_with("thunderforge.db.pool.")
            {
                assert!(seen.contains_key(*name), "{name} not reported: {seen:?}");
            }
        }
    }

    /// Other tests in this binary bump the same atomics, so a value is read
    /// between "before plus our bump" and "the atomic just after".
    fn between(m: &TestMeter, name: &str, before: u64, after: impl Fn() -> u64) {
        let got = m.collect()[name] as u64;
        assert!(
            before <= got && got <= after(),
            "{name}: {before} <= {got} <= {}",
            after()
        );
    }

    #[test]
    fn each_value_is_its_atomic() {
        let m = TestMeter::new();
        observe_atomics(&m.meter);

        let d = delivery();
        d.sent.fetch_add(7, Ordering::Relaxed);
        let before = d.sent.load(Ordering::Relaxed);
        between(&m, "thunderforge.backplane.sent", before, || {
            delivery().sent.load(Ordering::Relaxed)
        });

        subs::DELIVERED.fetch_add(3, Ordering::Relaxed);
        let before = subs::DELIVERED.load(Ordering::Relaxed);
        between(&m, "thunderforge.subscriptions.delivered", before, || {
            subs::DELIVERED.load(Ordering::Relaxed)
        });

        WORLD_CHANNELS_REAPED.fetch_add(2, Ordering::Relaxed);
        let before = WORLD_CHANNELS_REAPED.load(Ordering::Relaxed);
        between(&m, "thunderforge.world_channels.reaped", before, || {
            WORLD_CHANNELS_REAPED.load(Ordering::Relaxed)
        });

        d.cursor.store(4242, Ordering::Relaxed);
        let cursor = m.collect()["thunderforge.backplane.cursor"] as i64;
        // Only the listener moves the cursor, and it never runs in unit tests.
        assert_eq!(cursor, d.cursor.load(Ordering::Relaxed));
    }

    #[test]
    fn every_recorder_is_a_policy_instrument() {
        let m = TestMeter::new();
        let r = Recorders::new(&m.meter);
        let none: [KeyValue; 0] = [];
        r.world_events.add(1, &none);
        r.world_event_record_failures.add(1, &none);
        r.rolls.add(1, &none);
        r.graphql_duration.record(0.01, &none);
        r.graphql_errors.add(1, &none);
        r.http_duration.record(0.01, &none);
        r.pool_checkout_wait.record(0.01, &none);
        r.pool_checkout_timeouts.add(1, &none);
        let seen = m.collect();
        for (name, kind, _) in INSTRUMENTS {
            if matches!(kind, InstrumentKind::Counter | InstrumentKind::Histogram) {
                assert!(seen.contains_key(*name), "{name} has no recorder");
            }
        }
    }

    #[test]
    fn a_roll_counts_as_an_event_and_a_roll_with_its_visibility() {
        use crate::world_events::{EVENT_CODE_ROLL_MADE, EVENT_CODE_TOKEN_CHANGED};
        let m = TestMeter::new();
        let r = Recorders::new(&m.meter);
        let payload = serde_json::json!({"rollId": "x", "visibility": "gm_only"});
        count_world_event(&r, EVENT_CODE_ROLL_MADE, Some(&payload), true);
        count_world_event(&r, EVENT_CODE_TOKEN_CHANGED, None, true);
        count_world_event(&r, EVENT_CODE_TOKEN_CHANGED, None, false);
        let seen = m.collect();
        assert_eq!(seen["thunderforge.world_events{event=roll_made}"], 1.0);
        assert_eq!(seen["thunderforge.world_events{event=token_changed}"], 1.0);
        assert_eq!(
            seen["thunderforge.world_event.record_failures{event=token_changed}"],
            1.0
        );
        assert_eq!(
            seen["thunderforge.rolls{event=roll_made,visibility=gm_only}"],
            1.0
        );
        assert_eq!(
            seen.keys()
                .filter(|k| k.starts_with("thunderforge.rolls"))
                .count(),
            1,
            "a token change is not a roll"
        );
    }
}
