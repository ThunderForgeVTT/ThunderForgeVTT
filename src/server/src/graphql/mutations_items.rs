//! Spec 013: Item creation, field-editing, deletion, and effect CRUD
//! (`createItem`, `updateItem`, `deleteItem`, `addItemEffect`,
//! `updateItemEffect`, `removeItemEffect`). See contracts/graphql-items.md.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::item_permissions::require_item_permission;
use crate::auth::world_membership::is_dm_of_world;
use crate::graphql::types::{
    ActorPermissionLevel, GraphQLItem, GraphQLItemEffect, ItemEffectTrigger, ItemEffectType,
};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{ItemEffect, NewItemEffect, NewWorldItem, WorldItem};
use crate::schema::{world_item_effects, world_items};
use crate::state::AppState;

#[derive(InputObject, Debug, Clone)]
pub struct CreateItemInput {
    pub world_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

#[derive(InputObject, Debug, Clone)]
pub struct UpdateItemInput {
    pub item_id: Uuid,
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(InputObject, Debug, Clone)]
pub struct ItemEffectInput {
    pub effect_type: ItemEffectType,
    pub formula: String,
    pub target: String,
    pub trigger_kind: Option<ItemEffectTrigger>,
    pub sort_order: Option<i32>,
}

/// FR-006: a minimal structural check — not a ruleset-aware evaluator
/// (data-model.md `world_item_effects`). Rejects empty/whitespace-only
/// formulas and formulas with no alphanumeric content at all; anything
/// past that (dice notation, bare stat words, `+`/`-` combinations) is
/// accepted as-authored since this spec never resolves the formula.
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

/// Testable core of `ItemMutation::create_item`. DM-only (FR-002);
/// `description`/icon are optional (Clarifications); no name-uniqueness
/// check is performed (FR-019).
pub async fn create_item_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: CreateItemInput,
) -> GraphQLResult<WorldItem> {
    if !is_dm_of_world(state, user_id, is_admin, input.world_id).await? {
        return Err(Error::new("Only the DM (Owner or GM) may create items"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let new_item = NewWorldItem {
        world_id: input.world_id,
        name: input.name,
        description: input.description,
        icon_asset_id: None,
        created_by: user_id,
    };

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(world_items::table)
            .values(&new_item)
            .returning(WorldItem::as_returning())
            .get_result::<WorldItem>(&mut conn)
            .map_err(|e| format!("Failed to create item: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `ItemMutation::update_item`. Requires Editor or Owner.
pub async fn update_item_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: UpdateItemInput,
) -> GraphQLResult<WorldItem> {
    require_item_permission(
        state,
        user_id,
        is_admin,
        input.item_id,
        ActorPermissionLevel::Editor,
    )
    .await?;

    let item_id = input.item_id;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let name = input.name.clone();
    let description = input.description.clone();

    tokio::task::spawn_blocking(move || {
        let existing = world_items::table
            .filter(world_items::id.eq(item_id))
            .select(WorldItem::as_select())
            .first::<WorldItem>(&mut conn)
            .map_err(|_| "Item not found".to_string())?;

        diesel::update(world_items::table.filter(world_items::id.eq(item_id)))
            .set((
                world_items::name.eq(name.unwrap_or(existing.name)),
                world_items::description.eq(description.or(existing.description)),
                world_items::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .returning(WorldItem::as_returning())
            .get_result::<WorldItem>(&mut conn)
            .map_err(|e| format!("Failed to update item: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `ItemMutation::delete_item`. Requires Owner (FR-018).
/// Never blocked by outstanding lore links or inventory references
/// (FR-017) — those FKs are `ON DELETE SET NULL`, not `RESTRICT`.
pub async fn delete_item_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    item_id: Uuid,
) -> GraphQLResult<bool> {
    require_item_permission(
        state,
        user_id,
        is_admin,
        item_id,
        ActorPermissionLevel::Owner,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::delete(world_items::table.filter(world_items::id.eq(item_id))).execute(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to delete item"))?;

    Ok(true)
}

/// Testable core of `ItemMutation::add_item_effect`. Requires Editor or
/// Owner on the parent item (FR-005).
pub async fn add_item_effect_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    item_id: Uuid,
    effect: ItemEffectInput,
) -> GraphQLResult<ItemEffect> {
    require_item_permission(
        state,
        user_id,
        is_admin,
        item_id,
        ActorPermissionLevel::Editor,
    )
    .await?;
    validate_formula(&effect.formula)?;
    validate_target(&effect.target)?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let new_effect = NewItemEffect {
        item_id,
        effect_type: effect.effect_type.as_db_str().to_string(),
        formula: effect.formula,
        target: effect.target,
        trigger_kind: effect.trigger_kind.map(|t| t.as_db_str().to_string()),
        sort_order: effect.sort_order.unwrap_or(0),
    };

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(world_item_effects::table)
            .values(&new_effect)
            .returning(ItemEffect::as_returning())
            .get_result::<ItemEffect>(&mut conn)
            .map_err(|e| format!("Failed to add item effect: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `ItemMutation::update_item_effect`. Requires Editor or
/// Owner on the parent item.
pub async fn update_item_effect_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    effect_id: Uuid,
    effect: ItemEffectInput,
) -> GraphQLResult<ItemEffect> {
    validate_formula(&effect.formula)?;
    validate_target(&effect.target)?;

    let mut lookup_conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let parent_item_id = tokio::task::spawn_blocking(move || {
        world_item_effects::table
            .filter(world_item_effects::id.eq(effect_id))
            .select(world_item_effects::item_id)
            .first::<Uuid>(&mut lookup_conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Item effect not found"))?;

    require_item_permission(
        state,
        user_id,
        is_admin,
        parent_item_id,
        ActorPermissionLevel::Editor,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let effect_type = effect.effect_type.as_db_str().to_string();
    let formula = effect.formula;
    let target = effect.target;
    let trigger_kind = effect.trigger_kind.map(|t| t.as_db_str().to_string());
    let sort_order = effect.sort_order;

    tokio::task::spawn_blocking(move || {
        let existing = world_item_effects::table
            .filter(world_item_effects::id.eq(effect_id))
            .select(ItemEffect::as_select())
            .first::<ItemEffect>(&mut conn)
            .map_err(|_| "Item effect not found".to_string())?;

        diesel::update(world_item_effects::table.filter(world_item_effects::id.eq(effect_id)))
            .set((
                world_item_effects::effect_type.eq(effect_type),
                world_item_effects::formula.eq(formula),
                world_item_effects::target.eq(target),
                world_item_effects::trigger_kind.eq(trigger_kind),
                world_item_effects::sort_order.eq(sort_order.unwrap_or(existing.sort_order)),
                world_item_effects::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .returning(ItemEffect::as_returning())
            .get_result::<ItemEffect>(&mut conn)
            .map_err(|e| format!("Failed to update item effect: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `ItemMutation::remove_item_effect`. Requires Editor or
/// Owner on the parent item.
pub async fn remove_item_effect_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    effect_id: Uuid,
) -> GraphQLResult<bool> {
    let mut lookup_conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let parent_item_id = tokio::task::spawn_blocking(move || {
        world_item_effects::table
            .filter(world_item_effects::id.eq(effect_id))
            .select(world_item_effects::item_id)
            .first::<Uuid>(&mut lookup_conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Item effect not found"))?;

    require_item_permission(
        state,
        user_id,
        is_admin,
        parent_item_id,
        ActorPermissionLevel::Editor,
    )
    .await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::delete(world_item_effects::table.filter(world_item_effects::id.eq(effect_id)))
            .execute(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to remove item effect"))?;

    Ok(true)
}

/// Loads an item's effects, ordered for display (`sort_order`).
async fn load_item_effects(state: &AppState, item_id: Uuid) -> GraphQLResult<Vec<ItemEffect>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_item_effects::table
            .filter(world_item_effects::item_id.eq(item_id))
            .order(world_item_effects::sort_order.asc())
            .select(ItemEffect::as_select())
            .load::<ItemEffect>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load item effects"))
}

async fn to_graphql_item(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    row: WorldItem,
) -> GraphQLResult<GraphQLItem> {
    let effects = load_item_effects(state, row.id).await?;
    let my_permission_level =
        crate::auth::item_permissions::effective_item_permission(state, user_id, is_admin, row.id)
            .await?;
    Ok(GraphQLItem::from_row(row, effects, my_permission_level))
}

#[derive(Default)]
pub struct ItemMutation;

#[async_graphql::Object]
impl ItemMutation {
    async fn create_item(
        &self,
        ctx: &Context<'_>,
        input: CreateItemInput,
    ) -> GraphQLResult<GraphQLItem> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let row = create_item_impl(state, auth_user.user_id, auth_user.is_admin, input).await?;
        to_graphql_item(state, auth_user.user_id, auth_user.is_admin, row).await
    }

    async fn update_item(
        &self,
        ctx: &Context<'_>,
        input: UpdateItemInput,
    ) -> GraphQLResult<GraphQLItem> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let row = update_item_impl(state, auth_user.user_id, auth_user.is_admin, input).await?;
        to_graphql_item(state, auth_user.user_id, auth_user.is_admin, row).await
    }

    async fn delete_item(&self, ctx: &Context<'_>, item_id: Uuid) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        delete_item_impl(state, auth_user.user_id, auth_user.is_admin, item_id).await
    }

    async fn add_item_effect(
        &self,
        ctx: &Context<'_>,
        item_id: Uuid,
        effect: ItemEffectInput,
    ) -> GraphQLResult<GraphQLItemEffect> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        add_item_effect_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            item_id,
            effect,
        )
        .await
        .map(GraphQLItemEffect::from)
    }

    async fn update_item_effect(
        &self,
        ctx: &Context<'_>,
        effect_id: Uuid,
        effect: ItemEffectInput,
    ) -> GraphQLResult<GraphQLItemEffect> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        update_item_effect_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            effect_id,
            effect,
        )
        .await
        .map(GraphQLItemEffect::from)
    }

    async fn remove_item_effect(&self, ctx: &Context<'_>, effect_id: Uuid) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        remove_item_effect_impl(state, auth_user.user_id, auth_user.is_admin, effect_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

    /// FR-002: only the DM may create an item.
    #[tokio::test]
    async fn only_dm_can_create_item() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let denied = create_item_impl(
            &state,
            player_id,
            false,
            CreateItemInput {
                world_id,
                name: "Should not exist".to_string(),
                description: None,
            },
        )
        .await;
        assert!(
            denied.is_err(),
            "a non-DM caller must not be able to create an item"
        );

        let created = create_item_impl(
            &state,
            owner_id,
            false,
            CreateItemInput {
                world_id,
                name: "Potion of Healing".to_string(),
                description: Some("Restores hit points".to_string()),
            },
        )
        .await
        .expect("DM should be able to create an item");
        assert_eq!(created.name, "Potion of Healing");
        assert!(
            created.icon_asset_id.is_none(),
            "icon is optional (Clarifications)"
        );
    }

    /// FR-019: two items may share the same name in the same world.
    #[tokio::test]
    async fn item_names_may_collide() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        for _ in 0..2 {
            create_item_impl(
                &state,
                owner_id,
                false,
                CreateItemInput {
                    world_id,
                    name: "Longsword".to_string(),
                    description: None,
                },
            )
            .await
            .expect("duplicate names must be allowed");
        }
    }

    /// FR-006: an empty formula is rejected before any write.
    #[tokio::test]
    async fn add_item_effect_rejects_empty_formula() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let item = create_item_impl(
            &state,
            owner_id,
            false,
            CreateItemInput {
                world_id,
                name: "Longsword".to_string(),
                description: None,
            },
        )
        .await
        .expect("DM should create item");

        let result = add_item_effect_impl(
            &state,
            owner_id,
            false,
            item.id,
            ItemEffectInput {
                effect_type: ItemEffectType::Damage,
                formula: "   ".to_string(),
                target: "Hit Points".to_string(),
                trigger_kind: None,
                sort_order: None,
            },
        )
        .await;
        assert!(result.is_err(), "an empty formula must be rejected");
    }

    /// FR-005: an item can carry more than one effect, added independently.
    #[tokio::test]
    async fn item_can_carry_multiple_effects() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let item = create_item_impl(
            &state,
            owner_id,
            false,
            CreateItemInput {
                world_id,
                name: "Longsword".to_string(),
                description: None,
            },
        )
        .await
        .expect("DM should create item");

        add_item_effect_impl(
            &state,
            owner_id,
            false,
            item.id,
            ItemEffectInput {
                effect_type: ItemEffectType::AttackRoll,
                formula: "1d20 + STAT + MODIFIERS".to_string(),
                target: "Attack Roll".to_string(),
                trigger_kind: None,
                sort_order: Some(0),
            },
        )
        .await
        .expect("attack-roll effect should be added");

        add_item_effect_impl(
            &state,
            owner_id,
            false,
            item.id,
            ItemEffectInput {
                effect_type: ItemEffectType::Damage,
                formula: "2d8".to_string(),
                target: "Hit Points".to_string(),
                trigger_kind: None,
                sort_order: Some(1),
            },
        )
        .await
        .expect("damage effect should be added");

        let effects = load_item_effects(&state, item.id)
            .await
            .expect("should load effects");
        assert_eq!(effects.len(), 2);
    }

    /// Spec 013 (T042, US3, FR-017): deleting an item is never blocked by
    /// an outstanding lore in-text link to it, and the referencing
    /// `world_lore_links` row's `target_item_id` is nulled (via the
    /// migration's `ON DELETE SET NULL`) rather than the row being
    /// removed — the existing broken-link render path (spec 012) treats
    /// a null-FK'd row as unresolved with no new code path required.
    #[tokio::test]
    async fn deleting_an_item_nulls_referencing_lore_links_instead_of_blocking() {
        use crate::models::LoreEntry;
        use crate::schema::{world_lore_entries, world_lore_links};

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let item = create_item_impl(
            &state,
            owner_id,
            false,
            CreateItemInput {
                world_id,
                name: "Potion of Healing".to_string(),
                description: None,
            },
        )
        .await
        .expect("DM should create item");

        let mut conn = state.db_pool.get().unwrap();
        let now = chrono::Utc::now().naive_utc();
        let lore_entry_id = uuid::Uuid::now_v7();
        diesel::insert_into(world_lore_entries::table)
            .values((
                world_lore_entries::id.eq(lore_entry_id),
                world_lore_entries::world_id.eq(world_id),
                world_lore_entries::title.eq("Alchemist's Notes"),
                world_lore_entries::slug.eq("alchemists-notes"),
                world_lore_entries::content.eq("See [[Potion of Healing]]."),
                world_lore_entries::created_by.eq(owner_id),
                world_lore_entries::created_at.eq(now),
                world_lore_entries::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("failed to insert test lore entry");

        let link_id = uuid::Uuid::now_v7();
        diesel::insert_into(world_lore_links::table)
            .values((
                world_lore_links::id.eq(link_id),
                world_lore_links::source_lore_entry_id.eq(lore_entry_id),
                world_lore_links::raw_title.eq("Potion of Healing"),
                world_lore_links::target_kind.eq("item"),
                world_lore_links::target_item_id.eq(item.id),
            ))
            .execute(&mut conn)
            .expect("failed to insert test lore link");
        drop(conn);

        delete_item_impl(&state, owner_id, false, item.id)
            .await
            .expect("delete must not be blocked by the outstanding lore link");

        let mut conn = state.db_pool.get().unwrap();
        let target_item_id: Option<uuid::Uuid> = world_lore_links::table
            .filter(world_lore_links::id.eq(link_id))
            .select(world_lore_links::target_item_id)
            .first(&mut conn)
            .expect("lore link row must still exist");
        assert_eq!(
            target_item_id, None,
            "target_item_id must be nulled by ON DELETE SET NULL, not left dangling"
        );

        // The source entry itself is untouched — only the link's target FK
        // was nulled, per data-model.md's ON DELETE SET NULL rationale.
        let _: LoreEntry = world_lore_entries::table
            .filter(world_lore_entries::id.eq(lore_entry_id))
            .select(LoreEntry::as_select())
            .first(&mut conn)
            .expect("source lore entry must be unaffected");
    }
}
