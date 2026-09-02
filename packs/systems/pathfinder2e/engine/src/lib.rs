//! Pathfinder Second Edition (Remaster) Engine Package — mirrors packs/systems/dnd5e/engine.

pub mod plugin;

pub use plugin::{Pathfinder2ePlugin, Pathfinder2eSystem};

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
        app.add_plugins(Pathfinder2ePlugin);
        app.update();
    }
}
