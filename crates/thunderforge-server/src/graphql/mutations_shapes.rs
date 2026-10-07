//! GraphQL mutations for scene shapes (native canvas authoring: freehand
//! strokes, rectangles, ellipses, lines, and text drawn directly on the
//! canvas). Mirrors `mutations_walls.rs`'s scene-ownership and real-time
//! NOTIFY pattern.

use async_graphql::{Context, Error, Result as GraphQLResult};
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::auth::shape_authority::{ShapeAuthority, shape_authority};
#[cfg(test)]
use crate::graphql::GraphQLShapeKind;
use crate::graphql::{
    GraphQLCreateShapeInput, GraphQLShape, GraphQLUpdateShapeInput, app_state, authenticated_user,
};
use crate::play_pause::gate::{refusal_or, refuse_scene_if_paused};
use crate::world_events::{EVENT_CODE_SHAPE_CHANGED, record_world_event, world_id_for_scene};

#[derive(Default)]
pub struct ShapeMutation;

#[async_graphql::Object]
impl ShapeMutation {
    /// Create a new shape on a scene: a Game Master, or a player holding the
    /// Shapes tool, whose shape is always visible to players (spec 082)
    async fn create_shape(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateShapeInput,
    ) -> GraphQLResult<GraphQLShape> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let is_admin = auth_user.is_admin;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let shape_id = uuid::Uuid::now_v7();
        let scene_id = input.scene_id;
        let level_id = input.level_id;
        let kind = input.kind.as_db_str().to_string();
        let geometry = input.geometry.0;
        let text = input.text;
        let style = input.style.map(|j| j.0);
        let visible_to_players = input.visible_to_players.unwrap_or(false);
        let metadata = input.metadata.map(|j| j.0);

        let inserted_shape = tokio::task::spawn_blocking(move || {
            use crate::schema::shapes;

            // 🔐 Spec 082: a Game Master draws anything; a player holding
            // the Shapes tool draws their own, always visible to players.
            // See `auth::shape_authority`.
            let visible_to_players =
                match shape_authority(&mut conn, user_id, is_admin, scene_id, None)? {
                    ShapeAuthority::Dm => visible_to_players,
                    ShapeAuthority::Creator => true,
                    ShapeAuthority::None => return Err(DieselError::NotFound),
                };
            refuse_scene_if_paused(&mut conn, scene_id)?;

            let shape = diesel::insert_into(shapes::table)
                .values((
                    shapes::shape_id.eq(shape_id),
                    shapes::scene_id.eq(scene_id),
                    // Left unset, the database puts it on the entry level.
                    level_id.map(|level| shapes::level_id.eq(level)),
                    shapes::kind.eq(&kind),
                    shapes::geometry.eq(&geometry),
                    shapes::text.eq(&text),
                    shapes::style.eq(&style),
                    shapes::visible_to_players.eq(visible_to_players),
                    shapes::metadata.eq(&metadata),
                    shapes::created_by.eq(user_id),
                    shapes::updated_by.eq(user_id),
                    shapes::created_at.eq(now),
                    shapes::updated_at.eq(now),
                ))
                .returning(crate::models::Shape::as_returning())
                .get_result(&mut conn)?;

            if let Ok(world_id) = world_id_for_scene(&mut conn, scene_id) {
                let _ = record_world_event(
                    &mut conn,
                    world_id,
                    EVENT_CODE_SHAPE_CHANGED,
                    Some(serde_json::json!({
                        "action": "created",
                        "shape_id": shape_id,
                        "scene_id": scene_id,
                    })),
                    user_id,
                );
            }

            Ok(shape)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|e| {
            refusal_or(
                e,
                "Failed to create shape (scene not found or not owned by you)",
            )
        })?;

        Ok(GraphQLShape::from(inserted_shape))
    }

    /// Update a shape: a Game Master any, a player only their own (spec 082)
    async fn update_shape(
        &self,
        ctx: &Context<'_>,
        shape_id: uuid::Uuid,
        input: GraphQLUpdateShapeInput,
    ) -> GraphQLResult<GraphQLShape> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let is_admin = auth_user.is_admin;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let update_data = crate::models::ShapeUpdate {
            geometry: input.geometry.map(|j| j.0),
            text: input.text,
            style: input.style.map(|j| j.0),
            visible_to_players: input.visible_to_players,
            metadata: input.metadata.map(|j| j.0),
            updated_by: user_id,
        };

