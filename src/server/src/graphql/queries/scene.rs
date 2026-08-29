//! Scene queries for loading scenes, tokens, and fog masks.

use async_graphql::Context;

use crate::graphql::*;

/// Testable core of `SceneQuery::scenes` (see `actor.rs`'s `_impl`
/// convention).
pub async fn scenes_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    world_id: uuid::Uuid,
) -> GraphQLResult<Vec<crate::models::Scene>> {
    // 🔐 SECURITY: Verify user has access to this world before returning scenes
    require_visible_world(state, user_id, is_admin, world_id).await?;

    // Spec 022 (FR-008/FR-009): GM/Owner see every scene, hidden or not;
    // everyone else only sees non-hidden scenes — mirrors the
    // GM-vs-player branching already used for `shapes.visible_to_players`.
    let is_dm =
        crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        use crate::schema::scenes;
        let mut query = scenes::table
            .filter(scenes::world_id.eq(world_id))
            .into_boxed();
        if !is_dm {
            query = query.filter(scenes::hidden.eq(false));
        }
        query
            .select(crate::models::Scene::as_select())
            .load::<crate::models::Scene>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load scenes"))
}

/// Testable core of `SceneQuery::scene`.
pub async fn scene_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    scene_id: uuid::Uuid,
) -> GraphQLResult<Option<crate::models::Scene>> {
    // 🔐 SECURITY: Get the world_id from the scene, then verify access
    let world_id = get_world_id_from_scene(state, scene_id).await?;
    if load_visible_world_by_id(state, user_id, is_admin, world_id)
        .await?
        .is_none()
    {
        return Ok(None);
    }

    // Spec 022 (FR-008): a non-GM caller must not be able to fetch a
    // hidden scene's detail even by guessing/bookmarking its URL — mirrors
    // `scenes_impl`'s list-level filtering.
    let is_dm =
        crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    // ...with one carve-out: the scene the world is *currently playing*.
    //
    // `hidden` is a rule about the scene table (FR-008's own words) — it
    // keeps a GM's unfinished prep out of the Scenes section so nothing is
    // spoiled. Launching a scene is the opposite act: it is the GM
    // deliberately putting that scene in front of everyone, and spec 022 is
    // explicit that Play "starts showing content as soon as a GM launches a
    // scene". Filtering it here anyway did not hide anything — a player
    // already loads its tokens, walls, lights and shapes, all of which gate
    // on world access rather than on `hidden` — it only withheld the scene's
    // own record, so a player's canvas got no map art and no grid while
    // being asked to play on it. A world's auto-created scene is hidden by
    // default (FR-003), so this was every player in every new world.
    //
    // Scoped to exactly one scene per world, chosen by the GM. Guessing a
    // different hidden scene's id still gets nothing.
    let active_scene_id = {
        use crate::schema::worlds;
        let mut lookup_conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        tokio::task::spawn_blocking(move || {
            worlds::table
                .filter(worlds::id.eq(world_id))
                .select(worlds::active_scene_id)
                .first::<Option<uuid::Uuid>>(&mut lookup_conn)
                .optional()
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load world"))?
        .flatten()
    };
    let is_active_scene = active_scene_id == Some(scene_id);

    tokio::task::spawn_blocking(move || {
        use crate::schema::scenes;
        let mut query = scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .into_boxed();
        if !is_dm && !is_active_scene {
            query = query.filter(scenes::hidden.eq(false));
        }
        query
            .select(crate::models::Scene::as_select())
            .first::<crate::models::Scene>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load scene"))
}

/// Testable core of `SceneQuery::shapes` — the GM-vs-player visibility
/// filter is the only real branching logic in this file.
pub async fn shapes_impl(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    scene_id: uuid::Uuid,
) -> GraphQLResult<Vec<crate::models::Shape>> {
    // 🔐 SECURITY: Get the world_id from the scene, then verify access
    let world_id = get_world_id_from_scene(state, scene_id).await?;
    require_visible_world(state, user_id, is_admin, world_id).await?;

    // DM-ness is resolved once, here, and moved into the blocking closure
    // below — the same shape `world_sync_plan` uses, and the reason this is
    // not an `is_dm_of_scene` call inside the closure: `world_id` is already
    // in hand and we are still in async context.
    //
    // It used to ask whether the caller *created* the scene. That made a
    // co-GM a player as far as shape visibility was concerned: they could
    // author on the scene but could not see the GM-only shapes already on
    // it, which is a worse failure than a plain refusal because it renders
    // silently.
    let is_dm =
        crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        use crate::schema::shapes;

        // The DM (world Owner or GM) sees every shape; anyone else only
        // sees shapes explicitly flagged visible to players.
        if is_dm {
            shapes::table
                .filter(shapes::scene_id.eq(scene_id))
                .select(crate::models::Shape::as_select())
                .load::<crate::models::Shape>(&mut conn)
        } else {
            shapes::table
                .filter(shapes::scene_id.eq(scene_id))
                .filter(shapes::visible_to_players.eq(true))
                .select(crate::models::Shape::as_select())
                .load::<crate::models::Shape>(&mut conn)
        }
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load shapes"))
}

#[derive(Default)]
pub struct SceneQuery;

#[async_graphql::Object]
impl SceneQuery {
    async fn scenes(
        &self,
        ctx: &Context<'_>,
        world_id: uuid::Uuid,
    ) -> GraphQLResult<Vec<GraphQLScene>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;

        let scenes = scenes_impl(state, auth_user.user_id, auth_user.is_admin, world_id).await?;

        Ok(scenes.into_iter().map(GraphQLScene::from).collect())
    }

    async fn scene(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
    ) -> GraphQLResult<Option<GraphQLScene>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;

        let scene = scene_impl(state, auth_user.user_id, auth_user.is_admin, scene_id).await?;

        Ok(scene.map(GraphQLScene::from))
    }

    async fn tokens(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
    ) -> GraphQLResult<Vec<GraphQLToken>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;

        // 🔐 SECURITY: Get the world_id from the scene, then verify access
        let world_id = get_world_id_from_scene(state, scene_id).await?;
        require_visible_world(state, auth_user.user_id, auth_user.is_admin, world_id).await?;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let tokens = tokio::task::spawn_blocking(move || {
            use crate::schema::tokens;
            tokens::table
                .filter(tokens::scene_id.eq(scene_id))
                .select(crate::models::Token::as_select())
                .load::<crate::models::Token>(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load tokens"))?;

        Ok(tokens.into_iter().map(GraphQLToken::from).collect())
    }

    async fn fog_mask(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
    ) -> GraphQLResult<Option<GraphQLFogMask>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;

        // 🔐 SECURITY: Get the world_id from the scene, then verify access
        let world_id = get_world_id_from_scene(state, scene_id).await?;
        if load_visible_world_by_id(state, auth_user.user_id, auth_user.is_admin, world_id)
            .await?
            .is_none()
        {
            return Ok(None);
        }

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let fog_mask = tokio::task::spawn_blocking(move || {
            use crate::schema::fog_masks;
            fog_masks::table
                .filter(fog_masks::scene_id.eq(scene_id))
                .select(crate::models::FogMask::as_select())
                .first::<crate::models::FogMask>(&mut conn)
                .optional()
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load fog mask"))?;

        Ok(fog_mask.map(GraphQLFogMask::from))
    }

    async fn walls(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
    ) -> GraphQLResult<Vec<GraphQLWall>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;

        // 🔐 SECURITY: Get the world_id from the scene, then verify access
        let world_id = get_world_id_from_scene(state, scene_id).await?;
        require_visible_world(state, auth_user.user_id, auth_user.is_admin, world_id).await?;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let walls = tokio::task::spawn_blocking(move || {
            use crate::schema::walls;
            walls::table
                .filter(walls::scene_id.eq(scene_id))
                .select(crate::models::Wall::as_select())
                .load::<crate::models::Wall>(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load walls"))?;

        Ok(walls.into_iter().map(GraphQLWall::from).collect())
    }

    /// Returns all light sources for the scene. Light effects are
    /// player-visible by nature (FR-005) — there is no GM-only light data,
    /// so any authenticated scene participant may read this list.
    async fn light_sources(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
    ) -> GraphQLResult<Vec<GraphQLLightSource>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;

        // 🔐 SECURITY: Get the world_id from the scene, then verify access
        let world_id = get_world_id_from_scene(state, scene_id).await?;
        require_visible_world(state, auth_user.user_id, auth_user.is_admin, world_id).await?;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let lights = tokio::task::spawn_blocking(move || {
            use crate::schema::light_sources;
            light_sources::table
                .filter(light_sources::scene_id.eq(scene_id))
                .select(crate::models::LightSource::as_select())
                .load::<crate::models::LightSource>(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load light sources"))?;

        Ok(lights.into_iter().map(GraphQLLightSource::from).collect())
    }

    /// Shapes drawn on a scene's canvas (native canvas authoring). Returns
    /// every shape to the scene owner/GM; returns only
    /// `visible_to_players = true` shapes to any other authenticated
    /// scene participant (FR-009: players never see GM-only shapes).
    async fn shapes(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
    ) -> GraphQLResult<Vec<GraphQLShape>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;

        let shapes = shapes_impl(state, auth_user.user_id, auth_user.is_admin, scene_id).await?;

        Ok(shapes.into_iter().map(GraphQLShape::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{scene_impl, scenes_impl, shapes_impl};
    use crate::schema::shapes;
    use crate::test_support::{
        insert_test_scene, insert_test_user, insert_test_world, test_app_state,
    };
    use diesel::prelude::*;

    fn insert_test_shape(
        conn: &mut diesel::PgConnection,
        scene_id: uuid::Uuid,
        owner_id: uuid::Uuid,
        visible_to_players: bool,
    ) -> uuid::Uuid {
        let id = uuid::Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(shapes::table)
            .values((
                shapes::shape_id.eq(id),
                shapes::scene_id.eq(scene_id),
                shapes::kind.eq("rect"),
                shapes::geometry.eq(serde_json::json!({})),
                shapes::visible_to_players.eq(visible_to_players),
                shapes::created_by.eq(owner_id),
                shapes::updated_by.eq(owner_id),
                shapes::created_at.eq(now),
                shapes::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("failed to insert test shape");
        id
    }

    #[tokio::test]
    async fn scenes_rejects_non_member_non_owner() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let outsider_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let result = scenes_impl(&state, outsider_id, false, world_id).await;

        assert!(
            result.is_err(),
            "a user with no ownership or world_members row must not list a world's scenes"
        );
    }

    #[tokio::test]
    async fn scenes_returns_rows_for_the_owner() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let scenes = scenes_impl(&state, owner_id, false, world_id)
            .await
            .expect("owner should be able to list scenes");

        assert_eq!(scenes.len(), 1);
    }

    #[tokio::test]
    async fn scene_returns_none_for_non_member_non_owner_instead_of_erroring() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let outsider_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let result = scene_impl(&state, outsider_id, false, scene_id)
            .await
            .expect("query itself should not error for a non-visible scene");

        assert!(
            result.is_none(),
            "a user with no ownership or world_members row must not see the scene"
        );
    }

    // ===== Spec 022: hidden-scene filtering (FR-008/FR-009) =====

    #[tokio::test]
    async fn scenes_excludes_hidden_scenes_for_a_non_dm_but_not_for_the_owner() {
        use crate::schema::scenes;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        crate::test_support::insert_test_world_member(&mut conn, world_id, player_id, "Player");

        // insert_test_scene's raw insert relies on the `hidden` column's
        // DB default (true) — explicit here for clarity. It also always
        // names the scene "Test Scene" (unique per world), so the second
        // scene is inserted directly with a distinct name.
        let hidden_scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        diesel::update(scenes::table.filter(scenes::scene_id.eq(hidden_scene_id)))
            .set(scenes::hidden.eq(true))
            .execute(&mut conn)
            .unwrap();
        let visible_scene_id = uuid::Uuid::now_v7();
        diesel::insert_into(scenes::table)
            .values((
                scenes::scene_id.eq(visible_scene_id),
                scenes::world_id.eq(world_id),
                scenes::name.eq("Second Test Scene"),
                scenes::type_.eq("battlemap"),
                scenes::grid_size.eq(5),
                scenes::grid_type.eq("square"),
                scenes::width.eq(100),
                scenes::height.eq(100),
                scenes::owner_id.eq(owner_id),
                scenes::hidden.eq(false),
            ))
            .execute(&mut conn)
            .unwrap();
        drop(conn);

        let owner_scenes = scenes_impl(&state, owner_id, false, world_id)
            .await
            .expect("owner should be able to list scenes");
        assert_eq!(
            owner_scenes.len(),
            2,
            "GM/Owner must see hidden and visible scenes (FR-009)"
        );

        let player_scenes = scenes_impl(&state, player_id, false, world_id)
            .await
            .expect("player should be able to list scenes");
        assert_eq!(
            player_scenes.len(),
            1,
            "a non-DM must only see non-hidden scenes (FR-008)"
        );
        assert_eq!(player_scenes[0].scene_id, visible_scene_id);
    }

    #[tokio::test]
    async fn scene_by_id_is_hidden_from_a_non_dm_but_visible_to_the_owner() {
        use crate::schema::scenes;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        crate::test_support::insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let hidden_scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        diesel::update(scenes::table.filter(scenes::scene_id.eq(hidden_scene_id)))
            .set(scenes::hidden.eq(true))
            .execute(&mut conn)
            .unwrap();
        drop(conn);

        let owner_result = scene_impl(&state, owner_id, false, hidden_scene_id)
            .await
            .expect("query should not error");
        assert!(
            owner_result.is_some(),
            "GM/Owner must be able to fetch a hidden scene's detail"
        );

        let player_result = scene_impl(&state, player_id, false, hidden_scene_id)
            .await
            .expect("query should not error");
        assert!(
            player_result.is_none(),
            "a non-DM must not be able to fetch a hidden scene's detail even by id (FR-008)"
        );
    }

    /// The carve-out, and its edge.
    ///
    /// Two hidden scenes in one world, one of them launched. The launched
    /// one must reach the player who is being asked to play on it — without
    /// it their canvas has no map art and no grid — and the other must stay
    /// exactly as hidden as it was, or "the active scene is readable" has
    /// quietly become "any hidden scene is readable". Asserting only the
    /// first half would pass on a change that removed the filter entirely.
    #[tokio::test]
    async fn a_player_may_read_the_scene_being_played_but_no_other_hidden_one() {
        use crate::schema::{scenes, worlds};

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        crate::test_support::insert_test_world_member(&mut conn, world_id, player_id, "Player");

        // Hidden is the default for a newly created scene (FR-003), which is
        // why this is the ordinary case rather than an exotic one.
        let played_scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let prep_scene_id = uuid::Uuid::now_v7();
        diesel::insert_into(scenes::table)
            .values((
                scenes::scene_id.eq(prep_scene_id),
                scenes::world_id.eq(world_id),
                scenes::name.eq("Unfinished Prep"),
                scenes::type_.eq("battlemap"),
                scenes::grid_size.eq(5),
                scenes::grid_type.eq("square"),
                scenes::width.eq(100),
                scenes::height.eq(100),
                scenes::owner_id.eq(owner_id),
                scenes::hidden.eq(true),
            ))
            .execute(&mut conn)
            .unwrap();
        diesel::update(scenes::table.filter(scenes::scene_id.eq(played_scene_id)))
            .set(scenes::hidden.eq(true))
            .execute(&mut conn)
            .unwrap();
        diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::active_scene_id.eq(Some(played_scene_id)))
            .execute(&mut conn)
            .unwrap();
        drop(conn);

        let played = scene_impl(&state, player_id, false, played_scene_id)
            .await
            .expect("query should not error");
        assert!(
            played.is_some(),
            "a player must be able to read the scene their world is playing, hidden or not",
        );

        let prep = scene_impl(&state, player_id, false, prep_scene_id)
            .await
            .expect("query should not error");
        assert!(
            prep.is_none(),
            "every other hidden scene must stay hidden from a player (FR-008)",
        );
    }

    /// Renamed from `shapes_returns_every_shape_to_the_scene_owner`: the
    /// fixture's `owner_id` is both the world's Owner and the scene's
    /// creator, and it is the *world role* that now decides what they see.
    #[tokio::test]
    async fn shapes_returns_every_shape_to_the_worlds_owner() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        insert_test_shape(&mut conn, scene_id, owner_id, true);
        insert_test_shape(&mut conn, scene_id, owner_id, false);
        drop(conn);

        let shapes = shapes_impl(&state, owner_id, false, scene_id)
            .await
            .expect("owner should be able to list shapes");

        assert_eq!(
            shapes.len(),
            2,
            "the scene owner must see GM-only (not-visible-to-players) shapes too"
        );
    }

    /// A GM who did not create the scene still sees the GM-only shapes on
    /// it. This is the read side of the same break the content mutations
    /// had: `shapes_impl` asked whether the caller *created* the scene, so a
    /// co-GM could author shapes on a scene and not see the ones already
    /// there — a silent wrong answer rather than a refusal.
    #[tokio::test]
    async fn shapes_returns_every_shape_to_a_gm_who_did_not_create_the_scene() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let gm_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        crate::test_support::insert_test_world_member(&mut conn, world_id, gm_id, "GM");
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        insert_test_shape(&mut conn, scene_id, owner_id, true);
        insert_test_shape(&mut conn, scene_id, owner_id, false);
        drop(conn);

        let shapes = shapes_impl(&state, gm_id, false, scene_id)
            .await
            .expect("a GM should be able to list shapes");

        assert_eq!(
            shapes.len(),
            2,
            "a GM must see GM-only shapes on a scene the Owner created"
        );
    }

    /// Renamed from `shapes_hides_gm_only_shapes_from_non_owner_participants`:
    /// what withholds the GM-only shapes is the caller's Player role, not
    /// the fact that they did not create the scene.
    #[tokio::test]
    async fn shapes_hides_gm_only_shapes_from_players() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let player_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        crate::test_support::insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        insert_test_shape(&mut conn, scene_id, owner_id, true);
        insert_test_shape(&mut conn, scene_id, owner_id, false);
        drop(conn);

        let shapes = shapes_impl(&state, player_id, false, scene_id)
            .await
            .expect("world member should be able to list shapes");

        assert_eq!(
            shapes.len(),
            1,
            "a Player must only see visible_to_players shapes (FR-009)"
        );
    }
}
