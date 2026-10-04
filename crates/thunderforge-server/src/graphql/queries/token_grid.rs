//! `tokenGrid(sceneId)` — how many squares each token in a scene fills (spec
//! 046 US4, contract §1, research R10).
//!
//! A sibling query keyed by token id, like `tokenVision`, for the same reason:
//! the answer comes from a system's manifest and every actor's sheet, loaded
//! once for the scene. The web hands each answer to the engine's
//! `set_token_grid`, which everything that should follow a creature's size —
//! snapping, hit-testing, keyboard moves, nameplates, bars — already reads.

use async_graphql::{Context, Error, Object, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use uuid::Uuid;

use crate::combat::size::{DEFAULT_FOOTPRINT, footprints_in_scene};
use crate::graphql::{app_state, authenticated_user};

/// One token's footprint, in squares a side.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TokenGrid")]
pub struct GraphQLTokenGrid {
    pub token_id: Uuid,
    pub footprint: f64,
}

#[derive(Default)]
pub struct TokenGridQuery;

#[Object]
impl TokenGridQuery {
    /// Every token in the scene that fills something other than one square:
    /// a linked token by its actor's size, a copy by its NPC's, as the world's
    /// game system declares sizes (`combat.sizes`).
    ///
    /// A token of one square is **omitted**. The client tells the engine `1`
    /// for every token it does not find here, so a creature that shrinks back
    /// to Medium shrinks on every board.
    async fn token_grid(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLTokenGrid>> {
        let user = authenticated_user(ctx)?;
        let (user_id, is_admin) = (user.user_id, user.is_admin);
        let state = app_state(ctx)?;
        let systems_dir = state.directories.systems_dir.clone();
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        tokio::task::spawn_blocking(move || {
            use crate::schema::scenes;

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

            let footprints = footprints_in_scene(&mut conn, &systems_dir, scene_id)
                .map_err(|e| Error::new(format!("Failed to read token sizes: {e}")))?;
            let mut out: Vec<GraphQLTokenGrid> = footprints
                .into_iter()
                .filter(|(_, footprint)| *footprint != DEFAULT_FOOTPRINT)
                .map(|(token_id, footprint)| GraphQLTokenGrid {
                    token_id,
                    footprint: footprint as f64,
                })
                .collect();
            out.sort_by_key(|grid| grid.token_id);
            Ok(out)
        })
        .await
        .map_err(|e| Error::new(format!("Task failed: {e}")))?
    }
}
