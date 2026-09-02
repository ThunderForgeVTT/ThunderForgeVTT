//! Year Zero Engine Bevy Plugin — mirrors packs/systems/dnd5e/engine/src/plugin.rs.

use bevy::prelude::*;
use std::sync::Arc;

pub struct YzeSystem;

impl YzeSystem {
    pub fn new() -> Self {
        Self
    }
    pub fn register() -> Arc<Self> {
        Arc::new(Self)
    }

    pub fn name(&self) -> String {
        "Year Zero Engine".to_string()
    }
    pub fn version(&self) -> String {
        "0.1.0".to_string()
    }
}

impl Default for YzeSystem {
    fn default() -> Self {
        Self::new()
    }
}

pub struct YzePlugin;

impl Plugin for YzePlugin {
    fn build(&self, _app: &mut App) {
        // No system-specific Bevy systems/resources yet.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_identity() {
        let system = YzeSystem;
        assert_eq!(system.name(), "Year Zero Engine");
    }
}
