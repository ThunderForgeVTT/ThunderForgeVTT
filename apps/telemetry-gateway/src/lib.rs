//! The public telemetry intake (spec 086 US9, contracts/telemetry-gateway.md).
//!
//! Every OTLP batch posted to `https://telemetry.thunderforge.dev` passes
//! through here before the public collector sees it: the policy crate's lists
//! decide what stays, the gateway labels where it came from, and the token
//! buckets bound what one address or one instance can send. No address leaves
//! this process.

pub mod config;
pub mod intake;
pub mod limits;
pub mod metrics;
pub mod router;
pub mod upstream;

pub use config::Config;
pub use router::{AppState, app};

#[cfg(test)]
mod fixture_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_drops;
#[cfg(test)]
mod tests_support;
