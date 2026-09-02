//! Genie Bevy Plugin
//!
//! Genie's system identity and rule calculations. Mirrors
//! packs/systems/dnd5e/engine/src/plugin.rs's structure.

use bevy::prelude::*;
use std::sync::Arc;

/// Genie System rule calculations.
pub struct GenieSystem;

impl GenieSystem {
    pub fn new() -> Self {
        Self
    }

    /// Register this system with the Bevy world
    pub fn register() -> Arc<Self> {
        Arc::new(Self)
    }

    pub fn name(&self) -> String {
        "Genie".to_string()
    }

    pub fn version(&self) -> String {
        "0.1.0".to_string()
    }
}

impl Default for GenieSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Bevy Plugin for the Genie System.
///
/// Session-loop state (Session Wish Pool, Doom Clock, Puzzle Clocks, Session
/// Resources — spec 018 data-model.md) is explicitly NOT registered here: it is
/// server/React-owned world state, not canvas simulation state, per Constitution
/// Principle I and plan.md's Constitution Check. This plugin only covers what
/// belongs in the ECS layer — none of that yet beyond the shared `grid.rs`
/// gridless-interaction change (src/engine/src/plugins/grid.rs, spec 018 US2),
/// which is not Genie-specific code.
pub struct GeniePlugin;

impl Plugin for GeniePlugin {
    fn build(&self, _app: &mut App) {
        // No Genie-specific Bevy systems/resources yet.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_identity() {
        let system = GenieSystem;
        assert_eq!(system.name(), "Genie");
        assert_eq!(system.version(), "0.1.0");
    }
}
