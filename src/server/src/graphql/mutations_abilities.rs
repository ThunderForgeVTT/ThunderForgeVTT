//! Spec 025: Ability creation, field-editing, deletion, and the DM-gated
//! GM-only visibility toggle (`createAbility`, `updateAbility`,
//! `deleteAbility`, `setAbilityGmOnly`). See contracts/graphql-abilities.md.
//!
//! Effect CRUD lands here too, in US2 (T035-T038).

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::ability_permissions::require_ability_permission;
use crate::auth::actor_permissions::is_dm_of_world;
use crate::graphql::types::{AbilityClassification, ActorPermissionLevel, GraphQLAbility};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{NewWorldAbility, WorldAbility};
use crate::schema::world_abilities;
use crate::state::AppState;

#[derive(InputObject, Debug, Clone)]
pub struct CreateAbilityInput {
    pub world_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub classification: AbilityClassification,
    /// FR-024a: optional, defaults false. Settable at create time so a GM can
    /// author a secret ability without a visible window between insert and
    /// hide.
    pub gm_only: Option<bool>,
}

/// FR-024c: `gm_only` is deliberately ABSENT here. `updateAbility` requires
/// only `Editor`, so folding visibility into it would let any Editor un-hide a
/// GM's secret ability. Visibility has its own DM-gated mutation
/// (`setAbilityGmOnly`), following the existing `updateSceneHidden` precedent.
#[derive(InputObject, Debug, Clone)]
pub struct UpdateAbilityInput {
    pub ability_id: Uuid,
    pub name: Option<String>,
    pub description: Option<String>,
    pub classification: Option<AbilityClassification>,
    /// Explicit clear, because `Option<String>` alone cannot distinguish
    /// "set to null" from "field omitted".
    ///
    /// `updateItem` (spec 013) applies `description.or(existing.description)`,
    /// which makes clearing a description **impossible** once set — a real
    /// defect this deliberately does not inherit (research.md §3, defect 1).
    pub clear_description: Option<bool>,
}

