//! Spec 086: what the server exports, where, and what the anonymous tier lets
//! leave.

pub mod anonymous;
pub mod install;
pub mod settings;
pub mod tier;

pub use settings::TelemetrySettings;

#[cfg(test)]
mod tests;
