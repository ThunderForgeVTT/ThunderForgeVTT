//! Core game system trait and registry
//!
//! This module defines the extensible architecture for loading game systems
//! (D&D 5e, Pathfinder, Fate, custom rulesets, etc).
//!
//! All game systems must implement the GameSystem trait, which provides:
//! - Ability definitions (Strength, Dexterity, etc)
//! - Skill definitions and associations
//! - Token validation rules
//! - Derived stat calculations
//! - Movement rules
//!
//! The SystemRegistry manages available systems and tracks the currently active system.

use bevy::prelude::Resource;
use std::collections::HashMap;
use std::sync::Arc;

/// Skill definition with ability association
#[derive(Clone, Debug, PartialEq)]
pub struct SkillDefinition {
    pub name: String,
    pub ability: String,
    pub description: Option<String>,
}

/// Derived statistics calculated from base token attributes
#[derive(Clone, Debug, Default)]
pub struct DerivedStats {
    pub effective_health: i32,
    pub armor_class: i32,
    pub initiative: i32,
    pub proficiency_bonus: Option<i32>,
}

/// Core trait for all game systems
///
/// Implement this trait to add support for a new game system (D&D 5e, Pathfinder, Fate, etc).
/// Systems are registered at engine startup and can be switched between worlds.
pub trait GameSystem: Send + Sync {
    /// Unique system identifier (e.g., "dnd5e", "pathfinder2e", "basic")
    fn id(&self) -> &'static str;

    /// Human-readable system name (e.g., "Dungeons & Dragons 5th Edition")
    fn name(&self) -> &'static str;

    /// List of ability score names this system uses (e.g., ["Strength", "Dexterity", ...])
    fn ability_names(&self) -> Vec<&'static str>;

    /// List of skill definitions with ability associations
    fn skill_definitions(&self) -> Vec<SkillDefinition>;

    /// Validate that a token's attributes are valid for this system
    /// Returns Ok(()) if valid, Err(reason) if invalid
    fn validate_token(&self, token: &crate::components::Token) -> Result<(), String>;

    /// Calculate derived statistics (health, AC, initiative, etc) from base token attributes
    fn calculate_derived_stats(&self, token: &crate::components::Token) -> DerivedStats;

    /// Calculate movement cost for a distance
    /// Basic: return distance (1:1 cost)
    /// D&D 5e: apply diagonal reduction, terrain, etc.
    fn calculate_movement_cost(&self, distance: f32) -> f32;
}

/// Registry of available game systems
#[derive(Clone, Resource)]
pub struct SystemRegistry {
    systems: Arc<HashMap<String, Arc<dyn GameSystem>>>,
    active_system: Option<String>,
}

impl SystemRegistry {
    /// Create a new empty system registry
    pub fn new() -> Self {
        SystemRegistry {
            systems: Arc::new(HashMap::new()),
            active_system: None,
        }
    }

    /// Register a new game system
    pub fn register(&mut self, system: Arc<dyn GameSystem>) {
        Arc::get_mut(&mut self.systems)
            .expect("Cannot register system while other references exist")
            .insert(system.id().to_string(), system);
    }

    /// Activate a system by ID
    pub fn activate(&mut self, system_id: &str) -> Result<(), String> {
        if self.systems.contains_key(system_id) {
            self.active_system = Some(system_id.to_string());
            Ok(())
        } else {
            Err(format!("System '{}' not found", system_id))
        }
    }

    /// Get the currently active system
    pub fn get_active(&self) -> Option<Arc<dyn GameSystem>> {
        self.active_system
            .as_ref()
            .and_then(|id| self.systems.get(id).cloned())
    }

    /// Get a system by ID
    pub fn get(&self, system_id: &str) -> Option<Arc<dyn GameSystem>> {
        self.systems.get(system_id).cloned()
    }

    /// List all available system IDs
    pub fn list_available(&self) -> Vec<String> {
        self.systems.keys().cloned().collect()
    }

    /// Get the ID of the active system (if any)
    pub fn active_id(&self) -> Option<&str> {
        self.active_system.as_deref()
    }
}

impl Default for SystemRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSystem;
    impl GameSystem for TestSystem {
        fn id(&self) -> &'static str {
            "test"
        }
        fn name(&self) -> &'static str {
            "Test System"
        }
        fn ability_names(&self) -> Vec<&'static str> {
            vec![]
        }
        fn skill_definitions(&self) -> Vec<SkillDefinition> {
            vec![]
        }
        fn validate_token(&self, _: &crate::components::Token) -> Result<(), String> {
            Ok(())
        }
        fn calculate_derived_stats(&self, _: &crate::components::Token) -> DerivedStats {
            DerivedStats::default()
        }
        fn calculate_movement_cost(&self, distance: f32) -> f32 {
            distance
        }
    }

    #[test]
    fn test_system_registry_register() {
        let mut registry = SystemRegistry::new();
        registry.register(Arc::new(TestSystem));
        assert!(registry.get("test").is_some());
    }

    #[test]
    fn test_system_registry_activate() {
        let mut registry = SystemRegistry::new();
        registry.register(Arc::new(TestSystem));
        assert!(registry.activate("test").is_ok());
        assert!(registry.get_active().is_some());
        assert_eq!(registry.active_id(), Some("test"));
    }

    #[test]
    fn test_system_registry_activate_nonexistent() {
        let mut registry = SystemRegistry::new();
        assert!(registry.activate("nonexistent").is_err());
    }

    #[test]
    fn test_system_registry_list_available() {
        let mut registry = SystemRegistry::new();
        registry.register(Arc::new(TestSystem));
        let available = registry.list_available();
        assert_eq!(available.len(), 1);
        assert!(available.contains(&"test".to_string()));
    }
}
