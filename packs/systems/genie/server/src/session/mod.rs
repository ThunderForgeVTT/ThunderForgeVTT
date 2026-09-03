//! Genie's session loop: the Wish Pool, the Doom Clock, Puzzle Clocks,
//! Session Resources, shop listings and two-party trades.
//!
//! # Why this lives in the pack
//!
//! It did not. All of it — six tables, eleven models, thirteen GraphQL
//! mutations and the queries beside them — lived in the shared server, some
//! 2,763 lines of one ruleset's rules in code that is supposed to know no
//! ruleset. `check-system-registry.mjs` passed over it honestly, because the
//! rule it enforces is that shared code must not *name* a system and these
//! files quoted "genie" only inside `#[cfg(test)]`. The check was right; the
//! code was still one system's character sheet built into the product.
//!
//! Spec 032 FR-004 and ADR-063 say a pack owns the tables it writes. This is
//! that, carried out: the tables are declared here, the rows are described
//! here, and the mutations that move them are contributed from here.
//!
//! Nothing in `src/server` mentions any of it now.

pub mod models;
pub mod mutations;
pub mod queries;
pub mod schema;

pub use mutations::GenieSessionMutation;
pub use queries::GenieSessionQuery;

/// The world event every mutation here records on success.
///
/// The number is the server's to allocate — event codes are one namespace
/// shared by everything that writes to `world_events` — but the *meaning* is
/// this pack's, so the constant lives with the code that raises it.
/// `thunderforge_server::world_events` records that 15 is spoken for.
pub const EVENT_CODE_GENIE_SESSION_STATE: i32 = 15;

/// Start this world's session the moment the world exists.
///
/// # Why the pack does this and the server used to
///
/// `create_world` in the shared server branched on this system's id and
/// inserted the row below. It was the last game system named in shared server
/// code. The row has not changed; who writes it has, and that is the whole
/// point of spec 032's FR-004 — the server runs whatever hooks are linked and
/// knows none of their names.
///
/// # Why a session exists from creation rather than on demand
///
/// `GenieSessionPanel` used to require the GM to click "Start Genie session"
/// before the Wish Pool, Doom Clock and grants became usable. That gate was
/// removed in favour of the session simply existing, and `doom_clock_max: 6`
/// is that button's old default carried forward — a number that belongs to
/// this ruleset and now lives in it.
fn start_session_for_new_world(
    conn: &mut diesel::pg::PgConnection,
    world: thunderforge_server::world_hooks::WorldCreated,
) -> diesel::QueryResult<()> {
    use diesel::prelude::*;

    diesel::insert_into(schema::world_genie_sessions::table)
        .values(&models::NewGenieSession {
            world_id: world.world_id,
            doom_clock_max: 6,
            created_by: world.created_by,
        })
        .execute(conn)?;
    Ok(())
}

inventory::submit! {
    thunderforge_server::world_hooks::WorldCreatedHook {
        system_id: crate::SYSTEM_ID,
        run: start_session_for_new_world,
    }
}
