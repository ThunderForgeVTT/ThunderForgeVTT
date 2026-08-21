//! GraphQL mutations for scene shapes (native canvas authoring: freehand
//! strokes, rectangles, ellipses, lines, and text drawn directly on the
//! canvas). Mirrors `mutations_walls.rs`'s scene-ownership and real-time
//! NOTIFY pattern.

use async_graphql::{Context, Error, Result as GraphQLResult};
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::graphql::{
    app_state, authenticated_user, GraphQLCreateShapeInput, GraphQLShape, GraphQLUpdateShapeInput,
};
#[cfg(test)]
use crate::graphql::GraphQLShapeKind;
use crate::world_events::{record_world_event, world_id_for_scene, EVENT_CODE_SHAPE_CHANGED};

#[derive(Default)]
pub struct ShapeMutation;

#[async_graphql::Object]
impl ShapeMutation {
    /// Create a new shape on a scene (scene owner only)
    async fn create_shape(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateShapeInput,
    ) -> GraphQLResult<GraphQLShape> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let shape_id = uuid::Uuid::now_v7();
        let scene_id = input.scene_id;
        let kind = input.kind.as_db_str().to_string();
        let geometry = input.geometry.0;
        let text = input.text;
        let style = input.style.map(|j| j.0);
        let visible_to_players = input.visible_to_players.unwrap_or(false);
        let metadata = input.metadata.map(|j| j.0);

