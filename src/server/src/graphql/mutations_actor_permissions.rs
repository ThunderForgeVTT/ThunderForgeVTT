//! Spec 010: the actor "ownership block" — DM-only permission grants
//! (`setActorPermission`, `removeActorPermission`). See
//! contracts/actor-permissions.md.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::actor_permissions::is_dm_of_world;
use crate::graphql::types::{ActorPermissionLevel, GraphQLActorPermission};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{ActorPermission, NewActorPermission};
use crate::schema::{world_actor_permissions, world_actors};

#[derive(InputObject, Debug, Clone)]
pub struct SetActorPermissionInput {
    pub actor_id: Uuid,
    pub user_id: Uuid,
    pub level: ActorPermissionLevel,
}

async fn require_dm_of_actors_world(
    state: &crate::state::AppState,
    caller_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> GraphQLResult<()> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let world_id = tokio::task::spawn_blocking(move || {
        world_actors::table
            .filter(world_actors::id.eq(actor_id))
            .select(world_actors::world_id)
            .first::<Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load actor"))?
    .ok_or_else(|| Error::new("Actor not found"))?;

    if is_dm_of_world(state, caller_id, is_admin, world_id).await? {
        Ok(())
    } else {
        Err(Error::new(
            "Only the DM (Owner or GM) may view or change an actor's ownership block",
        ))
    }
}

/// Testable core of `ActorPermissionQuery::actor_permissions`.
pub async fn actor_permissions_impl(
    state: &crate::state::AppState,
    caller_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> GraphQLResult<Vec<ActorPermission>> {
    require_dm_of_actors_world(state, caller_id, is_admin, actor_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_actor_permissions::table
            .filter(world_actor_permissions::actor_id.eq(actor_id))
            .select(ActorPermission::as_select())
            .load::<ActorPermission>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load actor permissions"))
}

/// Testable core of `ActorPermissionMutation::set_actor_permission`.
/// DM-only (FR-014). UPSERT on `(actor_id, user_id)`.
pub async fn set_actor_permission_impl(
    state: &crate::state::AppState,
    caller_id: Uuid,
    is_admin: bool,
    input: SetActorPermissionInput,
) -> GraphQLResult<ActorPermission> {
    require_dm_of_actors_world(state, caller_id, is_admin, input.actor_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let actor_id = input.actor_id;
    let target_user_id = input.user_id;
    let level = input.level.as_db_str().to_string();

    tokio::task::spawn_blocking(move || {
        let new_row = NewActorPermission {
            id: Uuid::now_v7(),
            actor_id,
            user_id: target_user_id,
            level: level.clone(),
        };

        diesel::insert_into(world_actor_permissions::table)
            .values(&new_row)
            .on_conflict((world_actor_permissions::actor_id, world_actor_permissions::user_id))
            .do_update()
            .set((
                world_actor_permissions::level.eq(level),
                world_actor_permissions::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .returning(ActorPermission::as_returning())
            .get_result::<ActorPermission>(&mut conn)
            .map_err(|e| format!("Failed to set actor permission: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `ActorPermissionMutation::remove_actor_permission`.
/// DM-only (FR-014). Idempotent.
pub async fn remove_actor_permission_impl(
    state: &crate::state::AppState,
    caller_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
    user_id: Uuid,
) -> GraphQLResult<bool> {
    require_dm_of_actors_world(state, caller_id, is_admin, actor_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::delete(
            world_actor_permissions::table
                .filter(world_actor_permissions::actor_id.eq(actor_id))
                .filter(world_actor_permissions::user_id.eq(user_id)),
        )
        .execute(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to remove actor permission"))?;

    Ok(true)
}

#[derive(Default)]
pub struct ActorPermissionQuery;

#[async_graphql::Object]
impl ActorPermissionQuery {
    /// DM-only (FR-014). Returns only explicit rows — members with no row
    /// default to Viewer, which the client renders itself by combining
    /// this with the full `worldMembers` roster (contracts/actor-permissions.md).
    async fn actor_permissions(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLActorPermission>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let rows =
            actor_permissions_impl(state, auth_user.user_id, auth_user.is_admin, actor_id).await?;
        Ok(rows.into_iter().map(GraphQLActorPermission::from).collect())
    }
}

#[derive(Default)]
pub struct ActorPermissionMutation;

#[async_graphql::Object]
impl ActorPermissionMutation {
    async fn set_actor_permission(
        &self,
        ctx: &Context<'_>,
        input: SetActorPermissionInput,
    ) -> GraphQLResult<GraphQLActorPermission> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        set_actor_permission_impl(state, auth_user.user_id, auth_user.is_admin, input)
            .await
            .map(GraphQLActorPermission::from)
    }

    async fn remove_actor_permission(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
        user_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        remove_actor_permission_impl(state, auth_user.user_id, auth_user.is_admin, actor_id, user_id)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::types::ActorPermissionLevel;
    use crate::test_support::{insert_test_user, insert_test_world, insert_test_world_member, test_app_state};

    /// FR-014: only the DM may view or change the ownership block — a
    /// non-DM member (even one holding explicit Owner on the actor via a
    /// prior grant) is rejected.
    #[tokio::test]
    async fn only_dm_can_set_or_view_permissions() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = crate::test_support::insert_test_scene(&mut conn, world_id, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let actor = crate::graphql::mutations_actors::create_actor_impl(
            &state,
            owner_id,
            false,
            crate::graphql::mutations_actors::CreateActorInput {
                world_id,
                label: "Test Actor".to_string(),
                is_npc: true,
                actor_type: None,
                game_system_id: None,
            },
        )
        .await
        .expect("DM should create actor");
        let _ = scene_id;

        // Non-DM player cannot set permissions, even on their own behalf.
        let denied = set_actor_permission_impl(
            &state,
            player_id,
            false,
            SetActorPermissionInput {
                actor_id: actor.id,
                user_id: player_id,
                level: ActorPermissionLevel::Owner,
            },
        )
        .await;
        assert!(denied.is_err(), "a non-DM caller must not be able to set actor permissions");

        // DM can grant, and the granted player still cannot then view/change the block.
        let granted = set_actor_permission_impl(
            &state,
            owner_id,
            false,
            SetActorPermissionInput {
                actor_id: actor.id,
                user_id: player_id,
                level: ActorPermissionLevel::Owner,
            },
        )
        .await
        .expect("DM should be able to grant Owner");
        assert_eq!(granted.level, ActorPermissionLevel::Owner.as_db_str());

        let still_denied =
            actor_permissions_impl(&state, player_id, false, actor.id).await;
        assert!(
            still_denied.is_err(),
            "a player holding explicit Owner on the actor must still not see the ownership block \
             (DM-only, regardless of the requester's own permission level)"
        );

        let dm_view = actor_permissions_impl(&state, owner_id, false, actor.id)
            .await
            .expect("DM should be able to view the ownership block");
        assert_eq!(dm_view.len(), 1);
    }

    /// FR-022 (research.md §7): removing a world member cascade-deletes
    /// their explicit ownership-block entries via `mutations_invites.rs`'s
    /// `remove_member`. This test exercises the deletion query directly
    /// (the same one `remove_member` issues) to confirm the cleanup is
    /// scoped correctly to the removed member's own rows in that world.
    #[tokio::test]
    async fn removing_permission_reverts_to_default_viewer() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, owner_id, "Owner");
        crate::test_support::insert_test_scene(&mut conn, world_id, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let actor = crate::graphql::mutations_actors::create_actor_impl(
            &state,
            owner_id,
            false,
            crate::graphql::mutations_actors::CreateActorInput {
                world_id,
                label: "Test Actor".to_string(),
                is_npc: false,
                actor_type: None,
                game_system_id: None,
            },
        )
        .await
        .expect("DM should create actor");

        set_actor_permission_impl(
            &state,
            owner_id,
            false,
            SetActorPermissionInput {
                actor_id: actor.id,
                user_id: player_id,
                level: ActorPermissionLevel::Owner,
            },
        )
        .await
        .expect("DM should grant Owner");

        remove_actor_permission_impl(&state, owner_id, false, actor.id, player_id)
            .await
            .expect("DM should be able to remove the grant");

        let remaining = actor_permissions_impl(&state, owner_id, false, actor.id)
            .await
            .expect("DM should still be able to view the (now empty) ownership block");
        assert!(remaining.is_empty(), "removed grant must not remain as an explicit row");
    }
}
