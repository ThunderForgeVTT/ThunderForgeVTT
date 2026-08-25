//! Spec 025: the ability "ownership block" — DM-only permission grants
//! (`abilityPermissions`, `setAbilityPermission`, `removeAbilityPermission`).
//! Structural mirror of `mutations_item_permissions.rs`.
//! See contracts/graphql-abilities.md.
//!
//! **This governs EDIT RIGHTS ONLY.** Visibility is `world_abilities.gm_only`,
//! changed through its own DM-gated mutation in `mutations_abilities.rs`. The
//! ownership block cannot express "hidden": its lowest level (`Viewer`) is
//! also its default for a member with no row.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::is_dm_of_world;
use crate::graphql::types::{ActorPermissionLevel, GraphQLAbilityPermission};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{AbilityPermission, NewAbilityPermission};
use crate::schema::{world_abilities, world_ability_permissions};
use crate::state::AppState;

#[derive(InputObject, Debug, Clone)]
pub struct SetAbilityPermissionInput {
    pub ability_id: Uuid,
    pub user_id: Uuid,
    pub level: ActorPermissionLevel,
}

/// FR-026: changing an ability's ownership block requires DM status. Editor —
/// and even ability-level Owner — is deliberately not sufficient.
async fn require_dm_of_abilitys_world(
    state: &AppState,
    caller_id: Uuid,
    is_admin: bool,
    ability_id: Uuid,
) -> GraphQLResult<()> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let world_id = tokio::task::spawn_blocking(move || {
        world_abilities::table
            .filter(world_abilities::id.eq(ability_id))
            .select(world_abilities::world_id)
            .first::<Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load ability"))?
    .ok_or_else(|| Error::new("Ability not found"))?;

    if is_dm_of_world(state, caller_id, is_admin, world_id).await? {
        Ok(())
    } else {
        Err(Error::new(
            "Only the DM (Owner or GM) may view or change an ability's ownership block",
        ))
    }
}

/// Testable core of `AbilityPermissionQuery::ability_permissions`.
pub async fn ability_permissions_impl(
    state: &AppState,
    caller_id: Uuid,
    is_admin: bool,
    ability_id: Uuid,
) -> GraphQLResult<Vec<AbilityPermission>> {
    require_dm_of_abilitys_world(state, caller_id, is_admin, ability_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_ability_permissions::table
            .filter(world_ability_permissions::ability_id.eq(ability_id))
            .select(AbilityPermission::as_select())
            .load::<AbilityPermission>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load ability permissions"))
}

/// Testable core of `setAbilityPermission`. Upserts on the
/// `(ability_id, user_id)` unique constraint.
pub async fn set_ability_permission_impl(
    state: &AppState,
    caller_id: Uuid,
    is_admin: bool,
    input: SetAbilityPermissionInput,
) -> GraphQLResult<AbilityPermission> {
    require_dm_of_abilitys_world(state, caller_id, is_admin, input.ability_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let new_row = NewAbilityPermission {
        id: Uuid::now_v7(),
        ability_id: input.ability_id,
        user_id: input.user_id,
        level: input.level.as_db_str().to_string(),
    };

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(world_ability_permissions::table)
            .values(&new_row)
            .on_conflict((
                world_ability_permissions::ability_id,
                world_ability_permissions::user_id,
            ))
            .do_update()
            .set((
                world_ability_permissions::level.eq(&new_row.level),
                world_ability_permissions::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .returning(AbilityPermission::as_returning())
            .get_result::<AbilityPermission>(&mut conn)
            .map_err(|e| format!("Failed to set ability permission: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `removeAbilityPermission`. Idempotent — removing a
/// nonexistent row is a no-op returning `false`, and removing an existing one
/// resets that member to the implicit `Viewer` default (FR-024).
pub async fn remove_ability_permission_impl(
    state: &AppState,
    caller_id: Uuid,
    is_admin: bool,
    ability_id: Uuid,
    user_id: Uuid,
) -> GraphQLResult<bool> {
    require_dm_of_abilitys_world(state, caller_id, is_admin, ability_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::delete(
            world_ability_permissions::table
                .filter(world_ability_permissions::ability_id.eq(ability_id))
                .filter(world_ability_permissions::user_id.eq(user_id)),
        )
        .execute(&mut conn)
        .map(|rows| rows > 0)
        .map_err(|e| format!("Failed to remove ability permission: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

#[derive(Default)]
pub struct AbilityPermissionQuery;

#[async_graphql::Object]
impl AbilityPermissionQuery {
    async fn ability_permissions(
        &self,
        ctx: &Context<'_>,
        ability_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLAbilityPermission>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let rows =
            ability_permissions_impl(state, auth_user.user_id, auth_user.is_admin, ability_id)
                .await?;
        Ok(rows.into_iter().map(GraphQLAbilityPermission::from).collect())
    }
}

#[derive(Default)]
pub struct AbilityPermissionMutation;

#[async_graphql::Object]
impl AbilityPermissionMutation {
    async fn set_ability_permission(
        &self,
        ctx: &Context<'_>,
        input: SetAbilityPermissionInput,
    ) -> GraphQLResult<GraphQLAbilityPermission> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let row =
            set_ability_permission_impl(state, auth_user.user_id, auth_user.is_admin, input).await?;
        Ok(GraphQLAbilityPermission::from(row))
    }

    async fn remove_ability_permission(
        &self,
        ctx: &Context<'_>,
        ability_id: Uuid,
        user_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        remove_ability_permission_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            ability_id,
            user_id,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::ability_permissions::effective_ability_permission;
    use crate::graphql::mutations_abilities::{create_ability_impl, CreateAbilityInput};
    use crate::graphql::types::AbilityClassification;
    use crate::test_support::*;

    fn ability_input(world_id: Uuid, name: &str) -> CreateAbilityInput {
        CreateAbilityInput {
            world_id,
            name: name.to_string(),
            description: None,
            classification: AbilityClassification::Spell,
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
