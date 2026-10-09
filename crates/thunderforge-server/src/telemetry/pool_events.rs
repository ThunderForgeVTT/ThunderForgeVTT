//! The database pool's instruments (R18, FR-013).
//!
//! r2d2 tells an event handler how long each checkout waited and when one
//! timed out; the connection counts are read from `pool.state()` only when
//! a reader collects.

use diesel::r2d2::event::{CheckoutEvent, TimeoutEvent};
use diesel::r2d2::{HandleEvent, ManageConnection, Pool};
use opentelemetry::KeyValue;
use opentelemetry::metrics::Meter;

use super::instruments::{Recorders, recorders};

/// Set with `Builder::event_handler`. The pool is built before telemetry is
/// installed, so by default the recorders are looked up at each event.
#[derive(Clone, Copy, Default)]
pub struct PoolEvents {
    fixed: Option<&'static Recorders>,
}

impl PoolEvents {
    /// Records into `recorders`; for tests.
    pub fn with(recorders: &'static Recorders) -> Self {
        Self {
            fixed: Some(recorders),
        }
    }

    fn recorders(&self) -> Option<&'static Recorders> {
        self.fixed.or_else(recorders)
    }
}

impl std::fmt::Debug for PoolEvents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PoolEvents")
    }
}

impl HandleEvent for PoolEvents {
    fn handle_checkout(&self, event: CheckoutEvent) {
        if let Some(r) = self.recorders() {
            r.pool_checkout_wait
                .record(event.duration().as_secs_f64(), &[]);
        }
    }

    fn handle_timeout(&self, _event: TimeoutEvent) {
        if let Some(r) = self.recorders() {
            r.pool_checkout_timeouts.add(1, &[]);
        }
    }
}

/// The pool's connection gauges, read from `pool.state()` when collected.
pub fn observe_pool<M: ManageConnection>(meter: &Meter, pool: Pool<M>) {
    let max = pool.max_size();
    meter
        .u64_observable_gauge("thunderforge.db.pool.connections")
        .with_unit("{connection}")
        .with_callback(move |o| {
            let state = pool.state();
            let idle = u64::from(state.idle_connections);
            let in_use = u64::from(state.connections).saturating_sub(idle);
            o.observe(idle, &[KeyValue::new("state", "idle")]);
            o.observe(in_use, &[KeyValue::new("state", "in_use")]);
        })
        .build();
    meter
        .u64_observable_gauge("thunderforge.db.pool.max_connections")
        .with_unit("{connection}")
        .with_callback(move |o| o.observe(u64::from(max), &[]))
        .build();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::instruments::testing::TestMeter;
    use std::time::Duration;
    use thunderforge_telemetry_policy::INSTRUMENTS;

    /// A pool of nothing: no database, and every checkout succeeds at once.
    #[derive(Debug)]
    struct Nothing;

    impl ManageConnection for Nothing {
        type Connection = ();
        type Error = std::io::Error;
        fn connect(&self) -> Result<(), Self::Error> {
            Ok(())
        }
        fn is_valid(&self, _: &mut ()) -> Result<(), Self::Error> {
            Ok(())
        }
        fn has_broken(&self, _: &mut ()) -> bool {
            false
        }
    }

    fn pool(events: PoolEvents) -> Pool<Nothing> {
        Pool::builder()
            .max_size(1)
            .min_idle(Some(1))
            .connection_timeout(Duration::from_millis(50))
            .event_handler(Box::new(events))
            .build(Nothing)
            .expect("pool")
    }

    #[test]
    fn a_checkout_records_its_wait_and_a_timeout_counts() {
        let m = TestMeter::new();
        let r: &'static Recorders = Box::leak(Box::new(Recorders::new(&m.meter)));
        let pool = pool(PoolEvents::with(r));
        let held = pool.get().expect("first checkout");
        assert!(pool.get().is_err(), "the only connection is held");
        drop(held);
        let seen = m.collect();
        assert_eq!(seen["thunderforge.db.pool.checkout_wait"], 1.0);
        assert_eq!(seen["thunderforge.db.pool.checkout_timeouts"], 1.0);
    }

    #[test]
    fn the_gauges_read_the_pool_state() {
        let m = TestMeter::new();
        let pool = pool(PoolEvents::default());
        observe_pool(&m.meter, pool.clone());
        let held = pool.get().expect("checkout");
        let seen = m.collect();
        assert_eq!(seen["thunderforge.db.pool.connections{state=in_use}"], 1.0);
        assert_eq!(seen["thunderforge.db.pool.connections{state=idle}"], 0.0);
        assert_eq!(seen["thunderforge.db.pool.max_connections"], 1.0);
        drop(held);
        let seen = m.collect();
        assert_eq!(seen["thunderforge.db.pool.connections{state=idle}"], 1.0);
    }

    #[test]
    fn the_gauge_units_are_the_policys() {
        for name in [
            "thunderforge.db.pool.connections",
            "thunderforge.db.pool.max_connections",
        ] {
            let (_, _, unit) = INSTRUMENTS
                .iter()
                .find(|(n, _, _)| *n == name)
                .expect("in the policy");
            assert_eq!(*unit, "{connection}", "{name}");
        }
    }
}
