//! GraphQL mutations for scene-scoped tokens (native canvas authoring).
//!
//! This is the persistence layer behind the token system used by the
//! Bevy engine's `ExternalCommand::UpsertToken`/`RemoveToken` plumbing once a
//! scene is loaded: tokens live in the `tokens` table, keyed by `scene_id`,
//! ownership-enforced exactly like walls/lights/shapes (see
//! `mutations_walls.rs`, which this module mirrors), and NOTIFY-synced via
//! `EVENT_CODE_TOKEN_CHANGED` so other clients watching the same world pick
//! up token moves in real time.
//!
//! This replaces an earlier `upsert_token`/`delete_token` pair that lived on
//! `SceneMutation` in `graphql.rs` with no scene-ownership check at all.

use async_graphql::{Context, Error, Result as GraphQLResult};
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::graphql::{app_state, authenticated_user, GraphQLCreateTokenInput, GraphQLToken, GraphQLUpdateTokenInput};
use crate::world_events::{record_world_event, world_id_for_scene, EVENT_CODE_TOKEN_CHANGED};

#[derive(Default)]
pub struct TokenMutation;

#[async_graphql::Object]
impl TokenMutation {
    /// Create a new token on a scene (scene owner only)
    async fn create_token(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateTokenInput,
    ) -> GraphQLResult<GraphQLToken> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let token_id = uuid::Uuid::now_v7();
        let scene_id = input.scene_id;
        let actor_id = input.actor_id;
        let x = input.x;
        let y = input.y;
        let rotation = input.rotation.unwrap_or(0.0);
        let scale = input.scale.unwrap_or(1.0);
        let metadata = input.metadata.map(|j| j.0);

