//! User queries for profile, worlds, tokens, policies, and data exports.

use async_graphql::Context;

use crate::graphql::*;
use crate::state::AppState;
use crate::users::export_user_data_payload;

/// Testable core of `UserQuery::me` (see `actor.rs`'s `_impl` convention).
pub async fn me_impl(
    state: &AppState,
    user_id: uuid::Uuid,
) -> GraphQLResult<Option<crate::models::User>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        use crate::models::User;
        use crate::schema::users;
        use diesel::prelude::*;
        users::table
            .filter(users::id.eq(user_id))
            .select(User::as_select())
            .first::<User>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load user"))
}

/// Testable core of `UserQuery::my_worlds_with_role` — every world the
/// caller owns or holds an accepted `world_members` row for, paired with
/// their role in it. Owned worlds win the role label ("Owner") over a
/// stray `world_members` row for the same world, since `created_by` is
/// this codebase's actual ownership source of truth (see
/// `auth/world_membership.rs`'s `require_world_member` for the same
/// created_by-then-world_members fallback pattern).
pub async fn my_worlds_with_role_impl(
    state: &AppState,
    user_id: uuid::Uuid,
) -> GraphQLResult<Vec<(crate::models::World, String)>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        use crate::models::World;
        use crate::schema::{world_members, worlds};
        use diesel::prelude::*;

        let owned = worlds::table
            .filter(worlds::created_by.eq(user_id))
            .select(World::as_select())
            .load::<World>(&mut conn)?;

        let member_rows: Vec<(uuid::Uuid, String)> = world_members::table
            .filter(world_members::user_id.eq(user_id))
            .select((world_members::world_id, world_members::role))
            .load::<(uuid::Uuid, String)>(&mut conn)?;
        let member_world_ids: Vec<uuid::Uuid> = member_rows.iter().map(|(id, _)| *id).collect();
        let member_worlds = worlds::table
            .filter(worlds::id.eq_any(member_world_ids))
            .select(World::as_select())
            .load::<World>(&mut conn)?;

        let mut combined: Vec<(World, String)> = owned
            .into_iter()
            .map(|w| (w, "Owner".to_string()))
            .collect();
        for world in member_worlds {
            if combined.iter().any(|(w, _)| w.id == world.id) {
                continue;
            }
            let role = member_rows
                .iter()
                .find(|(id, _)| *id == world.id)
                .map(|(_, role)| role.clone())
                .unwrap_or_else(|| "Player".to_string());
            combined.push((world, role));
        }

        Ok::<_, diesel::result::Error>(combined)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query worlds"))
}

/// Testable core of `UserQuery::my_dm_worlds` — combines owned worlds with
/// worlds where the caller holds an accepted `GM` `world_members` row,
/// deduplicated (spec 010, research.md §8).
pub async fn my_dm_worlds_impl(
    state: &AppState,
    user_id: uuid::Uuid,
) -> GraphQLResult<Vec<crate::models::World>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        use crate::models::World;
        use crate::schema::{world_members, worlds};
        use diesel::prelude::*;

        let owned = worlds::table
            .filter(worlds::created_by.eq(user_id))
            .select(World::as_select())
            .load::<World>(&mut conn)?;

        let gm_world_ids = world_members::table
            .filter(world_members::user_id.eq(user_id))
            .filter(world_members::role.eq("GM"))
            .select(world_members::world_id)
            .load::<uuid::Uuid>(&mut conn)?;

        let gm_worlds = worlds::table
            .filter(worlds::id.eq_any(gm_world_ids))
            .select(World::as_select())
            .load::<World>(&mut conn)?;

        let mut combined = owned;
        for world in gm_worlds {
            if !combined.iter().any(|w| w.id == world.id) {
                combined.push(world);
            }
        }

        Ok::<_, diesel::result::Error>(combined)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load DM worlds"))
}

#[derive(Default)]
pub struct UserQuery;