        let updated_shape = tokio::task::spawn_blocking(move || {
            use crate::schema::shapes;
            let mut update_data = update_data;

            // 🔐 Spec 082: a Game Master edits any shape; a player edits only
            // their own, and cannot hide it. See `auth::shape_authority`.
            let Some((scene_id, created_by)) = shapes::table
                .filter(shapes::shape_id.eq(shape_id))
                .select((shapes::scene_id, shapes::created_by))
                .first::<(uuid::Uuid, uuid::Uuid)>(&mut conn)
                .optional()?
            else {
                return Err(DieselError::NotFound);
            };
            match shape_authority(&mut conn, user_id, is_admin, scene_id, Some(created_by))? {
                ShapeAuthority::Dm => {}
                ShapeAuthority::Creator => update_data.visible_to_players = None,
                ShapeAuthority::None => return Err(DieselError::NotFound),
            }
            refuse_scene_if_paused(&mut conn, scene_id)?;

            let shape = diesel::update(shapes::table.filter(shapes::shape_id.eq(shape_id)))
                .set(update_data)
                .returning(crate::models::Shape::as_returning())
                .get_result(&mut conn)?;

            if let Ok(world_id) = world_id_for_scene(&mut conn, shape.scene_id) {
                let _ = record_world_event(
                    &mut conn,
                    world_id,
                    EVENT_CODE_SHAPE_CHANGED,
                    Some(serde_json::json!({
                        "action": "updated",
                        "shape_id": shape_id,
                        "scene_id": shape.scene_id,
                    })),
                    user_id,
                );
            }

            Ok::<_, DieselError>(shape)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|e| refusal_or(e, "Failed to update shape (not found or not owned by you)"))?;

        Ok(GraphQLShape::from(updated_shape))
    }

    /// Delete a shape: a Game Master any, a player only their own (spec 082)
    async fn delete_shape(&self, ctx: &Context<'_>, shape_id: uuid::Uuid) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let is_admin = auth_user.is_admin;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::shapes;

            // Look up the scene before deleting so we still have it for the NOTIFY payload.
            let found = shapes::table
                .filter(shapes::shape_id.eq(shape_id))
                .select((shapes::scene_id, shapes::created_by))
                .first::<(uuid::Uuid, uuid::Uuid)>(&mut conn)
                .optional()?;
            let scene_id = found.map(|(scene_id, _)| scene_id);

            // 🔐 Spec 082: a Game Master deletes any shape; a player only
            // their own. See `auth::shape_authority`.
            let authorized = match found {
                Some((scene_id, created_by)) => !matches!(
                    shape_authority(&mut conn, user_id, is_admin, scene_id, Some(created_by))?,
                    ShapeAuthority::None
                ),
                None => false,
            };
            if !authorized {
                // Nothing was deleted, which is what an unauthorized
                // caller has always been told — the refusal reads the
                // same as "no such shape" and leaks nothing either way.
                return Ok(0);
            }
            if let Some(scene_id) = scene_id {
                refuse_scene_if_paused(&mut conn, scene_id)?;
            }

            let deleted_count = diesel::delete(shapes::table.filter(shapes::shape_id.eq(shape_id)))
                .execute(&mut conn)?;

            if deleted_count > 0
                && let Some(scene_id) = scene_id
                && let Ok(world_id) = world_id_for_scene(&mut conn, scene_id)
            {
                let _ = record_world_event(
                    &mut conn,
                    world_id,
                    EVENT_CODE_SHAPE_CHANGED,
                    Some(serde_json::json!({
                        "action": "deleted",
                        "shape_id": shape_id,
                        "scene_id": scene_id,
                    })),
                    user_id,
                );
            }

            Ok::<_, DieselError>(deleted_count)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|e| refusal_or(e, "Failed to delete shape"))?;

        Ok(deleted > 0)
    }
}

#[cfg(test)]
#[path = "mutations_shapes_tests.rs"]
pub(crate) mod tests;
