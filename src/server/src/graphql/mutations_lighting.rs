//! GraphQL mutations for scene light sources (native canvas authoring).

use async_graphql::{Context, Error, Result as GraphQLResult};
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::graphql::{
    GraphQLCreateLightSourceInput, GraphQLLightSource, GraphQLUpdateLightSourceInput, app_state,
    authenticated_user,
};
use crate::play_pause::gate::{refusal_or, refuse_scene_if_paused};
use crate::world_events::{
    EVENT_CODE_LIGHT_SOURCE_CHANGED, record_world_event, world_id_for_scene,
};

/// A light's bright reach, checked against its dim reach (spec 045 FR-061).
///
/// Refused rather than clamped when a caller names both and they disagree: a
/// Game Master who typed a bright reach past the dim one has made a mistake
/// worth being told about, not one to be quietly corrected into a different
/// light.
fn checked_reaches(radius: f64, bright_radius: f64) -> Result<(), Error> {
    if !bright_radius.is_finite() || bright_radius < 0.0 {
        return Err(Error::new("A light's bright reach cannot be negative"));
    }
    if bright_radius > radius {
        return Err(Error::new(
            "A light's bright reach cannot be further than its dim reach",
        ));
    }
    Ok(())
}

/// Read the stored reaches and write the new ones in **one transaction**,
/// with the light's row locked while it happens.
///
/// The clamping rules need what is stored now — a dim reach pulled in brings
/// the bright one with it, and a bright reach is never stored past the dim —
/// so a read followed by an unguarded write clamps against a reach another
/// writer may already have moved. What the table then refuses is the honest
/// write: `light_sources_bright_within_dim` rejects the row, and a Game Master
/// who pushed the bright reach out while the dim one was shrinking is told
/// their change failed for no reason they can see.
///
/// The client queues its writes, but the server is what makes the rule true,
/// so it locks the row it is about to write, exactly as `combat::hit_points`
/// does. A second writer waits here and then reads what the first left.
fn update_light_source_sync(
    conn: &mut PgConnection,
    light_id: uuid::Uuid,
    user_id: uuid::Uuid,
    is_admin: bool,
    mut update_data: crate::models::LightSourceUpdate,
) -> Result<crate::models::LightSource, DieselError> {
    use crate::schema::light_sources;

    conn.transaction(|conn| {
        // The lock, and the read the clamp is decided from: one statement, so
        // nothing can change the reaches between them.
        let stored = light_sources::table
            .filter(light_sources::light_id.eq(light_id))
            .select((
                light_sources::scene_id,
                light_sources::radius,
                light_sources::bright_radius,
            ))
            .for_update()
            .first::<(uuid::Uuid, f64, f64)>(conn)
            .optional()?;

        // 🔐 Authority to author content on a scene follows the world
        // role — the Owner and any GM, never a Player — not who happened
        // to create the scene. See `world_membership::is_dm_of_scene`.
        let Some((scene_id, stored_radius, stored_bright)) = stored else {
            return Err(DieselError::NotFound);
        };
        if !crate::auth::world_membership::is_dm_of_scene(conn, user_id, is_admin, scene_id)? {
            return Err(DieselError::NotFound);
        }
        refuse_scene_if_paused(conn, scene_id)?;

        // Whichever reach was not named, the other is kept within:
        // a dim reach pulled in past the bright one brings the bright
        // one with it, and a bright reach is never stored past the dim.
        let radius = update_data.radius.unwrap_or(stored_radius);
        let bright = update_data.bright_radius.unwrap_or(stored_bright);
        if update_data.radius.is_some() || update_data.bright_radius.is_some() {
            update_data.bright_radius = Some(bright.clamp(0.0, radius.max(0.0)));
        }

        let light =
            diesel::update(light_sources::table.filter(light_sources::light_id.eq(light_id)))
                .set(update_data)
                .returning(crate::models::LightSource::as_returning())
                .get_result(conn)?;

        // In the transaction too: a client told the light changed must not be
        // able to read it before the change is there.
        if let Ok(world_id) = world_id_for_scene(conn, light.scene_id) {
            let _ = record_world_event(
                conn,
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

        Ok(light)
    })
}

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
        // FR-062: a light created without a bright reach looks as every light
        // did before it had one.
        let bright_radius = input.bright_radius.unwrap_or(radius * 0.5);
        checked_reaches(radius, bright_radius)?;
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
            refuse_scene_if_paused(&mut conn, scene_id)?;

            let light = diesel::insert_into(light_sources::table)
                .values((
                    light_sources::light_id.eq(light_id),
                    light_sources::scene_id.eq(scene_id),
                    light_sources::x.eq(x),
                    light_sources::y.eq(y),
                    light_sources::radius.eq(radius),
                    light_sources::bright_radius.eq(bright_radius),
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
        .map_err(|e| {
            refusal_or(
                e,
                "Failed to create light source (scene not found or not owned by you)",
            )
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

        if let (Some(radius), Some(bright_radius)) = (input.radius, input.bright_radius) {
            checked_reaches(radius, bright_radius)?;
        }
        let update_data = crate::models::LightSourceUpdate {
            bright_radius: input.bright_radius,
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
            update_light_source_sync(&mut conn, light_id, user_id, is_admin, update_data)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|e| {
            refusal_or(
                e,
                "Failed to update light source (not found or not owned by you)",
            )
        })?;

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
            if let Some(scene_id) = scene_id {
                refuse_scene_if_paused(&mut conn, scene_id)?;
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
        .map_err(|e| refusal_or(e, "Failed to delete light source"))?;

        Ok(deleted > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::PgConnection;

    /// A world, a scene and a light on it, at a dim reach of 100 and a bright
    /// reach of 50.
    fn a_light(conn: &mut PgConnection, gm: uuid::Uuid) -> (uuid::Uuid, uuid::Uuid) {
        use crate::schema::light_sources;
        use crate::test_support::{insert_test_scene, insert_test_world};

        let world_id = insert_test_world(conn, gm);
        let scene_id = insert_test_scene(conn, world_id, gm);
        let light_id = uuid::Uuid::now_v7();
        let now = Utc::now().naive_utc();
        diesel::insert_into(light_sources::table)
            .values((
                light_sources::light_id.eq(light_id),
                light_sources::scene_id.eq(scene_id),
                light_sources::x.eq(0.0),
                light_sources::y.eq(0.0),
                light_sources::radius.eq(100.0),
                light_sources::bright_radius.eq(50.0),
                light_sources::intensity.eq(1.0),
                light_sources::casts_shadows.eq(true),
                light_sources::created_by.eq(gm),
                light_sources::updated_by.eq(gm),
                light_sources::created_at.eq(now),
                light_sources::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("insert light");
        (scene_id, light_id)
    }

    fn reaches(conn: &mut PgConnection, light_id: uuid::Uuid) -> (f64, f64) {
        use crate::schema::light_sources;
        light_sources::table
            .filter(light_sources::light_id.eq(light_id))
            .select((light_sources::radius, light_sources::bright_radius))
            .first::<(f64, f64)>(conn)
            .expect("the light")
    }

    fn write(
        conn: &mut PgConnection,
        light_id: uuid::Uuid,
        gm: uuid::Uuid,
        radius: Option<f64>,
        bright_radius: Option<f64>,
    ) -> Result<crate::models::LightSource, DieselError> {
        update_light_source_sync(
            conn,
            light_id,
            gm,
            false,
            crate::models::LightSourceUpdate {
                x: None,
                y: None,
                radius,
                intensity: None,
                color: None,
                attached_token_id: None,
                casts_shadows: None,
                metadata: None,
                updated_by: gm,
                bright_radius,
            },
        )
    }

    /// The reaches are read and written in one transaction, with the row
    /// locked — so two writers who arrive together cannot clamp against a
    /// reach the other has already moved.
    ///
    /// One shrinks the dim reach to 10, which pulls the bright reach in with
    /// it; the other pushes the bright reach out to 80, which is only ever
    /// stored within the dim one. Both orders end at the same place —
    /// `(10, 10)` — so the answer is the test's to assert rather than the
    /// scheduler's. Without the lock the second writer clamps 80 against the
    /// 100 the first has already replaced, and the table refuses the row it
    /// then tries to write (`light_sources_bright_within_dim`): the write is
    /// lost, and the caller is told nothing it can act on.
    #[test]
    fn two_writers_at_once_leave_one_light_and_lose_no_clamp() {
        let state = crate::test_support::test_app_state();
        let gm = {
            let mut conn = state.db_pool.get().expect("conn");
            crate::test_support::insert_test_user(&mut conn)
        };
        let light_id = {
            let mut conn = state.db_pool.get().expect("conn");
            a_light(&mut conn, gm).1
        };

        // Enough rounds that an unguarded read-then-write loses a clamp in
        // one of them; the lock makes every round the same.
        for round in 0..25 {
            {
                let mut conn = state.db_pool.get().expect("conn");
                write(&mut conn, light_id, gm, Some(100.0), Some(50.0)).expect("reset");
            }
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
            let handles: Vec<_> = [(Some(10.0), None), (None, Some(80.0))]
                .into_iter()
                .map(|(radius, bright)| {
                    let pool = state.db_pool.clone();
                    let barrier = barrier.clone();
                    std::thread::spawn(move || {
                        let mut conn = pool.get().expect("conn");
                        barrier.wait();
                        write(&mut conn, light_id, gm, radius, bright).expect("written");
                    })
                })
                .collect();
            for handle in handles {
                handle.join().expect("thread");
            }

            let mut conn = state.db_pool.get().expect("conn");
            assert_eq!(
                reaches(&mut conn, light_id),
                (10.0, 10.0),
                "round {round}: both writes landed, and the bright reach stayed within the dim one"
            );
        }
    }

    /// And the rules themselves, which the transaction must not have changed:
    /// a dim reach pulled in brings the bright one with it, and a bright reach
    /// named alone is stored within the dim reach that is already there.
    #[test]
    fn shrinking_the_dim_reach_pulls_the_bright_one_in() {
        let state = crate::test_support::test_app_state();
        let mut conn = state.db_pool.get().expect("conn");
        let gm = crate::test_support::insert_test_user(&mut conn);
        let light_id = a_light(&mut conn, gm).1;

        write(&mut conn, light_id, gm, Some(20.0), None).expect("dim in");
        assert_eq!(reaches(&mut conn, light_id), (20.0, 20.0));

        write(&mut conn, light_id, gm, None, Some(5.0)).expect("bright in");
        assert_eq!(reaches(&mut conn, light_id), (20.0, 5.0));

        write(&mut conn, light_id, gm, None, Some(500.0)).expect("bright out");
        assert_eq!(
            reaches(&mut conn, light_id),
            (20.0, 20.0),
            "a bright reach is never stored past the dim one"
        );
    }

    /// Establishes a connection to the test database (see
    /// `test_support::test_database_url`). Skips (rather than fails)
    /// when no dev database is reachable, since this is a real-DB
    /// integration test, not a unit test.
    fn try_connect() -> Option<PgConnection> {
        crate::test_support::try_test_connection()
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
                "skipping light_source_authority_follows_the_world_role_not_the_scene_creator: no test database reachable"
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
