//! Cypher System Bevy Plugin — mirrors packs/systems/dnd5e/engine/src/plugin.rs.

use bevy::prelude::*;
use std::sync::Arc;

pub struct CypherSystem;

impl CypherSystem {
    pub fn new() -> Self {
        Self
    }
    pub fn register() -> Arc<dyn GameSystemTrait> {
        Arc::new(Self)
    }
}

impl Default for CypherSystem {
    fn default() -> Self {
        Self::new()
    }
}

pub trait GameSystemTrait: Send + Sync {
    fn name(&self) -> String;
    fn version(&self) -> String;
}

impl GameSystemTrait for CypherSystem {
    fn name(&self) -> String {
        "Cypher System".to_string()
    }
    fn version(&self) -> String {
        "0.1.0".to_string()
    }
}

pub struct CypherPlugin;

impl Plugin for CypherPlugin {
    fn build(&self, _app: &mut App) {
        // No system-specific Bevy systems/resources yet.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_identity() {
        let system = CypherSystem;
        assert_eq!(system.name(), "Cypher System");
    }
}
