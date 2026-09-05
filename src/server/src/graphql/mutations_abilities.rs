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
    /// The value on this type's declared grade, where it declares one
    /// (spec 033 FR-021). Refused for an ungraded type, and refused outside
    /// the declared range (FR-023).
    pub grade: Option<i32>,
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
    pub grade: Option<i32>,
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
    require_authorable_type_and_grade(state, world_id, classification, None)
        .await
        .map(|(classification, _)| classification)
}

/// The type, and a grade the type will accept.
///
/// FR-023 checks the range **here**, at authoring time, against the vocabulary
/// in force — not in the database. A system that narrows its range later must
/// not silently clamp or discard a value authored under the old one, and a
/// column constraint could not tell the difference.
async fn require_authorable_type_and_grade(
    state: &AppState,
    world_id: Uuid,
    classification: &str,
    grade: Option<i32>,
) -> GraphQLResult<(String, Option<i32>)> {
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
        let facet = vocabulary
            .get(&wanted)
            .and_then(|kind| kind.grade.as_ref())
            .map(|grade| (grade.label.clone(), grade.min, grade.max));
        Ok::<_, diesel::result::Error>((vocabulary.recognises(&wanted), facet))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to read this world's system"))?;

    let (recognised, grade_facet) = recognised;
    if !recognised {
        return Err(Error::new(format!(
            "This world's game system does not recognise the ability type \"{classification}\""
        )));
    }

    match (grade, grade_facet) {
        // A value on a type that declares no grade is meaningless, and storing
        // it would show a number on a sheet nothing explains.
        (Some(_), None) => Err(Error::new(format!(
            "The ability type \"{classification}\" is not graded"
        ))),
        (Some(value), Some((label, min, max))) if value < min || value > max => Err(Error::new(
            format!("{label} must be between {min} and {max} for \"{classification}\""),
        )),
        _ => Ok((classification, grade)),
    }
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

    let (classification, grade) = require_authorable_type_and_grade(
        state,
        input.world_id,
        &input.classification,
        input.grade,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let new_ability = NewWorldAbility {
        world_id: input.world_id,
        name: input.name,
        description: input.description,
        classification: classification.clone(),
        grade,
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
#[path = "mutations_abilities_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "mutations_abilities_effect_tests.rs"]
mod effect_tests;
