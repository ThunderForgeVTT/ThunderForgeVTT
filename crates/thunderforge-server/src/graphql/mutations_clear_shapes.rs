//! Spec 082 US3/US4: the Game Master clears a scene's drawings at once —
//! all of them, or only some players' (research R5).

use async_graphql::{Context, Error, Result as GraphQLResult};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use uuid::Uuid;

use crate::auth::shape_authority::{ShapeAuthority, shape_authority};
use crate::graphql::{app_state, authenticated_user};
use crate::play_pause::gate::{refusal_or, refuse_scene_if_paused};
use crate::world_events::{EVENT_CODE_SHAPE_CHANGED, record_world_event, world_id_for_scene};

#[derive(Default)]
pub struct ClearShapesMutation;

#[async_graphql::Object]
impl ClearShapesMutation {
    /// Delete a scene's shapes, on every level: every one when `createdBy`
    /// is left out, only those creators' when it is given (`[]` deletes
    /// nothing). A Game Master of the scene only; anyone else is told the
    /// scene was not found. Answers how many were deleted.
    async fn clear_shapes(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
        created_by: Option<Vec<Uuid>>,
    ) -> GraphQLResult<i32> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let is_admin = auth_user.is_admin;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let cleared = tokio::task::spawn_blocking(move || {
            // 🔐 A player may delete their own shapes one at a time; clearing
            // is the Game Master's. See `auth::shape_authority`.
            if shape_authority(&mut conn, user_id, is_admin, scene_id, None)? != ShapeAuthority::Dm
            {
                return Err(DieselError::NotFound);
            }
            refuse_scene_if_paused(&mut conn, scene_id)?;
            if created_by.as_ref().is_some_and(Vec::is_empty) {
                return Ok(0);
            }
            let world_id =
                world_id_for_scene(&mut conn, scene_id).map_err(|_| DieselError::NotFound)?;

            conn.transaction(|conn| {
                use crate::schema::shapes;

                let on_scene = shapes::table.filter(shapes::scene_id.eq(scene_id));
                let deleted: Vec<Uuid> = match &created_by {
                    Some(creators) => {
                        diesel::delete(on_scene.filter(shapes::created_by.eq_any(creators)))
                            .returning(shapes::shape_id)
                            .get_results(conn)?
                    }
                    None => diesel::delete(on_scene)
                        .returning(shapes::shape_id)
                        .get_results(conn)?,
                };
                // One event per shape, which every client already handles,
                // recorded with the delete so neither lands without the other.
                for shape_id in &deleted {
                    record_world_event(
                        conn,
                        world_id,
                        EVENT_CODE_SHAPE_CHANGED,
                        Some(serde_json::json!({
                            "action": "deleted",
                            "shape_id": shape_id,
                            "scene_id": scene_id,
                        })),
                        user_id,
                    )
                    .map_err(|_| DieselError::RollbackTransaction)?;
                }
                Ok(deleted.len())
            })
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|e| refusal_or(e, "Failed to clear shapes (scene not found)"))?;

        Ok(cleared as i32)
    }
}

#[cfg(test)]
#[path = "mutations_clear_shapes_tests.rs"]
mod tests;
