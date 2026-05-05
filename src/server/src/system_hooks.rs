//! System Hooks Registry
//!
//! This module manages the registration and invocation of system-specific hooks.
//! Hooks allow game systems to customize core VTT behavior without modifying the engine.
//!
//! Hooks are defined in system manifest files and loaded dynamically at runtime.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The contract for all system-provided hooks.
/// Systems implement these interfaces to customize VTT behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHookContract {
    /// Called when a token is about to move.
    /// Returns true to allow the move, false to reject it.
    pub on_token_move: Option<String>, // Function name in system module
    
    /// Called to compute derived token statistics (AC, initiative, etc.)
    /// Receives base stats, returns computed derived stats.
    pub compute_derived_stats: Option<String>,
    
    /// Called to validate a roll (e.g., "4d6", "2d20kh1").
    /// Returns parsed roll or error.
    pub validate_roll: Option<String>,
    
    /// Called to format damage output (e.g., "2d6+3" -> "2d6+3 (avg: 10)").
    pub format_damage: Option<String>,
    
    /// Called when a token's conditions change.
    /// Allows system to reject or modify conditions.
    pub on_condition_change: Option<String>,
    
    /// Called to check if a token can see another token (fog of war).
    pub check_token_visibility: Option<String>,
    
    /// Called to compute armor class for a token.
    pub compute_armor_class: Option<String>,
}

impl Default for SystemHookContract {
    fn default() -> Self {
        Self {
            on_token_move: None,
            compute_derived_stats: None,
            validate_roll: None,
            format_damage: None,
            on_condition_change: None,
            check_token_visibility: None,
            compute_armor_class: None,
        }
    }
}

/// Registry of all active system hooks per world/system.
/// This is stored in AppState and consulted during token operations.
#[derive(Debug, Clone, Default)]
pub struct SystemHookRegistry {
    /// Map of system_id -> SystemHookContract
    hooks: HashMap<String, SystemHookContract>,
}

impl SystemHookRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            hooks: HashMap::new(),
        }
    }

    /// Register hooks for a system
    pub fn register_system(&mut self, system_id: String, hooks: SystemHookContract) {
        self.hooks.insert(system_id, hooks);
    }

    /// Get hooks for a system (returns empty contract if not found)
    pub fn get_system_hooks(&self, system_id: &str) -> SystemHookContract {
        self.hooks
            .get(system_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Check if a hook is registered for a system
    pub fn has_hook(&self, system_id: &str, hook_name: &str) -> bool {
        if let Some(hooks) = self.hooks.get(system_id) {
            match hook_name {
                "onTokenMove" => hooks.on_token_move.is_some(),
                "computeDerivedStats" => hooks.compute_derived_stats.is_some(),
                "validateRoll" => hooks.validate_roll.is_some(),
                "formatDamage" => hooks.format_damage.is_some(),
                "onConditionChange" => hooks.on_condition_change.is_some(),
                "checkTokenVisibility" => hooks.check_token_visibility.is_some(),
                "computeArmorClass" => hooks.compute_armor_class.is_some(),
                _ => false,
            }
        } else {
            false
        }
    }

    /// Get the function name for a hook
    pub fn get_hook_function(&self, system_id: &str, hook_name: &str) -> Option<String> {
        self.hooks.get(system_id).and_then(|hooks| match hook_name {
            "onTokenMove" => hooks.on_token_move.clone(),
            "computeDerivedStats" => hooks.compute_derived_stats.clone(),
            "validateRoll" => hooks.validate_roll.clone(),
            "formatDamage" => hooks.format_damage.clone(),
            "onConditionChange" => hooks.on_condition_change.clone(),
            "checkTokenVisibility" => hooks.check_token_visibility.clone(),
            "computeArmorClass" => hooks.compute_armor_class.clone(),
            _ => None,
        })
    }

    /// Remove all hooks for a system (e.g., when system is uninstalled)
    pub fn unregister_system(&mut self, system_id: &str) {
        self.hooks.remove(system_id);
    }

    /// Clear all hooks
    pub fn clear(&mut self) {
        self.hooks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_retrieve_hooks() {
        let mut registry = SystemHookRegistry::new();
        let hooks = SystemHookContract {
            on_token_move: Some("validateMove".to_string()),
            compute_derived_stats: Some("d20ComputeStats".to_string()),
            ..Default::default()
        };

        registry.register_system("d20-5e".to_string(), hooks);

        let retrieved = registry.get_system_hooks("d20-5e");
        assert_eq!(retrieved.on_token_move, Some("validateMove".to_string()));
        assert_eq!(
            retrieved.compute_derived_stats,
            Some("d20ComputeStats".to_string())
        );
    }

    #[test]
    fn test_has_hook() {
        let mut registry = SystemHookRegistry::new();
        let hooks = SystemHookContract {
            on_token_move: Some("validateMove".to_string()),
            ..Default::default()
        };

        registry.register_system("d20-5e".to_string(), hooks);

        assert!(registry.has_hook("d20-5e", "onTokenMove"));
        assert!(!registry.has_hook("d20-5e", "validateRoll"));
        assert!(!registry.has_hook("unknown-system", "onTokenMove"));
    }

    #[test]
    fn test_unregister_system() {
        let mut registry = SystemHookRegistry::new();
        let hooks = SystemHookContract::default();
        registry.register_system("d20-5e".to_string(), hooks);

        assert!(registry.has_hook("d20-5e", "onTokenMove") || !registry.get_system_hooks("d20-5e").on_token_move.is_some());

        registry.unregister_system("d20-5e");
        let retrieved = registry.get_system_hooks("d20-5e");
        assert_eq!(retrieved.on_token_move, None);
    }
}
