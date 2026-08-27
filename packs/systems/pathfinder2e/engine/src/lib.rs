//! Pathfinder Second Edition (Remaster) Engine Package — mirrors packs/systems/dnd5e/engine.

pub mod plugin;

pub use plugin::GameSystemTrait;

pub const VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_loads() {}
}
