//! Spec 031 (T032b, FR-046): a Game Master handing one player one tool.
//!
//! # Why a row per tool rather than a list on the member
//!
//! The alternative was a `authoring_tools TEXT[]` column on `world_members`,
//! written whole. It loses the two things this shape gives for nothing: a
//! grant carries its own provenance (which Game Master handed it out, and
//! when), and two Game Masters toggling different tools in the same minute
//! cannot overwrite each other, because they are writing different rows.
//! Whole-list writes would make the second one silently undo the first.
//!
//! # Why granting is an upsert and revoking is a delete
//!
//! A grant is held or it is not. A `granted BOOLEAN` column would spell "not
//! held" two ways — a false row and no row — and only one of those survives
//! the `ON DELETE CASCADE` that removes a departed member's grants, so the
//! two spellings would drift the moment somebody left and rejoined.
//!
//! # Who may write
//!
//! Only a DM of the world, checked here with the same `is_dm_of_world` every
//! other GM-only world mutation uses (Constitution Principle III: the refusal
//! lives at the data boundary, not in the settings page that renders the
//! toggles). A player calling this directly is not a DM of their own world
//! and is refused — which is the whole point, since the one thing a
//! permission like this must never allow is its subject widening it.

use async_graphql::{Context, Error, ErrorExtensions, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::authoring_tools::AUTHORING_TOOLS;
use crate::auth::world_membership::is_dm_of_world;
use crate::graphql::{app_state, authenticated_user};
use crate::models::NewWorldAuthoringToolGrant;
use crate::schema::{world_authoring_tool_grants, world_members};
use crate::state::AppState;

/// Which tools this membership has been granted, in declaration order.
///
/// The administrative read — "what has this player been given" — as opposed to
/// `auth::authoring_tools::effective_authoring_tools`, which answers "what may
/// this person use" and folds in the DM's implicit everything. A Game Master
/// looking at the toggles needs the first: their own row would otherwise show
/// six switches that mean nothing, since a DM holds every tool whatever this
/// table says.
pub fn granted_tools_for_member(
    conn: &mut PgConnection,
    world_member_id: Uuid,
) -> QueryResult<Vec<String>> {
    let held = world_authoring_tool_grants::table
        .filter(world_authoring_tool_grants::world_member_id.eq(world_member_id))
        .select(world_authoring_tool_grants::tool)
        .load::<String>(conn)?;

    Ok(AUTHORING_TOOLS
        .iter()
        .filter(|tool| held.iter().any(|row| row == *tool))
        .map(|tool| (*tool).to_string())
        .collect())
}

/// Testable core of `AuthoringToolMutation::set_authoring_tool_grant`.
///
/// Returns the member's grants after the write, so the settings page renders
/// what the table says rather than what it hoped the click did. Idempotent in
/// both directions: granting what is already granted refreshes `updated_by`,
/// revoking what is not granted removes nothing and is not an error.
pub async fn set_authoring_tool_grant_impl(
    state: &AppState,
    caller_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    world_member_id: Uuid,
    tool: String,
    granted: bool,
) -> GraphQLResult<Vec<String>> {
    if !is_dm_of_world(state, caller_id, is_admin, world_id).await? {
        return Err(
            Error::new("Only Owners and GMs can change a player's authoring tools")
                .extend_with(|_, ext| ext.set("code", "FORBIDDEN")),
        );
    }

    // Checked against the declaration before anything is written. The column
    // is open text on purpose (renaming a tool must not be a migration), so
    // this is the door that keeps a typo out of it — and a row naming a tool
    // no build has would be a permission nobody could ever see or revoke.
    if !AUTHORING_TOOLS.contains(&tool.as_str()) {
        return Err(Error::new(format!("Unknown authoring tool '{tool}'")));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let tools = tokio::task::spawn_blocking(move || {
        conn.transaction(|conn| -> QueryResult<Vec<String>> {
            // Both the id *and* the world. `world_member_id` alone would let a
            // Game Master of one world edit a membership of another simply by
            // naming it, since the DM check above is about `world_id` and this
            // row is the only thing that ties the two together.
            let member_id = world_members::table
                .filter(world_members::id.eq(world_member_id))
                .filter(world_members::world_id.eq(world_id))
                .select(world_members::id)
                .first::<Uuid>(conn)?;

            if granted {
                diesel::insert_into(world_authoring_tool_grants::table)
                    .values(&NewWorldAuthoringToolGrant {
                        world_member_id: member_id,
                        tool: tool.clone(),
                        created_by: caller_id,
                        updated_by: caller_id,
                    })
                    .on_conflict((
                        world_authoring_tool_grants::world_member_id,
                        world_authoring_tool_grants::tool,
                    ))
                    // `created_by` is deliberately not touched: it records who
                    // first handed this tool out, and a second Game Master
                    // clicking an already-lit toggle has not granted anything.
                    .do_update()
                    .set((
                        world_authoring_tool_grants::updated_by.eq(caller_id),
                        world_authoring_tool_grants::updated_at.eq(diesel::dsl::now),
                    ))
                    .execute(conn)?;
            } else {
                diesel::delete(
                    world_authoring_tool_grants::table
                        .filter(world_authoring_tool_grants::world_member_id.eq(member_id))
                        .filter(world_authoring_tool_grants::tool.eq(&tool)),
                )
                .execute(conn)?;
            }

            granted_tools_for_member(conn, member_id)
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| match e {
        diesel::result::Error::NotFound => Error::new("That player is not a member of this world"),
        other => Error::new(format!("Failed to update authoring tools: {other}")),
    })?;

    Ok(tools)
}

#[derive(Default)]
pub struct AuthoringToolMutation;

#[async_graphql::Object]
impl AuthoringToolMutation {
    /// Grant (`granted: true`) or revoke a single authoring tool for a single
    /// member of a world. Owner/GM only. Returns that member's grants after
    /// the write.
    async fn set_authoring_tool_grant(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        world_member_id: Uuid,
        tool: String,
        granted: bool,
    ) -> GraphQLResult<Vec<String>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        set_authoring_tool_grant_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            world_member_id,
            tool,
            granted,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::authoring_tools::effective_authoring_tools;
    use crate::graphql::mutations_invites::remove_member_impl;
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

    /// The rows a member holds, read straight from the table.
    ///
    /// Every assertion below goes through this rather than through a
    /// mutation's return value: a mutation that returned the right list while
    /// writing the wrong rows would pass a test written the other way, and the
    /// rows are what the resolver and the cascade both see.
    fn rows_for(conn: &mut PgConnection, world_member_id: Uuid) -> Vec<String> {
        world_authoring_tool_grants::table
            .filter(world_authoring_tool_grants::world_member_id.eq(world_member_id))
            .select(world_authoring_tool_grants::tool)
            .order(world_authoring_tool_grants::tool.asc())
            .load::<String>(conn)
            .expect("grant rows")
    }

    /// The member row id for a user in a world — what the mutation names.
    fn member_id_of(conn: &mut PgConnection, world_id: Uuid, user_id: Uuid) -> Uuid {
        world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(user_id))
            .select(world_members::id)
            .first::<Uuid>(conn)
            .expect("membership")
    }

    /// FR-046: a Game Master grants one tool, and a row appears naming it.
    #[tokio::test]
    async fn a_grant_writes_one_row_and_the_player_resolves_to_it() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let member_id = member_id_of(&mut conn, world_id, player_id);
        drop(conn);

        set_authoring_tool_grant_impl(
            &state,
            owner_id,
            false,
            world_id,
            member_id,
            "walls".to_string(),
            true,
        )
        .await
        .expect("grant");

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(rows_for(&mut conn, member_id), vec!["walls".to_string()]);

        // Provenance, per Constitution Principle III: the Game Master who
        // handed it out is on the row, not the player it was handed to.
        let (created_by, updated_by) = world_authoring_tool_grants::table
            .filter(world_authoring_tool_grants::world_member_id.eq(member_id))
            .select((
                world_authoring_tool_grants::created_by,
                world_authoring_tool_grants::updated_by,
            ))
            .first::<(Uuid, Uuid)>(&mut conn)
            .expect("provenance");
        assert_eq!((created_by, updated_by), (owner_id, owner_id));
        drop(conn);

        let tools = effective_authoring_tools(&state, player_id, false, world_id)
            .await
            .expect("resolution");
        assert_eq!(tools, vec!["walls".to_string()]);
    }

    /// FR-046's other half: a revoke removes the row, and the tool goes with
    /// it. Asserted on the table because "the toggle is off" is a picture.
    #[tokio::test]
    async fn a_revoke_removes_the_row() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let member_id = member_id_of(&mut conn, world_id, player_id);
        drop(conn);

        for tool in ["walls", "lights"] {
            set_authoring_tool_grant_impl(
                &state,
                owner_id,
                false,
                world_id,
                member_id,
                tool.to_string(),
                true,
            )
            .await
            .expect("grant");
        }

        set_authoring_tool_grant_impl(
            &state,
            owner_id,
            false,
            world_id,
            member_id,
            "walls".to_string(),
            false,
        )
        .await
        .expect("revoke");

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            rows_for(&mut conn, member_id),
            vec!["lights".to_string()],
            "revoking one tool must leave the others alone"
        );
        drop(conn);

        let tools = effective_authoring_tools(&state, player_id, false, world_id)
            .await
            .expect("resolution");
        assert_eq!(tools, vec!["lights".to_string()]);
    }

    /// The refusal that matters: a player cannot grant themselves anything.
    ///
    /// Asserted on the table rather than on the error, because an error
    /// returned after a successful write would be the same bug wearing a
    /// message.
    #[tokio::test]
    async fn a_player_cannot_grant_themselves_a_tool() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let member_id = member_id_of(&mut conn, world_id, player_id);
        drop(conn);

        let refused = set_authoring_tool_grant_impl(
            &state,
            player_id,
            false,
            world_id,
            member_id,
            "walls".to_string(),
            true,
        )
        .await;
        assert!(
            refused.is_err(),
            "a player must not grant themselves a tool"
        );

        // And not for another player either — a player with a friend is still
        // not a Game Master.
        let mut conn = state.db_pool.get().unwrap();
        let other_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, other_id, "Player");
        let other_member_id = member_id_of(&mut conn, world_id, other_id);
        drop(conn);

        assert!(
            set_authoring_tool_grant_impl(
                &state,
                player_id,
                false,
                world_id,
                other_member_id,
                "walls".to_string(),
                true,
            )
            .await
            .is_err()
        );

        let mut conn = state.db_pool.get().unwrap();
        assert!(rows_for(&mut conn, member_id).is_empty());
        assert!(rows_for(&mut conn, other_member_id).is_empty());
    }

    /// A tool name the declaration does not carry is refused, so nothing can
    /// write a permission that no surface could ever show or take back.
    #[tokio::test]
    async fn an_undeclared_tool_is_refused() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let member_id = member_id_of(&mut conn, world_id, player_id);
        drop(conn);

        assert!(
            set_authoring_tool_grant_impl(
                &state,
                owner_id,
                false,
                world_id,
                member_id,
                "wombat".to_string(),
                true,
            )
            .await
            .is_err()
        );

        let mut conn = state.db_pool.get().unwrap();
        assert!(rows_for(&mut conn, member_id).is_empty());
    }

    /// Removing a member leaves no grants behind — the leak ADR-050 was
    /// written about, where a removed member's rows survived and were restored
    /// on readmission.
    ///
    /// Nothing in `remove_member_impl` mentions this table: the rows hang off
    /// `world_members(id)` and go with it by `ON DELETE CASCADE`. This test
    /// exists to prove that, and it is the reason the table is not keyed on
    /// `(world_id, user_id)`, which could not cascade and would have needed a
    /// sixth hand-written cleanup block.
    #[tokio::test]
    async fn removing_a_member_takes_their_grants_with_them() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let member_id = member_id_of(&mut conn, world_id, player_id);
        drop(conn);

        for tool in ["walls", "lights"] {
            set_authoring_tool_grant_impl(
                &state,
                owner_id,
                false,
                world_id,
                member_id,
                tool.to_string(),
                true,
            )
            .await
            .expect("grant");
        }

        remove_member_impl(&state, owner_id, world_id, player_id)
            .await
            .expect("removal");

        let mut conn = state.db_pool.get().unwrap();
        assert!(
            rows_for(&mut conn, member_id).is_empty(),
            "a removed member must leave no grant rows"
        );

        // Readmitted, they are a new membership with nothing on it.
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let new_member_id = member_id_of(&mut conn, world_id, player_id);
        assert!(rows_for(&mut conn, new_member_id).is_empty());
        drop(conn);

        let tools = effective_authoring_tools(&state, player_id, false, world_id)
            .await
            .expect("resolution");
        assert!(
            tools.is_empty(),
            "readmission must not restore what was granted before, got {tools:?}"
        );
    }

    /// A grant belongs to one world. Naming a membership of another world,
    /// while holding a Game Master's chair in this one, must write nothing.
    #[tokio::test]
    async fn a_membership_of_another_world_cannot_be_granted() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let other_owner_id = insert_test_user(&mut conn);
        let other_world_id = insert_test_world(&mut conn, other_owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, other_world_id, player_id, "Player");
        let foreign_member_id = member_id_of(&mut conn, other_world_id, player_id);
        drop(conn);

        assert!(
            set_authoring_tool_grant_impl(
                &state,
                owner_id,
                false,
                world_id,
                foreign_member_id,
                "walls".to_string(),
                true,
            )
            .await
            .is_err()
        );

        let mut conn = state.db_pool.get().unwrap();
        assert!(rows_for(&mut conn, foreign_member_id).is_empty());
    }

    /// Granting twice is one row, not two — the settings page's toggle is a
    /// state, and a double click is still that state.
    #[tokio::test]
    async fn granting_the_same_tool_twice_keeps_one_row() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let gm_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, gm_id, "GM");
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let member_id = member_id_of(&mut conn, world_id, player_id);
        drop(conn);

        for caller in [owner_id, gm_id] {
            set_authoring_tool_grant_impl(
                &state,
                caller,
                false,
                world_id,
                member_id,
                "shapes".to_string(),
                true,
            )
            .await
            .expect("grant");
        }

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(rows_for(&mut conn, member_id), vec!["shapes".to_string()]);

        // The second Game Master touched it last, and the first still holds
        // authorship — which is what an argument about a player's tools asks.
        let (created_by, updated_by) = world_authoring_tool_grants::table
            .filter(world_authoring_tool_grants::world_member_id.eq(member_id))
            .select((
                world_authoring_tool_grants::created_by,
                world_authoring_tool_grants::updated_by,
            ))
            .first::<(Uuid, Uuid)>(&mut conn)
            .expect("provenance");
        assert_eq!((created_by, updated_by), (owner_id, gm_id));
    }
}
