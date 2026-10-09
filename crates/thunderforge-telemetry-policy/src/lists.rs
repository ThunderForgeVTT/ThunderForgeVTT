//! The allow-lists and the instruments (FR-009, FR-038, R27).
//!
//! Each list is enumerated by a test, so an addition shows in review. Adding
//! to an anonymous list is a Principle VII change (docs/CONTRIBUTING.md).

/// The services the public intake accepts.
pub const SERVICE_NAMES: &[&str] = &[
    "thunderforge",
    "thunderforge-landing",
    "thunderforge-demo",
    "thunderforge-web",
];

/// What a server span keeps on the anonymous tier.
pub const ANONYMOUS_SPAN_ATTRIBUTES: &[&str] = &[
    "graphql.operation.type",
    "graphql.root_field",
    "graphql.error.codes",
    "root_fields",
    "outcome",
    "http.route",
    "http.request.method",
    "http.response.status_code",
    "event",
    "visibility",
    "state",
];

/// The server's resource on the anonymous tier, built by hand.
pub const ANONYMOUS_RESOURCE_ATTRIBUTES: &[&str] = &[
    "service.name",
    "service.version",
    "thunderforge.instance.id",
    "thunderforge.tier",
    "os.type",
    "host.arch",
    "deployment.environment",
];

/// The one log record the anonymous tier sends: `server.error`.
pub const SERVER_ERROR_ATTRIBUTES: &[&str] =
    &["event.name", "error.type", "error.message", "error.stack"];

/// The span event the anonymous tier keeps: `exception`, redacted.
pub const SERVER_SPAN_EVENT_ATTRIBUTES: &[&str] = &[
    "exception.type",
    "exception.message",
    "exception.stacktrace",
];

/// Every label an instrument in [`INSTRUMENTS`] carries.
pub const SERVER_METRIC_ATTRIBUTES: &[&str] = &[
    "operation_type",
    "root_field",
    "outcome",
    "code",
    "route",
    "method",
    "status_class",
    "state",
    "event",
    "visibility",
];

/// A browser's resource (contracts/browser-events.md).
pub const BROWSER_RESOURCE_ATTRIBUTES: &[&str] = &[
    "service.name",
    "service.version",
    "deployment.environment",
    "thunderforge.tier",
    "thunderforge.instance.id",
    "browser.family",
    "browser.major",
    "os.family",
    "device.mobile",
    "viewport.bucket",
    "device.memory.bucket",
    "device.cores.bucket",
];

/// A browser record's attributes: `ALLOWED_ATTRIBUTES` in
/// `packages/telemetry/src/allowList.ts`, which a test holds this to.
pub const BROWSER_RECORD_ATTRIBUTES: &[&str] = &[
    "event.name",
    "session.id",
    "t.ms",
    "route",
    "referrer.origin",
    "utm.source",
    "utm.medium",
    "utm.campaign",
    "nav.type",
    "step",
    "entry",
    "cta",
    "placement",
    "action",
    "root_field",
    "section",
    "error.source",
    "error.type",
    "error.message",
    "error.stack",
    "error.count",
    "reason",
    "stage",
    "metric",
    "value",
    "rating",
    "duration.ms",
    "dns.ms",
    "connect.ms",
    "ttfb.ms",
    "dom.ms",
    "load.ms",
    "transfer.bytes",
    "fps.p5",
    "fps.p50",
    "frame_ms.p95",
    "tokens.bucket",
    "internal_errors",
    "dropped",
];

/// A browser span's attributes: `SPAN_ATTRIBUTES` in `allowList.ts`.
pub const BROWSER_SPAN_ATTRIBUTES: &[&str] = &[
    "session.id",
    "route",
    "nav.type",
    "bytes",
    "resumed",
    "graphql.operation.type",
    "graphql.root_field",
];

/// What the gateway sets on every forwarded resource (FR-041).
pub const GATEWAY_LABELS: &[&str] = &[
    "thunderforge.ingress",
    "thunderforge.source",
    "thunderforge.origin.host",
    "thunderforge.instance.id",
    "thunderforge.client.version",
    "thunderforge.user_agent.family",
    "thunderforge.user_agent.major",
    "thunderforge.country",
];

