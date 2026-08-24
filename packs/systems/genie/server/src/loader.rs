//! Genie Server Loader
//!
//! GraphQL and mutation registration for the Genie system.
//! Injected at core server startup, mirroring packs/systems/dnd5e/server/src/loader.rs.

/// Register Genie GraphQL schema and mutations with the core server.
///
/// The session-loop mutations (spendWish, advanceDoomClock, advancePuzzleClock,
/// proposeResourceTrade, acceptResourceTrade, spendResourceOnPuzzleClock) live in
/// src/server/src/graphql/mutations/genie_session.rs (spec 018 contracts/genie-session-loop.md)
/// rather than here, since they operate on world-scoped session tables, not
/// per-actor system data — this loader only documents the integration point for
/// this pack's own actor-data validators (see validators.rs).
pub fn register_genie_mutations() {
    // Actor-data validation is registered via register_genie_system()
    // in src/server/src/systems.rs, mirroring register_dnd5e_system.
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_loader_stub() {
        // Loader registration tested via GraphQL integration tests.
    }
}
