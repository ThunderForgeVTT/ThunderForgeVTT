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

use crate::graphql::{
    GraphQLCreateTokenInput, GraphQLToken, GraphQLUpdateTokenInput, app_state, authenticated_user,
};
use crate::scene_fingerprint::refresh_scene_fingerprint;
use crate::world_events::{EVENT_CODE_TOKEN_CHANGED, record_world_event, world_id_for_scene};
use async_graphql::MaybeUndefined;
use thunderforge_canvas_core::token_kind::TokenKind;

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
        let is_admin = auth_user.is_admin;
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

        // Validated here rather than stored as given. This column feeds the
        // renderer, so a kind nothing can draw is a token that appears
        // mislabelled — or, in the fallback, silently identical to a player
        // character. Rejecting an unknown value is the only point at which
        // that is cheap to say.
        let token_type = parse_token_kind(input.token_type.as_deref())?;

        let inserted_token = tokio::task::spawn_blocking(move || {
            use crate::schema::tokens;

            // 🔐 Authority to author content on a scene follows the world
            // role — the Owner and any GM, never a Player — not who happened
            // to create the scene. See `world_membership::is_dm_of_scene`.
            if !crate::auth::world_membership::is_dm_of_scene(
                &mut conn, user_id, is_admin, scene_id,
            )? {
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
                    tokens::token_type.eq(token_type.as_stored()),
                    tokens::created_at.eq(now),
                    tokens::updated_at.eq(now),
                ))
                .returning(crate::models::Token::as_returning())
                .get_result(&mut conn)?;

            // Spec 028 FR-006: the scene's fingerprint must move with the
            // change that caused it. A stale one would tell a client its copy
            // is current when it is not — the one failure this feature must
            // never produce.
            refresh_scene_fingerprint(&mut conn, scene_id, user_id);

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
        let is_admin = auth_user.is_admin;
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
            owner_user_id: input.owner_user_id,
            is_primary: input.is_primary,
            // Undefined leaves the column alone; an explicit null clears
            // it back to the flat colour swatch the engine draws for a
            // token with no art.
            photo_url: match input.photo_url {
                MaybeUndefined::Undefined => None,
                MaybeUndefined::Null => Some(None),
                MaybeUndefined::Value(url) => Some(Some(url)),
            },
            health: input.health,
            max_health: input.max_health,
            // Validated on the way in, exactly as on create: an unknown kind
            // is refused rather than written, because the column decides how
            // the token is drawn.
            token_type: match input.token_type.as_deref() {
                None => None,
                Some(raw) => Some(parse_token_kind(Some(raw))?.as_stored().to_string()),
            },
        };
        let setting_primary = input.is_primary == Some(true);
        let input_owner_user_id = input.owner_user_id;

        let updated_token = tokio::task::spawn_blocking(move || {
            use crate::schema::tokens;

            conn.transaction(|conn| {
                // 🔐 Authority to author content on a scene follows the
                // world role — the Owner and any GM, never a Player — not
                // who happened to create the scene. See
                // `world_membership::is_dm_of_scene`.
                //
                // It is checked once, up front, and the writes below key on
                // `token_id` alone. Each write used to carry its own "scenes
                // I own" subquery, which is how one rule came to be enforced
                // in three places and be wrong in all of them.
                let (existing_scene, existing_owner): (uuid::Uuid, Option<uuid::Uuid>) =
                    tokens::table
                        .filter(tokens::token_id.eq(token_id))
                        .select((tokens::scene_id, tokens::owner_user_id))
                        .first(conn)?;
                if !crate::auth::world_membership::is_dm_of_scene(
                    conn,
                    user_id,
                    is_admin,
                    existing_scene,
                )? {
                    return Err(DieselError::NotFound);
                }

                if setting_primary {
                    // Determine the owner this update will apply to: the
                    // input's owner_user_id if provided, else the token's
                    // current one.
                    let target_owner = input_owner_user_id.or(existing_owner);

                    if let Some(target_owner) = target_owner {
                        // Clear any other primary token for this
                        // (scene_id, owner_user_id) before setting this
                        // one, so the partial unique index never sees two
                        // primaries at once.
                        diesel::update(
                            tokens::table
                                .filter(tokens::scene_id.eq(existing_scene))
                                .filter(tokens::owner_user_id.eq(target_owner))
                                .filter(tokens::is_primary.eq(true))
                                .filter(tokens::token_id.ne(token_id)),
                        )
                        .set(tokens::is_primary.eq(false))
                        .execute(conn)?;
                    }
                }

                let token = diesel::update(tokens::table.filter(tokens::token_id.eq(token_id)))
                    .set(update_data)
                    .returning(crate::models::Token::as_returning())
                    .get_result(conn)?;

                refresh_scene_fingerprint(conn, token.scene_id, user_id);

                if let Ok(world_id) = world_id_for_scene(conn, token.scene_id) {
                    let _ = record_world_event(
                        conn,
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
        let is_admin = auth_user.is_admin;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::tokens;

            // Look up the scene before deleting so we still have it for the NOTIFY payload.
            let scene_id = tokens::table
                .filter(tokens::token_id.eq(token_id))
                .select(tokens::scene_id)
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
                // Nothing was deleted, which is exactly what an unauthorized
                // caller was told before — the refusal reads the same as
                // "no such token" and leaks nothing either way.
                return Ok(0);
            }

            let deleted_count = diesel::delete(tokens::table.filter(tokens::token_id.eq(token_id)))
                .execute(&mut conn)?;

            if deleted_count > 0
                && let Some(scene_id) = scene_id
            {
                refresh_scene_fingerprint(&mut conn, scene_id, user_id);
            }

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

    /// Move a token the caller controls (their primary token, or one the GM
    /// granted them) — position only, no scene-ownership required. Spec 004
    /// FR-009: a player may drag any token whose `owner_user_id` is them.
    /// Spec 010 (research.md §5, FR-018): additionally, if the token is
    /// linked to an actor (`tokens.actor_id`), a caller holding effective
    /// `Owner` permission on that actor (or the DM, always) may also move
    /// it — this is the live-play enforcement point for the actor
    /// ownership block, extending rather than replacing the existing
    /// `owner_user_id` check. Multiple simultaneous Owner-level members
    /// are all independently authorized here (no locking) — whichever one
    /// most recently moves the token "wins," matching the spec's stated
    /// conflict resolution.
    async fn move_own_token(
        &self,
        ctx: &Context<'_>,
        token_id: uuid::Uuid,
        x: f64,
        y: f64,
    ) -> GraphQLResult<GraphQLToken> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let is_admin = auth_user.is_admin;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let existing = tokio::task::spawn_blocking(move || {
            use crate::schema::tokens;
            tokens::table
                .filter(tokens::token_id.eq(token_id))
                .select(crate::models::Token::as_select())
                .first::<crate::models::Token>(&mut conn)
                .optional()
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load token"))?
        .ok_or_else(|| Error::new("Move token failed (not found or not controlled by you)"))?;

        let is_direct_owner = existing.owner_user_id == Some(user_id);
        let is_actor_owner = match existing.actor_id {
            Some(actor_id) => crate::auth::actor_permissions::effective_actor_permission(
                state, user_id, is_admin, actor_id,
            )
            .await
            .map(|level| level.rank() >= crate::graphql::types::ActorPermissionLevel::Owner.rank())
            .unwrap_or(false),
            None => false,
        };

        if !is_direct_owner && !is_actor_owner {
            return Err(Error::new(
                "Move token failed (not found or not controlled by you)",
            ));
        }

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let updated_token = tokio::task::spawn_blocking(move || {
            use crate::schema::tokens;

            let token = diesel::update(tokens::table.filter(tokens::token_id.eq(token_id)))
                .set((tokens::x.eq(x), tokens::y.eq(y)))
                .returning(crate::models::Token::as_returning())
                .get_result(&mut conn)?;

            refresh_scene_fingerprint(&mut conn, token.scene_id, user_id);

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
        .map_err(|_| Error::new("Failed to move token (not found or not controlled by you)"))?;

        Ok(GraphQLToken::from(updated_token))
    }

    /// Change the photo/avatar of the caller's own primary token. Spec 004
    /// FR-009a: only the token marked `is_primary` for this caller may have
    /// its photo set this way — not any other token they control.
    async fn set_own_primary_token_photo(
        &self,
        ctx: &Context<'_>,
        token_id: uuid::Uuid,
        photo_url: String,
    ) -> GraphQLResult<GraphQLToken> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let updated_token = tokio::task::spawn_blocking(move || {
            use crate::schema::tokens;

            let token = diesel::update(
                tokens::table
                    .filter(tokens::token_id.eq(token_id))
                    .filter(tokens::owner_user_id.eq(user_id))
                    .filter(tokens::is_primary.eq(true)),
            )
            .set(tokens::photo_url.eq(photo_url))
            .returning(crate::models::Token::as_returning())
            .get_result(&mut conn)?;

            refresh_scene_fingerprint(&mut conn, token.scene_id, user_id);

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
        .map_err(|_| Error::new("Failed to set photo (not your primary token)"))?;

        Ok(GraphQLToken::from(updated_token))
    }
}

/// Turn a client-supplied kind into a [`TokenKind`], or refuse.
///
/// `None` means the caller did not ask, which is the column default. An
/// unrecognised string is an error rather than a silent fallback: falling back
/// would put a token on the board wearing the wrong meaning, and the Game
/// Master would have no way to tell it had happened.
fn parse_token_kind(raw: Option<&str>) -> GraphQLResult<TokenKind> {
    match raw {
        None => Ok(TokenKind::default()),
        Some(value) => TokenKind::from_stored(value).ok_or_else(|| {
            let known: Vec<&str> = TokenKind::ALL.iter().map(|k| k.as_stored()).collect();
            Error::new(format!(
                "Unknown token type {value:?}. Expected one of: {}",
                known.join(", ")
            ))
        }),
    }
}

#[cfg(test)]
#[path = "mutations_tokens_tests.rs"]
mod tests;
