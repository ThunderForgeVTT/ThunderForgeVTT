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

/// Testable core of `ActorQuery::actor_system_data` — the read counterpart
/// to `update_actor_system_data` (graphql.rs's `ActorSystemDataMutation`).
///
/// Found missing while verifying the `pathfinder2e`/`genie` system packs'
/// NPC size-category feature end-to-end (spec 018): only a mutation existed
/// to *write* `world_actor_system_data`, no query existed to *read* it back
/// — the client-side `worldActorSystemDataCollection.ts` RxDB collection's
/// header comment claims "Automatic replication via GraphQL subscriptions"
/// but no replication was ever actually registered for it, so nothing in
/// the running app could ever populate an actor's ability/resource/
/// proficiency/trait data after the initial page load, for any system.
/// This query is the minimal fix: a direct, on-demand read, independent of
/// the (separately, still-unfixed) RxDB replication gap.
pub async fn actor_system_data_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    actor_id: uuid::Uuid,
) -> GraphQLResult<Option<crate::models::ActorSystemData>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    // Look up the actor's world first so we can apply the same
    // membership-visibility rule every other actor-scoped read uses —
    // mirrors world_actors_impl's require_visible_world check.
    let world_id = {
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        tokio::task::spawn_blocking(move || {
            world_actors::table
                .filter(world_actors::id.eq(actor_id))
                .select(world_actors::world_id)
                .first::<uuid::Uuid>(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Actor not found"))?
    };

    require_visible_world(state, user_id, is_admin, world_id).await?;

    let row = tokio::task::spawn_blocking(move || {
        crate::schema::world_actor_system_data::table
            .filter(crate::schema::world_actor_system_data::actor_id.eq(actor_id))
            .select(crate::models::ActorSystemData::as_select())
            .first::<crate::models::ActorSystemData>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load actor system data"))?;

    Ok(row)
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

    /// Read an actor's system-specific data (ability/resource/proficiency/
    /// trait/spell_data) — the read counterpart to `updateActorSystemData`.
    /// Returns `null` if the actor has no system data row yet.
    async fn actor_system_data(
        &self,
        ctx: &Context<'_>,
        actor_id: uuid::Uuid,
    ) -> GraphQLResult<Option<GraphQLActorSystemData>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;

        let row =
            actor_system_data_impl(state, auth_user.user_id, auth_user.is_admin, actor_id)
                .await?;

        Ok(row.map(GraphQLActorSystemData::from))
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
    use super::{actor_system_data_impl, search_actors_impl, world_actors_impl};
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

    /// Regression coverage for the missing read query found while verifying
    /// the pathfinder2e/genie packs' NPC size-category feature (spec 018):
    /// an actor with no `world_actor_system_data` row yet returns `None`,
    /// not an error.
    #[tokio::test]
    async fn actor_system_data_returns_none_when_no_row_exists() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = crate::test_support::insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_actor(&mut conn, world_id, scene_id, owner_id, true);
        drop(conn);

        let result = actor_system_data_impl(&state, owner_id, false, actor_id)
            .await
            .expect("a member should be able to read (the absence of) actor system data");

        assert!(result.is_none());
    }

    /// The core fix: a row written the same way `updateActorSystemData`
    /// writes one (an upsert into `world_actor_system_data`) is readable
    /// back through `actor_system_data_impl` — e.g. an NPC's
    /// `trait_data.size_category`, which previously had no query path back
    /// to any client at all.
    #[tokio::test]
    async fn actor_system_data_returns_the_row_once_written() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = crate::test_support::insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_actor(&mut conn, world_id, scene_id, owner_id, true);

        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(crate::schema::world_actor_system_data::table)
            .values((
                crate::schema::world_actor_system_data::id.eq(uuid::Uuid::now_v7()),
                crate::schema::world_actor_system_data::actor_id.eq(actor_id),
                crate::schema::world_actor_system_data::game_system_id.eq("genie"),
                crate::schema::world_actor_system_data::trait_data
                    .eq(serde_json::json!({"size_category": "colossal"})),
                crate::schema::world_actor_system_data::created_by.eq(owner_id),
                crate::schema::world_actor_system_data::updated_by.eq(owner_id),
                crate::schema::world_actor_system_data::created_at.eq(now),
                crate::schema::world_actor_system_data::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("failed to insert test actor system data");
        drop(conn);

        let result = actor_system_data_impl(&state, owner_id, false, actor_id)
            .await
            .expect("a member should be able to read actor system data")
            .expect("the row that was just written should be found");

        assert_eq!(
            result.trait_data.unwrap()["size_category"],
            serde_json::json!("colossal")
        );
    }

    /// Same visibility rule as `world_actors_rejects_non_member_non_owner`.
    #[tokio::test]
    async fn actor_system_data_rejects_non_member_non_owner() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let outsider_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = crate::test_support::insert_test_scene(&mut conn, world_id, owner_id);
        let actor_id = insert_test_actor(&mut conn, world_id, scene_id, owner_id, true);
        drop(conn);

        let result = actor_system_data_impl(&state, outsider_id, false, actor_id).await;

        assert!(
            result.is_err(),
            "a user with no ownership or world_members row must not be able to read the actor's system data"
        );
    }
}
