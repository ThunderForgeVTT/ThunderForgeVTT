//! Spec 086: what the server exports, where, and what the anonymous tier lets
//! leave.

pub mod anonymous;
pub mod bunyan;
pub mod http;
pub mod install;
pub mod settings;
pub mod startup_line;
pub mod tier;

pub use settings::TelemetrySettings;

#[cfg(test)]
mod disclosure_tests;
#[cfg(test)]
mod pipeline_tests;
#[cfg(test)]
mod tests;
