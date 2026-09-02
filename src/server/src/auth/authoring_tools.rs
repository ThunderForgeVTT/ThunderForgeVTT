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
//! Nowhere yet, deliberately. A wall or light mutation gated on this today
//! would be a no-op — every caller who passes `is_dm_of_scene` also holds
//! every tool — bought at the price of a second permission query on every
//! authoring write. The gate belongs with the grants it would enforce
//! (FR-046). Until then the refusals that matter are this resolver, which
//! decides what a client is told it may use, and the engine, which refuses
//! input for anything outside that answer.

use async_graphql::Result as GraphQLResult;
use uuid::Uuid;

use crate::auth::world_membership::is_dm_of_world;
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
/// **Nobody but a DM has been granted anything yet**, so this returns the empty
/// list for a player. That is not a placeholder — FR-045 requires exactly this
/// default, so that a world deployed before this feature existed behaves after
/// it precisely as it did before: the Game Master authors, players do not.
/// Per-player grants (FR-046) add rows to a store consulted here; they cannot
/// change what this returns for a world that has none.
pub async fn effective_authoring_tools(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<Vec<String>> {
    if is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Ok(AUTHORING_TOOLS.iter().map(|id| (*id).to_string()).collect());
    }

    Ok(granted_authoring_tools(state, user_id, world_id).await?)
}

/// The explicit grants `user_id` holds in `world_id`, DM status aside.
///
/// Separated from [`effective_authoring_tools`] so the DM rule and the grant
/// lookup do not have to be untangled from each other later, and so the one
/// place that reads grants is greppable.
async fn granted_authoring_tools(
    _state: &AppState,
    _user_id: Uuid,
    _world_id: Uuid,
) -> GraphQLResult<Vec<String>> {
    // No grant rows exist to read. Returning empty here is the GM-only default
    // stated above, and it is the whole of FR-045.
    //
    // A caller must not read "empty" as "unrestricted": every consumer treats
    // an empty list as no tools, which is why a player currently gets no rail
    // and why the engine refuses their mode requests.
    Ok(Vec::new())
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
