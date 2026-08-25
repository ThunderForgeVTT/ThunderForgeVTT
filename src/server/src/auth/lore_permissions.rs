//! Spec 012: lore entry ownership/permission enforcement — generalizes
//! `auth::actor_permissions` (spec 010) to `world_lore_entries`: the
//! world's DM (Owner or GM role) always has implicit, un-removable
//! `Owner`-equivalent access to every lore entry in their world; every
//! other member defaults to `Viewer` unless an explicit
//! `world_lore_permissions` row says otherwise. See
//! `specs/012-lore-wiki/data-model.md` and `contracts/lore-crud.md`.



// Spec 027 (US5): the `effective_lore_permission` / `require_lore_permission` pair that
// lived here is now generated from the single declaration in
// `auth::permissioned_entities`, under the same names and signatures — so no
// caller changed. Three other modules carried a near-verbatim copy of the same
// logic; one of them shipped without its member-removal cleanup, which is the
// privilege leak that motivated consolidating them.
//
// Re-exported here so existing import paths keep working.
pub use crate::auth::permissioned_entities::{effective_lore_permission, require_lore_permission};

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::prelude::*;
    use uuid::Uuid;

    use crate::graphql::types::ActorPermissionLevel;
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
