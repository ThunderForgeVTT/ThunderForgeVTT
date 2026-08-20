//! Derived Data Calculation System
//!
//! Computes derived game statistics from base token data.
//! This reduces network payload by never sending computed values over the wire.
//! All computations happen locally using the system hooks API.

use bevy::prelude::*;
use crate::components::*;

/// System to calculate derived stats whenever base token data changes
/// Uses Changed<Token> to detect updates from server
pub fn calculate_derived_stats(
    mut query: Query<(&Token, &mut DerivedStats), Changed<Token>>,
    system_hooks: Option<Res<SystemHooksRegistry>>,
) {
    for (token, mut derived) in query.iter_mut() {
        *derived = compute_derived_stats(token, system_hooks.as_deref());
    }
}

/// System to update derived stats when abilities change
pub fn calculate_ability_stats(
    mut query: Query<(&TokenAbilities, &mut DerivedStats), Changed<TokenAbilities>>,
    system_hooks: Option<Res<SystemHooksRegistry>>,
) {
    for (abilities, mut derived) in query.iter_mut() {
        // Recompute AC, initiative, etc. based on abilities
        compute_ability_derived_stats(abilities, &mut derived, system_hooks.as_deref());
    }
}

/// Compute all derived statistics from base token data
/// This is the core computation logic
pub fn compute_derived_stats(
    token: &Token,
    _hooks: Option<&SystemHooksRegistry>,
) -> DerivedStats {
    let mut stats = DerivedStats::default();
    
    // Health percentage and status
    if let (Some(health), Some(max_health)) = (token.health, token.max_health) {
        if max_health > 0 {
            stats.health_percentage = Some((health as f32 / max_health as f32) * 100.0);
        }
        stats.is_dead = health <= 0;
        stats.is_full_health = health >= max_health;
    }
    
    // Compute ability-based stats
    compute_ability_derived_stats(&token.abilities, &mut stats, _hooks);
    
    stats
}

/// Compute stats that depend on ability scores
/// This logic would be enhanced by system hooks in Phase 4.3
pub fn compute_ability_derived_stats(
    abilities: &TokenAbilities,
    stats: &mut DerivedStats,
    _hooks: Option<&SystemHooksRegistry>,
) {
    // Compute ability modifiers (D&D-style: (ability - 10) / 2)
    let ability_mods = compute_ability_modifiers(abilities);
    
    // Base AC: 10 + DEX modifier
    stats.armor_class = ability_mods.dex.map(|dex_mod| 10 + dex_mod);
    
    // Initiative: DEX modifier
    stats.initiative = ability_mods.dex;
    
    // Default movement speed (30 ft = 5 tiles per round in D&D)
    stats.movement_speed = Some(30);
    
    // Proficiency bonus (default +2, scales with level in real systems)
    stats.proficiency_bonus = Some(2);
    
    // Future: Hook system would customize these
    // if let Some(hooks) = hooks {
    //     if hooks.has_hook("d20-5e", "computeArmorClass") {
    //         // Call system hook
    //     }
    // }
}

/// Compute ability modifiers from ability scores
/// Uses D&D 5e formula: (ability - 10) / 2, rounded down
#[derive(Debug, Clone)]
pub struct AbilityModifiers {
    pub str: Option<i32>,
    pub dex: Option<i32>,
    pub con: Option<i32>,
    pub int: Option<i32>,
    pub wis: Option<i32>,
    pub cha: Option<i32>,
}

pub fn compute_ability_modifiers(abilities: &TokenAbilities) -> AbilityModifiers {
    AbilityModifiers {
        str: abilities.strength.map(|s| (s - 10) / 2),
        dex: abilities.dexterity.map(|d| (d - 10) / 2),
        con: abilities.constitution.map(|c| (c - 10) / 2),
        int: abilities.intelligence.map(|i| (i - 10) / 2),
        wis: abilities.wisdom.map(|w| (w - 10) / 2),
        cha: abilities.charisma.map(|c| (c - 10) / 2),
    }
}

/// Compute health percentage with custom logic
pub fn compute_health_percentage(health: Option<i32>, max_health: Option<i32>) -> Option<f32> {
    match (health, max_health) {
        (Some(h), Some(max)) if max > 0 => Some((h as f32 / max as f32) * 100.0),
        _ => None,
    }
}

/// Compute AC using D&D 5e formula
/// Default: 10 + DEX modifier
/// Can be enhanced with armor, magic items, etc. via hooks
pub fn compute_armor_class(abilities: &TokenAbilities) -> Option<i32> {
    abilities
        .dexterity
        .map(|dex| 10 + (dex - 10) / 2)
}

/// Compute initiative modifier (D&D style)
pub fn compute_initiative(abilities: &TokenAbilities) -> Option<i32> {
    abilities
        .dexterity
        .map(|dex| (dex - 10) / 2)
}

