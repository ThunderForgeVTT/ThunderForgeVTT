//! GraphQL mutations for scene light sources (native canvas authoring).

use async_graphql::{Context, Error, Result as GraphQLResult};
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::graphql::{
    GraphQLCreateLightSourceInput, GraphQLLightSource, GraphQLUpdateLightSourceInput, app_state,
    authenticated_user,
};
use crate::world_events::{
    EVENT_CODE_LIGHT_SOURCE_CHANGED, record_world_event, world_id_for_scene,
};

#[derive(Default)]
pub struct LightSourceMutation;

#[async_graphql::Object]
impl LightSourceMutation {
    /// Create a new light source on a scene (scene owner only)
    async fn create_light_source(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateLightSourceInput,
    ) -> GraphQLResult<GraphQLLightSource> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let light_id = uuid::Uuid::now_v7();
        let scene_id = input.scene_id;
        let x = input.x;
        let y = input.y;
        let radius = input.radius;
        let intensity = input.intensity.unwrap_or(1.0);
        let color = input.color;
        let attached_token_id = input.attached_token_id;
        let casts_shadows = input.casts_shadows.unwrap_or(true);
        let metadata = input.metadata.map(|j| j.0);

        let inserted_light = tokio::task::spawn_blocking(move || {
            use crate::schema::{light_sources, scenes};

            // 🔐 Ownership: caller must own the parent scene before a light can be attached to it
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

            let light = diesel::insert_into(light_sources::table)
                .values((
                    light_sources::light_id.eq(light_id),
                    light_sources::scene_id.eq(scene_id),
                    light_sources::x.eq(x),
                    light_sources::y.eq(y),
                    light_sources::radius.eq(radius),
                    light_sources::intensity.eq(intensity),
                    light_sources::color.eq(&color),
                    light_sources::attached_token_id.eq(attached_token_id),
                    light_sources::casts_shadows.eq(casts_shadows),
                    light_sources::metadata.eq(&metadata),
                    light_sources::created_by.eq(user_id),
                    light_sources::updated_by.eq(user_id),
                    light_sources::created_at.eq(now),
                    light_sources::updated_at.eq(now),
                ))
                .returning(crate::models::LightSource::as_returning())
                .get_result(&mut conn)?;

            if let Ok(world_id) = world_id_for_scene(&mut conn, scene_id) {
                let _ = record_world_event(
                    &mut conn,
                    world_id,
                    EVENT_CODE_LIGHT_SOURCE_CHANGED,
                    Some(serde_json::json!({
                        "action": "created",
                        "light_id": light_id,
                        "scene_id": scene_id,
                    })),
                    user_id,
                );
            }

            Ok(light)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| {
            Error::new("Failed to create light source (scene not found or not owned by you)")
        })?;

        Ok(GraphQLLightSource::from(inserted_light))
    }

    /// Update an existing light source (scene owner only)
    async fn update_light_source(
        &self,
        ctx: &Context<'_>,
        light_id: uuid::Uuid,
        input: GraphQLUpdateLightSourceInput,
    ) -> GraphQLResult<GraphQLLightSource> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let update_data = crate::models::LightSourceUpdate {
            x: input.x,
            y: input.y,
            radius: input.radius,
            intensity: input.intensity,
            color: input.color,
            attached_token_id: input.attached_token_id,
            casts_shadows: input.casts_shadows,
            metadata: input.metadata.map(|j| j.0),
            updated_by: user_id,
        };

