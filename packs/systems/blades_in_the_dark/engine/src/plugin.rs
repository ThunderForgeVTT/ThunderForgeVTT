//! Blades in the Dark Bevy Plugin — mirrors packs/systems/dnd5e/engine/src/plugin.rs.

use bevy::prelude::*;
use std::sync::Arc;

pub struct BladesSystem;

impl BladesSystem {
    pub fn new() -> Self { Self }
    pub fn register() -> Arc<dyn GameSystemTrait> { Arc::new(Self) }
}

impl Default for BladesSystem {
    fn default() -> Self { Self::new() }
}

pub trait GameSystemTrait: Send + Sync {
    fn name(&self) -> String;
    fn version(&self) -> String;
}

impl GameSystemTrait for BladesSystem {
    fn name(&self) -> String { "Blades in the Dark".to_string() }
    fn version(&self) -> String { "0.1.0".to_string() }
}

pub struct BladesPlugin;

impl Plugin for BladesPlugin {
    fn build(&self, _app: &mut App) {
        // No system-specific Bevy systems/resources yet.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_identity() {
        let system = BladesSystem;
        assert_eq!(system.name(), "Blades in the Dark");
    }
}