/// Compute movement speed
/// Different systems handle this differently
/// D&D: 30 feet (5 squares) per round
/// Pathfinder: 30 feet (8 squares) per round
/// Etc.
pub fn compute_movement_speed(_abilities: &TokenAbilities, _system: Option<&str>) -> i32 {
    // Default to 30 feet per round
    // System hooks would customize this
    30
}

/// Compute proficiency bonus (level-based in most systems)
/// This should ideally come from actor level, but we compute default here
pub fn compute_proficiency_bonus() -> i32 {
    // Default for level 1
    2
}

/// Check if token is dead/alive based on health
pub fn is_token_dead(health: Option<i32>) -> bool {
    match health {
        Some(h) => h <= 0,
        None => false, // Assume alive if no health tracking
    }
}

/// Check if token is at full health
pub fn is_token_full_health(health: Option<i32>, max_health: Option<i32>) -> bool {
    match (health, max_health) {
        (Some(h), Some(max)) => h >= max,
        _ => true, // Assume full if can't determine
    }
}

/// System hook registry (placeholder)
/// Will be filled in from the system loader hooks in Phase 4.3
#[derive(Resource, Clone)]
pub struct SystemHooksRegistry {
    pub hooks: Option<serde_json::Value>,
}

impl SystemHooksRegistry {
    pub fn has_hook(&self, _system_id: &str, _hook_name: &str) -> bool {
        // Future: check if hook is registered
        false
    }
}

/// Example: Apply D&D 5e rules
/// This demonstrates how system hooks would work
pub fn apply_d20_rules(token: &Token, stats: &mut DerivedStats) {
    // D&D 5e specific computations
    if let Some(dex) = token.abilities.dexterity {
        let dex_mod = (dex - 10) / 2;
        stats.armor_class = Some(10 + dex_mod);
        stats.initiative = Some(dex_mod);
    }
    
    // Health percentage
    if let (Some(health), Some(max_health)) = (token.health, token.max_health) {
        stats.health_percentage = Some((health as f32 / max_health as f32) * 100.0);
        stats.is_dead = health <= 0;
    }
    
    // Default D&D speed
    stats.movement_speed = Some(30);
    stats.proficiency_bonus = Some(2);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ability_modifiers() {
        let abilities = TokenAbilities {
            strength: Some(14),
            dexterity: Some(10),
            constitution: Some(12),
            ..Default::default()
        };
        
        let mods = compute_ability_modifiers(&abilities);
        assert_eq!(mods.str, Some(2));
        assert_eq!(mods.dex, Some(0));
        assert_eq!(mods.con, Some(1));
    }

    #[test]
    fn test_armor_class_computation() {
        let abilities = TokenAbilities {
            dexterity: Some(14),
            ..Default::default()
        };
        
        let ac = compute_armor_class(&abilities);
        assert_eq!(ac, Some(12)); // 10 + 2 (DEX mod)
    }

    #[test]
    fn test_initiative_computation() {
        let abilities = TokenAbilities {
            dexterity: Some(16),
            ..Default::default()
        };
        
        let initiative = compute_initiative(&abilities);
        assert_eq!(initiative, Some(3)); // (16 - 10) / 2 = 3
    }

    #[test]
    fn test_health_percentage() {
        let pct = compute_health_percentage(Some(50), Some(100));
        assert_eq!(pct, Some(50.0));
        
        let pct = compute_health_percentage(Some(75), Some(100));
        assert_eq!(pct, Some(75.0));
    }

    #[test]
    fn test_is_dead() {
        assert!(is_token_dead(Some(0)));
        assert!(is_token_dead(Some(-5)));
        assert!(!is_token_dead(Some(1)));
        assert!(!is_token_dead(None));
    }

    #[test]
    fn test_full_health() {
        assert!(is_token_full_health(Some(100), Some(100)));
        assert!(!is_token_full_health(Some(99), Some(100)));
        assert!(is_token_full_health(None, None));
    }

    #[test]
    fn test_derived_stats_calculation() {
        let token = Token {
            id: "test".to_string(),
            world_id: "world".to_string(),
            label: Some("Goblin".to_string()),
            health: Some(7),
            max_health: Some(7),
            abilities: TokenAbilities {
                strength: Some(15),
                dexterity: Some(12),
                constitution: Some(13),
                ..Default::default()
            },
            schema_version: 1,
            ..Default::default()
        };

        let stats = compute_derived_stats(&token, None);
        
        assert_eq!(stats.health_percentage, Some(100.0));
        assert!(!stats.is_dead);
        assert!(stats.is_full_health);
        assert_eq!(stats.armor_class, Some(11)); // 10 + 1 (DEX mod)
        assert_eq!(stats.initiative, Some(1));
        assert_eq!(stats.movement_speed, Some(30));
    }
}
