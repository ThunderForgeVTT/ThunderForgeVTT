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

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        use crate::schema::scenes;
        scenes::table
            .filter(scenes::world_id.eq(world_id))
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

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        use crate::schema::scenes;
        scenes::table
            .filter(scenes::scene_id.eq(scene_id))
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

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        use crate::schema::{scenes, shapes};

        // The scene owner (GM) sees every shape; anyone else only sees
        // shapes explicitly flagged visible to players.
        let is_owner = scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .filter(scenes::owner_id.eq(user_id))
            .select(scenes::scene_id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()?
            .is_some();

        if is_owner {
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

        let scenes =
            scenes_impl(state, auth_user.user_id, auth_user.is_admin, world_id).await?;

        Ok(scenes.into_iter().map(GraphQLScene::from).collect())
    }

    async fn scene(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
    ) -> GraphQLResult<Option<GraphQLScene>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;

        let scene =
            scene_impl(state, auth_user.user_id, auth_user.is_admin, scene_id).await?;

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

        let shapes =
            shapes_impl(state, auth_user.user_id, auth_user.is_admin, scene_id).await?;

        Ok(shapes.into_iter().map(GraphQLShape::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{scene_impl, scenes_impl, shapes_impl};
    use crate::schema::shapes;
    use crate::test_support::{insert_test_scene, insert_test_user, insert_test_world, test_app_state};
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

    #[tokio::test]
    async fn shapes_returns_every_shape_to_the_scene_owner() {
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

    #[tokio::test]
    async fn shapes_hides_gm_only_shapes_from_non_owner_participants() {
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
            "a non-owner participant must only see visible_to_players shapes (FR-009)"
        );
    }
}
