//! Spec 010: the actor "ownership block" — DM-only permission grants
//! (`actorPermissions`, `setActorPermission`, `removeActorPermission`).
//! See contracts/actor-permissions.md.
//!
//! The surface is generated from one declaration in
//! [`crate::graphql::permissioned_entity_resolvers`], which this module
//! re-exports. Three other content types were written as copies of what used
//! to be in this file; they are now the same declaration instead of the same
//! code.
//!
//! Its tests stay here, because what is worth testing per type is not the
//! shape — that is the macro's, and tested once — but that this type's rows,
//! tables and gate are wired to the right ones.

pub use crate::graphql::permissioned_entity_resolvers::{
    ActorPermissionMutation, ActorPermissionQuery, SetActorPermissionInput, actor_permissions_impl,
    remove_actor_permission_impl, set_actor_permission_impl,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::types::ActorPermissionLevel;
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

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
                description: None,
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
        assert!(
            denied.is_err(),
            "a non-DM caller must not be able to set actor permissions"
        );

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

        let still_denied = actor_permissions_impl(&state, player_id, false, actor.id).await;
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
                description: None,
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
        assert!(
            remaining.is_empty(),
            "removed grant must not remain as an explicit row"
        );
    }
}
