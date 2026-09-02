//! D&D 5e Engine Package
//!
//! WASM-compatible Bevy plugin implementing the D&D 5e ruleset.
//! Handles derived data calculations, spell management, and d20 rolls.
//!
//! ## Architecture
//!
//! - **plugin.rs**: DnD5eSystem rule calculations
//!   - Staged for T051 (`SystemRules` implementation)
//!   - Provides rule calculations (ability modifiers, proficiency bonuses, spell slots)
//!
//! - **dice.rs**: Deterministic d20 dice roller
//!   - Supports normal, advantage, disadvantage rolls
//!   - Seeded RNG for reproducibility
//!
//! - **derived.rs**: (Deferred to Phase 4.8.1)
//!   - Bevy system calculating derived stats from base data
//!   - Stores results in DerivedDnD5eStats component
//!
//! - **spellcasting.rs**: (Deferred to Phase 4.8.1)
//!   - Spell slot management and recovery
//!   - Spellcasting ability and saving throw calculations

pub mod dice;
pub mod plugin;

pub use dice::{roll_d20, roll_d20_seeded, D20Roll, RollAdvantage};
pub use plugin::{DnD5ePlugin, DnD5eSystem};

/// D&D 5e Engine Version
pub const VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_loads() {
        // Smoke test: all modules load without panic
    }
}