/// An instrument's kind, which decides its Prometheus name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentKind {
    Counter,
    ObservableCounter,
    ObservableGauge,
    Histogram,
}

use InstrumentKind::{Counter, Histogram, ObservableCounter, ObservableGauge};

/// The server's instruments: name, kind, unit
/// (contracts/server-instruments.md). The gateway accepts exactly these
/// names (R27).
pub const INSTRUMENTS: &[(&str, InstrumentKind, &str)] = &[
    ("thunderforge.backplane.sent", ObservableCounter, "{event}"),
    (
        "thunderforge.backplane.dropped",
        ObservableCounter,
        "{event}",
    ),
    ("thunderforge.backplane.polls", ObservableCounter, "{poll}"),
    (
        "thunderforge.backplane.errors",
        ObservableCounter,
        "{error}",
    ),
    (
        "thunderforge.backplane.panics",
        ObservableCounter,
        "{panic}",
    ),
    (
        "thunderforge.backplane.timeouts",
        ObservableCounter,
        "{timeout}",
    ),
    ("thunderforge.backplane.cursor", ObservableGauge, "{event}"),
    (
        "thunderforge.subscriptions.opened",
        ObservableCounter,
        "{subscription}",
    ),
    (
        "thunderforge.subscriptions.refused",
        ObservableCounter,
        "{subscription}",
    ),
    (
        "thunderforge.subscriptions.delivered",
        ObservableCounter,
        "{event}",
    ),
    (
        "thunderforge.subscriptions.lagged",
        ObservableCounter,
        "{event}",
    ),
    (
        "thunderforge.websocket.sockets_open",
        ObservableGauge,
        "{socket}",
    ),
    (
        "thunderforge.world_channels.reaped",
        ObservableCounter,
        "{channel}",
    ),
    ("thunderforge.graphql.operation.duration", Histogram, "s"),
    ("thunderforge.graphql.errors", Counter, "{error}"),
    ("thunderforge.http.server.duration", Histogram, "s"),
    (
        "thunderforge.db.pool.connections",
        ObservableGauge,
        "{connection}",
    ),
    (
        "thunderforge.db.pool.max_connections",
        ObservableGauge,
        "{connection}",
    ),
    ("thunderforge.db.pool.checkout_wait", Histogram, "s"),
    (
        "thunderforge.db.pool.checkout_timeouts",
        Counter,
        "{timeout}",
    ),
    ("thunderforge.world_events", Counter, "{event}"),
    (
        "thunderforge.world_event.record_failures",
        Counter,
        "{event}",
    ),
    ("thunderforge.rolls", Counter, "{roll}"),
];

/// The gateway's own instruments (contracts/telemetry-gateway.md).
pub const GATEWAY_INSTRUMENTS: &[(&str, InstrumentKind, &str)] = &[
    (
        "thunderforge.telemetry_gateway.dropped",
        Counter,
        "{request}",
    ),
    (
        "thunderforge.telemetry_gateway.accepted",
        Counter,
        "{request}",
    ),
    (
        "thunderforge.telemetry_gateway.attributes_stripped",
        Counter,
        "{attribute}",
    ),
    (
        "thunderforge.telemetry_gateway.upstream.duration",
        Histogram,
        "s",
    ),
];

/// Mirrors the collector's `filter/public`. The gateway is stricter.
pub const PUBLIC_METRIC_NAME_FILTER: &str = r"^(thunderforge\.|http\.server\.|db\.client\.)";

/// Where an attribute sits in an OTLP payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Resource,
    Span,
    SpanEvent,
    LogRecord,
    DataPoint,
}

