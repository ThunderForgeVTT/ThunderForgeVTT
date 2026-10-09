//! What telemetry may say, decided once (spec 086, FR-009, FR-038).
//!
//! The server's anonymous tier and the public telemetry gateway both read
//! these constants, so the two can never hold different lists. A name changes
//! here first, then in `specs/086-full-telemetry/contracts/`.
//!
//! The crate is pure: `std` only, no SDK, no IO, no clock reads and no
//! randomness. The caller passes the time in, and the gateway owns its
//! hashing key.

mod caps;
mod labels;
mod lists;
mod rate;
mod tier;

pub use caps::{CAP_DEFAULT, CAP_LOG_BODY, CAP_MESSAGE, CAP_STACK, cap_for, truncate_chars};
pub use labels::{
    DropReason, Source, client_version, country, instance_id_required, is_instance_id,
    reduce_user_agent, source_for,
};
pub use lists::{
    ANONYMOUS_RESOURCE_ATTRIBUTES, ANONYMOUS_SPAN_ATTRIBUTES, BROWSER_RECORD_ATTRIBUTES,
    BROWSER_RESOURCE_ATTRIBUTES, BROWSER_SPAN_ATTRIBUTES, GATEWAY_INSTRUMENTS, GATEWAY_LABELS,
    INSTRUMENTS, InstrumentKind, PUBLIC_METRIC_NAME_FILTER, Place, SERVER_ERROR_ATTRIBUTES,
    SERVER_METRIC_ATTRIBUTES, SERVER_SPAN_EVENT_ATTRIBUTES, SERVICE_NAMES, attribute_allowed,
    metric_allowed, print_instruments, prometheus_names,
};
pub use rate::TokenBucket;
pub use tier::{PROJECT_TELEMETRY_ENDPOINT, Tier, tier_for};
