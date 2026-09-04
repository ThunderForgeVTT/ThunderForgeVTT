//! Spec 025: Ability creation, field-editing, deletion, and the DM-gated
//! GM-only visibility toggle (`createAbility`, `updateAbility`,
//! `deleteAbility`, `setAbilityGmOnly`). See contracts/graphql-abilities.md.
//!
//! Effect CRUD lands here too, in US2 (T035-T038).

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::ability_permissions::require_ability_permission;
use crate::auth::world_membership::is_dm_of_world;
use crate::graphql::types::{
    AbilityEffectTrigger, AbilityEffectType, ActorPermissionLevel, GraphQLAbility,
    GraphQLAbilityEffect,
};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{AbilityEffect, NewAbilityEffect, NewWorldAbility, WorldAbility};
use crate::schema::{world_abilities, world_ability_effects};
use crate::state::AppState;

#[derive(InputObject, Debug, Clone)]
pub struct CreateAbilityInput {
    pub world_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub classification: String,
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
    pub classification: Option<String>,
    /// Explicit clear, because `Option<String>` alone cannot distinguish
    /// "set to null" from "field omitted".
    ///
    /// `updateItem` (spec 013) applies `description.or(existing.description)`,
    /// which makes clearing a description **impossible** once set — a real
    /// defect this deliberately does not inherit (research.md §3, defect 1).
    pub clear_description: Option<bool>,
}

#[derive(InputObject, Debug, Clone)]
pub struct AbilityEffectInput {
    pub effect_type: AbilityEffectType,
    pub formula: String,
    pub target: String,
    pub trigger_kind: Option<AbilityEffectTrigger>,
    pub sort_order: Option<i32>,
}

/// FR-018: a minimal *structural* check, not a ruleset-aware evaluator.
///
/// Rejects empty/whitespace-only formulas and formulas with no alphanumeric
/// content at all. Anything past that — dice notation, bare stat words,
/// `+`/`-` combinations — is accepted as authored, because FR-019 forbids this
/// spec from ever resolving the formula. Copied from `mutations_items.rs`.
fn validate_formula(formula: &str) -> GraphQLResult<()> {
    let trimmed = formula.trim();
    if trimmed.is_empty() {
        return Err(Error::new("Effect formula must not be empty"));
    }
    if !trimmed.chars().any(|c| c.is_ascii_alphanumeric()) {
        return Err(Error::new(
            "Effect formula must contain at least one letter or digit (e.g. \"3d6\", \"STAT\")",
        ));
    }
    Ok(())
}

fn validate_target(target: &str) -> GraphQLResult<()> {
    if target.trim().is_empty() {
        return Err(Error::new("Effect target must not be empty"));
    }
    Ok(())
}

/// Loads an ability's effects in display order.
pub async fn load_ability_effects(
    state: &AppState,
    ability_id: Uuid,
) -> GraphQLResult<Vec<AbilityEffect>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_ability_effects::table
            .filter(world_ability_effects::ability_id.eq(ability_id))
            .order(world_ability_effects::sort_order.asc())
            .select(AbilityEffect::as_select())
            .load::<AbilityEffect>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load ability effects"))
}

/// Resolves the ability an effect belongs to, so permission can be checked
/// against the parent rather than the effect row.
async fn parent_ability_id(state: &AppState, effect_id: Uuid) -> GraphQLResult<Uuid> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_ability_effects::table
            .filter(world_ability_effects::id.eq(effect_id))
            .select(world_ability_effects::ability_id)
            .first::<Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load ability effect"))?
    .ok_or_else(|| Error::new("Ability effect not found"))
}

