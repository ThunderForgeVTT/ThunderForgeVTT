//! Spec 031 (FR-044, FR-045): which authoring tools a person may use in one
//! world. Spec 082 changes the default: a player selects and draws unless a
//! Game Master has taken that away ([`PLAYER_DEFAULT_TOOLS`]).
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
//! [`actor_in_world`] rather than restating it.
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

use crate::auth::world_membership::actor_in_world;
use crate::schema::{world_authoring_tool_grants, world_authoring_tool_revocations, world_members};
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

/// Spec 082: the tools every player holds until a Game Master takes them
/// away. A player selects and draws on the board; everything else is a
/// Game Master's to hand out.
pub const PLAYER_DEFAULT_TOOLS: [&str; 2] = ["select", "shapes"];

/// Which tools `user_id` may use in `world_id`.
///
/// A DM of the world holds everything, implicitly and un-removably. A player
/// holds [`PLAYER_DEFAULT_TOOLS`] less what has been revoked from them, plus
/// what has been granted. A non-member holds nothing.
///
/// Spec 082 replaces FR-045's "a player holds nothing" default: the defaults
/// live here, in code, so every world has them on its next request.
pub async fn effective_authoring_tools(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<Vec<String>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || effective_tools_on(&mut conn, user_id, is_admin, world_id))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load authoring tools"))
}

/// [`effective_authoring_tools`] on a connection the caller already holds,
/// for the shape mutations that decide inside their own `spawn_blocking`.
pub fn effective_tools_on(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> QueryResult<Vec<String>> {
    if actor_in_world(conn, user_id, is_admin, world_id).runs_the_world() {
        return Ok(AUTHORING_TOOLS.iter().map(|id| (*id).to_string()).collect());
    }

    // Rows hang off the membership, so a removed member's grants and
    // revocations went with it, and a stranger has neither nor the defaults.
    let Some(member_id) = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .filter(world_members::user_id.eq(user_id))
        .select(world_members::id)
        .first::<Uuid>(conn)
        .optional()?
    else {
        return Ok(Vec::new());
    };

    let granted = world_authoring_tool_grants::table
        .filter(world_authoring_tool_grants::world_member_id.eq(member_id))
        .select(world_authoring_tool_grants::tool)
        .load::<String>(conn)?;
    let revoked = world_authoring_tool_revocations::table
        .filter(world_authoring_tool_revocations::world_member_id.eq(member_id))
        .select(world_authoring_tool_revocations::tool)
        .load::<String>(conn)?;

    Ok(resolve_member_tools(&granted, &revoked))
}

/// A player's tools from their rows: the defaults less what was revoked,
/// plus what was granted, in [`AUTHORING_TOOLS`] order.
///
/// Filtered through the declaration rather than returned raw, so the rail is
/// ordered by it and a row naming a tool this build does not have resolves
/// to nothing rather than being handed on to a client.
pub fn resolve_member_tools(granted: &[String], revoked: &[String]) -> Vec<String> {
    AUTHORING_TOOLS
        .iter()
        .filter(|tool| {
            let by_default =
                PLAYER_DEFAULT_TOOLS.contains(tool) && !revoked.iter().any(|r| r == *tool);
            by_default || granted.iter().any(|g| g == *tool)
        })
        .map(|tool| (*tool).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

    /// Spec 082: a world with no rows lets its players select and draw, and
    /// nothing else.
    #[tokio::test]
    async fn a_player_in_an_untouched_world_may_select_and_draw() {
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

        assert_eq!(tools, vec!["select", "shapes"]);
    }

    /// A revoked default is gone; a granted tool joins the defaults, in the
    /// declaration's order.
    #[tokio::test]
    async fn revocations_narrow_and_grants_widen_a_players_tools() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let revoked = insert_test_user(&mut conn);
        let granted = insert_test_user(&mut conn);
        let revoked_member = join(&mut conn, world_id, revoked);
        let granted_member = join(&mut conn, world_id, granted);
        revoke(&mut conn, revoked_member, "shapes", owner_id);
        grant(&mut conn, granted_member, "walls", owner_id);
        drop(conn);

        let tools = effective_authoring_tools(&state, revoked, false, world_id)
            .await
            .expect("resolution");
        assert_eq!(tools, vec!["select"]);

        let tools = effective_authoring_tools(&state, granted, false, world_id)
            .await
            .expect("resolution");
        assert_eq!(tools, vec!["select", "walls", "shapes"]);
    }

    /// Removing a member takes their revocations with them, so a player who
    /// rejoins starts at the defaults.
    #[tokio::test]
    async fn a_removed_members_revocations_go_with_them() {
        use crate::schema::world_authoring_tool_revocations as r;
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, owner_id, "Owner");
        let player = insert_test_user(&mut conn);
        let member = join(&mut conn, world_id, player);
        revoke(&mut conn, member, "select", owner_id);
        drop(conn);

        crate::graphql::mutations_invites::remove_member_impl(&state, owner_id, world_id, player)
            .await
            .expect("removal");

        let mut conn = state.db_pool.get().unwrap();
        let left: i64 = r::table
            .filter(r::world_member_id.eq(member))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(left, 0, "the revocation must cascade with the membership");
    }

    /// The database refuses a grant row for a default tool, so "held" has one
    /// spelling.
    #[tokio::test]
    async fn a_grant_never_names_a_default_tool() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player = insert_test_user(&mut conn);
        let member = join(&mut conn, world_id, player);
        let refused = diesel::insert_into(world_authoring_tool_grants::table)
            .values(crate::models::NewWorldAuthoringToolGrant {
                world_member_id: member,
                tool: "shapes".into(),
                created_by: owner_id,
                updated_by: owner_id,
            })
            .execute(&mut conn);
        assert!(refused.is_err());
    }

    fn join(conn: &mut PgConnection, world_id: Uuid, user_id: Uuid) -> Uuid {
        insert_test_world_member(conn, world_id, user_id, "Player");
        world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(user_id))
            .select(world_members::id)
            .first(conn)
            .expect("membership")
    }

    fn revoke(conn: &mut PgConnection, member: Uuid, tool: &str, by: Uuid) {
        diesel::insert_into(world_authoring_tool_revocations::table)
            .values(crate::models::NewWorldAuthoringToolRevocation {
                world_member_id: member,
                tool: tool.into(),
                revoked_by: Some(by),
            })
            .execute(conn)
            .expect("revocation");
    }

    fn grant(conn: &mut PgConnection, member: Uuid, tool: &str, by: Uuid) {
        diesel::insert_into(world_authoring_tool_grants::table)
            .values(crate::models::NewWorldAuthoringToolGrant {
                world_member_id: member,
                tool: tool.into(),
                created_by: by,
                updated_by: by,
            })
            .execute(conn)
            .expect("grant");
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
