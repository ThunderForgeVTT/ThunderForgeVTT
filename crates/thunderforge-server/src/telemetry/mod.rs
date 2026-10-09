//! Spec 086: telemetry, the API half.
//!
//! This crate never decides a tier and never installs a provider. The app
//! (`apps/thunderforge/src/telemetry`) decides the tier with
//! `thunderforge_telemetry_policy::tier_for`, builds the values below, and
//! hands them to `AppState`. What lives here is what a request reads: the
//! instance id, the served browser config and the admin status.

pub mod event_names;
pub mod graphql_extension;
pub mod instance_id;
pub mod instruments;
pub mod pool_events;
pub mod redact;
pub mod served_config;
pub mod status;

/// The target of every span this crate opens for OpenTelemetry. The app's
/// Bunyan layer prints no START or END line for it, so stdout is unchanged
/// (SC-001), and its env filter always lets it through.
pub const SPAN_TARGET: &str = "thunderforge::otel";

/// Every field those spans declare, except `trace_id` and `span_id`. Bunyan
/// skips them, so a log line written inside one of these spans gains only
/// the two ids (FR-004, R17) and is otherwise what it was.
pub const SPAN_FIELDS: &[&str] = &[
    "otel.name",
    "otel.kind",
    "http.route",
    "http.request.method",
    "http.response.status_code",
    "graphql.operation.type",
    "graphql.root_field",
    "graphql.operation.name",
    "graphql.error.codes",
    "root_fields",
    "outcome",
    "event",
    "visibility",
    "world.id",
];

pub use instance_id::{INSTANCE_ID_KEY, ensure_instance_id};
pub use served_config::BrowserTelemetry;
pub use status::TelemetryStatus;
/// Re-exported, never redefined: `tier_for` is the only decision (FR-006).
pub use thunderforge_telemetry_policy::{PROJECT_TELEMETRY_ENDPOINT, Tier};
