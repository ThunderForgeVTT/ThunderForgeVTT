//! `sceneExploration`, `setSceneExploration` and `resetSceneExploration`.
//!
//! Spec 045 US7. A query of its own rather than fields on `GraphQLScene`,
//! because the answer depends on **who is asking**: two players reading the
//! same scene get different numbers, and a `SimpleObject` built from a row
//! has no viewer to resolve against.

use async_graphql::{Context, Error, Object, Result as GraphQLResult, SimpleObject};
use uuid::Uuid;

use crate::graphql::{app_state, authenticated_user};

/// What a client needs to decide whether the map it has stored is still good.
#[derive(SimpleObject, Debug, Clone, Copy)]
pub struct GraphQLSceneExploration {
    /// Whether this scene remembers where players have been at all.
    pub enabled: bool,
    /// The scene's own epoch — the last reset that reached everyone.
    pub epoch: i32,
    /// The epoch **this viewer** must compare against: the greater of the
    /// scene's and their own reset.
    ///
    /// Resolved here rather than handing a client both numbers and asking it
    /// to take the greater. A client that took the wrong one would keep a map
    /// it had been told to drop, and nothing would ever say so.
    pub mine: i32,
}

#[derive(Default)]
pub struct ExplorationQuery;

#[Object]
impl ExplorationQuery {
    async fn scene_exploration(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
    ) -> GraphQLResult<GraphQLSceneExploration> {
        let user = authenticated_user(ctx)?;
        let state = app_state(ctx)?;
        let user_id = user.user_id;
        let is_admin = user.is_admin;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        tokio::task::spawn_blocking(move || {
            use crate::schema::scenes;
            use diesel::prelude::*;

            // Membership first. What a scene does is world-scoped, and a
            // reset epoch would otherwise let anyone probe for a scene's
            // existence.
            let world_id: Uuid = scenes::table
                .filter(scenes::scene_id.eq(scene_id))
                .select(scenes::world_id)
                .first(&mut conn)
                .map_err(|_| Error::new("Scene not found"))?;
            let actor = crate::auth::world_membership::actor_in_world(
                &mut conn, user_id, is_admin, world_id,
            );
            if actor.role.is_none() && !actor.is_site_admin {
                return Err(Error::new("Not a member of this world"));
            }

            let state = crate::exploration::state_for(&mut conn, scene_id, user_id)
                .map_err(|e| Error::new(format!("Failed to read exploration: {e}")))?;
            Ok(GraphQLSceneExploration {
                enabled: state.enabled,
                epoch: state.epoch,
                mine: state.mine,
            })
        })
        .await
        .map_err(|e| Error::new(format!("Task failed: {e}")))?
    }
}

#[derive(Default)]
pub struct ExplorationMutation;

#[Object]
impl ExplorationMutation {
    /// Turn a scene's memory on or off (FR-070, Game Master only).
    async fn set_scene_exploration(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
        enabled: bool,
    ) -> GraphQLResult<bool> {
        let user = authenticated_user(ctx)?;
        let state = app_state(ctx)?;
        crate::exploration::set_enabled(state, user.user_id, user.is_admin, scene_id, enabled).await
    }

    /// Reset what has been explored, for one player or for everyone.
    ///
    /// `forUser` absent means everyone. Returns the epoch a client must now
    /// be at or above.
    async fn reset_scene_exploration(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
        for_user: Option<Uuid>,
    ) -> GraphQLResult<i32> {
        let user = authenticated_user(ctx)?;
        let state = app_state(ctx)?;
        crate::exploration::reset(state, user.user_id, user.is_admin, scene_id, for_user).await
    }
}