        let inserted_shape = tokio::task::spawn_blocking(move || {
            use crate::schema::{scenes, shapes};

            // 🔐 Ownership: caller must own the parent scene before a shape can be attached to it
            let owns_scene = scenes::table
                .filter(scenes::scene_id.eq(scene_id))
                .filter(scenes::owner_id.eq(user_id))
                .select(scenes::scene_id)
                .first::<uuid::Uuid>(&mut conn)
                .optional()?
                .is_some();
            if !owns_scene {
                return Err(DieselError::NotFound);
            }

            let shape = diesel::insert_into(shapes::table)
                .values((
                    shapes::shape_id.eq(shape_id),
                    shapes::scene_id.eq(scene_id),
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
        .map_err(|_| Error::new("Failed to create shape (scene not found or not owned by you)"))?;

        Ok(GraphQLShape::from(inserted_shape))
    }

    /// Update an existing shape (scene owner only)
    async fn update_shape(
        &self,
        ctx: &Context<'_>,
        shape_id: uuid::Uuid,
        input: GraphQLUpdateShapeInput,
    ) -> GraphQLResult<GraphQLShape> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
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
            use crate::schema::{scenes, shapes};

            let shape = diesel::update(
                shapes::table
                    .filter(shapes::shape_id.eq(shape_id))
                    .filter(
                        shapes::scene_id.eq_any(
                            scenes::table
                                .filter(scenes::owner_id.eq(user_id))
                                .select(scenes::scene_id),
                        ),
                    ),
            )
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
        .map_err(|_| Error::new("Failed to update shape (not found or not owned by you)"))?;

        Ok(GraphQLShape::from(updated_shape))
    }

    /// Delete a shape (scene owner only)
    async fn delete_shape(&self, ctx: &Context<'_>, shape_id: uuid::Uuid) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::{scenes, shapes};

            // Look up the scene before deleting so we still have it for the NOTIFY payload.
            let scene_id = shapes::table
                .filter(shapes::shape_id.eq(shape_id))
                .filter(
                    shapes::scene_id.eq_any(
                        scenes::table
                            .filter(scenes::owner_id.eq(user_id))
                            .select(scenes::scene_id),
                    ),
                )
                .select(shapes::scene_id)
                .first::<uuid::Uuid>(&mut conn)
                .optional()?;

            let deleted_count = diesel::delete(
                shapes::table
                    .filter(shapes::shape_id.eq(shape_id))
                    .filter(
                        shapes::scene_id.eq_any(
                            scenes::table
                                .filter(scenes::owner_id.eq(user_id))
                                .select(scenes::scene_id),
                        ),
                    ),
            )
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
        .map_err(|_| Error::new("Failed to delete shape"))?;

        Ok(deleted > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::PgConnection;

    /// Establishes a connection to the dev database configured via
    /// DATABASE_URL (same source main.rs uses). Skips (rather than fails)
    /// when no dev database is reachable, since this is a real-DB
    /// integration test, not a unit test.
    fn try_connect() -> Option<PgConnection> {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL").ok()?;
        PgConnection::establish(&url).ok()
    }

    /// Verifies the ownership filter `create_shape`/`update_shape`/
    /// `delete_shape` all share: a shape attached to scene X can only be
    /// mutated by a caller who owns scene X (FR-010, FR-017 equivalents
    /// for shapes). Runs entirely inside a `test_transaction`, which
    /// Diesel always rolls back, so it never leaves fixture rows behind.
    #[test]
    fn shape_mutations_are_scoped_to_scene_owner() {
        let Some(mut conn) = try_connect() else {
            eprintln!("skipping shape_mutations_are_scoped_to_scene_owner: no DATABASE_URL/dev DB reachable");
            return;
        };

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            use crate::schema::{scenes, shapes, users, worlds};

            let owner_id = uuid::Uuid::now_v7();
            let intruder_id = uuid::Uuid::now_v7();
            let world_id = uuid::Uuid::now_v7();
            let scene_id = uuid::Uuid::now_v7();
            let shape_id = uuid::Uuid::now_v7();
            let now = chrono::Utc::now().naive_utc();

            for (id, username) in [(owner_id, "shape-test-owner"), (intruder_id, "shape-test-intruder")] {
                diesel::insert_into(users::table)
                    .values((
                        users::id.eq(id),
                        users::username.eq(format!("{username}-{id}")),
                        users::password_hash.eq("test-hash"),
                        users::email.eq(format!("{username}-{id}@example.test")),
                        users::created_at.eq(now),
                        users::updated_at.eq(now),
                    ))
                    .execute(conn)?;
            }

            diesel::insert_into(worlds::table)
                .values((
                    worlds::id.eq(world_id),
                    worlds::name.eq("Shape Test World"),
                    worlds::created_by.eq(owner_id),
                    worlds::updated_by.eq(owner_id),
                    worlds::created_at.eq(now),
                    worlds::updated_at.eq(now),
                ))
                .execute(conn)?;

            diesel::insert_into(scenes::table)
                .values((
                    scenes::scene_id.eq(scene_id),
                    scenes::world_id.eq(world_id),
                    scenes::name.eq("Shape Test Scene"),
                    scenes::type_.eq("battlemap"),
                    scenes::grid_size.eq(32),
                    scenes::grid_type.eq("square"),
                    scenes::width.eq(1000),
                    scenes::height.eq(1000),
                    scenes::owner_id.eq(owner_id),
                    scenes::created_at.eq(now),
                    scenes::updated_at.eq(now),
                ))
                .execute(conn)?;

            diesel::insert_into(shapes::table)
                .values((
                    shapes::shape_id.eq(shape_id),
                    shapes::scene_id.eq(scene_id),
                    shapes::kind.eq("rect"),
                    shapes::geometry.eq(serde_json::json!({"x": 0, "y": 0, "w": 10, "h": 10})),
                    shapes::visible_to_players.eq(false),
                    shapes::created_by.eq(owner_id),
                    shapes::updated_by.eq(owner_id),
                    shapes::created_at.eq(now),
                    shapes::updated_at.eq(now),
                ))
                .execute(conn)?;

            // The intruder's ownership-scoped update query must match zero rows.
            let intruder_update_count = diesel::update(
                shapes::table.filter(shapes::shape_id.eq(shape_id)).filter(
                    shapes::scene_id.eq_any(
                        scenes::table
                            .filter(scenes::owner_id.eq(intruder_id))
                            .select(scenes::scene_id),
                    ),
                ),
            )
            .set(shapes::visible_to_players.eq(true))
            .execute(conn)?;
            assert_eq!(
                intruder_update_count, 0,
                "a non-owner's update filter must not match another owner's shape"
            );

            // The owner's identical filter must match exactly one row.
            let owner_update_count = diesel::update(
                shapes::table.filter(shapes::shape_id.eq(shape_id)).filter(
                    shapes::scene_id.eq_any(
                        scenes::table
                            .filter(scenes::owner_id.eq(owner_id))
                            .select(scenes::scene_id),
                    ),
                ),
            )
            .set(shapes::visible_to_players.eq(true))
            .execute(conn)?;
            assert_eq!(
                owner_update_count, 1,
                "the scene owner's update filter must match their own shape"
            );

            // Same shape of check for delete.
            let intruder_delete_count = diesel::delete(
                shapes::table.filter(shapes::shape_id.eq(shape_id)).filter(
                    shapes::scene_id.eq_any(
                        scenes::table
                            .filter(scenes::owner_id.eq(intruder_id))
                            .select(scenes::scene_id),
                    ),
                ),
            )
            .execute(conn)?;
            assert_eq!(
                intruder_delete_count, 0,
                "a non-owner's delete filter must not match another owner's shape"
            );

            Ok(())
        });
    }

    #[test]
    fn shape_kind_round_trips_through_db_string_representation() {
        for kind in [
            GraphQLShapeKind::Stroke,
            GraphQLShapeKind::Rect,
            GraphQLShapeKind::Ellipse,
            GraphQLShapeKind::Line,
            GraphQLShapeKind::Text,
        ] {
            assert_eq!(GraphQLShapeKind::from_db_str(kind.as_db_str()), kind);
        }

        // Unknown stored values fall back to "stroke" rather than panicking.
        assert_eq!(
            GraphQLShapeKind::from_db_str("garbage"),
            GraphQLShapeKind::Stroke
        );
    }

    /// Verifies the `shapes` query's player-visibility filter (FR-009):
    /// a non-owner caller must only see `visible_to_players = true`
    /// shapes, while the scene owner sees everything regardless of the
    /// flag. Runs entirely inside a `test_transaction`, which Diesel
    /// always rolls back.
    #[test]
    fn shapes_query_hides_gm_only_shapes_from_non_owners() {
        let Some(mut conn) = try_connect() else {
            eprintln!("skipping shapes_query_hides_gm_only_shapes_from_non_owners: no DATABASE_URL/dev DB reachable");
            return;
        };

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            use crate::schema::{scenes, shapes, users, worlds};

            let owner_id = uuid::Uuid::now_v7();
            let intruder_id = uuid::Uuid::now_v7();
            let world_id = uuid::Uuid::now_v7();
            let scene_id = uuid::Uuid::now_v7();
            let gm_only_shape_id = uuid::Uuid::now_v7();
            let player_visible_shape_id = uuid::Uuid::now_v7();
            let now = chrono::Utc::now().naive_utc();

            for (id, username) in [
                (owner_id, "shape-query-owner"),
                (intruder_id, "shape-query-intruder"),
            ] {
                diesel::insert_into(users::table)
                    .values((
                        users::id.eq(id),
                        users::username.eq(format!("{username}-{id}")),
                        users::password_hash.eq("test-hash"),
                        users::email.eq(format!("{username}-{id}@example.test")),
                        users::created_at.eq(now),
                        users::updated_at.eq(now),
                    ))
                    .execute(conn)?;
            }

            diesel::insert_into(worlds::table)
                .values((
                    worlds::id.eq(world_id),
                    worlds::name.eq("Shape Query Test World"),
                    worlds::created_by.eq(owner_id),
                    worlds::updated_by.eq(owner_id),
                    worlds::created_at.eq(now),
                    worlds::updated_at.eq(now),
                ))
                .execute(conn)?;

            diesel::insert_into(scenes::table)
                .values((
                    scenes::scene_id.eq(scene_id),
                    scenes::world_id.eq(world_id),
                    scenes::name.eq("Shape Query Test Scene"),
                    scenes::type_.eq("battlemap"),
                    scenes::grid_size.eq(32),
                    scenes::grid_type.eq("square"),
                    scenes::width.eq(1000),
                    scenes::height.eq(1000),
                    scenes::owner_id.eq(owner_id),
                    scenes::created_at.eq(now),
                    scenes::updated_at.eq(now),
                ))
                .execute(conn)?;

            diesel::insert_into(shapes::table)
                .values((
                    shapes::shape_id.eq(gm_only_shape_id),
                    shapes::scene_id.eq(scene_id),
                    shapes::kind.eq("rect"),
                    shapes::geometry.eq(serde_json::json!({"x": 0, "y": 0, "w": 10, "h": 10})),
                    shapes::visible_to_players.eq(false),
                    shapes::created_by.eq(owner_id),
                    shapes::updated_by.eq(owner_id),
                    shapes::created_at.eq(now),
                    shapes::updated_at.eq(now),
                ))
                .execute(conn)?;

            diesel::insert_into(shapes::table)
                .values((
                    shapes::shape_id.eq(player_visible_shape_id),
                    shapes::scene_id.eq(scene_id),
                    shapes::kind.eq("ellipse"),
                    shapes::geometry.eq(serde_json::json!({"x": 5, "y": 5, "rx": 3, "ry": 3})),
                    shapes::visible_to_players.eq(true),
                    shapes::created_by.eq(owner_id),
                    shapes::updated_by.eq(owner_id),
                    shapes::created_at.eq(now),
                    shapes::updated_at.eq(now),
                ))
                .execute(conn)?;

            // Owner sees both shapes (no visible_to_players filter applied).
            let owner_is_owner = scenes::table
                .filter(scenes::scene_id.eq(scene_id))
                .filter(scenes::owner_id.eq(owner_id))
                .select(scenes::scene_id)
                .first::<uuid::Uuid>(conn)
                .optional()?
                .is_some();
            assert!(owner_is_owner);
            let owner_visible_ids: Vec<uuid::Uuid> = shapes::table
                .filter(shapes::scene_id.eq(scene_id))
                .select(shapes::shape_id)
                .load(conn)?;
            assert_eq!(
                owner_visible_ids.len(),
                2,
                "the scene owner must see all shapes, including GM-only ones"
            );

            // Non-owner (intruder/player) sees only the visible_to_players shape.
            let intruder_is_owner = scenes::table
                .filter(scenes::scene_id.eq(scene_id))
                .filter(scenes::owner_id.eq(intruder_id))
                .select(scenes::scene_id)
                .first::<uuid::Uuid>(conn)
                .optional()?
                .is_some();
            assert!(!intruder_is_owner);
            let player_visible_ids: Vec<uuid::Uuid> = shapes::table
                .filter(shapes::scene_id.eq(scene_id))
                .filter(shapes::visible_to_players.eq(true))
                .select(shapes::shape_id)
                .load(conn)?;
            assert_eq!(
                player_visible_ids,
                vec![player_visible_shape_id],
                "a non-owner must only see visible_to_players = true shapes"
            );

            Ok(())
        });
    }
}
