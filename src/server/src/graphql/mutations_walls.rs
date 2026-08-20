//! GraphQL mutations for scene walls (Phase 6: Walls and Lighting)

use async_graphql::{Context, Error, Result as GraphQLResult};
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::graphql::{app_state, authenticated_user, GraphQLCreateWallInput, GraphQLUpdateWallInput, GraphQLWall};

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

            diesel::insert_into(walls::table)
                .values((
                    walls::wall_id.eq(wall_id),
                    walls::scene_id.eq(scene_id),
                    walls::x1.eq(x1),
                    walls::y1.eq(y1),
                    walls::x2.eq(x2),
                    walls::y2.eq(y2),
                    walls::blocks_vision.eq(blocks_vision),
                    walls::blocks_movement.eq(blocks_movement),
                    walls::metadata.eq(&metadata),
                    walls::created_by.eq(user_id),
                    walls::updated_by.eq(user_id),
                    walls::created_at.eq(now),
                    walls::updated_at.eq(now),
                ))
                .returning(crate::models::Wall::as_returning())
                .get_result(&mut conn)
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
            metadata: input.metadata.map(|j| j.0),
            updated_by: user_id,
        };

        let updated_wall = tokio::task::spawn_blocking(move || {
            use crate::schema::{scenes, walls};

            diesel::update(
                walls::table
                    .filter(walls::wall_id.eq(wall_id))
                    .filter(
                        walls::scene_id.eq_any(
                            scenes::table
                                .filter(scenes::owner_id.eq(user_id))
                                .select(scenes::scene_id),
                        ),
                    ),
            )
            .set(update_data)
            .returning(crate::models::Wall::as_returning())
            .get_result(&mut conn)
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

            diesel::delete(
                walls::table
                    .filter(walls::wall_id.eq(wall_id))
                    .filter(
                        walls::scene_id.eq_any(
                            scenes::table
                                .filter(scenes::owner_id.eq(user_id))
                                .select(scenes::scene_id),
                        ),
                    ),
            )
            .execute(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to delete wall"))?;

        Ok(deleted > 0)
    }
}