/// Whether `key` may stay at `place` in a payload from `service`.
pub fn attribute_allowed(service: &str, place: Place, key: &str) -> bool {
    let list: &[&str] = match (service, place) {
        ("thunderforge", Place::Resource) => ANONYMOUS_RESOURCE_ATTRIBUTES,
        ("thunderforge", Place::Span) => ANONYMOUS_SPAN_ATTRIBUTES,
        ("thunderforge", Place::SpanEvent) => SERVER_SPAN_EVENT_ATTRIBUTES,
        ("thunderforge", Place::LogRecord) => SERVER_ERROR_ATTRIBUTES,
        ("thunderforge", Place::DataPoint) => SERVER_METRIC_ATTRIBUTES,
        ("thunderforge-landing" | "thunderforge-demo" | "thunderforge-web", place) => match place {
            Place::Resource => BROWSER_RESOURCE_ATTRIBUTES,
            Place::Span => BROWSER_SPAN_ATTRIBUTES,
            Place::LogRecord => BROWSER_RECORD_ATTRIBUTES,
            Place::SpanEvent | Place::DataPoint => &[],
        },
        _ => &[],
    };
    list.contains(&key)
}

/// Whether the public intake accepts a metric of this name.
pub fn metric_allowed(name: &str) -> bool {
    INSTRUMENTS.iter().any(|(n, _, _)| *n == name)
}

/// The Prometheus series an instrument becomes (R4): dots to underscores, a
/// monotonic counter gains `_total`, unit `s` becomes `_seconds`, and a
/// histogram is its `_bucket`, `_sum` and `_count`.
pub fn prometheus_names(name: &str, kind: InstrumentKind, unit: &str) -> Vec<String> {
    let mut base = name.replace('.', "_");
    if unit == "s" {
        base.push_str("_seconds");
    }
    match kind {
        Counter | ObservableCounter => vec![format!("{base}_total")],
        ObservableGauge => vec![base],
        Histogram => ["bucket", "sum", "count"]
            .iter()
            .map(|s| format!("{base}_{s}"))
            .collect(),
    }
}

