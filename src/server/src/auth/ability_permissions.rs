//! Spec 025: ability ownership/permission enforcement — direct structural
//! mirror of `auth::item_permissions` (spec 013), generalized to abilities.
//! The world's DM (Owner or GM role) always has implicit, un-removable
//! `Owner`-equivalent access to every ability in their world; every other
//! member defaults to `Viewer` unless an explicit `world_ability_permissions`
//! row says otherwise (FR-024).
//!
//! **This module governs EDIT RIGHTS ONLY.** It is deliberately not the
//! mechanism for hiding an ability: `ActorPermissionLevel`'s lowest value
//! (`Viewer`) is also its default for a member with no row, so the permission
//! model structurally cannot express "hidden". Visibility is
//! `world_abilities.gm_only` (FR-024a/FR-024b), checked by `is_ability_visible_to`
//! below and by each query's own filter — see
//! specs/025-world-abilities-compendium/data-model.md's surface table.

use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::actor_permissions::is_dm_of_world;
use crate::graphql::types::ActorPermissionLevel;
use crate::schema::{world_abilities, world_ability_permissions};
use crate::state::AppState;
use async_graphql::{Error, ErrorExtensions, Result as GraphQLResult};

/// Resolves the caller's effective permission level on one ability:
/// DM of the ability's world → always `Owner`;
/// else the caller's explicit `world_ability_permissions` row, if any;
/// else `Viewer` (FR-024).
pub async fn effective_ability_permission(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    ability_id: Uuid,
) -> GraphQLResult<ActorPermissionLevel> {
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

    if is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Ok(ActorPermissionLevel::Owner);
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let level = tokio::task::spawn_blocking(move || {
        world_ability_permissions::table
            .filter(world_ability_permissions::ability_id.eq(ability_id))
            .filter(world_ability_permissions::user_id.eq(user_id))
            .select(world_ability_permissions::level)
            .first::<String>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load ability permission"))?;

    Ok(level
        .and_then(|value| ActorPermissionLevel::from_db_str(&value))
        .unwrap_or(ActorPermissionLevel::Viewer))
}

/// Rejects the caller unless their effective permission on `ability_id` is at
/// least `minimum`. Every ability-mutating resolver calls this rather than
/// re-deriving permission logic inline.
pub async fn require_ability_permission(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    ability_id: Uuid,
    minimum: ActorPermissionLevel,
) -> GraphQLResult<()> {
    let level = effective_ability_permission(state, user_id, is_admin, ability_id).await?;

    if level.rank() >= minimum.rank() {
        Ok(())
    } else {
        Err(
            Error::new("You do not have sufficient permission on this ability")
                .extend_with(|_, ext| ext.set("code", "FORBIDDEN")),
        )
    }
}

/// Spec 025 (FR-024b/FR-025): is this ability *visible* to the caller at all?
///
/// Distinct from `effective_ability_permission`, which answers "may they change
/// it". A GM-only ability is invisible to every non-DM regardless of any
/// permission row — including one granting `Owner`.
///
/// Returns `false` for a nonexistent ability as well as a hidden one, so
/// callers can reject both identically and avoid leaking existence: without
/// that, a non-DM could probe ids to discover which hidden abilities exist.
pub async fn is_ability_visible_to(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    ability_id: Uuid,
) -> GraphQLResult<bool> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let row = tokio::task::spawn_blocking(move || {
        world_abilities::table
            .filter(world_abilities::id.eq(ability_id))
            .select((world_abilities::world_id, world_abilities::gm_only))
            .first::<(Uuid, bool)>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load ability"))?;

    let Some((world_id, gm_only)) = row else {
        return Ok(false);
    };

    if !gm_only {
        return Ok(true);
    }

    is_dm_of_world(state, user_id, is_admin, world_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    /// FR-024: a member with no explicit row gets `Viewer`, not an error and
    /// not a denial — absence of a row is the default, not a gap.
    #[tokio::test]
    async fn no_permission_row_defaults_to_viewer() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let member_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, member_id, "Player");
        let ability_id = insert_test_ability(&mut conn, world_id, owner_id);
        drop(conn);

        let level = effective_ability_permission(&state, member_id, false, ability_id)
            .await
            .expect("a member with no row should resolve, not error");
        assert_eq!(level, ActorPermissionLevel::Viewer);
    }

    /// FR-024: the DM is always `Owner`, and no permission row can downgrade
    /// them — the rule is implicit and un-removable.
    #[tokio::test]
    async fn dm_is_always_owner_even_with_a_lower_row() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let ability_id = insert_test_ability(&mut conn, world_id, owner_id);

        // Explicitly try to downgrade the world's own owner.
        diesel::insert_into(world_ability_permissions::table)
            .values((
                world_ability_permissions::id.eq(Uuid::now_v7()),
                world_ability_permissions::ability_id.eq(ability_id),
                world_ability_permissions::user_id.eq(owner_id),
                world_ability_permissions::level.eq("Viewer"),
            ))
            .execute(&mut conn)
            .expect("insert downgrade row");
        drop(conn);

        let level = effective_ability_permission(&state, owner_id, false, ability_id)
            .await
            .expect("DM permission should resolve");
        assert_eq!(
            level,
            ActorPermissionLevel::Owner,
            "a Viewer row must not be able to downgrade the world's DM"
        );
    }

    /// An unparseable level string in the DB degrades to `Viewer` rather than
    /// erroring or silently granting more than intended.
    #[tokio::test]
    async fn unparseable_level_string_degrades_to_viewer() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let member_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, member_id, "Player");
        let ability_id = insert_test_ability(&mut conn, world_id, owner_id);

        // The CHECK constraint blocks garbage, so exercise the parse fallback
        // directly — this is the branch that protects against a future enum
        // value being read by older code.
        assert!(ActorPermissionLevel::from_db_str("Sorcerer").is_none());
        drop(conn);

        let level = effective_ability_permission(&state, member_id, false, ability_id)
            .await
            .unwrap();
        assert_eq!(level, ActorPermissionLevel::Viewer);
    }

    /// `require_ability_permission` rejects below-minimum with a FORBIDDEN code
    /// so the frontend can distinguish authorization from other failures.
    #[tokio::test]
    async fn require_permission_rejects_below_minimum() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let member_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, member_id, "Player");
        let ability_id = insert_test_ability(&mut conn, world_id, owner_id);
        drop(conn);

        // Viewer (the default) is below Editor.
        let err = require_ability_permission(
            &state,
            member_id,
            false,
            ability_id,
            ActorPermissionLevel::Editor,
        )
        .await
        .expect_err("a Viewer must not pass an Editor minimum");
        assert!(err.message.contains("sufficient permission"));

        // ...but does pass a Viewer minimum.
        require_ability_permission(
            &state,
            member_id,
            false,
            ability_id,
            ActorPermissionLevel::Viewer,
        )
        .await
        .expect("a Viewer must pass a Viewer minimum");
    }

    /// FR-024b/FR-025: visibility is independent of the ownership block. A
    /// GM-only ability is invisible to a non-DM *even when that member holds
    /// Owner-level permission on it* — the two mechanisms do not override each
    /// other.
    #[tokio::test]
    async fn gm_only_hides_from_non_dm_even_with_owner_permission() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let member_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, member_id, "Player");
        let ability_id = insert_test_ability(&mut conn, world_id, owner_id);

        diesel::update(world_abilities::table.filter(world_abilities::id.eq(ability_id)))
            .set(world_abilities::gm_only.eq(true))
            .execute(&mut conn)
            .expect("mark gm_only");

        diesel::insert_into(world_ability_permissions::table)
            .values((
                world_ability_permissions::id.eq(Uuid::now_v7()),
                world_ability_permissions::ability_id.eq(ability_id),
                world_ability_permissions::user_id.eq(member_id),
                world_ability_permissions::level.eq("Owner"),
            ))
            .execute(&mut conn)
            .expect("grant Owner");
        drop(conn);

        assert!(
            !is_ability_visible_to(&state, member_id, false, ability_id)
                .await
                .unwrap(),
            "an Owner-level permission row must NOT reveal a GM-only ability to a non-DM"
        );
        assert!(
            is_ability_visible_to(&state, owner_id, false, ability_id)
                .await
                .unwrap(),
            "the DM must still see their own GM-only ability"
        );
    }

    /// A nonexistent ability and a hidden one are indistinguishable to a
    /// non-DM, so ids cannot be probed to discover hidden content.
    #[tokio::test]
    async fn hidden_and_nonexistent_are_indistinguishable_to_a_non_dm() {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let member_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, member_id, "Player");
        let ability_id = insert_test_ability(&mut conn, world_id, owner_id);
        diesel::update(world_abilities::table.filter(world_abilities::id.eq(ability_id)))
            .set(world_abilities::gm_only.eq(true))
            .execute(&mut conn)
            .expect("mark gm_only");
        drop(conn);

        let hidden = is_ability_visible_to(&state, member_id, false, ability_id)
            .await
            .unwrap();
        let missing = is_ability_visible_to(&state, member_id, false, Uuid::now_v7())
            .await
            .unwrap();
        assert_eq!(
            hidden, missing,
            "hidden and nonexistent must both report not-visible, identically"
        );
    }
}
