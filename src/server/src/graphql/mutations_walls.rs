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
        let is_admin = auth_user.is_admin;
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
            use crate::schema::walls;

            // 🔐 Authority to author content on a scene follows the world
            // role — the Owner and any GM, never a Player — not who happened
            // to create the scene. See `world_membership::is_dm_of_scene`.
            if !crate::auth::world_membership::is_dm_of_scene(
                &mut conn, user_id, is_admin, scene_id,
            )? {
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
        let is_admin = auth_user.is_admin;
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
            use crate::schema::walls;

            // 🔐 Authority to author content on a scene follows the world
            // role — the Owner and any GM, never a Player — not who happened
            // to create the scene. See `world_membership::is_dm_of_scene`.
            let scene_id = walls::table
                .filter(walls::wall_id.eq(wall_id))
                .select(walls::scene_id)
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

            let wall = diesel::update(walls::table.filter(walls::wall_id.eq(wall_id)))
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
        let is_admin = auth_user.is_admin;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::walls;

            // Look up the scene before deleting so we still have it for the NOTIFY payload.
            let scene_id = walls::table
                .filter(walls::wall_id.eq(wall_id))
                .select(walls::scene_id)
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
                // same as "no such wall" and leaks nothing either way.
                return Ok(0);
            }

            let deleted_count = diesel::delete(walls::table.filter(walls::wall_id.eq(wall_id)))
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

    /// The rule every wall mutation (`create_wall`/`update_wall`/`delete_wall`) now asks, in the one place they all ask
    /// it: authority to author content on a scene is the caller's **world
    /// role** — Owner or GM — not who happened to create the scene.
    ///
    /// This replaces `wall_mutations_are_scoped_to_scene_owner`, which asserted the old rule faithfully.
    /// That rule was the bug: two people both holding GM authority in one
    /// world, writing to one scene, had exactly half the writes refused,
    /// because whichever of them had not created the scene was refused every
    /// time. Both directions of that break are asserted below, along with the
    /// two answers that must stay refusals — a GM's new authority must not
    /// leak down to Players or out to non-members.
    #[test]
    fn wall_authority_follows_the_world_role_not_the_scene_creator() {
        let Some(mut conn) = try_connect() else {
            eprintln!(
                "skipping wall_authority_follows_the_world_role_not_the_scene_creator: no DATABASE_URL/dev DB reachable"
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
                "a member promoted to GM must be able to edit walls on a scene the Owner created"
            );
            assert!(
                is_dm_of_scene(conn, owner_id, false, gms_scene)?,
                "the world's Owner must be able to edit walls on a scene a GM created"
            );
            assert!(
                !is_dm_of_scene(conn, player_id, false, owners_scene)?,
                "a plain Player must not gain content authority from world membership"
            );
            assert!(
                !is_dm_of_scene(conn, stranger_id, false, owners_scene)?,
                "a non-member must not be able to edit walls in this world at all"
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