/// Prints every instrument's Prometheus series, one per line, for
/// `scripts/check-observability.mjs`.
pub fn print_instruments() {
    use std::io::Write as _;
    let mut out = std::io::stdout().lock();
    for (name, kind, unit) in INSTRUMENTS.iter().chain(GATEWAY_INSTRUMENTS) {
        for series in prometheus_names(name, *kind, unit) {
            let _ = writeln!(out, "prometheus: {series}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &'static [(&'static str, InstrumentKind, &'static str)]) -> Vec<&'static str> {
        list.iter().map(|(n, _, _)| *n).collect()
    }

    /// `cargo test -p thunderforge-telemetry-policy -- print_instruments --nocapture --exact`
    #[test]
    fn print_instruments() {
        super::print_instruments();
    }

    #[test]
    fn lists_are_enumerated() {
        assert_eq!(SERVICE_NAMES.len(), 4);
        assert_eq!(ANONYMOUS_SPAN_ATTRIBUTES.len(), 11);
        assert_eq!(ANONYMOUS_RESOURCE_ATTRIBUTES.len(), 7);
        assert_eq!(SERVER_ERROR_ATTRIBUTES.len(), 4);
        assert_eq!(BROWSER_RESOURCE_ATTRIBUTES.len(), 12);
        assert_eq!(BROWSER_RECORD_ATTRIBUTES.len(), 39);
        assert_eq!(BROWSER_SPAN_ATTRIBUTES.len(), 7);
        assert_eq!(GATEWAY_LABELS.len(), 8);
        assert_eq!(INSTRUMENTS.len(), 23);
        assert_eq!(GATEWAY_INSTRUMENTS.len(), 4);
    }

    #[test]
    fn the_anonymous_resource_names_nothing_identifying() {
        for key in ANONYMOUS_RESOURCE_ATTRIBUTES {
            assert_ne!(*key, "service.instance.id");
            assert_ne!(*key, "host.name");
            for prefix in ["process.", "container.", "k8s."] {
                assert!(!key.starts_with(prefix), "{key}");
            }
        }
    }

    #[test]
    fn instrument_names_are_unique_and_pass_the_public_filter() {
        let filter = regex::Regex::new(PUBLIC_METRIC_NAME_FILTER).unwrap();
        let mut all = names(INSTRUMENTS);
        all.extend(names(GATEWAY_INSTRUMENTS));
        for name in &all {
            assert!(name.starts_with("thunderforge."), "{name}");
            assert!(filter.is_match(name), "{name}");
        }
        let mut sorted = all.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), all.len(), "duplicate instrument name");
    }

    #[test]
    fn prometheus_conversion() {
        assert_eq!(
            prometheus_names("thunderforge.backplane.sent", ObservableCounter, "{event}"),
            ["thunderforge_backplane_sent_total"]
        );
        assert_eq!(
            prometheus_names("thunderforge.backplane.cursor", ObservableGauge, "{event}"),
            ["thunderforge_backplane_cursor"]
        );
        assert_eq!(
            prometheus_names("thunderforge.http.server.duration", Histogram, "s"),
            [
                "thunderforge_http_server_duration_seconds_bucket",
                "thunderforge_http_server_duration_seconds_sum",
                "thunderforge_http_server_duration_seconds_count",
            ]
        );
    }

    /// SC-015: the gateway and the browser hold one list.
    #[test]
    fn browser_record_attributes_equal_the_packages_allow_list() {
        let source = include_str!("../../../packages/telemetry/src/allowList.ts");
        let block = source
            .split("export const ALLOWED_ATTRIBUTES = [")
            .nth(1)
            .and_then(|rest| rest.split("] as const").next())
            .expect("ALLOWED_ATTRIBUTES in allowList.ts");
        let keys: Vec<&str> = block
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with('"'))
            .map(|l| l.trim_end_matches(',').trim_matches('"'))
            .collect();
        assert_eq!(keys, BROWSER_RECORD_ATTRIBUTES);
        let spans = source
            .split("export const SPAN_ATTRIBUTES = [")
            .nth(1)
            .and_then(|rest| rest.split("] as const").next())
            .expect("SPAN_ATTRIBUTES in allowList.ts");
        let span_keys: Vec<&str> = spans
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with('"'))
            .map(|l| l.trim_end_matches(',').trim_matches('"'))
            .collect();
        assert_eq!(span_keys, BROWSER_SPAN_ATTRIBUTES);
    }

    #[test]
    fn attribute_allowed_by_place_and_service() {
        assert!(attribute_allowed("thunderforge", Place::Span, "http.route"));
        assert!(!attribute_allowed("thunderforge", Place::Span, "url.path"));
        assert!(!attribute_allowed(
            "thunderforge",
            Place::Span,
            "graphql.operation.name"
        ));
        assert!(attribute_allowed(
            "thunderforge",
            Place::Resource,
            "thunderforge.instance.id"
        ));
        assert!(!attribute_allowed(
            "thunderforge",
            Place::Resource,
            "host.name"
        ));
        assert!(attribute_allowed(
            "thunderforge",
            Place::LogRecord,
            "error.message"
        ));
        assert!(attribute_allowed(
            "thunderforge",
            Place::SpanEvent,
            "exception.stacktrace"
        ));
        assert!(attribute_allowed(
            "thunderforge",
            Place::DataPoint,
            "root_field"
        ));
        assert!(!attribute_allowed(
            "thunderforge",
            Place::DataPoint,
            "world.id"
        ));
        assert!(attribute_allowed(
            "thunderforge-demo",
            Place::LogRecord,
            "step"
        ));
        assert!(attribute_allowed(
            "thunderforge-web",
            Place::Resource,
            "viewport.bucket"
        ));
        assert!(attribute_allowed(
            "thunderforge-landing",
            Place::Span,
            "route"
        ));
        assert!(!attribute_allowed(
            "thunderforge-landing",
            Place::DataPoint,
            "route"
        ));
        assert!(!attribute_allowed(
            "thunderforge-demo",
            Place::LogRecord,
            "chat.text"
        ));
        assert!(!attribute_allowed(
            "someone-else",
            Place::Resource,
            "service.name"
        ));
    }

    #[test]
    fn metric_allowed_is_exactly_the_instruments() {
        assert!(metric_allowed("thunderforge.rolls"));
        assert!(!metric_allowed("thunderforge.anything"));
        assert!(!metric_allowed("http.server.duration"));
        assert!(!metric_allowed("thunderforge.telemetry_gateway.dropped"));
    }
}
