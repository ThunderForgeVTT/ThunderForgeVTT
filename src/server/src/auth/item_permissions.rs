//! Spec 013: item ownership/permission enforcement — direct structural
//! mirror of `auth::actor_permissions` (spec 010), generalized to items.
//! The world's DM (Owner or GM role) always has implicit, un-removable
//! `Owner`-equivalent access to every item in their world; every other
//! member defaults to `Viewer` unless an explicit `world_item_permissions`
//! row says otherwise. See specs/013-items-inventory/research.md.

use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::actor_permissions::is_dm_of_world;
use crate::graphql::types::ActorPermissionLevel;
use crate::schema::{world_item_permissions, world_items};
use crate::state::AppState;
use async_graphql::{Error, ErrorExtensions, Result as GraphQLResult};

/// Resolves the caller's effective permission level on one item:
/// DM of the item's world → always `Owner` (mirrors FR-017 of spec 010);
/// else the caller's explicit `world_item_permissions` row, if any;
/// else `Viewer` (FR-003).
pub async fn effective_item_permission(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    item_id: Uuid,
) -> GraphQLResult<ActorPermissionLevel> {
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

    if is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Ok(ActorPermissionLevel::Owner);
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let level = tokio::task::spawn_blocking(move || {
        world_item_permissions::table
            .filter(world_item_permissions::item_id.eq(item_id))
            .filter(world_item_permissions::user_id.eq(user_id))
            .select(world_item_permissions::level)
            .first::<String>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load item permission"))?;

    Ok(level
        .and_then(|value| ActorPermissionLevel::from_db_str(&value))
        .unwrap_or(ActorPermissionLevel::Viewer))
}

/// Rejects the caller unless their effective permission on `item_id` is
/// at least `minimum`. Every item-mutating GraphQL resolver in spec 013
/// (`updateItem`, `addItemEffect`, ownership-block edits, share-link
/// creation) calls this instead of re-deriving permission logic inline.
pub async fn require_item_permission(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    item_id: Uuid,
    minimum: ActorPermissionLevel,
) -> GraphQLResult<()> {
    let level = effective_item_permission(state, user_id, is_admin, item_id).await?;

    if level.rank() >= minimum.rank() {
        Ok(())
    } else {
        Err(Error::new("You do not have sufficient permission on this item")
            .extend_with(|_, ext| ext.set("code", "FORBIDDEN")))
    }
}
