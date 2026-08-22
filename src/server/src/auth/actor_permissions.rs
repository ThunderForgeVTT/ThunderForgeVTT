//! Spec 010: actor ownership/permission enforcement.
//!
//! Replaces `world_actors.owned_by` (a single, non-null `Uuid`) as the
//! authorization source for actor edits and live-play token control with
//! a real, multi-entry permission model (`world_actor_permissions`):
//! the world's DM (Owner or GM role) always has implicit, un-removable
//! `Owner`-equivalent access to every actor in their world; every other
//! member defaults to `Viewer` unless an explicit row says otherwise.
//! See `specs/010-world-staging-actors/research.md` §3/§4.

use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::require_world_member;
use crate::graphql::types::ActorPermissionLevel;
use crate::schema::{world_actor_permissions, world_actors};
use crate::state::AppState;
use async_graphql::{Error, ErrorExtensions, Result as GraphQLResult};

/// Whether `user_id` is "the DM" of `world_id` — holds the world's Owner
/// or GM role (or is an admin). This is the single check every
/// DM-only mutation in spec 010 (actor creation, ownership-block edits,
/// share-link revocation) should call, per research.md §3.
pub async fn is_dm_of_world(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<bool> {
    if is_admin {
        return Ok(true);
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        match require_world_member(&mut conn, user_id, world_id) {
            Ok(role) => Ok(role == "Owner" || role == "GM"),
            Err(_) => Ok(false),
        }
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Resolves the caller's effective permission level on one actor:
/// DM of the actor's world → always `Owner` (FR-017); else the caller's
/// explicit `world_actor_permissions` row, if any; else `Viewer` (FR-016).
pub async fn effective_actor_permission(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> GraphQLResult<ActorPermissionLevel> {
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

    if is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Ok(ActorPermissionLevel::Owner);
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let level = tokio::task::spawn_blocking(move || {
        world_actor_permissions::table
            .filter(world_actor_permissions::actor_id.eq(actor_id))
            .filter(world_actor_permissions::user_id.eq(user_id))
            .select(world_actor_permissions::level)
            .first::<String>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load actor permission"))?;

    Ok(level
        .and_then(|value| ActorPermissionLevel::from_db_str(&value))
        .unwrap_or(ActorPermissionLevel::Viewer))
}

/// Rejects the caller unless their effective permission on `actor_id` is
/// at least `minimum`. Every actor-mutating GraphQL resolver in spec 010
/// (`updateActor`, `updateActorSystemData`, token-drag) calls this instead
/// of re-deriving permission logic inline.
pub async fn require_actor_permission(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
    minimum: ActorPermissionLevel,
) -> GraphQLResult<()> {
    let level = effective_actor_permission(state, user_id, is_admin, actor_id).await?;

    if level.rank() >= minimum.rank() {
        Ok(())
    } else {
        Err(Error::new("You do not have sufficient permission on this actor")
            .extend_with(|_, ext| ext.set("code", "FORBIDDEN")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{world_actor_permissions, world_actors};
    use crate::test_support::{
        insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
        test_app_state,
    };

    fn insert_test_actor(conn: &mut diesel::PgConnection, world_id: Uuid, scene_id: Uuid, owner_id: Uuid) -> Uuid {
        let id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_actors::table)
            .values((
                world_actors::id.eq(id),
                world_actors::world_id.eq(world_id),
                world_actors::scene_id.eq(scene_id),
                world_actors::actor_type.eq("npc"),
                world_actors::game_system_id.eq("dnd5e"),
                world_actors::label.eq("Test Actor"),
                world_actors::created_by.eq(owner_id),
                world_actors::owned_by.eq(owner_id),
                world_actors::is_public.eq(false),
                world_actors::is_npc.eq(true),
                world_actors::created_at.eq(now),
                world_actors::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("failed to insert test actor");
        id
    }

    /// Spec 010: the world's DM (Owner via `worlds.created_by` fallback)
    /// always resolves to `Owner`, even with zero explicit permission rows.
    #[tokio::test]
    async fn dm_always_resolves_to_owner_with_no_explicit_rows() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        drop(conn);

        let level = effective_actor_permission(&state, owner_id, false, actor_id)
            .await
            .expect("DM permission resolution should succeed");

        assert_eq!(level, ActorPermissionLevel::Owner);
    }

    /// FR-016: a member with no explicit `world_actor_permissions` row
    /// defaults to Viewer.
    #[tokio::test]
    async fn member_with_no_explicit_row_defaults_to_viewer() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let level = effective_actor_permission(&state, player_id, false, actor_id)
            .await
            .expect("permission resolution should succeed");

        assert_eq!(level, ActorPermissionLevel::Viewer);
    }

    /// Clarification: Owner is uncapped — multiple simultaneous
    /// Owner-level members are all accepted by `require_actor_permission`.
    #[tokio::test]
    async fn multiple_simultaneous_owners_are_all_accepted() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_actor(&mut conn, world_id, scene_id, owner_id);

        let player_a = insert_test_user(&mut conn);
        let player_b = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_a, "Player");
        insert_test_world_member(&mut conn, world_id, player_b, "Player");

        let now = chrono::Utc::now().naive_utc();
        for user_id in [player_a, player_b] {
            diesel::insert_into(world_actor_permissions::table)
                .values((
                    world_actor_permissions::id.eq(Uuid::now_v7()),
                    world_actor_permissions::actor_id.eq(actor_id),
                    world_actor_permissions::user_id.eq(user_id),
                    world_actor_permissions::level.eq("Owner"),
                    world_actor_permissions::created_at.eq(now),
                    world_actor_permissions::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .expect("failed to insert permission row");
        }
        drop(conn);

        for user_id in [player_a, player_b] {
            require_actor_permission(&state, user_id, false, actor_id, ActorPermissionLevel::Owner)
                .await
                .expect("both simultaneous Owners should be accepted");
        }
    }

    /// FR-021 (research.md §3): a GM-role member counts as DM, same as Owner.
    #[tokio::test]
    async fn gm_role_counts_as_dm() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let gm_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, gm_id, "GM");
        drop(conn);

        let is_dm = is_dm_of_world(&state, gm_id, false, world_id)
            .await
            .expect("dm check should succeed");

        assert!(is_dm, "a GM-role member must count as DM");
    }

    /// A Player-role member is not DM.
    #[tokio::test]
    async fn player_role_is_not_dm() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let is_dm = is_dm_of_world(&state, player_id, false, world_id)
            .await
            .expect("dm check should succeed");

        assert!(!is_dm, "a Player-role member must not count as DM");
    }
}
