//! `sceneLevels(sceneId)` — the floors of a scene, as this viewer may know
//! them.
//!
//! A Game Master is given every level, with how many tokens stand on each.
//! Anyone else is given only the levels `auth::level_visibility` lets them
//! read — usually one — and no head count: that a floor exists above them,
//! what it is called, and who is on it are all things a player is not told.

use async_graphql::{Context, Error, Object, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use uuid::Uuid;

use crate::graphql::{app_state, authenticated_user};
use crate::models::SceneLevel;

/// One level of a scene.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLSceneLevel {
    pub level_id: Uuid,
    pub scene_id: Uuid,
    pub name: String,
    /// Position among the scene's levels, lowest floor first.
    pub sort_order: i32,
    /// The level a scene opens on, and the one a thing is placed on when
    /// nobody names another. Exactly one per scene.
    pub is_entry: bool,
    /// A Game Master's note that the floor is not ready. It is not what
    /// keeps players out — see `auth::level_visibility`.
    pub hidden: bool,
    pub background_asset_id: Option<Uuid>,
    /// Where the level's background can be fetched, when it has one. Built
    /// exactly as `Scene.backgroundUrl` is.
    pub background_url: Option<String>,
    pub width: i32,
    pub height: i32,
    /// `bright`, `dim` or `dark`.
    pub ambient_light: String,
    /// How many tokens stand on the level. Game Master only; `null` for
    /// anyone else.
    pub token_count: Option<i32>,
}

impl GraphQLSceneLevel {
    pub fn from_row(level: SceneLevel, token_count: Option<i32>) -> Self {
        Self {
            level_id: level.level_id,
            scene_id: level.scene_id,
            name: level.name,
            sort_order: level.sort_order,
            is_entry: level.is_entry,
            hidden: level.hidden,
            // The `.webp` suffix is what lets the engine's asset loader
            // pick an image loader; see `GraphQLScene::background_url`.
            background_url: level
                .background_asset_id
                .map(|id| format!("/api/canvas-assets/{id}.webp"))
                .or(level.background_image_path),
            background_asset_id: level.background_asset_id,
            width: level.width,
            height: level.height,
            ambient_light: level.ambient_light,
            token_count,
        }
    }
}

/// The levels of `scene_id` this viewer may read, in order.
pub(crate) fn scene_levels_for(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    scene_id: Uuid,
) -> QueryResult<Vec<GraphQLSceneLevel>> {
    use crate::schema::scene_levels;

    let is_gm = crate::auth::world_membership::is_dm_of_scene(conn, user_id, is_admin, scene_id)?;
    let may_read = crate::auth::level_visibility::readable_levels(conn, user_id, is_gm, scene_id)?;
    let counts = if is_gm {
        Some(crate::scene_levels::token_counts(conn, scene_id)?)
    } else {
        None
    };

    Ok(scene_levels::table
        .filter(scene_levels::scene_id.eq(scene_id))
        .filter(scene_levels::level_id.eq_any(&may_read))
        .order(scene_levels::sort_order.asc())
        .select(SceneLevel::as_select())
        .load::<SceneLevel>(conn)?
        .into_iter()
        .map(|level| {
            let count = counts.as_ref().map(|counts| {
                i32::try_from(counts.get(&level.level_id).copied().unwrap_or(0)).unwrap_or(i32::MAX)
            });
            GraphQLSceneLevel::from_row(level, count)
        })
        .collect())
}

#[derive(Default)]
pub struct SceneLevelQuery;

#[Object]
impl SceneLevelQuery {
    /// The levels of a scene, lowest first. A Game Master sees all of them;
    /// a player sees the ones they have a token on, or the entry level when
    /// they have none.
    async fn scene_levels(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLSceneLevel>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let (user_id, is_admin) = (auth_user.user_id, auth_user.is_admin);

        // The same door every scene read comes through: a member of a world
        // that is visible to them, and a scene that has not been taken down.
        let world_id = crate::graphql::get_world_id_from_scene(state, scene_id).await?;
        crate::graphql::require_visible_world(state, user_id, is_admin, world_id).await?;
        if crate::moderation::effective_status(
            state,
            crate::graphql::types::ModerationEntityType::Scene.as_db_str(),
            scene_id,
        )
        .await?
        .is_some()
        {
            return Ok(Vec::new());
        }

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        tokio::task::spawn_blocking(move || {
            scene_levels_for(&mut conn, user_id, is_admin, scene_id)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load levels"))
    }
}