/// FR-017: add one effect. Requires `Editor` on the parent ability.
pub async fn add_ability_effect_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    ability_id: Uuid,
    effect: AbilityEffectInput,
) -> GraphQLResult<AbilityEffect> {
    require_ability_permission(
        state,
        user_id,
        is_admin,
        ability_id,
        ActorPermissionLevel::Editor,
    )
    .await?;

    validate_formula(&effect.formula)?;
    validate_target(&effect.target)?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let new_effect = NewAbilityEffect {
        ability_id,
        effect_type: effect.effect_type.as_db_str().to_string(),
        formula: effect.formula,
        target: effect.target,
        trigger_kind: effect.trigger_kind.map(|t| t.as_db_str().to_string()),
        sort_order: effect.sort_order.unwrap_or(0),
    };

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(world_ability_effects::table)
            .values(&new_effect)
            .returning(AbilityEffect::as_returning())
            .get_result::<AbilityEffect>(&mut conn)
            .map_err(|e| format!("Failed to add ability effect: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// FR-017: edit one effect without disturbing its siblings.
pub async fn update_ability_effect_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    effect_id: Uuid,
    effect: AbilityEffectInput,
) -> GraphQLResult<AbilityEffect> {
    validate_formula(&effect.formula)?;
    validate_target(&effect.target)?;

    let ability_id = parent_ability_id(state, effect_id).await?;
    require_ability_permission(
        state,
        user_id,
        is_admin,
        ability_id,
        ActorPermissionLevel::Editor,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::update(world_ability_effects::table.filter(world_ability_effects::id.eq(effect_id)))
            .set((
                world_ability_effects::effect_type.eq(effect.effect_type.as_db_str()),
                world_ability_effects::formula.eq(effect.formula),
                world_ability_effects::target.eq(effect.target),
                world_ability_effects::trigger_kind
                    .eq(effect.trigger_kind.map(|t| t.as_db_str().to_string())),
                world_ability_effects::sort_order.eq(effect.sort_order.unwrap_or(0)),
                world_ability_effects::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .returning(AbilityEffect::as_returning())
            .get_result::<AbilityEffect>(&mut conn)
            .map_err(|e| format!("Failed to update ability effect: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// FR-017: remove one effect.
pub async fn remove_ability_effect_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    effect_id: Uuid,
) -> GraphQLResult<bool> {
    let ability_id = parent_ability_id(state, effect_id).await?;
    require_ability_permission(
        state,
        user_id,
        is_admin,
        ability_id,
        ActorPermissionLevel::Editor,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::delete(world_ability_effects::table.filter(world_ability_effects::id.eq(effect_id)))
            .execute(&mut conn)
            .map(|rows| rows > 0)
            .map_err(|e| format!("Failed to remove ability effect: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `AbilityMutation::create_ability` (FR-002).
/// Whether this world may author an ability of `classification`.
///
/// This is where "valid" lives now. The database used to answer it with a
/// CHECK constraint listing four values, which could not express the rule that
/// actually applies: FR-013 asks whether a type is recognised **in this
/// world**, and a table-wide constraint cannot see the world's system.
///
/// A type the active system does not recognise is refused for *authoring*
/// only. Abilities already carrying one stay readable and editable — those are
/// different questions, and only the first is refused (FR-034).
async fn require_authorable_type(
    state: &AppState,
    world_id: Uuid,
    classification: &str,
) -> GraphQLResult<String> {
    let classification = crate::graphql::types::normalise_classification(classification);
    if classification.is_empty() {
        return Err(Error::new("An ability needs a type"));
    }

    let systems_dir = state.directories.systems_dir.clone();
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let wanted = classification.clone();
    let recognised = tokio::task::spawn_blocking(move || {
        let system_id: Option<String> = crate::schema::worlds::table
            .filter(crate::schema::worlds::id.eq(world_id))
            .select(crate::schema::worlds::game_system_id)
            .first::<Option<String>>(&mut conn)?;

        // Assembled with the wanted type counted as in use, so a built-in the
        // system never mentioned is still authorable — FR-011a governs what is
        // *shown*, not what may be written (FR-017).
        let vocabulary = crate::ability_vocabulary::for_system(
            &systems_dir,
            system_id.as_deref(),
            std::slice::from_ref(&wanted),
        );
        Ok::<_, diesel::result::Error>(vocabulary.recognises(&wanted))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to read this world's system"))?;

    if !recognised {
        return Err(Error::new(format!(
            "This world's game system does not recognise the ability type \"{classification}\""
        )));
    }
    Ok(classification)
}

pub async fn create_ability_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: CreateAbilityInput,
) -> GraphQLResult<WorldAbility> {
    if !is_dm_of_world(state, user_id, is_admin, input.world_id).await? {
        return Err(Error::new("Only the DM (Owner or GM) may create abilities"));
    }

    let classification =
        require_authorable_type(state, input.world_id, &input.classification).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let new_ability = NewWorldAbility {
        world_id: input.world_id,
        name: input.name,
        description: input.description,
        classification: classification.clone(),
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

    // Re-typing is authoring by another route, and goes through the same gate
    // (FR-013). A GM may re-type an ability deliberately (FR-038) — to a type
    // this world recognises, which is what this checks.
    let next_classification = match input.classification.as_deref() {
        None => None,
        Some(wanted) => {
            let world_id: Uuid = {
                let mut lookup = state
                    .db_pool
                    .get()
                    .map_err(|_| Error::new("Failed to get DB connection"))?;
                world_abilities::table
                    .filter(world_abilities::id.eq(ability_id))
                    .select(world_abilities::world_id)
                    .first::<Uuid>(&mut lookup)
                    .map_err(|_| Error::new("Ability not found"))?
            };
            Some(require_authorable_type(state, world_id, wanted).await?)
        }
    };

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
                world_abilities::classification
                    .eq(next_classification.unwrap_or(existing.classification)),
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
    let effects = load_ability_effects(state, row.id)
        .await?
        .into_iter()
        .map(GraphQLAbilityEffect::from)
        .collect();
    Ok(GraphQLAbility::from_row(row, effects, my_permission_level))
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
        let row = create_ability_impl(state, auth_user.user_id, auth_user.is_admin, input).await?;
        to_graphql_ability(state, auth_user.user_id, auth_user.is_admin, row).await
    }

    async fn update_ability(
        &self,
        ctx: &Context<'_>,
        input: UpdateAbilityInput,
    ) -> GraphQLResult<GraphQLAbility> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let row = update_ability_impl(state, auth_user.user_id, auth_user.is_admin, input).await?;
        to_graphql_ability(state, auth_user.user_id, auth_user.is_admin, row).await
    }

    async fn delete_ability(&self, ctx: &Context<'_>, ability_id: Uuid) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        delete_ability_impl(state, auth_user.user_id, auth_user.is_admin, ability_id).await
    }

    async fn add_ability_effect(
        &self,
        ctx: &Context<'_>,
        ability_id: Uuid,
        effect: AbilityEffectInput,
    ) -> GraphQLResult<GraphQLAbilityEffect> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let row = add_ability_effect_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            ability_id,
            effect,
        )
        .await?;
        Ok(GraphQLAbilityEffect::from(row))
    }

    async fn update_ability_effect(
        &self,
        ctx: &Context<'_>,
        effect_id: Uuid,
        effect: AbilityEffectInput,
    ) -> GraphQLResult<GraphQLAbilityEffect> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let row = update_ability_effect_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            effect_id,
            effect,
        )
        .await?;
        Ok(GraphQLAbilityEffect::from(row))
    }

    async fn remove_ability_effect(
        &self,
        ctx: &Context<'_>,
        effect_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        remove_ability_effect_impl(state, auth_user.user_id, auth_user.is_admin, effect_id).await
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
            classification: "spell".to_string(),
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

        let ability =
            create_ability_impl(&state, owner_id, false, create_input(world_id, "Secret"))
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

#[cfg(test)]
mod effect_tests {
    use super::*;
    use crate::test_support::*;

    fn ability_input(world_id: Uuid, name: &str) -> CreateAbilityInput {
        CreateAbilityInput {
            world_id,
            name: name.to_string(),
            description: None,
            classification: "spell".to_string(),
            gm_only: None,
        }
    }

    fn effect_input(formula: &str, target: &str) -> AbilityEffectInput {
        AbilityEffectInput {
            effect_type: AbilityEffectType::Damage,
            formula: formula.to_string(),
            target: target.to_string(),
            trigger_kind: None,
            sort_order: None,
        }
    }

    /// FR-018: an empty/whitespace-only formula errors before any write.
    #[tokio::test]
    async fn add_ability_effect_rejects_empty_formula() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Bolt"))
            .await
            .unwrap();

        let err = add_ability_effect_impl(
            &state,
            owner_id,
            false,
            ability.id,
            effect_input("   ", "Hit Points"),
        )
        .await
        .expect_err("a whitespace-only formula must be rejected");
        assert!(err.message.contains("must not be empty"));

        // A formula with no alphanumeric content is also structurally invalid.
        let err = add_ability_effect_impl(
            &state,
            owner_id,
            false,
            ability.id,
            effect_input("+++", "Hit Points"),
        )
        .await
        .expect_err("a formula with no letters or digits must be rejected");
        assert!(err.message.contains("at least one letter or digit"));

        // Nothing was persisted by either rejection.
        assert!(
            load_ability_effects(&state, ability.id)
                .await
                .unwrap()
                .is_empty(),
            "a rejected effect must not be written"
        );

        // An empty target is rejected too.
        add_ability_effect_impl(
            &state,
            owner_id,
            false,
            ability.id,
            effect_input("3d6", "  "),
        )
        .await
        .expect_err("an empty target must be rejected");
    }

    /// FR-017: effects are independent — editing or removing one leaves the
    /// others untouched.
    #[tokio::test]
    async fn ability_can_carry_multiple_effects() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let ability =
            create_ability_impl(&state, owner_id, false, ability_input(world_id, "Fireball"))
                .await
                .unwrap();

        let mut first = effect_input("3d6", "Hit Points");
        first.sort_order = Some(0);
        let first = add_ability_effect_impl(&state, owner_id, false, ability.id, first)
            .await
            .unwrap();

        let mut second = effect_input("1d20 + STAT", "Attack Roll");
        second.effect_type = AbilityEffectType::AttackRoll;
        second.sort_order = Some(1);
        let second = add_ability_effect_impl(&state, owner_id, false, ability.id, second)
            .await
            .unwrap();

        assert_eq!(
            load_ability_effects(&state, ability.id)
                .await
                .unwrap()
                .len(),
            2
        );

        // Editing the first must not disturb the second.
        let mut edited = effect_input("4d6", "Hit Points");
        edited.sort_order = Some(0);
        update_ability_effect_impl(&state, owner_id, false, first.id, edited)
            .await
            .unwrap();

        let reloaded = load_ability_effects(&state, ability.id).await.unwrap();
        assert_eq!(reloaded.len(), 2);
        let untouched = reloaded.iter().find(|e| e.id == second.id).unwrap();
        assert_eq!(untouched.formula, "1d20 + STAT");
        assert_eq!(untouched.target, "Attack Roll");

        // Removing one leaves the other.
        assert!(
            remove_ability_effect_impl(&state, owner_id, false, first.id)
                .await
                .unwrap()
        );
        let remaining = load_ability_effects(&state, ability.id).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, second.id);
    }

    /// FR-019: effects are inert authored data. Nothing here resolves, rolls,
    /// or evaluates a formula — it round-trips byte-for-byte, including
    /// notation this spec deliberately does not understand.
    #[tokio::test]
    async fn ability_effect_formula_is_not_evaluated() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let ability = create_ability_impl(&state, owner_id, false, ability_input(world_id, "Odd"))
            .await
            .unwrap();

        // Ruleset-specific notation this spec has no opinion about.
        let exotic = "2d8kh1 + PROF - resistance(fire)";
        let created = add_ability_effect_impl(
            &state,
            owner_id,
            false,
            ability.id,
            effect_input(exotic, "Mana"),
        )
        .await
        .expect("structurally valid notation must be accepted as-authored");

        assert_eq!(
            created.formula, exotic,
            "the formula must be stored verbatim"
        );
        assert_eq!(
            created.target, "Mana",
            "a target naming a resource this system lacks is still accepted"
        );

        let reloaded = load_ability_effects(&state, ability.id).await.unwrap();
        assert_eq!(
            reloaded[0].formula, exotic,
            "and it must round-trip unchanged"
        );
        // FR-020: trigger_kind is scaffolded but nothing sets or evaluates it.
        assert_eq!(reloaded[0].trigger_kind, None);
    }

    /// Effect edits require Editor on the parent ability, not on the effect
    /// row — a Viewer must not be able to rewrite an ability's mechanics.
    #[tokio::test]
    async fn effect_edits_require_editor_on_the_parent_ability() {
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

        add_ability_effect_impl(
            &state,
            member_id,
            false,
            ability.id,
            effect_input("2d6", "Hit Points"),
        )
        .await
        .expect_err("a Viewer must not add effects");
    }

    /// FR-031: deleting an ability must not be blocked by lore linking to it;
    /// the link row survives with a null FK and renders unresolved.
    #[tokio::test]
    async fn deleting_an_ability_nulls_referencing_lore_links_instead_of_blocking() {
        use crate::schema::{world_lore_entries, world_lore_links};
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let ability =
            create_ability_impl(&state, owner_id, false, ability_input(world_id, "Linked"))
                .await
                .unwrap();

        let mut conn = state.db_pool.get().unwrap();
        let entry_id = Uuid::now_v7();
        diesel::insert_into(world_lore_entries::table)
            .values((
                world_lore_entries::id.eq(entry_id),
                world_lore_entries::world_id.eq(world_id),
                world_lore_entries::title.eq("Source Entry"),
                world_lore_entries::slug.eq(format!("source-{}", entry_id.simple())),
                world_lore_entries::content.eq("Refers to [[Linked]]."),
                world_lore_entries::created_by.eq(owner_id),
            ))
            .execute(&mut conn)
            .expect("insert lore entry");
        let link_id = Uuid::now_v7();
        diesel::insert_into(world_lore_links::table)
            .values((
                world_lore_links::id.eq(link_id),
                world_lore_links::source_lore_entry_id.eq(entry_id),
                world_lore_links::raw_title.eq("Linked"),
                world_lore_links::target_kind.eq("ability"),
                world_lore_links::target_ability_id.eq(ability.id),
            ))
            .execute(&mut conn)
            .expect("insert lore link");
        drop(conn);

        assert!(
            delete_ability_impl(&state, owner_id, false, ability.id)
                .await
                .unwrap(),
            "deletion must not be blocked by an inbound lore link"
        );

        let mut conn = state.db_pool.get().unwrap();
        let (surviving, target): (Uuid, Option<Uuid>) = world_lore_links::table
            .filter(world_lore_links::id.eq(link_id))
            .select((world_lore_links::id, world_lore_links::target_ability_id))
            .first(&mut conn)
            .expect("the link row must survive");
        assert_eq!(surviving, link_id);
        assert_eq!(target, None, "its FK is nulled, so it renders unresolved");

        // The source entry itself is untouched.
        let title: String = world_lore_entries::table
            .filter(world_lore_entries::id.eq(entry_id))
            .select(world_lore_entries::title)
            .first(&mut conn)
            .expect("source entry must be untouched");
        assert_eq!(title, "Source Entry");
    }
}
