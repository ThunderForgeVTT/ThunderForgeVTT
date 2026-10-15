//! `setSceneTimeOfDay`: the Game Master sets the table's time (spec 099 US5).
//!
//! The scene stores the hour its clock shows. Every client showing the scene
//! hands it to its engine's clock (`set_time_of_day`), which eases the dark,
//! the night map and the night lights to it over `overSecs`. The update and
//! its announcement are one transaction, as the scene's light is.

use async_graphql::{Context, Error, Object, Result as GraphQLResult};
use uuid::Uuid;

use crate::graphql::types_scene::GraphQLScene;
use crate::graphql::{app_state, authenticated_user};
use crate::play_pause::gate::refuse_if_paused;
use crate::state::AppState;

/// The longest a Game Master may pass time over, in seconds: a scene's
/// evening, not a session's.
pub const MAX_OVER_SECS: f64 = 600.0;

/// Testable core of `setSceneTimeOfDay`. `hour` is 0 to 24 (24 is midnight,
/// stored as 0), or `None` to stop the clock; `over_secs` passes the time
/// over that many seconds rather than at once.
pub async fn set_scene_time_of_day_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    scene_id: Uuid,
    hour: Option<f64>,
    over_secs: Option<f64>,
) -> GraphQLResult<GraphQLScene> {
    use crate::schema::scenes;
    use diesel::prelude::*;

    if hour.is_some_and(|h| !(0.0..=24.0).contains(&h)) {
        return Err(Error::new("An hour is from 0 to 24"));
    }
    if over_secs.is_some_and(|s| !(0.0..=MAX_OVER_SECS).contains(&s)) {
        return Err(Error::new("Time passes over 0 to 600 seconds"));
    }
    let hour = hour.map(|h| (h % 24.0) as f32);

    let mut lookup_conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = tokio::task::spawn_blocking(move || {
        scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .select(scenes::world_id)
            .first::<Uuid>(&mut lookup_conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Scene not found"))?;

    if !crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new(
            "Only the DM (Owner or GM) may change a scene's time",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    refuse_if_paused(&mut conn, world_id)?;
    let updated = tokio::task::spawn_blocking(move || {
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let scene = diesel::update(scenes::table.filter(scenes::scene_id.eq(scene_id)))
                .set(scenes::time_of_day.eq(hour))
                .returning(crate::models::Scene::as_returning())
                .get_result(conn)?;
            crate::world_events::record_world_event(
                conn,
                world_id,
                crate::world_events::EVENT_CODE_SCENE_TIME_CHANGED,
                Some(serde_json::json!({
                    "sceneId": scene_id.to_string(),
                    "hour": hour,
                    "overSecs": over_secs,
                })),
                user_id,
            )
            .map_err(|_| diesel::result::Error::RollbackTransaction)?;
            Ok(scene)
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to change the scene's time"))?;

    Ok(GraphQLScene::from(updated))
}

#[derive(Default)]
pub struct SceneTimeMutation;

#[Object]
impl SceneTimeMutation {
    /// Spec 099 US5: set the hour a scene's clock shows, at once or over
    /// `overSecs`; `null` stops the clock. Game Masters only.
    async fn set_scene_time_of_day(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
        hour: Option<f64>,
        over_secs: Option<f64>,
    ) -> GraphQLResult<GraphQLScene> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        set_scene_time_of_day_impl(
            state,
            user.user_id,
            user.is_admin,
            scene_id,
            hour,
            over_secs,
        )
        .await
    }
}

#[cfg(test)]
#[path = "scene_time_tests.rs"]
mod tests;
