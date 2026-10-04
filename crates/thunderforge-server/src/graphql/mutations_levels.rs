//! GraphQL mutations for a scene's levels. Every one is Game-Master-only;
//! the rules themselves live in `crate::scene_levels`, which these only
//! carry arguments to.

use async_graphql::{Context, Error, InputObject, Object, Result as GraphQLResult};
use uuid::Uuid;

use crate::graphql::queries::levels::GraphQLSceneLevel;
use crate::graphql::{GraphQLToken, app_state, authenticated_user};
use crate::play_pause::gate::refusal_or;
use crate::scene_levels::{self, LevelChanges, LevelError, NewLevel};

#[derive(InputObject, Debug, Clone)]
pub struct GraphQLCreateSceneLevelInput {
    pub scene_id: Uuid,
    pub name: String,
    pub hidden: Option<bool>,
    /// Left out: the scene's own size.
    pub width: Option<i32>,
    pub height: Option<i32>,
    /// `bright`, `dim` or `dark`. Left out: the scene's own.
    pub ambient_light: Option<String>,
    /// An image already uploaded to this world.
    pub background_asset_id: Option<Uuid>,
}

/// Every field is optional; one left out is left as it is.
#[derive(InputObject, Debug, Clone)]
pub struct GraphQLUpdateSceneLevelInput {
    pub name: Option<String>,
    pub hidden: Option<bool>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    /// `bright`, `dim` or `dark`.
    pub ambient_light: Option<String>,
    /// An image already uploaded to this world.
    pub background_asset_id: Option<Uuid>,
    /// Remove the background. Wins over `backgroundAssetId`.
    pub clear_background: Option<bool>,
    /// `true` makes this the scene's entry level. There is no un-making one:
    /// name another level the entry instead.
    pub make_entry: Option<bool>,
}

fn said(e: LevelError, fallback: &str) -> Error {
    match e {
        LevelError::Refused(why) => Error::new(why),
        // A paused world's refusal travels inside the database error.
        LevelError::Database(e) => refusal_or(e, fallback),
    }
}

/// Run one of `scene_levels`' functions on a pooled connection.
async fn run<T, F>(ctx: &Context<'_>, fallback: &'static str, work: F) -> GraphQLResult<T>
where
    T: Send + 'static,
    F: FnOnce(&mut diesel::PgConnection, Uuid, bool) -> Result<T, LevelError> + Send + 'static,
{
    let auth_user = authenticated_user(ctx)?;
    let (user_id, is_admin) = (auth_user.user_id, auth_user.is_admin);
    let mut conn = app_state(ctx)?
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || work(&mut conn, user_id, is_admin))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|e| said(e, fallback))
}

/// A level as its Game Master sees it, head count included.
fn gm_view(conn: &mut diesel::PgConnection, level: crate::models::SceneLevel) -> GraphQLSceneLevel {
    let count = scene_levels::token_counts(conn, level.scene_id)
        .ok()
        .map(|counts| counts.get(&level.level_id).copied().unwrap_or(0))
        .map(|count| i32::try_from(count).unwrap_or(i32::MAX));
    GraphQLSceneLevel::from_row(level, count)
}

#[derive(Default)]
pub struct SceneLevelMutation;

#[Object]
impl SceneLevelMutation {
    /// Add a level above the scene's others. A scene has at most 12.
    async fn create_scene_level(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateSceneLevelInput,
    ) -> GraphQLResult<GraphQLSceneLevel> {
        run(
            ctx,
            "Failed to create the level",
            move |conn, user_id, is_admin| {
                let level = scene_levels::create_level(
                    conn,
                    user_id,
                    is_admin,
                    input.scene_id,
                    NewLevel {
                        name: input.name,
                        hidden: input.hidden,
                        width: input.width,
                        height: input.height,
                        ambient_light: input.ambient_light,
                        background_asset_id: input.background_asset_id,
                    },
                )?;
                Ok(gm_view(conn, level))
            },
        )
        .await
    }

    /// Rename a level, hide it, change its board, or make it the entry.
    async fn update_scene_level(
        &self,
        ctx: &Context<'_>,
        level_id: Uuid,
        input: GraphQLUpdateSceneLevelInput,
    ) -> GraphQLResult<GraphQLSceneLevel> {
        run(
            ctx,
            "Failed to update the level",
            move |conn, user_id, is_admin| {
                let level = scene_levels::update_level(
                    conn,
                    user_id,
                    is_admin,
                    level_id,
                    LevelChanges {
                        name: input.name,
                        hidden: input.hidden,
                        width: input.width,
                        height: input.height,
                        ambient_light: input.ambient_light,
                        background_asset_id: input.background_asset_id,
                        clear_background: input.clear_background.unwrap_or(false),
                        make_entry: input.make_entry.unwrap_or(false),
                    },
                )?;
                Ok(gm_view(conn, level))
            },
        )
        .await
    }

    /// Put the scene's levels in the order given, lowest first. `levelIds`
    /// names every level of the scene exactly once.
    async fn reorder_scene_levels(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
        level_ids: Vec<Uuid>,
    ) -> GraphQLResult<Vec<GraphQLSceneLevel>> {
        run(
            ctx,
            "Failed to reorder the levels",
            move |conn, user_id, is_admin| {
                let levels =
                    scene_levels::reorder_levels(conn, user_id, is_admin, scene_id, &level_ids)?;
                Ok(levels
                    .into_iter()
                    .map(|level| gm_view(conn, level))
                    .collect())
            },
        )
        .await
    }

    /// Delete a level and the walls, lights, shapes, interactives and fog on
    /// it. Refused for the last level, for the entry level, and while any
    /// token stands on it. Returns the deleted level's id.
    async fn delete_scene_level(&self, ctx: &Context<'_>, level_id: Uuid) -> GraphQLResult<Uuid> {
        run(
            ctx,
            "Failed to delete the level",
            move |conn, user_id, is_admin| {
                scene_levels::delete_level(conn, user_id, is_admin, level_id)
            },
        )
        .await
    }

    /// Carry tokens to another level of their scene. With `x` and `y` they
    /// are set down around that point; without, each keeps its position.
    async fn move_tokens_to_level(
        &self,
        ctx: &Context<'_>,
        token_ids: Vec<Uuid>,
        level_id: Uuid,
        x: Option<f64>,
        y: Option<f64>,
    ) -> GraphQLResult<Vec<GraphQLToken>> {
        let at = match (x, y) {
            (Some(x), Some(y)) => Some((x, y)),
            (None, None) => None,
            _ => return Err(Error::new("Give both x and y, or neither")),
        };
        run(
            ctx,
            "Failed to move the tokens",
            move |conn, user_id, is_admin| {
                let tokens = scene_levels::move_tokens_to_level(
                    conn, user_id, is_admin, &token_ids, level_id, at,
                )?;
                Ok(tokens.into_iter().map(GraphQLToken::from).collect())
            },
        )
        .await
    }
}
