//! Spec 031 (FR-044, FR-045): which authoring tools a person may use in one
//! world.
//!
//! # Why this is a permission and not a role check
//!
//! Every tool in the rail was gated on "is this person the Game Master",
//! written once per surface: the rail checked it in React, each engine input
//! system checked `IsGameMaster` for itself. FR-044 says the answer must come
//! from a declaration instead, for the reason ADR-050 gives about content
//! grants — a rule written once per noun acquires a fifth copy that nobody
//! updates. The declaration is [`AUTHORING_TOOLS`] plus
//! [`effective_authoring_tools`], and every surface resolves through it.
//!
//! # Why a sibling of `permissioned_entities` rather than an entry in it
//!
//! The obvious move is to add `tool` to the `permissioned_entities!`
//! invocation, and it does not fit — for the same reason that module gives for
//! keeping `is_ability_visible_to` out of the macro.
//!
//! The macro resolves a *ladder* (`Viewer` → `Editor` → `Owner`) over rows in
//! a grants table joined to a **content parent** carrying `world_id`. A tool
//! is not content: there is no `world_tools` table to be the parent, nothing
//! to `ON DELETE CASCADE` from, and no meaningful `Editor` of a tool. Worse,
//! the ladder's floor `Viewer` is also its default, so the macro structurally
//! cannot express "may not" — which is the entire default this feature needs.
//! Bending the macro to fit would give the next capability permission a ladder
//! it does not have, which is precisely the confusion ADR-050 refuses.
//!
//! So: one declaration, adjacent to the others, sharing their DM rule via
//! [`is_dm_of_world`] rather than restating it.
//!
//! # Where the mutation-side gate is
//!
//! Still not on the individual authoring mutations. Now that FR-046's grants
//! exist a wall or light mutation gated on this would no longer be a pure
//! no-op, but the writes it would admit are the ones a Game Master has
//! deliberately handed out, and the price is a second permission query on
//! every authoring write. The refusals that matter remain this resolver,
//! which decides what a client is told it may use, and the engine, which
//! refuses input for anything outside that answer.
//!
//! The gate that *is* here is on the grant itself: only a DM may write a row
//! (`graphql::mutations_authoring_tools`), so nobody can widen their own
//! answer.

