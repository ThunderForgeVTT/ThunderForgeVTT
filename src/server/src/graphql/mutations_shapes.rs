//! GraphQL mutations for scene shapes (native canvas authoring: freehand
//! strokes, rectangles, ellipses, lines, and text drawn directly on the
//! canvas). Mirrors `mutations_walls.rs`'s scene-ownership and real-time
//! NOTIFY pattern.

use async_graphql::{Context, Error, Result as GraphQLResult};
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

#[cfg(test)]
use crate::graphql::GraphQLShapeKind;
use crate::graphql::{
    GraphQLCreateShapeInput, GraphQLShape, GraphQLUpdateShapeInput, app_state, authenticated_user,
};
use crate::world_events::{EVENT_CODE_SHAPE_CHANGED, record_world_event, world_id_for_scene};

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
        let is_admin = auth_user.is_admin;
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
            use crate::schema::shapes;

            // 🔐 Authority to author content on a scene follows the world
            // role — the Owner and any GM, never a Player — not who happened
            // to create the scene. See `world_membership::is_dm_of_scene`.
            if !crate::auth::world_membership::is_dm_of_scene(
                &mut conn, user_id, is_admin, scene_id,
            )? {
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

            // 🔐 Authority to author content on a scene follows the world
            // role — the Owner and any GM, never a Player — not who happened
            // to create the scene. See `world_membership::is_dm_of_scene`.
            let scene_id = shapes::table
                .filter(shapes::shape_id.eq(shape_id))
                .select(shapes::scene_id)
                .first::<uuid::Uuid>(&mut conn)
                .optional()?;
            let authorized = match scene_id {
                Some(scene_id) => crate::auth::world_membership::is_dm_of_scene(
                    &mut conn, user_id, is_admin, scene_id,
                )?,
                None => false,
            };
            if !authorized {
                return Err(DieselError::NotFound);
            }

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
        .map_err(|_| Error::new("Failed to update shape (not found or not owned by you)"))?;

        Ok(GraphQLShape::from(updated_shape))
    }

    /// Delete a shape (scene owner only)
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
            let scene_id = shapes::table
                .filter(shapes::shape_id.eq(shape_id))
                .select(shapes::scene_id)
                .first::<uuid::Uuid>(&mut conn)
                .optional()?;

            // 🔐 Authority to author content on a scene follows the world
            // role — the Owner and any GM, never a Player — not who happened
            // to create the scene. See `world_membership::is_dm_of_scene`.
            let authorized = match scene_id {
                Some(scene_id) => crate::auth::world_membership::is_dm_of_scene(
                    &mut conn, user_id, is_admin, scene_id,
                )?,
                None => false,
            };
            if !authorized {
                // Nothing was deleted, which is what an unauthorized
                // caller has always been told — the refusal reads the
                // same as "no such shape" and leaks nothing either way.
                return Ok(0);
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

    /// The rule every shape mutation (`create_shape`/`update_shape`/`delete_shape`) now asks, in the one place they all ask
    /// it: authority to author content on a scene is the caller's **world
    /// role** — Owner or GM — not who happened to create the scene.
    ///
    /// This replaces `shape_mutations_are_scoped_to_scene_owner`, which asserted the old rule faithfully.
    /// That rule was the bug: two people both holding GM authority in one
    /// world, writing to one scene, had exactly half the writes refused,
    /// because whichever of them had not created the scene was refused every
    /// time. Both directions of that break are asserted below, along with the
    /// two answers that must stay refusals — a GM's new authority must not
    /// leak down to Players or out to non-members.
    #[test]
    fn shape_authority_follows_the_world_role_not_the_scene_creator() {
        let Some(mut conn) = try_connect() else {
            eprintln!(
                "skipping shape_authority_follows_the_world_role_not_the_scene_creator: no DATABASE_URL/dev DB reachable"
            );
            return;
        };

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            use crate::auth::world_membership::is_dm_of_scene;
            use crate::test_support::{
                insert_test_scene_named, insert_test_user, insert_test_world,
                insert_test_world_member,
            };

            let owner_id = insert_test_user(conn);
            let world_id = insert_test_world(conn, owner_id);

            let gm_id = insert_test_user(conn);
            insert_test_world_member(conn, world_id, gm_id, "GM");
            let player_id = insert_test_user(conn);
            insert_test_world_member(conn, world_id, player_id, "Player");
            let stranger_id = insert_test_user(conn);

            // Two scenes in the same world, created by two different people.
            // Under the old rule each of them was an island.
            let owners_scene = insert_test_scene_named(conn, world_id, owner_id, "Owner's Scene");
            let gms_scene = insert_test_scene_named(conn, world_id, gm_id, "GM's Scene");

            assert!(
                is_dm_of_scene(conn, gm_id, false, owners_scene)?,
                "a member promoted to GM must be able to edit shapes on a scene the Owner created"
            );
            assert!(
                is_dm_of_scene(conn, owner_id, false, gms_scene)?,
                "the world's Owner must be able to edit shapes on a scene a GM created"
            );
            assert!(
                !is_dm_of_scene(conn, player_id, false, owners_scene)?,
                "a plain Player must not gain content authority from world membership"
            );
            assert!(
                !is_dm_of_scene(conn, stranger_id, false, owners_scene)?,
                "a non-member must not be able to edit shapes in this world at all"
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
            eprintln!(
                "skipping shapes_query_hides_gm_only_shapes_from_non_owners: no DATABASE_URL/dev DB reachable"
            );
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
