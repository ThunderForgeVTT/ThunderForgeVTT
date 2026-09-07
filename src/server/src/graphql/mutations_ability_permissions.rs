//! Spec 025: the ability "ownership block" — DM-only permission grants
//! (`abilityPermissions`, `setAbilityPermission`, `removeAbilityPermission`).
//! See contracts/graphql-abilities.md.
//!
//! **This governs EDIT RIGHTS ONLY.** Visibility is `world_abilities.gm_only`,
//! changed through its own DM-gated mutation in `mutations_abilities.rs`. The
//! ownership block cannot express "hidden": its lowest level (`Viewer`) is
//! also its default for a member with no row. That distinction is why the
//! generator behind this module has no visibility parameter and must never
//! gain one.
//!
//! The surface is generated from one declaration in
//! [`crate::graphql::permissioned_entity_resolvers`], which this module
//! re-exports. This file used to be a "structural mirror of
//! `mutations_item_permissions.rs`" and said so in its own header.
//!
//! Its tests stay here, because what is worth testing per type is not the
//! shape — that is the macro's, and tested once — but that this type's rows,
//! tables and gate are wired to the right ones.

pub use crate::graphql::permissioned_entity_resolvers::{
    AbilityPermissionMutation, AbilityPermissionQuery, SetAbilityPermissionInput,
    ability_permissions_impl, remove_ability_permission_impl, set_ability_permission_impl,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::ability_permissions::effective_ability_permission;
    use crate::graphql::mutations_abilities::{CreateAbilityInput, create_ability_impl};
    use crate::graphql::types::ActorPermissionLevel;
    use crate::test_support::*;
    use uuid::Uuid;

    fn ability_input(world_id: Uuid, name: &str) -> CreateAbilityInput {
        CreateAbilityInput {
            world_id,
            name: name.to_string(),
            description: None,
            classification: "spell".to_string(),
            grade: None,
            gm_only: None,
        }
    }

    /// FR-026: the ownership block is DM-only to read *and* write.
    #[tokio::test]
    async fn only_dm_can_set_or_view_ability_permissions() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let member_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, member_id, "Player");
        drop(conn);

        let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Ward"))
            .await
            .unwrap();

        // A plain member can neither read nor write the block.
        ability_permissions_impl(&state, member_id, false, ability.id)
            .await
            .expect_err("a non-DM must not read the ownership block");
        set_ability_permission_impl(
            &state,
            member_id,
            false,
            SetAbilityPermissionInput {
                ability_id: ability.id,
                user_id: member_id,
                level: ActorPermissionLevel::Owner,
            },
        )
        .await
        .expect_err("a non-DM must not grant themselves access");

        // The DM can, and the grant takes effect.
        let granted = set_ability_permission_impl(
            &state,
            owner_id,
            false,
            SetAbilityPermissionInput {
                ability_id: ability.id,
                user_id: member_id,
                level: ActorPermissionLevel::Editor,
            },
        )
        .await
        .expect("the DM may grant access");
        assert_eq!(granted.level, "Editor");

        assert_eq!(
            effective_ability_permission(&state, member_id, false, ability.id)
                .await
                .unwrap(),
            ActorPermissionLevel::Editor,
        );

        // Upsert, not duplicate: re-granting a different level replaces it.
        set_ability_permission_impl(
            &state,
            owner_id,
            false,
            SetAbilityPermissionInput {
                ability_id: ability.id,
                user_id: member_id,
                level: ActorPermissionLevel::Owner,
            },
        )
        .await
        .unwrap();
        let rows = ability_permissions_impl(&state, owner_id, false, ability.id)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1, "the grant is upserted, not duplicated");
        assert_eq!(rows[0].level, "Owner");
    }

    /// FR-024: removing a row reverts the member to the implicit Viewer
    /// default, and removing a nonexistent row is a harmless no-op.
    #[tokio::test]
    async fn removing_a_permission_reverts_to_implicit_viewer() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let member_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, member_id, "Player");
        drop(conn);

        let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Bolt"))
            .await
            .unwrap();

        set_ability_permission_impl(
            &state,
            owner_id,
            false,
            SetAbilityPermissionInput {
                ability_id: ability.id,
                user_id: member_id,
                level: ActorPermissionLevel::Editor,
            },
        )
        .await
        .unwrap();

        assert!(
            remove_ability_permission_impl(&state, owner_id, false, ability.id, member_id)
                .await
                .unwrap(),
            "removing an existing grant reports true"
        );
        assert_eq!(
            effective_ability_permission(&state, member_id, false, ability.id)
                .await
                .unwrap(),
            ActorPermissionLevel::Viewer,
            "the member falls back to the implicit default"
        );

        // Idempotent: a second removal is a no-op, not an error.
        assert!(
            !remove_ability_permission_impl(&state, owner_id, false, ability.id, member_id)
                .await
                .unwrap(),
            "removing a nonexistent grant reports false without erroring"
        );
    }
}
