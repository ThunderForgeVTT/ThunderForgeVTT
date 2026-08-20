//! Scene queries for loading scenes, tokens, and fog masks.

use async_graphql::Context;

use crate::graphql::*;

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
        
        // 🔐 SECURITY: Verify user has access to this world before returning scenes
        let _ = load_visible_world_by_id(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
        )
        .await?;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let scenes = tokio::task::spawn_blocking(move || {
            use crate::schema::scenes;
            scenes::table
                .filter(scenes::world_id.eq(world_id))
                .select(crate::models::Scene::as_select())
                .load::<crate::models::Scene>(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load scenes"))?;

        Ok(scenes.into_iter().map(GraphQLScene::from).collect())
    }

    async fn scene(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
    ) -> GraphQLResult<Option<GraphQLScene>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        
        // 🔐 SECURITY: Get the world_id from the scene, then verify access
        let world_id = get_world_id_from_scene(state, scene_id).await?;
        let _ = load_visible_world_by_id(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
        )
        .await?;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let scene = tokio::task::spawn_blocking(move || {
            use crate::schema::scenes;
            scenes::table
                .filter(scenes::scene_id.eq(scene_id))
                .select(crate::models::Scene::as_select())
                .first::<crate::models::Scene>(&mut conn)
                .optional()
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load scene"))?;

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
        let _ = load_visible_world_by_id(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
        )
        .await?;

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
        let _ = load_visible_world_by_id(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
        )
        .await?;

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
        let _ = load_visible_world_by_id(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
        )
        .await?;

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
        let _ = load_visible_world_by_id(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
        )
        .await?;

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
}
