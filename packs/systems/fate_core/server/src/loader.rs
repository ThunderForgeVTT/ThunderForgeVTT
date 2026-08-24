//! Fate Core Server Loader
//!
//! GraphQL and mutation registration for the Fate Core system.
//! Mirrors packs/systems/dnd5e/server/src/loader.rs — documents the
//! integration point; server-side mutation registration is a follow-up
//! once this pack has real gameplay mutations beyond generic actor data.

pub fn register_mutations() {
    // No system-specific mutations yet — actor data validation is
    // registered via register_system() in src/server/src/systems.rs.
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_loader_stub() {}
}