#[async_graphql::Object]
impl UserQuery {
    async fn me(&self, ctx: &Context<'_>) -> GraphQLResult<Option<GraphQLUser>> {
        let state = app_state(ctx)?;
        let user_id = authenticated_user(ctx)?.user_id;
        let user = me_impl(state, user_id).await?;
        Ok(user.map(GraphQLUser::from))
    }

    async fn game_systems(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLGameSystem>> {
        let state = app_state(ctx)?;
        load_game_systems(state)
            .await
            .map(|items| items.into_iter().map(GraphQLGameSystem::from).collect())
    }

    async fn my_worlds(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLWorld>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_worlds(state, auth_user.user_id)
            .await
            .map(|items| items.into_iter().map(GraphQLWorld::from).collect())
    }

    /// Every world the caller owns or is an accepted member of (any role),
    /// paired with their role — powers the Welcome page hub, which should
    /// show a user's full roster (owned AND joined-as-player worlds), not
    /// just `myWorlds`' owned-only list.
    async fn my_worlds_with_role(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<Vec<GraphQLMyWorldEntry>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let entries = my_worlds_with_role_impl(state, auth_user.user_id).await?;
        Ok(entries
            .into_iter()
            .map(|(world, role)| GraphQLMyWorldEntry {
                world: GraphQLWorld::from(world),
                role,
            })
            .collect())
    }

    /// Spec 010 (research.md §8): worlds where the caller holds DM-level
    /// access — Owner (`created_by`) OR an accepted `GM` `world_members`
    /// row. Deliberately additive alongside `myWorlds` (which only covers
    /// `created_by`) rather than changing that query's existing behavior;
    /// used by the "Copy to World" destination picker.
    async fn my_dm_worlds(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLWorld>> {
        let state = app_state(ctx)?;
        let user_id = authenticated_user(ctx)?.user_id;
        let worlds_list = my_dm_worlds_impl(state, user_id).await?;
        Ok(worlds_list.into_iter().map(GraphQLWorld::from).collect())
    }

    async fn world(
        &self,
        ctx: &Context<'_>,
        id: uuid::Uuid,
    ) -> GraphQLResult<Option<GraphQLWorld>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_visible_world_by_id(state, auth_user.user_id, auth_user.is_admin, id)
            .await
            .map(|item| item.map(GraphQLWorld::from))
    }

    async fn my_world_tokens(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLWorldToken>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_world_tokens(state, auth_user.user_id)
            .await
            .map(|items| items.into_iter().map(GraphQLWorldToken::from).collect())
    }

    async fn world_token(
        &self,
        ctx: &Context<'_>,
        token_id: String,
    ) -> GraphQLResult<Option<GraphQLWorldToken>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_world_token_by_id(state, auth_user.user_id, token_id)
            .await
            .map(|item| item.map(GraphQLWorldToken::from))
    }

    async fn my_world_events(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLWorldEvent>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_world_events(state, auth_user.user_id)
            .await
            .map(|items| items.into_iter().map(GraphQLWorldEvent::from).collect())
    }

    async fn world_event(
        &self,
        ctx: &Context<'_>,
        event_id: i64,
    ) -> GraphQLResult<Option<GraphQLWorldEvent>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_world_event_by_id(state, auth_user.user_id, event_id)
            .await
            .map(|item| item.map(GraphQLWorldEvent::from))
    }

    // async fn my_policies(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLPolicy>> {
    //     let state = app_state(ctx)?;
    //     let auth_user = authenticated_user(ctx)?;
    //     load_owned_policies(state, auth_user.user_id)
    //         .await
    //         .map(|items| items.into_iter().map(GraphQLPolicy::from).collect())
    // }
    //
    // async fn policy(
    //     &self,
    //     ctx: &Context<'_>,
    //     policy_id: uuid::Uuid,
    // ) -> GraphQLResult<Option<GraphQLPolicy>> {
    //     let state = app_state(ctx)?;
    //     let auth_user = authenticated_user(ctx)?;
    //     load_owned_policy_by_id(state, auth_user.user_id, policy_id)
    //         .await
    //         .map(|item| item.map(GraphQLPolicy::from))
    // }

    async fn export_my_data(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLExportMyDataPayload> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        export_user_data_payload(state, auth_user.user_id)
            .await
            .map(GraphQLExportMyDataPayload::from)
            .map_err(Error::new)
    }
}

