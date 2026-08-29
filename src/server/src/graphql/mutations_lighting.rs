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
        let is_admin = auth_user.is_admin;
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
            use crate::schema::light_sources;

            // 🔐 Authority to author content on a scene follows the world
            // role — the Owner and any GM, never a Player — not who happened
            // to create the scene. See `world_membership::is_dm_of_scene`.
            if !crate::auth::world_membership::is_dm_of_scene(
                &mut conn, user_id, is_admin, scene_id,
            )? {
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
        let is_admin = auth_user.is_admin;
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
            use crate::schema::light_sources;

            // 🔐 Authority to author content on a scene follows the world
            // role — the Owner and any GM, never a Player — not who happened
            // to create the scene. See `world_membership::is_dm_of_scene`.
            let scene_id = light_sources::table
                .filter(light_sources::light_id.eq(light_id))
                .select(light_sources::scene_id)
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

            let light =
                diesel::update(light_sources::table.filter(light_sources::light_id.eq(light_id)))
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
        let is_admin = auth_user.is_admin;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::light_sources;

            // Look up the scene before deleting so we still have it for the NOTIFY payload.
            let scene_id = light_sources::table
                .filter(light_sources::light_id.eq(light_id))
                .select(light_sources::scene_id)
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
                // same as "no such light source" and leaks nothing either way.
                return Ok(0);
            }

            let deleted_count =
                diesel::delete(light_sources::table.filter(light_sources::light_id.eq(light_id)))
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

    /// The rule every light-source mutation now asks, in the one place they all ask
    /// it: authority to author content on a scene is the caller's **world
    /// role** — Owner or GM — not who happened to create the scene.
    ///
    /// This replaces `light_source_mutations_are_scoped_to_scene_owner`, which asserted the old rule faithfully.
    /// That rule was the bug: two people both holding GM authority in one
    /// world, writing to one scene, had exactly half the writes refused,
    /// because whichever of them had not created the scene was refused every
    /// time. Both directions of that break are asserted below, along with the
    /// two answers that must stay refusals — a GM's new authority must not
    /// leak down to Players or out to non-members.
    #[test]
    fn light_source_authority_follows_the_world_role_not_the_scene_creator() {
        let Some(mut conn) = try_connect() else {
            eprintln!(
                "skipping light_source_authority_follows_the_world_role_not_the_scene_creator: no DATABASE_URL/dev DB reachable"
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
                "a member promoted to GM must be able to edit lights on a scene the Owner created"
            );
            assert!(
                is_dm_of_scene(conn, owner_id, false, gms_scene)?,
                "the world's Owner must be able to edit lights on a scene a GM created"
            );
            assert!(
                !is_dm_of_scene(conn, player_id, false, owners_scene)?,
                "a plain Player must not gain content authority from world membership"
            );
            assert!(
                !is_dm_of_scene(conn, stranger_id, false, owners_scene)?,
                "a non-member must not be able to edit lights in this world at all"
            );

            Ok(())
        });
    }
}
