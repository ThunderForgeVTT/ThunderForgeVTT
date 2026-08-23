//! Spec 013: the item "ownership block" — DM-only permission grants
//! (`setItemPermission`). Direct structural mirror of
//! `mutations_actor_permissions.rs`. See contracts/graphql-items.md.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::actor_permissions::is_dm_of_world;
use crate::graphql::types::{ActorPermissionLevel, GraphQLItemPermission};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{ItemPermission, NewItemPermission};
use crate::schema::{world_item_permissions, world_items};
use crate::state::AppState;

#[derive(InputObject, Debug, Clone)]
pub struct SetItemPermissionInput {
    pub item_id: Uuid,
    pub user_id: Uuid,
    pub level: ActorPermissionLevel,
}

async fn require_dm_of_items_world(
    state: &AppState,
    caller_id: Uuid,
    is_admin: bool,
    item_id: Uuid,
) -> GraphQLResult<()> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let world_id = tokio::task::spawn_blocking(move || {
        world_items::table
            .filter(world_items::id.eq(item_id))
            .select(world_items::world_id)
            .first::<Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load item"))?
    .ok_or_else(|| Error::new("Item not found"))?;

    if is_dm_of_world(state, caller_id, is_admin, world_id).await? {
        Ok(())
    } else {
        Err(Error::new(
            "Only the DM (Owner or GM) may view or change an item's ownership block",
        ))
    }
}

/// Testable core of `ItemPermissionQuery::item_permissions`.
pub async fn item_permissions_impl(
    state: &AppState,
    caller_id: Uuid,
    is_admin: bool,
    item_id: Uuid,
) -> GraphQLResult<Vec<ItemPermission>> {
    require_dm_of_items_world(state, caller_id, is_admin, item_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_item_permissions::table
            .filter(world_item_permissions::item_id.eq(item_id))
            .select(ItemPermission::as_select())
            .load::<ItemPermission>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load item permissions"))
}

/// Testable core of `ItemPermissionMutation::set_item_permission`. DM-only
/// (FR-003). UPSERT on `(item_id, user_id)`.
pub async fn set_item_permission_impl(
    state: &AppState,
    caller_id: Uuid,
    is_admin: bool,
    input: SetItemPermissionInput,
) -> GraphQLResult<ItemPermission> {
    require_dm_of_items_world(state, caller_id, is_admin, input.item_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let item_id = input.item_id;
    let target_user_id = input.user_id;
    let level = input.level.as_db_str().to_string();

    tokio::task::spawn_blocking(move || {
        let new_row = NewItemPermission {
            id: Uuid::now_v7(),
            item_id,
            user_id: target_user_id,
            level: level.clone(),
        };

        diesel::insert_into(world_item_permissions::table)
            .values(&new_row)
            .on_conflict((world_item_permissions::item_id, world_item_permissions::user_id))
            .do_update()
            .set((
                world_item_permissions::level.eq(level),
                world_item_permissions::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .returning(ItemPermission::as_returning())
            .get_result::<ItemPermission>(&mut conn)
            .map_err(|e| format!("Failed to set item permission: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Testable core of `ItemPermissionMutation::remove_item_permission`.
/// DM-only. Idempotent — resets a member back to the implicit default
/// Viewer level (mirrors `remove_actor_permission_impl`).
pub async fn remove_item_permission_impl(
    state: &AppState,
    caller_id: Uuid,
    is_admin: bool,
    item_id: Uuid,
    user_id: Uuid,
) -> GraphQLResult<bool> {
    require_dm_of_items_world(state, caller_id, is_admin, item_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        diesel::delete(
            world_item_permissions::table
                .filter(world_item_permissions::item_id.eq(item_id))
                .filter(world_item_permissions::user_id.eq(user_id)),
        )
        .execute(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to remove item permission"))?;

    Ok(true)
}

#[derive(Default)]
pub struct ItemPermissionQuery;

#[async_graphql::Object]
impl ItemPermissionQuery {
    /// DM-only (FR-003). Returns only explicit rows — members with no row
    /// default to Viewer.
    async fn item_permissions(&self, ctx: &Context<'_>, item_id: Uuid) -> GraphQLResult<Vec<GraphQLItemPermission>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let rows = item_permissions_impl(state, auth_user.user_id, auth_user.is_admin, item_id).await?;
        Ok(rows.into_iter().map(GraphQLItemPermission::from).collect())
    }
}

#[derive(Default)]
pub struct ItemPermissionMutation;

#[async_graphql::Object]
impl ItemPermissionMutation {
    async fn set_item_permission(
        &self,
        ctx: &Context<'_>,
        input: SetItemPermissionInput,
    ) -> GraphQLResult<GraphQLItemPermission> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        set_item_permission_impl(state, auth_user.user_id, auth_user.is_admin, input)
            .await
            .map(GraphQLItemPermission::from)
    }

    async fn remove_item_permission(&self, ctx: &Context<'_>, item_id: Uuid, user_id: Uuid) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        remove_item_permission_impl(state, auth_user.user_id, auth_user.is_admin, item_id, user_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_items::{create_item_impl, CreateItemInput};
    use crate::test_support::{insert_test_user, insert_test_world, insert_test_world_member, test_app_state};

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
        assert!(denied.is_err(), "a non-DM caller must not be able to set item permissions");

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
