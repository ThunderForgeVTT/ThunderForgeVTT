//! GraphQL mutations for scene walls (Phase 6: Walls and Lighting; door
//! semantics and real-time NOTIFY added for native canvas authoring)

use async_graphql::{Context, Error, Result as GraphQLResult};
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::graphql::{
    GraphQLCreateWallInput, GraphQLDoorState, GraphQLUpdateWallInput, GraphQLWall, app_state,
    authenticated_user,
};
use crate::world_events::{EVENT_CODE_WALL_CHANGED, record_world_event, world_id_for_scene};

#[derive(Default)]
pub struct WallMutation;

#[async_graphql::Object]
impl WallMutation {
    /// Create a new wall on a scene (scene owner only)
    async fn create_wall(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateWallInput,
    ) -> GraphQLResult<GraphQLWall> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let wall_id = uuid::Uuid::now_v7();
        let scene_id = input.scene_id;
        let x1 = input.x1;
        let y1 = input.y1;
        let x2 = input.x2;
        let y2 = input.y2;
        let blocks_vision = input.blocks_vision.unwrap_or(true);
        let blocks_movement = input.blocks_movement.unwrap_or(false);
        let door_state = input
            .door_state
            .unwrap_or(GraphQLDoorState::None)
            .as_db_str()
            .to_string();
        let metadata = input.metadata.map(|j| j.0);

