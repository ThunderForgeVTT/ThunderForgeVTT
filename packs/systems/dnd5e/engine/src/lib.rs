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
    use super::*;
    use bevy::prelude::*;

    /// The pack's plugin builds into an app of its own and survives a frame.
    ///
    /// A test with an empty body stood here, named for the one thing a
    /// successful compile already proves. This asserts what compiling does
    /// not: a Bevy plugin that reads a resource it never inserts builds
    /// cleanly and panics on the first update, and only if some *other*
    /// plugin happened to insert that resource does it appear to work.
    ///
    /// That exact bug has shipped twice here — `WallPlugin` and
    /// `LightingPlugin` each read a resource a neighbouring plugin owned.
    /// `build()` in this pack is empty today, which makes now the cheapest
    /// possible moment to put the guard in place rather than the moment after
    /// the first system is added to it.
    #[test]
    fn the_plugin_builds_and_runs_a_frame_without_its_neighbours() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(DnD5ePlugin);
        app.update();
    }
}
