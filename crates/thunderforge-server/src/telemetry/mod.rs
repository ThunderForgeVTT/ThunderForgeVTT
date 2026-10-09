//! Spec 086: telemetry, the API half.
//!
//! This crate never decides a tier and never installs a provider. The app
//! (`apps/thunderforge/src/telemetry`) decides the tier with
//! `thunderforge_telemetry_policy::tier_for`, builds the values below, and
//! hands them to `AppState`. What lives here is what a request reads: the
//! instance id, the served browser config and the admin status.

pub mod instance_id;
pub mod redact;
pub mod served_config;
pub mod status;

pub use instance_id::{INSTANCE_ID_KEY, ensure_instance_id};
pub use served_config::BrowserTelemetry;
pub use status::TelemetryStatus;
/// Re-exported, never redefined: `tier_for` is the only decision (FR-006).
pub use thunderforge_telemetry_policy::{PROJECT_TELEMETRY_ENDPOINT, Tier};
