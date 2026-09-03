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
    for hook in hooks().filter(|hook| hook.system_id == system_id) {
        (hook.run)(conn, world)?;
    }
    Ok(())
}