/// Testable core of `AbilityMutation::create_ability` (FR-002).
pub async fn create_ability_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: CreateAbilityInput,
) -> GraphQLResult<WorldAbility> {
    if !is_dm_of_world(state, user_id, is_admin, input.world_id).await? {
        return Err(Error::new("Only the DM (Owner or GM) may create abilities"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let new_ability = NewWorldAbility {
        world_id: input.world_id,
        name: input.name,
        description: input.description,
        classification: input.classification.as_db_str().to_string(),
        gm_only: input.gm_only.unwrap_or(false),
        created_by: user_id,
        updated_by: user_id,
    };

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(world_abilities::table)
            .values(&new_ability)
            .returning(WorldAbility::as_returning())
            .get_result::<WorldAbility>(&mut conn)
            .map_err(|e| format!("Failed to create ability: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `AbilityMutation::update_ability`. Requires `Editor`.
pub async fn update_ability_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: UpdateAbilityInput,
) -> GraphQLResult<WorldAbility> {
    require_ability_permission(
        state,
        user_id,
        is_admin,
        input.ability_id,
        ActorPermissionLevel::Editor,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let ability_id = input.ability_id;
    let clear_description = input.clear_description.unwrap_or(false);

    tokio::task::spawn_blocking(move || {
        let existing = world_abilities::table
            .filter(world_abilities::id.eq(ability_id))
            .select(WorldAbility::as_select())
            .first::<WorldAbility>(&mut conn)
            .map_err(|_| "Ability not found".to_string())?;

        // Explicit clear wins; otherwise a provided value sets, and an omitted
        // one leaves the existing description untouched.
        let next_description = if clear_description {
            None
        } else {
            input.description.or(existing.description)
        };

        diesel::update(world_abilities::table.filter(world_abilities::id.eq(ability_id)))
            .set((
                world_abilities::name.eq(input.name.unwrap_or(existing.name)),
                world_abilities::description.eq(next_description),
                world_abilities::classification.eq(input
                    .classification
                    .map(|c| c.as_db_str().to_string())
                    .unwrap_or(existing.classification)),
                world_abilities::updated_by.eq(user_id),
                world_abilities::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .returning(WorldAbility::as_returning())
            .get_result::<WorldAbility>(&mut conn)
            .map_err(|e| format!("Failed to update ability: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `AbilityMutation::delete_ability`. Requires `Owner`.
///
/// Deletion is never blocked by references: actor known-ability entries and
/// lore links both use `ON DELETE SET NULL` and survive as tombstones
/// (FR-023, FR-031).
pub async fn delete_ability_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    ability_id: Uuid,
) -> GraphQLResult<bool> {
    require_ability_permission(
        state,
        user_id,
        is_admin,
        ability_id,
        ActorPermissionLevel::Owner,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::delete(world_abilities::table.filter(world_abilities::id.eq(ability_id)))
            .execute(&mut conn)
            .map(|rows| rows > 0)
            .map_err(|e| format!("Failed to delete ability: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `AbilityMutation::set_ability_gm_only` (FR-024c).
///
/// **DM-only.** Owner-level permission on the ability is deliberately NOT
/// sufficient — see `UpdateAbilityInput`'s comment for why this is a separate
/// mutation rather than a field.
pub async fn set_ability_gm_only_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    ability_id: Uuid,
    gm_only: bool,
) -> GraphQLResult<WorldAbility> {
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

    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new(
            "Only the DM (Owner or GM) may change an ability's GM-only visibility",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::update(world_abilities::table.filter(world_abilities::id.eq(ability_id)))
            .set((
                world_abilities::gm_only.eq(gm_only),
                world_abilities::updated_by.eq(user_id),
                world_abilities::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .returning(WorldAbility::as_returning())
            .get_result::<WorldAbility>(&mut conn)
            .map_err(|e| format!("Failed to set ability visibility: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

async fn to_graphql_ability(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    row: WorldAbility,
) -> GraphQLResult<GraphQLAbility> {
    let my_permission_level = crate::auth::ability_permissions::effective_ability_permission(
        state, user_id, is_admin, row.id,
    )
    .await?;
    Ok(GraphQLAbility::from_row(row, Vec::new(), my_permission_level))
}

#[derive(Default)]
pub struct AbilityMutation;

#[async_graphql::Object]
impl AbilityMutation {
    async fn create_ability(
        &self,
        ctx: &Context<'_>,
        input: CreateAbilityInput,
    ) -> GraphQLResult<GraphQLAbility> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let row =
            create_ability_impl(state, auth_user.user_id, auth_user.is_admin, input).await?;
        to_graphql_ability(state, auth_user.user_id, auth_user.is_admin, row).await
    }

    async fn update_ability(
        &self,
        ctx: &Context<'_>,
        input: UpdateAbilityInput,
    ) -> GraphQLResult<GraphQLAbility> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let row =
            update_ability_impl(state, auth_user.user_id, auth_user.is_admin, input).await?;
        to_graphql_ability(state, auth_user.user_id, auth_user.is_admin, row).await
    }

    async fn delete_ability(
        &self,
        ctx: &Context<'_>,
        ability_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        delete_ability_impl(state, auth_user.user_id, auth_user.is_admin, ability_id).await
    }

    async fn set_ability_gm_only(
        &self,
        ctx: &Context<'_>,
        ability_id: Uuid,
        gm_only: bool,
    ) -> GraphQLResult<GraphQLAbility> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let row = set_ability_gm_only_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            ability_id,
            gm_only,
        )
        .await?;
        to_graphql_ability(state, auth_user.user_id, auth_user.is_admin, row).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn create_input(world_id: Uuid, name: &str) -> CreateAbilityInput {
        CreateAbilityInput {
            world_id,
            name: name.to_string(),
            description: None,
            classification: AbilityClassification::Spell,
            gm_only: None,
        }
    }

    /// FR-002: only the DM may create.
    #[tokio::test]
    async fn only_dm_can_create_ability() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let player_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let err = create_ability_impl(&state, player_id, false, create_input(world_id, "Nope"))
            .await
            .expect_err("a Player must not create abilities");
        assert!(err.message.contains("Only the DM"));

        let created =
            create_ability_impl(&state, owner_id, false, create_input(world_id, "Fireball"))
                .await
                .expect("the world owner may create");
        assert_eq!(created.name, "Fireball");
        assert!(!created.gm_only, "abilities default to visible (FR-024a)");
    }

    /// FR-006: duplicate names are permitted within a world.
    #[tokio::test]
    async fn ability_names_may_collide() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let a = create_ability_impl(&state, owner_id, false, create_input(world_id, "Fireball"))
            .await
            .expect("first insert");
        let b = create_ability_impl(&state, owner_id, false, create_input(world_id, "Fireball"))
            .await
            .expect("a duplicate name must be permitted (FR-006)");
        assert_ne!(a.id, b.id);
        assert_eq!(a.name, b.name);
    }

    /// research.md §3 defect 1: `updateItem` cannot clear a description because
    /// `description.or(existing)` treats null as "unchanged". The ability
    /// version must not inherit that.
    #[tokio::test]
    async fn update_ability_can_clear_description() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let mut input = create_input(world_id, "Fireball");
        input.description = Some("A ball of fire.".to_string());
        let created = create_ability_impl(&state, owner_id, false, input)
            .await
            .unwrap();
        assert!(created.description.is_some());

        // Omitting the field leaves it untouched...
        let untouched = update_ability_impl(
            &state,
            owner_id,
            false,
            UpdateAbilityInput {
                ability_id: created.id,
                name: Some("Fireball II".to_string()),
                description: None,
                classification: None,
                clear_description: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            untouched.description.as_deref(),
            Some("A ball of fire."),
            "an omitted description must not be silently cleared"
        );

        // ...and the explicit flag actually clears it.
        let cleared = update_ability_impl(
            &state,
            owner_id,
            false,
            UpdateAbilityInput {
                ability_id: created.id,
                name: None,
                description: None,
                classification: None,
                clear_description: Some(true),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            cleared.description, None,
            "clear_description must actually clear it — the item version cannot"
        );
    }

    /// FR-024c: visibility is DM-only. An ability-level Owner is not enough,
    /// which is the whole reason this is a separate mutation.
    #[tokio::test]
    async fn only_dm_can_set_gm_only() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let member_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, member_id, "Player");
        drop(conn);

        let ability = create_ability_impl(&state, owner_id, false, create_input(world_id, "Secret"))
            .await
            .unwrap();

        // Grant the member Owner-level permission on the ability itself.
        let mut conn = state.db_pool.get().unwrap();
        diesel::insert_into(crate::schema::world_ability_permissions::table)
            .values((
                crate::schema::world_ability_permissions::id.eq(Uuid::now_v7()),
                crate::schema::world_ability_permissions::ability_id.eq(ability.id),
                crate::schema::world_ability_permissions::user_id.eq(member_id),
                crate::schema::world_ability_permissions::level.eq("Owner"),
            ))
            .execute(&mut conn)
            .unwrap();
        drop(conn);

        let err = set_ability_gm_only_impl(&state, member_id, false, ability.id, true)
            .await
            .expect_err("ability-level Owner must NOT be able to change visibility");
        assert!(err.message.contains("Only the DM"));

        let hidden = set_ability_gm_only_impl(&state, owner_id, false, ability.id, true)
            .await
            .expect("the DM may hide it");
        assert!(hidden.gm_only);

        let shown = set_ability_gm_only_impl(&state, owner_id, false, ability.id, false)
            .await
            .expect("the DM may reveal it again");
        assert!(!shown.gm_only, "unhiding must be possible (US5 scenario 3)");
    }
}
