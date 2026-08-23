//! Spec 012: the lore entry "ownership block" — DM-only permission
//! grants (`setLorePermission`, `loreEntryPermissions`). Direct
//! structural mirror of `mutations_actor_permissions.rs` (spec 010),
//! generalized to lore entries. See
//! `specs/012-lore-wiki/contracts/lore-permissions.md`.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::lore_permissions::is_dm_of_world;
use crate::graphql::types::{ActorPermissionLevel, GraphQLLorePermission};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{LorePermission, NewLorePermission};
use crate::schema::{world_lore_entries, world_lore_permissions};

#[derive(InputObject, Debug, Clone)]
pub struct SetLorePermissionInput {
    pub lore_entry_id: Uuid,
    pub user_id: Uuid,
    pub level: ActorPermissionLevel,
}

async fn require_dm_of_entrys_world(
    state: &crate::state::AppState,
    caller_id: Uuid,
    is_admin: bool,
    lore_entry_id: Uuid,
) -> GraphQLResult<()> {
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

    if is_dm_of_world(state, caller_id, is_admin, world_id).await? {
        Ok(())
    } else {
        Err(Error::new(
            "Only the DM (Owner or GM) may view or change a lore entry's ownership block",
        ))
    }
}

/// Testable core of `LorePermissionQuery::lore_entry_permissions`.
pub async fn lore_entry_permissions_impl(
    state: &crate::state::AppState,
    caller_id: Uuid,
    is_admin: bool,
    lore_entry_id: Uuid,
) -> GraphQLResult<Vec<LorePermission>> {
    require_dm_of_entrys_world(state, caller_id, is_admin, lore_entry_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_lore_permissions::table
            .filter(world_lore_permissions::lore_entry_id.eq(lore_entry_id))
            .select(LorePermission::as_select())
            .load::<LorePermission>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load lore entry permissions"))
}

/// Testable core of `LorePermissionMutation::set_lore_permission`.
/// DM-only. UPSERT on `(lore_entry_id, world_member_user_id)`.
pub async fn set_lore_permission_impl(
    state: &crate::state::AppState,
    caller_id: Uuid,
    is_admin: bool,
    input: SetLorePermissionInput,
) -> GraphQLResult<LorePermission> {
    require_dm_of_entrys_world(state, caller_id, is_admin, input.lore_entry_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let lore_entry_id = input.lore_entry_id;
    let target_user_id = input.user_id;
    let level = input.level.as_db_str().to_string();

    tokio::task::spawn_blocking(move || {
        let new_row = NewLorePermission {
            id: Uuid::now_v7(),
            lore_entry_id,
            world_member_user_id: target_user_id,
            level: level.clone(),
        };

        diesel::insert_into(world_lore_permissions::table)
            .values(&new_row)
            .on_conflict((
                world_lore_permissions::lore_entry_id,
                world_lore_permissions::world_member_user_id,
            ))
            .do_update()
            .set((
                world_lore_permissions::level.eq(level),
                world_lore_permissions::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .returning(LorePermission::as_returning())
            .get_result::<LorePermission>(&mut conn)
            .map_err(|e| format!("Failed to set lore permission: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

#[derive(Default)]
pub struct LorePermissionQuery;

#[async_graphql::Object]
impl LorePermissionQuery {
    /// DM-only. Returns only explicit rows — members with no row default
    /// to Viewer, which the client renders itself by combining this with
    /// the full world-member roster (contracts/lore-permissions.md).
    async fn lore_entry_permissions(
        &self,
        ctx: &Context<'_>,
        lore_entry_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLLorePermission>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let rows =
            lore_entry_permissions_impl(state, auth_user.user_id, auth_user.is_admin, lore_entry_id).await?;
        Ok(rows.into_iter().map(GraphQLLorePermission::from).collect())
    }
}

#[derive(Default)]
pub struct LorePermissionMutation;

#[async_graphql::Object]
impl LorePermissionMutation {
    async fn set_lore_permission(
        &self,
        ctx: &Context<'_>,
        input: SetLorePermissionInput,
    ) -> GraphQLResult<GraphQLLorePermission> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        set_lore_permission_impl(state, auth_user.user_id, auth_user.is_admin, input)
            .await
            .map(GraphQLLorePermission::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_lore::{create_lore_entry_impl, CreateLoreEntryInput};
    use crate::test_support::{insert_test_user, insert_test_world, insert_test_world_member, test_app_state};

    /// FR-003: only the DM may view or change the ownership block; a
    /// non-DM member (even one holding explicit Owner via a prior grant)
    /// is rejected — mirrors spec 010's actor precedent exactly.
    #[tokio::test]
    async fn only_dm_can_set_or_view_lore_permissions() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let entry = create_lore_entry_impl(
            &state,
            owner_id,
            false,
            CreateLoreEntryInput { world_id, title: "Test Entry".to_string(), content: None },
        )
        .await
        .expect("DM should create entry");

        let denied = set_lore_permission_impl(
            &state,
            player_id,
            false,
            SetLorePermissionInput { lore_entry_id: entry.id, user_id: player_id, level: ActorPermissionLevel::Owner },
        )
        .await;
        assert!(denied.is_err(), "a non-DM caller must not be able to set lore permissions");

        let granted = set_lore_permission_impl(
            &state,
            owner_id,
            false,
            SetLorePermissionInput { lore_entry_id: entry.id, user_id: player_id, level: ActorPermissionLevel::Owner },
        )
        .await
        .expect("DM should be able to grant Owner");
        assert_eq!(granted.level, ActorPermissionLevel::Owner.as_db_str());

        let still_denied = lore_entry_permissions_impl(&state, player_id, false, entry.id).await;
        assert!(
            still_denied.is_err(),
            "a player holding explicit Owner on the entry must still not see the ownership block"
        );

        let dm_view = lore_entry_permissions_impl(&state, owner_id, false, entry.id)
            .await
            .expect("DM should be able to view the ownership block");
        assert_eq!(dm_view.len(), 1);
    }

    /// T054 (data-model.md, mirrors FR-022 from spec 010 verbatim): when
    /// a world member holding a lore entry ownership-block entry is
    /// removed from the world, that entry is deleted — this exercises
    /// the exact same scoped-delete query `remove_member`
    /// (`mutations_invites.rs`) issues, confirming it targets only the
    /// removed member's own rows within that world's lore entries (a
    /// same-titled row belonging to a *different* member, or one in a
    /// *different* world, must survive).
    #[tokio::test]
    async fn removed_world_member_lore_permissions_are_cleaned_up_scoped_to_their_world() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let other_player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, other_player_id, "Player");
        drop(conn);

        let entry = create_lore_entry_impl(
            &state,
            owner_id,
            false,
            CreateLoreEntryInput { world_id, title: "Test Entry".to_string(), content: None },
        )
        .await
        .expect("DM should create entry");

        set_lore_permission_impl(
            &state,
            owner_id,
            false,
            SetLorePermissionInput { lore_entry_id: entry.id, user_id: player_id, level: ActorPermissionLevel::Editor },
        )
        .await
        .expect("DM should grant Editor to the departing player");
        set_lore_permission_impl(
            &state,
            owner_id,
            false,
            SetLorePermissionInput { lore_entry_id: entry.id, user_id: other_player_id, level: ActorPermissionLevel::Owner },
        )
        .await
        .expect("DM should grant Owner to the staying player");

        // The exact scoped-delete query `remove_member` issues.
        let mut conn = state.db_pool.get().unwrap();
        diesel::delete(
            world_lore_permissions::table
                .filter(world_lore_permissions::world_member_user_id.eq(player_id))
                .filter(
                    world_lore_permissions::lore_entry_id.eq_any(
                        world_lore_entries::table.filter(world_lore_entries::world_id.eq(world_id)).select(world_lore_entries::id),
                    ),
                ),
        )
        .execute(&mut conn)
        .expect("cascade delete should succeed");
        drop(conn);

        let remaining = lore_entry_permissions_impl(&state, owner_id, false, entry.id)
            .await
            .expect("DM should still be able to view the ownership block");
        assert_eq!(remaining.len(), 1, "only the departed player's entry should be removed");
        assert_eq!(remaining[0].world_member_user_id, other_player_id, "the staying player's grant must survive");
    }
}
