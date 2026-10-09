//! The tier and the anonymous allow-lists, from `thunderforge-telemetry-policy`
//! (FR-009). Re-exported only: `tier_for` is the one decision (FR-006), and
//! the gateway reads the same constants.

#[allow(unused_imports)]
pub use thunderforge_telemetry_policy::{
    ANONYMOUS_RESOURCE_ATTRIBUTES, ANONYMOUS_SPAN_ATTRIBUTES, INSTRUMENTS,
    PROJECT_TELEMETRY_ENDPOINT, SERVER_ERROR_ATTRIBUTES, Tier, tier_for,
};
