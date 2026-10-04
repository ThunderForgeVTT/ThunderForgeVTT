//! Spec 012: the lore entry "ownership block" — DM-only permission grants
//! (`loreEntryPermissions`, `setLorePermission`, `removeLorePermission`).
//! See contracts/lore-permissions.md.
//!
//! The surface is generated from one declaration in
//! [`crate::graphql::permissioned_entity_resolvers`], which this module
//! re-exports. Lore is the one type whose grant table names its user column
//! `world_member_user_id` rather than `user_id`, and whose noun is two words;
//! both are parameters of that declaration rather than reasons to keep a
//! hand-written copy.
//!
//! Its tests stay here, because what is worth testing per type is not the
//! shape — that is the macro's, and tested once — but that this type's rows,
//! tables and gate are wired to the right ones.

pub use crate::graphql::permissioned_entity_resolvers::{
    LorePermissionMutation, LorePermissionQuery, SetLorePermissionInput,
    lore_entry_permissions_impl, remove_lore_permission_impl, set_lore_permission_impl,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_lore::{CreateLoreEntryInput, create_lore_entry_impl};
    use crate::graphql::types::ActorPermissionLevel;
    use crate::schema::{world_lore_entries, world_lore_permissions};
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };
    use diesel::prelude::*;

    /// FR-003: only the DM may view or change the ownership block; a
    /// non-DM member (even one holding explicit Owner via a prior grant)
    /// is rejected — mirrors spec 010's actor precedent exactly.
    #[tokio::test]
    async fn only_dm_can_set_or_view_lore_permissions() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let entry = create_lore_entry_impl(
            &state,
            owner_id,
            false,
            CreateLoreEntryInput {
                world_id,
                title: "Test Entry".to_string(),
                content: None,
            },
        )
        .await
        .expect("DM should create entry");

        let denied = set_lore_permission_impl(
            &state,
            player_id,
            false,
            SetLorePermissionInput {
                lore_entry_id: entry.id,
                user_id: player_id,
                level: ActorPermissionLevel::Owner,
            },
        )
        .await;
        assert!(
            denied.is_err(),
            "a non-DM caller must not be able to set lore permissions"
        );

        let granted = set_lore_permission_impl(
            &state,
            owner_id,
            false,
            SetLorePermissionInput {
                lore_entry_id: entry.id,
                user_id: player_id,
                level: ActorPermissionLevel::Owner,
            },
        )
        .await
        .expect("DM should be able to grant Owner");
        assert_eq!(granted.level, ActorPermissionLevel::Owner.as_db_str());

        let still_denied = lore_entry_permissions_impl(&state, player_id, false, entry.id).await;
        assert!(
            still_denied.is_err(),
            "a player holding explicit Owner on the entry must still not see the ownership block"
        );

        let dm_view = lore_entry_permissions_impl(&state, owner_id, false, entry.id)
            .await
            .expect("DM should be able to view the ownership block");
        assert_eq!(dm_view.len(), 1);
    }

    /// T054 (data-model.md, mirrors FR-022 from spec 010 verbatim): when
    /// a world member holding a lore entry ownership-block entry is
    /// removed from the world, that entry is deleted — this exercises
    /// the exact same scoped-delete query `remove_member`
    /// (`mutations_invites.rs`) issues, confirming it targets only the
    /// removed member's own rows within that world's lore entries (a
    /// same-titled row belonging to a *different* member, or one in a
    /// *different* world, must survive).
    #[tokio::test]
    async fn removed_world_member_lore_permissions_are_cleaned_up_scoped_to_their_world() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let other_player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, other_player_id, "Player");
        drop(conn);

        let entry = create_lore_entry_impl(
            &state,
            owner_id,
            false,
            CreateLoreEntryInput {
                world_id,
                title: "Test Entry".to_string(),
                content: None,
            },
        )
        .await
        .expect("DM should create entry");

        set_lore_permission_impl(
            &state,
            owner_id,
            false,
            SetLorePermissionInput {
                lore_entry_id: entry.id,
                user_id: player_id,
                level: ActorPermissionLevel::Editor,
            },
        )
        .await
        .expect("DM should grant Editor to the departing player");
        set_lore_permission_impl(
            &state,
            owner_id,
            false,
            SetLorePermissionInput {
                lore_entry_id: entry.id,
                user_id: other_player_id,
                level: ActorPermissionLevel::Owner,
            },
        )
        .await
        .expect("DM should grant Owner to the staying player");

        // The exact scoped-delete query `remove_member` issues.
        let mut conn = state.db_pool.get().unwrap();
        diesel::delete(
            world_lore_permissions::table
                .filter(world_lore_permissions::world_member_user_id.eq(player_id))
                .filter(
                    world_lore_permissions::lore_entry_id.eq_any(
                        world_lore_entries::table
                            .filter(world_lore_entries::world_id.eq(world_id))
                            .select(world_lore_entries::id),
                    ),
                ),
        )
        .execute(&mut conn)
        .expect("cascade delete should succeed");
        drop(conn);

        let remaining = lore_entry_permissions_impl(&state, owner_id, false, entry.id)
            .await
            .expect("DM should still be able to view the ownership block");
        assert_eq!(
            remaining.len(),
            1,
            "only the departed player's entry should be removed"
        );
        assert_eq!(
            remaining[0].world_member_user_id, other_player_id,
            "the staying player's grant must survive"
        );
    }
}