        let inserted_wall = tokio::task::spawn_blocking(move || {
            use crate::schema::{scenes, walls};

            // 🔐 Ownership: caller must own the parent scene before a wall can be attached to it
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

            let wall = diesel::insert_into(walls::table)
                .values((
                    walls::wall_id.eq(wall_id),
                    walls::scene_id.eq(scene_id),
                    walls::x1.eq(x1),
                    walls::y1.eq(y1),
                    walls::x2.eq(x2),
                    walls::y2.eq(y2),
                    walls::blocks_vision.eq(blocks_vision),
                    walls::blocks_movement.eq(blocks_movement),
                    walls::door_state.eq(&door_state),
                    walls::metadata.eq(&metadata),
                    walls::created_by.eq(user_id),
                    walls::updated_by.eq(user_id),
                    walls::created_at.eq(now),
                    walls::updated_at.eq(now),
                ))
                .returning(crate::models::Wall::as_returning())
                .get_result(&mut conn)?;

            if let Ok(world_id) = world_id_for_scene(&mut conn, scene_id) {
                let _ = record_world_event(
                    &mut conn,
                    world_id,
                    EVENT_CODE_WALL_CHANGED,
                    Some(serde_json::json!({
                        "action": "created",
                        "wall_id": wall_id,
                        "scene_id": scene_id,
                    })),
                    user_id,
                );
            }

            Ok(wall)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to create wall (scene not found or not owned by you)"))?;

        Ok(GraphQLWall::from(inserted_wall))
    }

    /// Update an existing wall (scene owner only)
    async fn update_wall(
        &self,
        ctx: &Context<'_>,
        wall_id: uuid::Uuid,
        input: GraphQLUpdateWallInput,
    ) -> GraphQLResult<GraphQLWall> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let update_data = crate::models::WallUpdate {
            x1: input.x1,
            y1: input.y1,
            x2: input.x2,
            y2: input.y2,
            blocks_vision: input.blocks_vision,
            blocks_movement: input.blocks_movement,
            door_state: input.door_state.map(|d| d.as_db_str().to_string()),
            metadata: input.metadata.map(|j| j.0),
            updated_by: user_id,
        };

        let updated_wall = tokio::task::spawn_blocking(move || {
            use crate::schema::{scenes, walls};

            let wall = diesel::update(
                walls::table.filter(walls::wall_id.eq(wall_id)).filter(
                    walls::scene_id.eq_any(
                        scenes::table
                            .filter(scenes::owner_id.eq(user_id))
                            .select(scenes::scene_id),
                    ),
                ),
            )
            .set(update_data)
            .returning(crate::models::Wall::as_returning())
            .get_result(&mut conn)?;

            if let Ok(world_id) = world_id_for_scene(&mut conn, wall.scene_id) {
                let _ = record_world_event(
                    &mut conn,
                    world_id,
                    EVENT_CODE_WALL_CHANGED,
                    Some(serde_json::json!({
                        "action": "updated",
                        "wall_id": wall_id,
                        "scene_id": wall.scene_id,
                    })),
                    user_id,
                );
            }

            Ok::<_, DieselError>(wall)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to update wall (not found or not owned by you)"))?;

        Ok(GraphQLWall::from(updated_wall))
    }

    /// Delete a wall (scene owner only)
    async fn delete_wall(&self, ctx: &Context<'_>, wall_id: uuid::Uuid) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::{scenes, walls};

            // Look up the scene before deleting so we still have it for the NOTIFY payload.
            let scene_id = walls::table
                .filter(walls::wall_id.eq(wall_id))
                .filter(
                    walls::scene_id.eq_any(
                        scenes::table
                            .filter(scenes::owner_id.eq(user_id))
                            .select(scenes::scene_id),
                    ),
                )
                .select(walls::scene_id)
                .first::<uuid::Uuid>(&mut conn)
                .optional()?;

            let deleted_count = diesel::delete(
                walls::table.filter(walls::wall_id.eq(wall_id)).filter(
                    walls::scene_id.eq_any(
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
                    EVENT_CODE_WALL_CHANGED,
                    Some(serde_json::json!({
                        "action": "deleted",
                        "wall_id": wall_id,
                        "scene_id": scene_id,
                    })),
                    user_id,
                );
            }

            Ok::<_, DieselError>(deleted_count)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to delete wall"))?;

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

    /// Verifies the ownership filter `create_wall`/`update_wall`/
    /// `delete_wall` all share: a wall attached to scene X can only be
    /// mutated by a caller who owns scene X (FR-010, FR-017). Runs
    /// entirely inside a `test_transaction`, which Diesel always rolls
    /// back, so it never leaves fixture rows behind.
    #[test]
    fn wall_mutations_are_scoped_to_scene_owner() {
        let Some(mut conn) = try_connect() else {
            eprintln!(
                "skipping wall_mutations_are_scoped_to_scene_owner: no DATABASE_URL/dev DB reachable"
            );
            return;
        };

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            use crate::schema::{scenes, users, walls, worlds};

            let owner_id = uuid::Uuid::now_v7();
            let intruder_id = uuid::Uuid::now_v7();
            let world_id = uuid::Uuid::now_v7();
            let scene_id = uuid::Uuid::now_v7();
            let wall_id = uuid::Uuid::now_v7();
            let now = chrono::Utc::now().naive_utc();

            for (id, username) in [
                (owner_id, "wall-test-owner"),
                (intruder_id, "wall-test-intruder"),
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
                    worlds::name.eq("Wall Test World"),
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
                    scenes::name.eq("Wall Test Scene"),
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

            diesel::insert_into(walls::table)
                .values((
                    walls::wall_id.eq(wall_id),
                    walls::scene_id.eq(scene_id),
                    walls::x1.eq(0.0),
                    walls::y1.eq(0.0),
                    walls::x2.eq(10.0),
                    walls::y2.eq(0.0),
                    walls::blocks_vision.eq(true),
                    walls::blocks_movement.eq(false),
                    walls::door_state.eq("none"),
                    walls::created_by.eq(owner_id),
                    walls::updated_by.eq(owner_id),
                    walls::created_at.eq(now),
                    walls::updated_at.eq(now),
                ))
                .execute(conn)?;

            // The intruder's ownership-scoped update query must match zero rows.
            let intruder_update_count = diesel::update(
                walls::table.filter(walls::wall_id.eq(wall_id)).filter(
                    walls::scene_id.eq_any(
                        scenes::table
                            .filter(scenes::owner_id.eq(intruder_id))
                            .select(scenes::scene_id),
                    ),
                ),
            )
            .set(walls::blocks_vision.eq(false))
            .execute(conn)?;
            assert_eq!(
                intruder_update_count, 0,
                "a non-owner's update filter must not match another owner's wall"
            );

            // The owner's identical filter must match exactly one row.
            let owner_update_count = diesel::update(
                walls::table.filter(walls::wall_id.eq(wall_id)).filter(
                    walls::scene_id.eq_any(
                        scenes::table
                            .filter(scenes::owner_id.eq(owner_id))
                            .select(scenes::scene_id),
                    ),
                ),
            )
            .set(walls::blocks_vision.eq(false))
            .execute(conn)?;
            assert_eq!(
                owner_update_count, 1,
                "the scene owner's update filter must match their own wall"
            );

            // Same shape of check for delete.
            let intruder_delete_count = diesel::delete(
                walls::table.filter(walls::wall_id.eq(wall_id)).filter(
                    walls::scene_id.eq_any(
                        scenes::table
                            .filter(scenes::owner_id.eq(intruder_id))
                            .select(scenes::scene_id),
                    ),
                ),
            )
            .execute(conn)?;
            assert_eq!(
                intruder_delete_count, 0,
                "a non-owner's delete filter must not match another owner's wall"
            );

            Ok(())
        });
    }

    #[test]
    fn door_state_round_trips_through_db_string_representation() {
        for state in [
            GraphQLDoorState::None,
            GraphQLDoorState::Open,
            GraphQLDoorState::Closed,
        ] {
            assert_eq!(GraphQLDoorState::from_db_str(state.as_db_str()), state);
        }

        // Unknown stored values fall back to "no door" rather than panicking.
        assert_eq!(
            GraphQLDoorState::from_db_str("garbage"),
            GraphQLDoorState::None
        );
    }
}
