//! Genie Engine Package
//!
//! WASM-compatible Bevy plugin for the Genie house system (spec 018-genie-house-system).
//! Mirrors packs/systems/dnd5e/engine's architecture.

pub mod plugin;

pub use plugin::{GeniePlugin, GenieSystem};

/// Genie Engine Version
pub const VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_loads() {
        // Smoke test: all modules load without panic
    }
}
