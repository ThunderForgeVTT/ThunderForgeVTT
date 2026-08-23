//! World actor queries (spec 009: GM staging page's NPC roster).

use async_graphql::Context;

use crate::graphql::*;
use crate::state::AppState;

/// Testable core of `ActorQuery::world_actors`, split out so tests don't
/// need a GraphQL `Context` (see `mutations_assets.rs`'s `_impl` convention).
pub async fn world_actors_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    world_id: uuid::Uuid,
) -> GraphQLResult<Vec<WorldActor>> {
    // 🔐 SECURITY: same visibility rule as `scenes(worldId)`.
    require_visible_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let rows = tokio::task::spawn_blocking(move || {
        world_actors::table
            .filter(world_actors::world_id.eq(world_id))
            .select(WorldActor::as_select())
            .load::<WorldActor>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load world actors"))?;

    // Spec 015 (contracts/graphql-moderation.md): a moderation-disabled
    // actor is excluded from every list query, for every caller.
    crate::moderation::filter_visible(state, "world_actor", rows, |a| a.id).await
}

/// Testable core of `ActorQuery::search_actors`. Server-side counterpart
/// to the client's FlexSearch index (`@/search/actorSearch.ts`) — a plain
/// case-insensitive substring match against `label`/`description`, for
/// callers that haven't mirrored the full roster locally. Same visibility
/// rule as `world_actors_impl`.
pub async fn search_actors_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    world_id: uuid::Uuid,
    query: &str,
) -> GraphQLResult<Vec<WorldActor>> {
    require_visible_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));

    let rows = tokio::task::spawn_blocking(move || {
        world_actors::table
            .filter(world_actors::world_id.eq(world_id))
            .filter(
                world_actors::label
                    .ilike(pattern.clone())
                    .or(world_actors::description.ilike(pattern)),
            )
            .select(WorldActor::as_select())
            .load::<WorldActor>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to search world actors"))?;

    crate::moderation::filter_visible(state, "world_actor", rows, |a| a.id).await
}

#[derive(Default)]
pub struct ActorQuery;

#[async_graphql::Object]
impl ActorQuery {
    /// All actors (NPCs and player characters, distinguished by `isNpc`)
    /// belonging to a world, across every scene. World-scoped (not
    /// scene-scoped) to match how a GM staging page shows one roster for
    /// the whole world (research.md §2).
    async fn world_actors(
        &self,
        ctx: &Context<'_>,
        world_id: uuid::Uuid,
    ) -> GraphQLResult<Vec<GraphQLWorldActor>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;

        let actors =
            world_actors_impl(state, auth_user.user_id, auth_user.is_admin, world_id).await?;

        Ok(actors.into_iter().map(GraphQLWorldActor::from).collect())
    }

    /// Server-side search pairing for the client's FlexSearch index —
    /// see `search_actors_impl`.
    async fn search_actors(
        &self,
        ctx: &Context<'_>,
        world_id: uuid::Uuid,
        query: String,
    ) -> GraphQLResult<Vec<GraphQLWorldActor>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;

        let actors = search_actors_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            &query,
        )
        .await?;

        Ok(actors.into_iter().map(GraphQLWorldActor::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{search_actors_impl, world_actors_impl};
    use crate::schema::world_actors;
    use crate::test_support::{insert_test_user, insert_test_world, test_app_state};
    use diesel::prelude::*;

    fn insert_test_actor(
        conn: &mut diesel::PgConnection,
        world_id: uuid::Uuid,
        scene_id: uuid::Uuid,
        owner_id: uuid::Uuid,
        is_npc: bool,
    ) -> uuid::Uuid {
        insert_test_actor_with_label(
            conn,
            world_id,
            scene_id,
            owner_id,
            is_npc,
            "Test Actor",
            None,
        )
    }

    fn insert_test_actor_with_label(
        conn: &mut diesel::PgConnection,
        world_id: uuid::Uuid,
        scene_id: uuid::Uuid,
        owner_id: uuid::Uuid,
        is_npc: bool,
        label: &str,
        description: Option<&str>,
    ) -> uuid::Uuid {
        let id = uuid::Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_actors::table)
            .values((
                world_actors::id.eq(id),
                world_actors::world_id.eq(world_id),
                world_actors::scene_id.eq(scene_id),
                world_actors::actor_type.eq(if is_npc { "npc" } else { "character" }),
                world_actors::game_system_id.eq("dnd5e"),
                world_actors::label.eq(label),
                world_actors::description.eq(description),
                world_actors::created_by.eq(owner_id),
                world_actors::owned_by.eq(owner_id),
                world_actors::is_public.eq(false),
                world_actors::is_npc.eq(is_npc),
                world_actors::created_at.eq(now),
                world_actors::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("failed to insert test actor");
        id
    }

    /// Spec 009 (T003): a world member sees the world's actors, including
    /// both NPC and non-NPC rows — `world_actors` returns the full set, the
    /// NPC-only filtering is a UI concern (contracts/world-actors-query.md).
    #[tokio::test]
    async fn world_actors_returns_npc_and_non_npc_rows_for_a_member() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = crate::test_support::insert_test_scene(&mut conn, world_id, owner_id);
        insert_test_actor(&mut conn, world_id, scene_id, owner_id, true);
        insert_test_actor(&mut conn, world_id, scene_id, owner_id, false);
        drop(conn);

        let actors = world_actors_impl(&state, owner_id, false, world_id)
            .await
            .expect("owner should be able to list actors");

        assert_eq!(actors.len(), 2);
        assert!(actors.iter().any(|a| a.is_npc));
        assert!(actors.iter().any(|a| !a.is_npc));
    }

    /// Spec 009 (T003): a non-member/non-owner is rejected, matching
    /// `scenes(worldId)`'s existing visibility rule (research.md §2).
    #[tokio::test]
    async fn world_actors_rejects_non_member_non_owner() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let outsider_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let result = world_actors_impl(&state, outsider_id, false, world_id).await;

        assert!(
            result.is_err(),
            "a user with no ownership or world_members row must not see the world's actors"
        );
    }

    /// Server-side counterpart to the client's FlexSearch index: a
    /// case-insensitive substring match against label OR description.
    #[tokio::test]
    async fn search_actors_matches_label_or_description_case_insensitively() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = crate::test_support::insert_test_scene(&mut conn, world_id, owner_id);
        insert_test_actor_with_label(
            &mut conn,
            world_id,
            scene_id,
            owner_id,
            true,
            "Bo Jangles",
            Some("A dancing skeleton bard"),
        );
        insert_test_actor_with_label(
            &mut conn,
            world_id,
            scene_id,
            owner_id,
            true,
            "Greta the Grim",
            None,
        );
        drop(conn);

        let by_label = search_actors_impl(&state, owner_id, false, world_id, "jangles")
            .await
            .expect("owner should be able to search actors");
        assert_eq!(by_label.len(), 1);
        assert_eq!(by_label[0].label, "Bo Jangles");

        let by_description = search_actors_impl(&state, owner_id, false, world_id, "DANCING")
            .await
            .expect("search should be case-insensitive");
        assert_eq!(by_description.len(), 1);
        assert_eq!(by_description[0].label, "Bo Jangles");

        let no_match = search_actors_impl(&state, owner_id, false, world_id, "nonexistent")
            .await
            .expect("a query with no matches is not an error");
        assert!(no_match.is_empty());
    }

    /// Same visibility rule as `world_actors_rejects_non_member_non_owner`.
    #[tokio::test]
    async fn search_actors_rejects_non_member_non_owner() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let outsider_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let result = search_actors_impl(&state, outsider_id, false, world_id, "anything").await;

        assert!(
            result.is_err(),
            "a user with no ownership or world_members row must not be able to search the world's actors"
        );
    }
}
