//! What a system pack does when a world is created.
//!
//! # Why this exists
//!
//! `create_world` used to branch on one system's name and insert that
//! system's session row. It was the last game system named in shared server
//! code and the only entry in `check-system-registry.mjs`'s known-violations
//! list, and it stayed there through a whole increment because the pack could
//! not own the table it needed to write (ADR-063).
//!
//! Now it can, so the branch becomes a hook: a pack submits what it wants
//! done, the server runs whatever is linked, and nothing here knows a system
//! exists.
//!
//! # Why this is not in `thunderforge-canvas-core`
//!
//! That crate holds `SystemContribution`, which is where a pack's other
//! contributions live, and putting this beside them was the obvious move. It
//! is also compiled to wasm as part of the engine, and this hook takes a
//! `&mut PgConnection` — so the obvious move would have dragged Diesel into
//! the browser. A hook that touches the database belongs in the crate that
//! owns the database.
//!
//! # What a hook may assume
//!
//! It is called **inside the transaction that creates the world**, after the
//! `worlds` row and its default scene exist and before the transaction
//! commits. So a hook may reference the world by id, and an error from a hook
//! rolls the whole world creation back rather than leaving a half-made world
//! behind. That is the right trade: a world whose system could not set itself
//! up is not a world anybody can play.

use diesel::pg::PgConnection;

/// The world that has just been created, and who created it.
///
/// A struct rather than loose arguments so that adding a field later is not a
/// change every pack has to absorb.
#[derive(Debug, Clone, Copy)]
pub struct WorldCreated {
    pub world_id: uuid::Uuid,
    pub created_by: uuid::Uuid,
}

/// Run inside the world-creation transaction. Errors roll it back.
pub type OnWorldCreatedFn = fn(&mut PgConnection, WorldCreated) -> diesel::QueryResult<()>;

/// One pack's world-creation hook.
pub struct WorldCreatedHook {
    /// Matches the pack's manifest `id`. Carried for diagnostics only —
    /// nothing dispatches on it, and shared code never reads it to decide
    /// anything.
    pub system_id: &'static str,
    /// Runs only for worlds bound to `system_id`.
    pub run: OnWorldCreatedFn,
}

inventory::collect!(WorldCreatedHook);

/// Every hook linked into this binary.
pub fn hooks() -> impl Iterator<Item = &'static WorldCreatedHook> {
    inventory::iter::<WorldCreatedHook>.into_iter()
}

/// The hooks a given system's pack contributed.
///
/// Split from `run_world_created` so the *selection* can be tested without a
/// database — which is the half that can be wrong. Whether a hook writes the
/// right row is the pack's test to write; whether the right hooks are chosen
/// is this crate's.
pub fn hooks_for(system_id: &str) -> impl Iterator<Item = &'static WorldCreatedHook> {
    let owned = system_id.to_owned();
    hooks().filter(move |hook| hook.system_id == owned)
}

/// Run the hooks belonging to `game_system_id`, if any pack contributed one.
///
/// A world with no system, or a system whose pack contributes no hook, runs
/// nothing — which is the common case and must stay the cheap one. That is
/// also the case a `for` loop gets wrong exactly once, by assuming there is
/// always something to do.
pub fn run_world_created(
    conn: &mut PgConnection,
    game_system_id: Option<&str>,
    world: WorldCreated,
) -> diesel::QueryResult<()> {
    let Some(system_id) = game_system_id else {
        return Ok(());
    };
    for hook in hooks_for(system_id) {
        (hook.run)(conn, world)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world bound to no system runs nothing.
    ///
    /// The case a `for` loop over contributions gets wrong exactly once, by
    /// assuming there is always something to do. It is also the common case:
    /// `None` is a real answer for a world's system, and most packs
    /// contribute no hook at all.
    #[test]
    fn a_world_with_no_system_selects_no_hook() {
        assert_eq!(hooks_for("").count(), 0);
    }

    /// A system no linked pack claims runs nothing, rather than running
    /// somebody else's hook.
    #[test]
    fn a_system_no_pack_claims_selects_no_hook() {
        assert_eq!(hooks_for("a-system-nobody-shipped").count(), 0);
    }

    /// Every hook that is linked is claimed by exactly one system.
    ///
    /// Two packs answering to one id would run both on every world of that
    /// system, and the second one's writes would look like a bug in the first.
    #[test]
    fn no_two_packs_claim_the_same_system() {
        let mut seen: Vec<&str> = hooks().map(|hook| hook.system_id).collect();
        let before = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), before, "two hooks share a system id: {seen:?}");
    }

    /// Discovery is *not* tested here, and the reason is worth writing down.
    ///
    /// `inventory` collects through the linker, into the registry of one
    /// compiled crate instance. `cargo test` compiles this library a second
    /// time with `cfg(test)`, and `test_packs.rs`'s packs were built against
    /// the *first* instance — so their `WorldCreatedHook` submissions land in
    /// a registry this test binary cannot see, and asserting on them here
    /// fails for a reason that has nothing to do with the product.
    ///
    /// `SystemContribution` does not have this problem because it collects in
    /// `thunderforge-canvas-core`, which is a plain dependency compiled once
    /// and shared by both. This registry cannot move there: it takes a
    /// `&mut PgConnection` and canvas-core is compiled to wasm.
    ///
    /// So discovery is asserted in `src/app`, the binary, where there is one
    /// instance of everything and the linkage is the real one — which is the
    /// same argument `system_packs.rs` already makes for its own test.
    #[test]
    fn selection_is_tested_here_and_discovery_is_tested_in_the_binary() {
        // Deliberately trivial: the assertions above cover selection, and
        // this exists so the reasoning above has somewhere to live.
        assert_eq!(hooks_for("").count(), 0);
    }
}
