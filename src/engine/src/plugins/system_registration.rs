use crate::systems::{BasicSystem, SystemRegistry};
use bevy::prelude::*;
use std::sync::Arc;

/// System Registration Plugin
///
/// Initializes the game system registry and registers built-in systems.
/// This plugin should be added early in the app startup sequence (before other systems
/// that depend on game rules).
pub struct SystemRegistrationPlugin;

impl Plugin for SystemRegistrationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, initialize_system_registry);
    }
}

/// Startup system that initializes the game system registry
fn initialize_system_registry(mut commands: Commands) {
    let mut registry = SystemRegistry::new();

    // Register built-in systems
    registry.register(Arc::new(BasicSystem));

    // Activate default system
    registry
        .activate("basic")
        .expect("Failed to activate basic system");

    // Add to Bevy world
    commands.insert_resource(registry);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_registry_plugin_builds() {
        let mut app = App::new();
        app.add_plugins(SystemRegistrationPlugin);

        app.update();

        let registry = app.world().resource::<SystemRegistry>();
        assert!(registry.get_active().is_some());
        assert_eq!(registry.active_id(), Some("basic"));
    }

    #[test]
    fn test_basic_system_registered() {
        let mut app = App::new();
        app.add_plugins(SystemRegistrationPlugin);

        app.update();

        let registry = app.world().resource::<SystemRegistry>();
        assert!(registry.get("basic").is_some());
    }
}
