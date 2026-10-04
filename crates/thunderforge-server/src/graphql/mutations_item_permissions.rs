//! Spec 013: the item "ownership block" — DM-only permission grants
//! (`itemPermissions`, `setItemPermission`, `removeItemPermission`).
//! See contracts/graphql-items.md.
//!
//! The surface is generated from one declaration in
//! [`crate::graphql::permissioned_entity_resolvers`], which this module
//! re-exports. This file used to be a "direct structural mirror of
//! `mutations_actor_permissions.rs`" and said so in its own header; it is now
//! the same declaration instead of the same code.
//!
//! Its tests stay here, because what is worth testing per type is not the
//! shape — that is the macro's, and tested once — but that this type's rows,
//! tables and gate are wired to the right ones.

pub use crate::graphql::permissioned_entity_resolvers::{
    ItemPermissionMutation, ItemPermissionQuery, SetItemPermissionInput, item_permissions_impl,
    remove_item_permission_impl, set_item_permission_impl,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_items::{CreateItemInput, create_item_impl};
    use crate::graphql::types::ActorPermissionLevel;
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

    /// FR-003: only the DM may view or change the ownership block.
    #[tokio::test]
    async fn only_dm_can_set_or_view_item_permissions() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let item = create_item_impl(
            &state,
            owner_id,
            false,
            CreateItemInput {
                world_id,
                name: "Test Item".to_string(),
                description: None,
            },
        )
        .await
        .expect("DM should create item");

        let denied = set_item_permission_impl(
            &state,
            player_id,
            false,
            SetItemPermissionInput {
                item_id: item.id,
                user_id: player_id,
                level: ActorPermissionLevel::Owner,
            },
        )
        .await;
        assert!(
            denied.is_err(),
            "a non-DM caller must not be able to set item permissions"
        );

        let granted = set_item_permission_impl(
            &state,
            owner_id,
            false,
            SetItemPermissionInput {
                item_id: item.id,
                user_id: player_id,
                level: ActorPermissionLevel::Editor,
            },
        )
        .await
        .expect("DM should be able to grant Editor");
        assert_eq!(granted.level, ActorPermissionLevel::Editor.as_db_str());
    }
}
