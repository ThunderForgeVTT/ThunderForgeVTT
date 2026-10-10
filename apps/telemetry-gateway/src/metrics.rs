//! The gateway's own four instruments (FR-045). No attribute holds an
//! address, an origin host, a country or an instance id.

use opentelemetry::KeyValue;
use opentelemetry::metrics::{Counter, Histogram, Meter};
use thunderforge_telemetry_policy::{DropReason, Source};

use crate::intake::Signal;

pub const DROPPED: &str = "thunderforge.telemetry_gateway.dropped";
pub const ACCEPTED: &str = "thunderforge.telemetry_gateway.accepted";
pub const ATTRIBUTES_STRIPPED: &str = "thunderforge.telemetry_gateway.attributes_stripped";
pub const UPSTREAM_DURATION: &str = "thunderforge.telemetry_gateway.upstream.duration";

/// Seconds, from 5 ms to the 5 s timeout and past it.
const DURATION_BOUNDARIES: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

#[derive(Clone)]
pub struct Metrics {
    dropped: Counter<u64>,
    accepted: Counter<u64>,
    stripped: Counter<u64>,
    upstream: Histogram<f64>,
}

impl Metrics {
    pub fn new(meter: &Meter) -> Self {
        Self {
            dropped: meter
                .u64_counter(DROPPED)
                .with_unit("{request}")
                .with_description("Requests, resources or records the gateway dropped, once per request per reason")
                .build(),
            accepted: meter
                .u64_counter(ACCEPTED)
                .with_unit("{request}")
                .with_description("Requests the gateway forwarded to the public collector")
                .build(),
            stripped: meter
                .u64_counter(ATTRIBUTES_STRIPPED)
                .with_unit("{attribute}")
                .with_description("Attributes removed for not being on the allow-list")
                .build(),
            upstream: meter
                .f64_histogram(UPSTREAM_DURATION)
                .with_unit("s")
                .with_description("How long the public collector took to answer")
                .with_boundaries(DURATION_BOUNDARIES.to_vec())
                .build(),
        }
    }

    pub fn dropped(&self, reason: DropReason) {
        self.dropped
            .add(1, &[KeyValue::new("reason", reason.as_str())]);
    }

    pub fn accepted(&self, signal: Signal, source: Source) {
        self.accepted.add(
            1,
            &[
                KeyValue::new("signal", signal.as_str()),
                KeyValue::new("source", source.as_str()),
            ],
        );
    }

    pub fn stripped(&self, signal: Signal, n: u64) {
        if n > 0 {
            self.stripped
                .add(n, &[KeyValue::new("signal", signal.as_str())]);
        }
    }

    pub fn upstream(&self, signal: Signal, ok: bool, seconds: f64) {
        self.upstream.record(
            seconds,
            &[
                KeyValue::new("signal", signal.as_str()),
                KeyValue::new("outcome", if ok { "ok" } else { "error" }),
            ],
        );
    }
}