#[cfg(test)]
mod tests {
    use super::{me_impl, my_dm_worlds_impl, my_worlds_with_role_impl};
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

    #[tokio::test]
    async fn me_returns_the_matching_user_row() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);

        let user = me_impl(&state, user_id)
            .await
            .expect("query should not error")
            .expect("the just-inserted user should be found");

        assert_eq!(user.id, user_id);
    }

    #[tokio::test]
    async fn me_returns_none_for_an_unknown_user_id() {
        let state = test_app_state();

        let user = me_impl(&state, uuid::Uuid::now_v7())
            .await
            .expect("query should not error");

        assert!(user.is_none());
    }

    #[tokio::test]
    async fn my_dm_worlds_includes_owned_and_gm_worlds_deduplicated() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        let owned_world_id = insert_test_world(&mut conn, user_id);
        let other_owner_id = insert_test_user(&mut conn);
        let gm_world_id = insert_test_world(&mut conn, other_owner_id);
        insert_test_world_member(&mut conn, gm_world_id, user_id, "GM");
        // A world the user merely plays in (Player role) must NOT appear.
        let player_world_owner = insert_test_user(&mut conn);
        let player_world_id = insert_test_world(&mut conn, player_world_owner);
        insert_test_world_member(&mut conn, player_world_id, user_id, "Player");
        drop(conn);

        let worlds = my_dm_worlds_impl(&state, user_id)
            .await
            .expect("query should not error");

        let ids: Vec<uuid::Uuid> = worlds.iter().map(|w| w.id).collect();
        assert!(
            ids.contains(&owned_world_id),
            "owned worlds must be included"
        );
        assert!(
            ids.contains(&gm_world_id),
            "GM-role worlds must be included"
        );
        assert!(
            !ids.contains(&player_world_id),
            "worlds where the user only holds Player role must NOT be included"
        );
        assert_eq!(
            ids.len(),
            2,
            "no world should appear twice even if owned and GM somehow overlapped"
        );
    }

    #[tokio::test]
    async fn my_worlds_with_role_includes_owned_gm_and_player_worlds_with_correct_roles() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        let owned_world_id = insert_test_world(&mut conn, user_id);

        let other_owner_id = insert_test_user(&mut conn);
        let gm_world_id = insert_test_world(&mut conn, other_owner_id);
        insert_test_world_member(&mut conn, gm_world_id, user_id, "GM");

        let player_world_owner = insert_test_user(&mut conn);
        let player_world_id = insert_test_world(&mut conn, player_world_owner);
        insert_test_world_member(&mut conn, player_world_id, user_id, "Player");

        // A world this user has no relationship to must never appear.
        let stranger_id = insert_test_user(&mut conn);
        let _stranger_world_id = insert_test_world(&mut conn, stranger_id);
        drop(conn);

        let entries = my_worlds_with_role_impl(&state, user_id)
            .await
            .expect("query should not error");

        let role_of = |world_id: uuid::Uuid| -> Option<String> {
            entries
                .iter()
                .find(|(w, _)| w.id == world_id)
                .map(|(_, role)| role.clone())
        };

        assert_eq!(role_of(owned_world_id), Some("Owner".to_string()));
        assert_eq!(role_of(gm_world_id), Some("GM".to_string()));
        assert_eq!(
            role_of(player_world_id),
            Some("Player".to_string()),
            "Player-role worlds must be included, unlike my_dm_worlds"
        );
        assert_eq!(
            entries.len(),
            3,
            "only worlds the user owns or is a member of should appear"
        );
    }
}
