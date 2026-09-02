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

    // The 5e rules that were staged here are gone: T051 moved them onto
    // `thunderforge_canvas_core::system_rules::SystemRules`, implemented in
    // `packs/systems/dnd5e/server/src/rules.rs`, which is the one place a
    // system says what it computes (spec 032 FR-027).
    //
    // Worth knowing why they were not simply copied. The staged
    // `ability_modifier` was `(score - 10) / 2`, and Rust's `/` rounds toward
    // zero — so a score of 7 gave -1 where the book says -2, and every odd
    // score below ten was wrong. The tests below it checked 10, 12, 8 and 16:
    // even scores and one odd score above ten, which is exactly the set that
    // cannot see the bug. The replacement uses `div_euclid` and is tested at
    // 7, 5, 3 and 1.

    /// **Still staged.** The by-level spell-slot table has no home yet.
    ///
    /// A slot is two numbers per level — total and expended — and the layout
    /// format cannot address them: `slotGrid` carries one identifier and a
    /// level count, and nothing says what level three's total is called. That
    /// is spec 032's T019a, still open. Moving this table onto `SystemRules`
    /// before then would publish twenty rows nothing can lay out.
    ///
    /// Kept verbatim, numbers unchanged, with its test, so the table survives
    /// until there is somewhere for it to go. Do not tidy it.
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