        let updated_light = tokio::task::spawn_blocking(move || {
            use crate::schema::{light_sources, scenes};

            let light = diesel::update(
                light_sources::table
                    .filter(light_sources::light_id.eq(light_id))
                    .filter(
                        light_sources::scene_id.eq_any(
                            scenes::table
                                .filter(scenes::owner_id.eq(user_id))
                                .select(scenes::scene_id),
                        ),
                    ),
            )
            .set(update_data)
            .returning(crate::models::LightSource::as_returning())
            .get_result(&mut conn)?;

            if let Ok(world_id) = world_id_for_scene(&mut conn, light.scene_id) {
                let _ = record_world_event(
                    &mut conn,
                    world_id,
                    EVENT_CODE_LIGHT_SOURCE_CHANGED,
                    Some(serde_json::json!({
                        "action": "updated",
                        "light_id": light_id,
                        "scene_id": light.scene_id,
                    })),
                    user_id,
                );
            }

            Ok::<_, DieselError>(light)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to update light source (not found or not owned by you)"))?;

        Ok(GraphQLLightSource::from(updated_light))
    }

    /// Delete a light source (scene owner only)
    async fn delete_light_source(
        &self,
        ctx: &Context<'_>,
        light_id: uuid::Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::{light_sources, scenes};

            // Look up the scene before deleting so we still have it for the NOTIFY payload.
            let scene_id = light_sources::table
                .filter(light_sources::light_id.eq(light_id))
                .filter(
                    light_sources::scene_id.eq_any(
                        scenes::table
                            .filter(scenes::owner_id.eq(user_id))
                            .select(scenes::scene_id),
                    ),
                )
                .select(light_sources::scene_id)
                .first::<uuid::Uuid>(&mut conn)
                .optional()?;

            let deleted_count = diesel::delete(
                light_sources::table
                    .filter(light_sources::light_id.eq(light_id))
                    .filter(
                        light_sources::scene_id.eq_any(
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
                    EVENT_CODE_LIGHT_SOURCE_CHANGED,
                    Some(serde_json::json!({
                        "action": "deleted",
                        "light_id": light_id,
                        "scene_id": scene_id,
                    })),
                    user_id,
                );
            }

            Ok::<_, DieselError>(deleted_count)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to delete light source"))?;

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

    /// Verifies the ownership filter `create_light_source`/
    /// `update_light_source`/`delete_light_source` all share: a light
    /// attached to scene X can only be mutated by a caller who owns scene
    /// X. Runs entirely inside a `test_transaction`, which Diesel always
    /// rolls back, so it never leaves fixture rows behind.
    #[test]
    fn light_source_mutations_are_scoped_to_scene_owner() {
        let Some(mut conn) = try_connect() else {
            eprintln!(
                "skipping light_source_mutations_are_scoped_to_scene_owner: no DATABASE_URL/dev DB reachable"
            );
            return;
        };

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            use crate::schema::{light_sources, scenes, users, worlds};

            let owner_id = uuid::Uuid::now_v7();
            let intruder_id = uuid::Uuid::now_v7();
            let world_id = uuid::Uuid::now_v7();
            let scene_id = uuid::Uuid::now_v7();
            let light_id = uuid::Uuid::now_v7();
            let now = chrono::Utc::now().naive_utc();

            for (id, username) in [
                (owner_id, "light-test-owner"),
                (intruder_id, "light-test-intruder"),
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
                    worlds::name.eq("Light Test World"),
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
                    scenes::name.eq("Light Test Scene"),
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

            diesel::insert_into(light_sources::table)
                .values((
                    light_sources::light_id.eq(light_id),
                    light_sources::scene_id.eq(scene_id),
                    light_sources::x.eq(0.0),
                    light_sources::y.eq(0.0),
                    light_sources::radius.eq(20.0),
                    light_sources::intensity.eq(1.0),
                    light_sources::casts_shadows.eq(true),
                    light_sources::created_by.eq(owner_id),
                    light_sources::updated_by.eq(owner_id),
                    light_sources::created_at.eq(now),
                    light_sources::updated_at.eq(now),
                ))
                .execute(conn)?;

            // The intruder's ownership-scoped update query must match zero rows.
            let intruder_update_count = diesel::update(
                light_sources::table
                    .filter(light_sources::light_id.eq(light_id))
                    .filter(
                        light_sources::scene_id.eq_any(
                            scenes::table
                                .filter(scenes::owner_id.eq(intruder_id))
                                .select(scenes::scene_id),
                        ),
                    ),
            )
            .set(light_sources::intensity.eq(0.5))
            .execute(conn)?;
            assert_eq!(
                intruder_update_count, 0,
                "a non-owner's update filter must not match another owner's light source"
            );

            // The owner's identical filter must match exactly one row.
            let owner_update_count = diesel::update(
                light_sources::table
                    .filter(light_sources::light_id.eq(light_id))
                    .filter(
                        light_sources::scene_id.eq_any(
                            scenes::table
                                .filter(scenes::owner_id.eq(owner_id))
                                .select(scenes::scene_id),
                        ),
                    ),
            )
            .set(light_sources::intensity.eq(0.5))
            .execute(conn)?;
            assert_eq!(
                owner_update_count, 1,
                "the scene owner's update filter must match their own light source"
            );

            // Same shape of check for delete.
            let intruder_delete_count = diesel::delete(
                light_sources::table
                    .filter(light_sources::light_id.eq(light_id))
                    .filter(
                        light_sources::scene_id.eq_any(
                            scenes::table
                                .filter(scenes::owner_id.eq(intruder_id))
                                .select(scenes::scene_id),
                        ),
                    ),
            )
            .execute(conn)?;
            assert_eq!(
                intruder_delete_count, 0,
                "a non-owner's delete filter must not match another owner's light source"
            );

            Ok(())
        });
    }
}
