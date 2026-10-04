//! Which of this pack's root fields a paused world refuses (spec 051).
//!
//! Every root field the pack merges into the schema is listed here once. The
//! app crate's `play_pause_surface_tests` fails on one that is not, and calls
//! each gated document against a paused world as its Game Master and as a site
//! admin who is a member.

use thunderforge_server::play_pause::surface::PackSurface;

inventory::submit! {
    PackSurface {
        system_id: crate::SYSTEM_ID,
        // Setting a world's optional rules is a change to the world, and
        // pausing stops changes to a world.
        gated: &[
            (
                "updateRollForShoesWorldSettings",
                r#"mutation { updateRollForShoesWorldSettings(input: { worldId: "{world}", difficultyMode: "free", tieSucceeds: false, statusesEnabled: false, skillSlotsEnabled: false, startingSkills: [] }) { worldId } }"#,
            ),
            // Saying what has to be beaten, and taking it back, are moves in
            // play — the thing a pause stops.
            (
                "setRollForShoesTableDifficulty",
                r#"mutation { setRollForShoesTableDifficulty(input: { worldId: "{world}", target: 6 }) { worldId } }"#,
            ),
            (
                "clearRollForShoesTableDifficulty",
                r#"mutation { clearRollForShoesTableDifficulty(worldId: "{world}") { worldId } }"#,
            ),
        ],
        not_world_scoped: &[],
        // What the settings panel and the character sheet read. Readable
        // while paused: pausing stops play, not looking.
        reads: &["rollForShoesWorldSettings", "rollForShoesTableDifficulty"],
        // Nothing to seed. The read answers for a world with no row, and the
        // write creates it.
        seed: &[],
    }
}