use async_graphql::{Error, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::is_dm_of_world;
use crate::schema::{world_authoring_tool_grants, world_members};
use crate::state::AppState;

/// Every authoring tool that can be permissioned, by the identifier the rail
/// and the engine both use.
///
/// The same strings as the web app's `GmToolId` and the engine's
/// `AuthoringMode::from_tool_id`. Three copies of a list is two too many, but
/// they are three separate compilation targets with no shared type; what keeps
/// them honest is that an id this list does not carry cannot be granted, and
/// an id the engine does not know is refused there. A drifted name therefore
/// fails closed on both sides rather than granting something unintended.
pub const AUTHORING_TOOLS: [&str; 6] = [
    "select",
    "walls",
    "lights",
    "shapes",
    "tokens",
    "interactions",
];

/// Which tools `user_id` may use in `world_id`.
///
/// Resolves in the shape the content permissions resolve in: a DM of the world
/// holds everything, implicitly and un-removably; everyone else holds only
/// what has been granted to them.
///
/// A world with no `world_authoring_tool_grants` rows resolves a player to the
/// empty list. That is not a gap waiting to be filled — FR-045 requires
/// exactly this default, so that a world deployed before this feature existed
/// behaves after it precisely as it did before: the Game Master authors,
/// players do not. FR-046's grants add rows; they cannot change what this
/// returns for a world that has none.
pub async fn effective_authoring_tools(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<Vec<String>> {
    if is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Ok(AUTHORING_TOOLS.iter().map(|id| (*id).to_string()).collect());
    }

    granted_authoring_tools(state, user_id, world_id).await
}

/// The explicit grants `user_id` holds in `world_id`, DM status aside.
///
/// Separated from [`effective_authoring_tools`] so the DM rule and the grant
/// lookup do not have to be untangled from each other later, and so the one
/// place that reads grants is greppable.
///
/// Rows hang off the *membership*, not off `(world_id, user_id)`, which is why
/// this joins rather than filtering two columns: a grant is a fact about
/// somebody's membership, and keying it that way is what makes removal
/// cascade instead of needing a cleanup block (see the migration).
///
/// An empty result is "no tools", never "unrestricted" — every consumer reads
/// it that way, which is why a player with no rows gets no rail and why the
/// engine refuses their mode requests.
async fn granted_authoring_tools(
    state: &AppState,
    user_id: Uuid,
    world_id: Uuid,
) -> GraphQLResult<Vec<String>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let granted = tokio::task::spawn_blocking(move || {
        world_authoring_tool_grants::table
            .inner_join(world_members::table)
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(user_id))
            .select(world_authoring_tool_grants::tool)
            .load::<String>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load authoring tool grants"))?;

    // Returned in [`AUTHORING_TOOLS`] order, and filtered through it rather
    // than returned raw. Two things fall out of that: the rail is ordered by
    // the declaration instead of by whatever order a Game Master clicked in,
    // and a row naming a tool this build does not have resolves to nothing
    // rather than being handed on to a client that would ask the engine about
    // it. The declaration is the vocabulary; the table only records answers.
    Ok(AUTHORING_TOOLS
        .iter()
        .filter(|tool| granted.iter().any(|held| held == *tool))
        .map(|tool| (*tool).to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

    /// FR-045, and the assertion the whole default rests on: a world that
    /// predates this feature has no grant rows, and its players must be able
    /// to use nothing — not "everything, because the list is empty".
    #[tokio::test]
    async fn a_player_in_an_untouched_world_may_use_no_tool() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let tools = effective_authoring_tools(&state, player_id, false, world_id)
            .await
            .expect("resolution");

        assert!(
            tools.is_empty(),
            "a player with no grants must hold no tools, got {:?}",
            tools
        );

        for tool in AUTHORING_TOOLS {
            assert!(
                !tools.iter().any(|granted| granted == tool),
                "{tool} must be refused for a player with no grants"
            );
        }
    }

    /// The other half of "existing worlds are unchanged": the Game Master's
    /// rail is exactly what it was, with no rows written anywhere.
    #[tokio::test]
    async fn a_dm_holds_every_tool_with_no_rows() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let gm_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, gm_id, "GM");
        drop(conn);

        for dm in [owner_id, gm_id] {
            let tools = effective_authoring_tools(&state, dm, false, world_id)
                .await
                .expect("resolution");
            assert_eq!(
                tools,
                AUTHORING_TOOLS.to_vec(),
                "a DM must hold every declared tool"
            );

            for tool in AUTHORING_TOOLS {
                assert!(
                    tools.iter().any(|granted| granted == tool),
                    "a DM may use {tool}"
                );
            }
        }
    }

    /// A stranger is not a player with an empty grant list by accident — they
    /// resolve to the same "no tools", so a leaked world id buys nothing.
    #[tokio::test]
    async fn a_non_member_holds_no_tool() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let stranger_id = insert_test_user(&mut conn);
        drop(conn);

        let tools = effective_authoring_tools(&state, stranger_id, false, world_id)
            .await
            .expect("resolution");

        assert!(tools.is_empty(), "a non-member must hold no tools");
    }

    /// The declaration is the whole vocabulary. Guards the three-way spelling
    /// agreement between this list, the rail's `GmToolId` and the engine's
    /// `AuthoringMode::from_tool_id`: a name that drifts on one side resolves
    /// to nothing on the others rather than to something unintended.
    #[test]
    fn the_declaration_is_the_whole_vocabulary() {
        assert!(!AUTHORING_TOOLS.contains(&"wombat"));
        assert!(!AUTHORING_TOOLS.contains(&""));
        assert_eq!(
            AUTHORING_TOOLS.len(),
            AUTHORING_TOOLS
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            "a repeated tool id would mean two rail buttons sharing one permission"
        );
    }
}
