//! Pathfinder Second Edition (Remaster) Bevy Plugin — mirrors packs/systems/dnd5e/engine/src/plugin.rs.

use bevy::prelude::*;
use std::sync::Arc;

pub struct Pathfinder2eSystem;

impl Pathfinder2eSystem {
    pub fn new() -> Self {
        Self
    }
    pub fn register() -> Arc<dyn GameSystemTrait> {
        Arc::new(Self)
    }
}

impl Default for Pathfinder2eSystem {
    fn default() -> Self {
        Self::new()
    }
}

pub trait GameSystemTrait: Send + Sync {
    fn name(&self) -> String;
    fn version(&self) -> String;
}

impl GameSystemTrait for Pathfinder2eSystem {
    fn name(&self) -> String {
        "Pathfinder Second Edition (Remaster)".to_string()
    }
    fn version(&self) -> String {
        "0.1.0".to_string()
    }
}

pub struct Pathfinder2ePlugin;

impl Plugin for Pathfinder2ePlugin {
    fn build(&self, _app: &mut App) {
        // No system-specific Bevy systems/resources yet.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_identity() {
        let system = Pathfinder2eSystem;
        assert_eq!(system.name(), "Pathfinder Second Edition (Remaster)");
    }
}
