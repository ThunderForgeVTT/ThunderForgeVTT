//! D&D 5e Bevy Plugin
//!
//! Rule calculations for the D&D 5e ruleset.
//! Provides derived data calculations, d20 rolls, spell slot management.

use bevy::prelude::*;
use std::sync::Arc;

/// D&D 5e System rule calculations.
pub struct DnD5eSystem;

impl DnD5eSystem {
    pub fn new() -> Self {
        Self
    }

    /// Register this system with the Bevy world
    pub fn register() -> Arc<Self> {
        Arc::new(Self)
    }

    /// Get system name
    pub fn name(&self) -> String {
        "D&D 5e".to_string()
    }

    /// Get system version
    pub fn version(&self) -> String {
        "0.1.0".to_string()
    }

    // ---------------------------------------------------------------------
    // 5e rules staged for T051 (5e's `SystemRules` implementation).
    //
    // These bodies were the default methods of the deleted `GameSystemTrait`
    // and are kept verbatim, numbers unchanged, so T051 can move them onto
    // `thunderforge_canvas_core::system_rules::SystemRules` without having to
    // re-derive the tables. Do not "tidy" the spell-slot table.
    // ---------------------------------------------------------------------

    /// Calculate ability modifier from ability score
    /// Formula: (score - 10) / 2, rounded down
    pub fn ability_modifier(&self, score: i32) -> i32 {
        (score - 10) / 2
    }

    /// Get skill bonus (modifier + proficiency if applicable)
    pub fn skill_bonus(
        &self,
        ability_mod: i32,
        is_proficient: bool,
        proficiency_bonus: i32,
    ) -> i32 {
        ability_mod + (if is_proficient { proficiency_bonus } else { 0 })
    }

    /// Get proficiency bonus for character level
    pub fn proficiency_bonus(&self, level: u32) -> i32 {
        match level {
            1..=4 => 2,
            5..=8 => 3,
            9..=12 => 4,
            13..=16 => 5,
            17..=20 => 6,
            _ => 2, // Default for out-of-range
        }
    }

    /// Calculate maximum spell slots for a given spell level at character level
    pub fn max_spell_slots(&self, character_level: u32, spell_level: usize) -> i32 {
        let spell_slots = match character_level {
            1 => vec![2, 0, 0, 0, 0, 0, 0, 0, 0],
            2 => vec![3, 2, 0, 0, 0, 0, 0, 0, 0],
            3 => vec![4, 3, 2, 0, 0, 0, 0, 0, 0],
            4 => vec![4, 3, 3, 2, 0, 0, 0, 0, 0],
            5 => vec![4, 4, 3, 3, 2, 0, 0, 0, 0],
            6 => vec![4, 4, 3, 3, 3, 2, 0, 0, 0],
            7 => vec![4, 4, 4, 3, 3, 3, 2, 0, 0],
            8 => vec![4, 4, 4, 3, 3, 3, 3, 2, 0],
            9 => vec![4, 4, 4, 4, 3, 3, 3, 3, 3],
            10 => vec![5, 4, 4, 4, 3, 3, 3, 3, 3],
            11 => vec![5, 4, 4, 4, 4, 3, 3, 3, 3],
            12 => vec![5, 4, 4, 4, 4, 3, 3, 3, 3],
            13 => vec![5, 4, 4, 4, 4, 4, 3, 3, 3],
            14 => vec![5, 4, 4, 4, 4, 4, 3, 3, 3],
            15 => vec![5, 4, 4, 4, 4, 4, 4, 3, 3],
            16 => vec![5, 4, 4, 4, 4, 4, 4, 3, 3],
            17 => vec![5, 5, 4, 4, 4, 4, 4, 4, 3],
            18 => vec![5, 5, 4, 4, 4, 4, 4, 4, 3],
            19 => vec![5, 5, 4, 4, 4, 4, 4, 4, 4],
            20 => vec![5, 5, 4, 4, 4, 4, 4, 4, 4],
            _ => return 0,
        };

        if spell_level < spell_slots.len() {
            spell_slots[spell_level]
        } else {
            0
        }
    }
}

impl Default for DnD5eSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Bevy Plugin for D&D 5e System
pub struct DnD5ePlugin;

impl Plugin for DnD5ePlugin {
    fn build(&self, _app: &mut App) {
        // Phase 4.8: Register D&D 5e systems and resources
        // info!("D&D 5e System initialized");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ability_modifier() {
        let system = DnD5eSystem;
        assert_eq!(system.ability_modifier(10), 0);
        assert_eq!(system.ability_modifier(12), 1);
        assert_eq!(system.ability_modifier(8), -1);
        assert_eq!(system.ability_modifier(16), 3);
    }

    #[test]
    fn test_proficiency_bonus() {
        let system = DnD5eSystem;
        assert_eq!(system.proficiency_bonus(1), 2);
        assert_eq!(system.proficiency_bonus(5), 3);
        assert_eq!(system.proficiency_bonus(9), 4);
        assert_eq!(system.proficiency_bonus(20), 6);
    }

    #[test]
    fn test_skill_bonus() {
        let system = DnD5eSystem;
        // DEX modifier +3, no proficiency
        assert_eq!(system.skill_bonus(3, false, 2), 3);
        // DEX modifier +3, proficiency +2
        assert_eq!(system.skill_bonus(3, true, 2), 5);
    }

    #[test]
    fn test_spell_slots() {
        let system = DnD5eSystem;
        // Level 1 character, level 0 spells (cantrips)
        assert_eq!(system.max_spell_slots(1, 0), 2);
        // Level 5 character, level 1 spells
        assert_eq!(system.max_spell_slots(5, 1), 4);
        // Level 5 character, level 2 spells
        assert_eq!(system.max_spell_slots(5, 2), 3);
    }
}
