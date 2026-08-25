//! Spec 010: actor ownership/permission enforcement.
//!
//! Replaces `world_actors.owned_by` (a single, non-null `Uuid`) as the
//! authorization source for actor edits and live-play token control with
//! a real, multi-entry permission model (`world_actor_permissions`):
//! the world's DM (Owner or GM role) always has implicit, un-removable
//! `Owner`-equivalent access to every actor in their world; every other
//! member defaults to `Viewer` unless an explicit row says otherwise.
//! See `specs/010-world-staging-actors/research.md` §3/§4.



// Spec 027 (US5): the `effective_actor_permission` / `require_actor_permission` pair that
// lived here is now generated from the single declaration in
// `auth::permissioned_entities`, under the same names and signatures — so no
// caller changed. Three other modules carried a near-verbatim copy of the same
// logic; one of them shipped without its member-removal cleanup, which is the
// privilege leak that motivated consolidating them.
//
// Re-exported here so existing import paths keep working.
pub use crate::auth::permissioned_entities::{effective_actor_permission, require_actor_permission};

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::prelude::*;
    use uuid::Uuid;

    use crate::graphql::types::ActorPermissionLevel;
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
}