        let inserted_token = tokio::task::spawn_blocking(move || {
            use crate::schema::{scenes, tokens};

            // 🔐 Ownership: caller must own the parent scene before a token can be attached to it
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

            let token = diesel::insert_into(tokens::table)
                .values((
                    tokens::token_id.eq(token_id),
                    tokens::scene_id.eq(scene_id),
                    tokens::actor_id.eq(actor_id),
                    tokens::x.eq(x),
                    tokens::y.eq(y),
                    tokens::rotation.eq(rotation),
                    tokens::scale.eq(scale),
                    tokens::metadata.eq(&metadata),
                    tokens::created_at.eq(now),
                    tokens::updated_at.eq(now),
                ))
                .returning(crate::models::Token::as_returning())
                .get_result(&mut conn)?;

            if let Ok(world_id) = world_id_for_scene(&mut conn, scene_id) {
                let _ = record_world_event(
                    &mut conn,
                    world_id,
                    EVENT_CODE_TOKEN_CHANGED,
                    Some(serde_json::json!({
                        "action": "created",
                        "token_id": token_id,
                        "scene_id": scene_id,
                    })),
                    user_id,
                );
            }

            Ok(token)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to create token (scene not found or not owned by you)"))?;

        Ok(GraphQLToken::from(inserted_token))
    }

    /// Update an existing token's position/properties (scene owner only)
    async fn update_token(
        &self,
        ctx: &Context<'_>,
        token_id: uuid::Uuid,
        input: GraphQLUpdateTokenInput,
    ) -> GraphQLResult<GraphQLToken> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let update_data = crate::models::TokenUpdate {
            actor_id: input.actor_id,
            x: input.x,
            y: input.y,
            rotation: input.rotation,
            scale: input.scale,
            metadata: input.metadata.map(|j| j.0),
        };

        let updated_token = tokio::task::spawn_blocking(move || {
            use crate::schema::{scenes, tokens};

            let token = diesel::update(
                tokens::table.filter(tokens::token_id.eq(token_id)).filter(
                    tokens::scene_id.eq_any(
                        scenes::table
                            .filter(scenes::owner_id.eq(user_id))
                            .select(scenes::scene_id),
                    ),
                ),
            )
            .set(update_data)
            .returning(crate::models::Token::as_returning())
            .get_result(&mut conn)?;

            if let Ok(world_id) = world_id_for_scene(&mut conn, token.scene_id) {
                let _ = record_world_event(
                    &mut conn,
                    world_id,
                    EVENT_CODE_TOKEN_CHANGED,
                    Some(serde_json::json!({
                        "action": "updated",
                        "token_id": token_id,
                        "scene_id": token.scene_id,
                    })),
                    user_id,
                );
            }

            Ok::<_, DieselError>(token)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to update token (not found or not owned by you)"))?;

        Ok(GraphQLToken::from(updated_token))
    }

    /// Delete a token (scene owner only)
    async fn delete_token(&self, ctx: &Context<'_>, token_id: uuid::Uuid) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::{scenes, tokens};

            // Look up the scene before deleting so we still have it for the NOTIFY payload.
            let scene_id = tokens::table
                .filter(tokens::token_id.eq(token_id))
                .filter(
                    tokens::scene_id.eq_any(
                        scenes::table
                            .filter(scenes::owner_id.eq(user_id))
                            .select(scenes::scene_id),
                    ),
                )
                .select(tokens::scene_id)
                .first::<uuid::Uuid>(&mut conn)
                .optional()?;

            let deleted_count = diesel::delete(
                tokens::table.filter(tokens::token_id.eq(token_id)).filter(
                    tokens::scene_id.eq_any(
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
                    EVENT_CODE_TOKEN_CHANGED,
                    Some(serde_json::json!({
                        "action": "deleted",
                        "token_id": token_id,
                        "scene_id": scene_id,
                    })),
                    user_id,
                );
            }

            Ok::<_, DieselError>(deleted_count)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to delete token"))?;

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

    /// Verifies the ownership filter `create_token`/`update_token`/
    /// `delete_token` all share: a token attached to scene X can only be
    /// mutated by a caller who owns scene X. Mirrors
    /// `wall_mutations_are_scoped_to_scene_owner` in `mutations_walls.rs`.
    /// Runs entirely inside a `test_transaction`, which Diesel always rolls
    /// back, so it never leaves fixture rows behind.
    #[test]
    fn token_mutations_are_scoped_to_scene_owner() {
        let Some(mut conn) = try_connect() else {
            eprintln!("skipping token_mutations_are_scoped_to_scene_owner: no DATABASE_URL/dev DB reachable");
            return;
        };

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            use crate::schema::{scenes, tokens, users, worlds};

            let owner_id = uuid::Uuid::now_v7();
            let intruder_id = uuid::Uuid::now_v7();
            let world_id = uuid::Uuid::now_v7();
            let scene_id = uuid::Uuid::now_v7();
            let token_id = uuid::Uuid::now_v7();
            let now = chrono::Utc::now().naive_utc();

            for (id, username) in [(owner_id, "token-test-owner"), (intruder_id, "token-test-intruder")] {
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
                    worlds::name.eq("Token Test World"),
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
                    scenes::name.eq("Token Test Scene"),
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

            diesel::insert_into(tokens::table)
                .values((
                    tokens::token_id.eq(token_id),
                    tokens::scene_id.eq(scene_id),
                    tokens::x.eq(0.0),
                    tokens::y.eq(0.0),
                    tokens::rotation.eq(0.0),
                    tokens::scale.eq(1.0),
                    tokens::created_at.eq(now),
                    tokens::updated_at.eq(now),
                ))
                .execute(conn)?;

            // The intruder's ownership-scoped update query must match zero rows.
            let intruder_update_count = diesel::update(
                tokens::table.filter(tokens::token_id.eq(token_id)).filter(
                    tokens::scene_id.eq_any(
                        scenes::table
                            .filter(scenes::owner_id.eq(intruder_id))
                            .select(scenes::scene_id),
                    ),
                ),
            )
            .set(tokens::x.eq(99.0))
            .execute(conn)?;
            assert_eq!(
                intruder_update_count, 0,
                "a non-owner's update filter must not match another owner's token"
            );

            // The owner's identical filter must match exactly one row.
            let owner_update_count = diesel::update(
                tokens::table.filter(tokens::token_id.eq(token_id)).filter(
                    tokens::scene_id.eq_any(
                        scenes::table
                            .filter(scenes::owner_id.eq(owner_id))
                            .select(scenes::scene_id),
                    ),
                ),
            )
            .set(tokens::x.eq(99.0))
            .execute(conn)?;
            assert_eq!(
                owner_update_count, 1,
                "the scene owner's update filter must match their own token"
            );

            // Same shape of check for delete.
            let intruder_delete_count = diesel::delete(
                tokens::table.filter(tokens::token_id.eq(token_id)).filter(
                    tokens::scene_id.eq_any(
                        scenes::table
                            .filter(scenes::owner_id.eq(intruder_id))
                            .select(scenes::scene_id),
                    ),
                ),
            )
            .execute(conn)?;
            assert_eq!(
                intruder_delete_count, 0,
                "a non-owner's delete filter must not match another owner's token"
            );

            Ok(())
        });
    }
}
