//! D&D 5e Server Loader
//!
//! GraphQL and mutation registration for the D&D 5e system.
//! Injected at core server startup (Phase 4.6 integration).

/// Register D&D 5e GraphQL schema and mutations with the core server
///
/// Phase 4.6 TODO: Implement actual GraphQL mutation registration
/// For now, this is a stub that documents the integration point.
///
/// # Example
///
/// ```ignore
/// // In src/server/src/main.rs or graphql setup
/// let mut router = Router::new();
/// dnd5e_server::register_dnd5e_mutations(&mut router);
/// ```
pub fn register_dnd5e_mutations() {
    // Phase 4.6: Register mutations like:
    // - updateActorAbilityScore(actorId, ability, score)
    // - updateActorProficiency(actorId, skill)
    // - castSpell(actorId, spellName, level)
    // - shortRest(actorId)
    // - longRest(actorId)
}
#[cfg(test)]
mod tests {
    use thunderforge_canvas_core::system_contribution::contribution_for;

    /// This pack's submission reaches the registry, under its own id, filling
    /// exactly the slots it means to fill.
    ///
    /// A test with an empty body stood here. What it was named for — that the
    /// loader module loads — a successful compile already establishes. What it
    /// did not cover is the one thing compiling cannot: `inventory` collects
    /// through the linker, so deleting the `submit!` block in lib.rs, changing
    /// the id it carries, or dropping a validator to `None` leaves every other
    /// test in this crate green.
    #[test]
    fn the_pack_contributes_itself_under_its_own_id() {
        let contributed =
            contribution_for(crate::SYSTEM_ID).expect("this pack registers no SystemContribution");

        assert_eq!(contributed.id, crate::SYSTEM_ID);
        assert!(contributed.ability_data.is_some());
        assert!(contributed.resource_data.is_some());
        assert!(contributed.proficiency_data.is_some());
        assert!(contributed.trait_data.is_some());

        // 5e is the only pack filling every slot: spell data validates, and
        // the rules constructor is what produces modifiers, saves, skills and
        // passive Perception rather than storing them.
        assert!(contributed.spell_data.is_some());
        assert!(contributed.rules.is_some());
    }
}
