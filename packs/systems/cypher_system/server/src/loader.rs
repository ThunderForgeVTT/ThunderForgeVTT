//! Cypher System Server Loader
//!
//! GraphQL and mutation registration for the Cypher System system.
//! Mirrors packs/systems/dnd5e/server/src/loader.rs — documents the
//! integration point; server-side mutation registration is a follow-up
//! once this pack has real gameplay mutations beyond generic actor data.

pub fn register_mutations() {
    // No system-specific mutations yet — actor data validation is
    // registered via register_system() in src/server/src/systems.rs.
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

        // Neither slot is filled by this pack. Asserting the absence is what
        // makes filling one later a deliberate edit to this test rather than a
        // silent widening of what the server will accept.
        assert!(contributed.spell_data.is_none());
        assert!(contributed.rules.is_none());
    }

    /// The Rust constant and the pack's manifest name the same system (T014b).
    ///
    /// A pack used to say its id three times: `SYSTEM_ID`, a literal passed to
    /// `SystemContribution::new`, and the manifest's `id`. The literal is gone
    /// — the submission passes the constant now — and this is what holds the
    /// remaining two together. They are read by different halves of the
    /// product and nothing else compares them, so a rename in either one is
    /// exactly the silent drift `check-system-registry.mjs` polices in shared
    /// code, happening inside a pack where that checker does not look.
    #[test]
    fn the_constant_and_the_manifest_name_the_same_system() {
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string("../system.json").expect("this pack's system.json"),
        )
        .expect("system.json parses");

        assert_eq!(
            manifest.get("id").and_then(|id| id.as_str()),
            Some(crate::SYSTEM_ID),
            "SYSTEM_ID and system.json disagree about this pack's id"
        );
    }
}
