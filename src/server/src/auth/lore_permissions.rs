//! Spec 012: lore entry ownership/permission enforcement — generalizes
//! `auth::actor_permissions` (spec 010) to `world_lore_entries`: the
//! world's DM (Owner or GM role) always has implicit, un-removable
//! `Owner`-equivalent access to every lore entry in their world; every
//! other member defaults to `Viewer` unless an explicit
//! `world_lore_permissions` row says otherwise. See
//! `specs/012-lore-wiki/data-model.md` and `contracts/lore-crud.md`.

use diesel::prelude::*;
use uuid::Uuid;

pub use crate::auth::actor_permissions::is_dm_of_world;
use crate::graphql::types::ActorPermissionLevel;
use crate::schema::{world_lore_entries, world_lore_permissions};
use crate::state::AppState;
use async_graphql::{Error, ErrorExtensions, Result as GraphQLResult};

/// Resolves the caller's effective permission level on one lore entry:
/// DM of the entry's world → always `Owner`; else the caller's explicit
/// `world_lore_permissions` row, if any; else `Viewer`.
pub async fn effective_lore_permission(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    lore_entry_id: Uuid,
) -> GraphQLResult<ActorPermissionLevel> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let world_id = tokio::task::spawn_blocking(move || {
        world_lore_entries::table
            .filter(world_lore_entries::id.eq(lore_entry_id))
            .select(world_lore_entries::world_id)
            .first::<Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load lore entry"))?
    .ok_or_else(|| Error::new("Lore entry not found"))?;

    if is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Ok(ActorPermissionLevel::Owner);
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let level = tokio::task::spawn_blocking(move || {
        world_lore_permissions::table
            .filter(world_lore_permissions::lore_entry_id.eq(lore_entry_id))
            .filter(world_lore_permissions::world_member_user_id.eq(user_id))
            .select(world_lore_permissions::level)
            .first::<String>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load lore permission"))?;

    Ok(level
        .and_then(|value| ActorPermissionLevel::from_db_str(&value))
        .unwrap_or(ActorPermissionLevel::Viewer))
}

/// Rejects the caller unless their effective permission on `lore_entry_id`
/// is at least `minimum`. Every lore-entry-mutating GraphQL resolver
/// (`updateLoreEntry`, `deleteLoreEntry`, `uploadLoreImage`,
/// `restoreLoreRevision`) calls this instead of re-deriving permission
/// logic inline.
pub async fn require_lore_permission(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    lore_entry_id: Uuid,
    minimum: ActorPermissionLevel,
) -> GraphQLResult<()> {
    let level = effective_lore_permission(state, user_id, is_admin, lore_entry_id).await?;

    if level.rank() >= minimum.rank() {
        Ok(())
    } else {
        Err(
            Error::new("You do not have sufficient permission on this lore entry")
                .extend_with(|_, ext| ext.set("code", "FORBIDDEN")),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::world_lore_entries;
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

    fn insert_test_lore_entry(
        conn: &mut diesel::PgConnection,
        world_id: Uuid,
        created_by: Uuid,
    ) -> Uuid {
        let id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_lore_entries::table)
            .values((
                world_lore_entries::id.eq(id),
                world_lore_entries::world_id.eq(world_id),
                world_lore_entries::title.eq("Test Lore Entry"),
                world_lore_entries::slug.eq("test-lore-entry"),
                world_lore_entries::content.eq(""),
                world_lore_entries::created_by.eq(created_by),
                world_lore_entries::created_at.eq(now),
                world_lore_entries::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("failed to insert test lore entry");
        id
    }

    /// The world's DM always resolves to `Owner`, even with zero explicit
    /// permission rows.
    #[tokio::test]
    async fn dm_always_resolves_to_owner_with_no_explicit_rows() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let entry_id = insert_test_lore_entry(&mut conn, world_id, owner_id);
        drop(conn);

        let level = effective_lore_permission(&state, owner_id, false, entry_id)
            .await
            .expect("DM permission resolution should succeed");

        assert_eq!(level, ActorPermissionLevel::Owner);
    }

    /// FR-003: a member with no explicit `world_lore_permissions` row
    /// defaults to Viewer.
    #[tokio::test]
    async fn member_with_no_explicit_row_defaults_to_viewer() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let entry_id = insert_test_lore_entry(&mut conn, world_id, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let level = effective_lore_permission(&state, player_id, false, entry_id)
            .await
            .expect("permission resolution should succeed");

        assert_eq!(level, ActorPermissionLevel::Viewer);
    }

    /// `require_lore_permission` rejects a below-minimum caller and
    /// accepts an at-or-above one.
    #[tokio::test]
    async fn require_lore_permission_enforces_minimum() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let entry_id = insert_test_lore_entry(&mut conn, world_id, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let denied =
            require_lore_permission(&state, player_id, false, entry_id, ActorPermissionLevel::Editor)
                .await;
        assert!(denied.is_err(), "default-Viewer caller must not meet Editor minimum");

        let allowed =
            require_lore_permission(&state, owner_id, false, entry_id, ActorPermissionLevel::Owner)
                .await;
        assert!(allowed.is_ok(), "the DM (implicit Owner) must meet Owner minimum");
    }
}
