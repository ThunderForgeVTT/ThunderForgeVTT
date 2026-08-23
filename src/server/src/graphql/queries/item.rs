//! Spec 013: Item catalog queries (`worldItems`, `item`, `suggestItemName`).
//! See contracts/graphql-items.md.

use async_graphql::Context;
use diesel::dsl::sql;
use diesel::sql_types::{Bool, Double, Text};

use crate::graphql::types::GraphQLItem;
use crate::graphql::*;
use crate::models::{ItemEffect, WorldItem};
use crate::schema::{world_item_effects, world_items};
use crate::state::AppState;

async fn load_item_effects(
    state: &AppState,
    item_id: uuid::Uuid,
) -> GraphQLResult<Vec<ItemEffect>> {
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
    user_id: uuid::Uuid,
    is_admin: bool,
    row: WorldItem,
) -> GraphQLResult<GraphQLItem> {
    let effects = load_item_effects(state, row.id).await?;
    let my_permission_level =
        crate::auth::item_permissions::effective_item_permission(state, user_id, is_admin, row.id)
            .await?;
    Ok(GraphQLItem::from_row(row, effects, my_permission_level))
}

/// Testable core of `ItemQuery::world_items`. Every world member sees
/// every item at at least Viewer level by default (FR-008); `search`
/// filters `name`/`description` case-insensitively, matching the NPC
/// catalog's existing search behavior.
pub async fn world_items_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    world_id: uuid::Uuid,
    search: Option<String>,
) -> GraphQLResult<Vec<WorldItem>> {
    require_visible_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        let mut query = world_items::table
            .filter(world_items::world_id.eq(world_id))
            .into_boxed();

        if let Some(term) = search.as_ref().filter(|s| !s.trim().is_empty()) {
            let pattern = format!("%{}%", term.replace('%', "\\%").replace('_', "\\_"));
            query = query.filter(
                world_items::name
                    .ilike(pattern.clone())
                    .or(world_items::description.ilike(pattern)),
            );
        }

        query
            .order(world_items::name.asc())
            .select(WorldItem::as_select())
            .load::<WorldItem>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load world items"))
}

/// Testable core of `ItemQuery::item`. Denies access to anyone without at
/// least Viewer access under the item's ownership block (FR-018) —
/// enforced here by requiring at least world visibility, then letting the
/// caller's `myPermissionLevel` on the response reflect their real level
/// (every world member defaults to Viewer, per FR-008/FR-003).
pub async fn item_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    item_id: uuid::Uuid,
) -> GraphQLResult<WorldItem> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let row = tokio::task::spawn_blocking(move || {
        world_items::table
            .filter(world_items::id.eq(item_id))
            .select(WorldItem::as_select())
            .first::<WorldItem>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load item"))?
    .ok_or_else(|| Error::new("Item not found"))?;

    require_visible_world(state, user_id, is_admin, row.world_id).await?;

    Ok(row)
}

/// Testable core of `ItemQuery::suggest_item_name`. Non-blocking "did you
/// mean?" nudge (FR-020) — uses the `pg_trgm` extension's `similarity()`
/// function against the trigram GIN index on `world_items.name`
/// (research.md §3). Never gates `createItem`.
pub async fn suggest_item_name_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    world_id: uuid::Uuid,
    name: String,
) -> GraphQLResult<Vec<WorldItem>> {
    require_visible_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_items::table
            .filter(world_items::world_id.eq(world_id))
            .filter(
                sql::<Bool>("similarity(name, ")
                    .bind::<Text, _>(name.clone())
                    .sql(") > 0.4"),
            )
            .order(
                sql::<Double>("similarity(name, ")
                    .bind::<Text, _>(name)
                    .sql(")")
                    .desc(),
            )
            .limit(5)
            .select(WorldItem::as_select())
            .load::<WorldItem>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to search item names"))
}

#[derive(Default)]
pub struct ItemQuery;

#[async_graphql::Object]
impl ItemQuery {
    async fn world_items(
        &self,
        ctx: &Context<'_>,
        world_id: uuid::Uuid,
        search: Option<String>,
    ) -> GraphQLResult<Vec<GraphQLItem>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let rows = world_items_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            search,
        )
        .await?;
        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            result.push(to_graphql_item(state, auth_user.user_id, auth_user.is_admin, row).await?);
        }
        Ok(result)
    }

    async fn item(&self, ctx: &Context<'_>, item_id: uuid::Uuid) -> GraphQLResult<GraphQLItem> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let row = item_impl(state, auth_user.user_id, auth_user.is_admin, item_id).await?;
        to_graphql_item(state, auth_user.user_id, auth_user.is_admin, row).await
    }

    async fn suggest_item_name(
        &self,
        ctx: &Context<'_>,
        world_id: uuid::Uuid,
        name: String,
    ) -> GraphQLResult<Vec<GraphQLItem>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let rows =
            suggest_item_name_impl(state, auth_user.user_id, auth_user.is_admin, world_id, name)
                .await?;
        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            result.push(to_graphql_item(state, auth_user.user_id, auth_user.is_admin, row).await?);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_items::{CreateItemInput, create_item_impl};
    use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

    /// FR-008: every world member sees every item at at least Viewer.
    #[tokio::test]
    async fn world_items_returns_all_items_for_a_member() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        create_item_impl(
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

        let items = world_items_impl(&state, owner_id, false, world_id, None)
            .await
            .expect("member should list items");
        assert_eq!(items.len(), 1);
    }

    /// FR-020: near-name matches surface via similarity, exact non-matches don't.
    #[tokio::test]
    async fn suggest_item_name_finds_close_matches() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        create_item_impl(
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

        let suggestions = suggest_item_name_impl(
            &state,
            owner_id,
            false,
            world_id,
            "Potion of Healng".to_string(),
        )
        .await
        .expect("suggest query should succeed");
        assert_eq!(
            suggestions.len(),
            1,
            "a close typo should still surface the existing item"
        );

        let none = suggest_item_name_impl(
            &state,
            owner_id,
            false,
            world_id,
            "Completely Unrelated Thing".to_string(),
        )
        .await
        .expect("suggest query should succeed");
        assert!(none.is_empty());
    }
}
