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

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::PgConnection;

    /// Every kind the client may send is accepted and stored verbatim.
    #[test]
    fn every_known_token_kind_is_accepted() {
        for kind in TokenKind::ALL {
            let parsed = parse_token_kind(Some(kind.as_stored()))
                .unwrap_or_else(|e| panic!("{kind:?} should parse: {e:?}"));
            assert_eq!(parsed, kind);
        }
    }

    /// Omitting the field is the column default, not an error.
    #[test]
    fn an_absent_kind_is_the_default_rather_than_a_refusal() {
        assert_eq!(parse_token_kind(None).unwrap(), TokenKind::Character);
        assert_eq!(TokenKind::Character.as_stored(), "character");
    }

    /// An unknown kind is refused rather than stored.
    ///
    /// The alternative — falling back to a default — would put a token on the
    /// board wearing the wrong meaning, and the Game Master would have no way
    /// to tell. The error names the valid set so the caller can fix it.
    #[test]
    fn an_unknown_kind_is_refused_and_the_error_says_what_is_valid() {
        let err = parse_token_kind(Some("dragon"))
            .expect_err("an unknown kind must not be silently accepted");
        let message = err.message;
        assert!(
            message.contains("dragon"),
            "should name the bad value: {message}"
        );
        for kind in TokenKind::ALL {
            assert!(
                message.contains(kind.as_stored()),
                "should list {}: {message}",
                kind.as_stored()
            );
        }
    }

    /// Casing is not forgiven, deliberately.
    ///
    /// These are stored values, not user input — the client sends what the
    /// schema says. Accepting "NPC" here would mean two spellings reaching
    /// the column and the renderer having to know about both.
    #[test]
    fn kind_matching_is_exact() {
        for wrong in ["NPC", "Character", "OBJECT", " npc"] {
            assert!(
                parse_token_kind(Some(wrong)).is_err(),
                "{wrong:?} must be refused"
            );
        }
    }

    /// Establishes a connection to the dev database configured via
    /// DATABASE_URL (same source main.rs uses). Skips (rather than fails)
    /// when no dev database is reachable, since this is a real-DB
    /// integration test, not a unit test.
    fn try_connect() -> Option<PgConnection> {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL").ok()?;
        PgConnection::establish(&url).ok()
    }

    /// The rule every token mutation (`create_token`/`update_token`/`delete_token`) now asks, in the one place they all ask
    /// it: authority to author content on a scene is the caller's **world
    /// role** — Owner or GM — not who happened to create the scene.
    ///
    /// This replaces `token_mutations_are_scoped_to_scene_owner`, which asserted the old rule faithfully.
    /// That rule was the bug: two people both holding GM authority in one
    /// world, writing to one scene, had exactly half the writes refused,
    /// because whichever of them had not created the scene was refused every
    /// time. Both directions of that break are asserted below, along with the
    /// two answers that must stay refusals — a GM's new authority must not
    /// leak down to Players or out to non-members.
    ///
    /// `move_own_token` is deliberately untouched by this and keeps its own,
    /// stricter rule — see `move_own_token_filter_rejects_non_owner` below.
    #[test]
    fn token_authority_follows_the_world_role_not_the_scene_creator() {
        let Some(mut conn) = try_connect() else {
            eprintln!(
                "skipping token_authority_follows_the_world_role_not_the_scene_creator: no DATABASE_URL/dev DB reachable"
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
                "a member promoted to GM must be able to edit tokens on a scene the Owner created"
            );
            assert!(
                is_dm_of_scene(conn, owner_id, false, gms_scene)?,
                "the world's Owner must be able to edit tokens on a scene a GM created"
            );
            assert!(
                !is_dm_of_scene(conn, player_id, false, owners_scene)?,
                "a plain Player must not gain content authority from world membership"
            );
            assert!(
                !is_dm_of_scene(conn, stranger_id, false, owners_scene)?,
                "a non-member must not be able to edit tokens in this world at all"
            );

            Ok(())
        });
    }

    /// Spec 004 T026: a non-owning player's `move_own_token`-shaped filter
    /// (owner_user_id = requester) must not match a token owned by someone
    /// else, and the token's position must be unchanged afterward.
    #[test]
    fn move_own_token_filter_rejects_non_owner() {
        let Some(mut conn) = try_connect() else {
            eprintln!(
                "skipping move_own_token_filter_rejects_non_owner: no DATABASE_URL/dev DB reachable"
            );
            return;
        };

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            use crate::schema::{scenes, tokens, users, worlds};

            let scene_owner_id = uuid::Uuid::now_v7();
            let controller_id = uuid::Uuid::now_v7();
            let intruder_id = uuid::Uuid::now_v7();
            let world_id = uuid::Uuid::now_v7();
            let scene_id = uuid::Uuid::now_v7();
            let token_id = uuid::Uuid::now_v7();
            let now = chrono::Utc::now().naive_utc();

            for (id, username) in [
                (scene_owner_id, "move-own-scene-owner"),
                (controller_id, "move-own-controller"),
                (intruder_id, "move-own-intruder"),
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
                    worlds::name.eq("Move Own Token Test World"),
                    worlds::created_by.eq(scene_owner_id),
                    worlds::updated_by.eq(scene_owner_id),
                    worlds::created_at.eq(now),
                    worlds::updated_at.eq(now),
                ))
                .execute(conn)?;

            diesel::insert_into(scenes::table)
                .values((
                    scenes::scene_id.eq(scene_id),
                    scenes::world_id.eq(world_id),
                    scenes::name.eq("Move Own Token Test Scene"),
                    scenes::type_.eq("battlemap"),
                    scenes::grid_size.eq(32),
                    scenes::grid_type.eq("square"),
                    scenes::width.eq(1000),
                    scenes::height.eq(1000),
                    scenes::owner_id.eq(scene_owner_id),
                    scenes::created_at.eq(now),
                    scenes::updated_at.eq(now),
                ))
                .execute(conn)?;

            diesel::insert_into(tokens::table)
                .values((
                    tokens::token_id.eq(token_id),
                    tokens::scene_id.eq(scene_id),
                    tokens::x.eq(5.0),
                    tokens::y.eq(5.0),
                    tokens::rotation.eq(0.0),
                    tokens::scale.eq(1.0),
                    tokens::owner_user_id.eq(controller_id),
                    tokens::created_at.eq(now),
                    tokens::updated_at.eq(now),
                ))
                .execute(conn)?;

            // The intruder's move_own_token-shaped filter must match zero rows.
            let intruder_move_count = diesel::update(
                tokens::table
                    .filter(tokens::token_id.eq(token_id))
                    .filter(tokens::owner_user_id.eq(intruder_id)),
            )
            .set((tokens::x.eq(99.0), tokens::y.eq(99.0)))
            .execute(conn)?;
            assert_eq!(
                intruder_move_count, 0,
                "a non-controller's move filter must not match another player's token"
            );

            let (x, y): (f64, f64) = tokens::table
                .filter(tokens::token_id.eq(token_id))
                .select((tokens::x, tokens::y))
                .first(conn)?;
            assert_eq!(
                (x, y),
                (5.0, 5.0),
                "position must be unchanged after a rejected move"
            );

            // The real controller's filter must match exactly one row.
            let controller_move_count = diesel::update(
                tokens::table
                    .filter(tokens::token_id.eq(token_id))
                    .filter(tokens::owner_user_id.eq(controller_id)),
            )
            .set((tokens::x.eq(10.0), tokens::y.eq(10.0)))
            .execute(conn)?;
            assert_eq!(
                controller_move_count, 1,
                "the token's controller must be able to move it"
            );

            Ok(())
        });
    }

    /// Spec 004 T027: setting `is_primary = true` for a second token under
    /// the same (scene_id, owner_user_id) must leave exactly one primary,
    /// respecting the partial unique index `tokens_one_primary_per_owner_per_scene`.
    #[test]
    fn setting_second_primary_replaces_the_first() {
        let Some(mut conn) = try_connect() else {
            eprintln!(
                "skipping setting_second_primary_replaces_the_first: no DATABASE_URL/dev DB reachable"
            );
            return;
        };

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            use crate::schema::{scenes, tokens, users, worlds};

            let scene_owner_id = uuid::Uuid::now_v7();
            let player_id = uuid::Uuid::now_v7();
            let world_id = uuid::Uuid::now_v7();
            let scene_id = uuid::Uuid::now_v7();
            let token_a_id = uuid::Uuid::now_v7();
            let token_b_id = uuid::Uuid::now_v7();
            let now = chrono::Utc::now().naive_utc();

            for (id, username) in [
                (scene_owner_id, "primary-test-scene-owner"),
                (player_id, "primary-test-player"),
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
                    worlds::name.eq("Primary Token Test World"),
                    worlds::created_by.eq(scene_owner_id),
                    worlds::updated_by.eq(scene_owner_id),
                    worlds::created_at.eq(now),
                    worlds::updated_at.eq(now),
                ))
                .execute(conn)?;

            diesel::insert_into(scenes::table)
                .values((
                    scenes::scene_id.eq(scene_id),
                    scenes::world_id.eq(world_id),
                    scenes::name.eq("Primary Token Test Scene"),
                    scenes::type_.eq("battlemap"),
                    scenes::grid_size.eq(32),
                    scenes::grid_type.eq("square"),
                    scenes::width.eq(1000),
                    scenes::height.eq(1000),
                    scenes::owner_id.eq(scene_owner_id),
                    scenes::created_at.eq(now),
                    scenes::updated_at.eq(now),
                ))
                .execute(conn)?;

            for token_id in [token_a_id, token_b_id] {
                diesel::insert_into(tokens::table)
                    .values((
                        tokens::token_id.eq(token_id),
                        tokens::scene_id.eq(scene_id),
                        tokens::x.eq(0.0),
                        tokens::y.eq(0.0),
                        tokens::rotation.eq(0.0),
                        tokens::scale.eq(1.0),
                        tokens::owner_user_id.eq(player_id),
                        tokens::created_at.eq(now),
                        tokens::updated_at.eq(now),
                    ))
                    .execute(conn)?;
            }

            // Mark token A primary first.
            diesel::update(tokens::table.filter(tokens::token_id.eq(token_a_id)))
                .set(tokens::is_primary.eq(true))
                .execute(conn)?;

            // Now replicate update_token's "clear prior primary" step before
            // marking token B primary (the actual mutation does this inside
            // one DB transaction; here we exercise the same two statements).
            diesel::update(
                tokens::table
                    .filter(tokens::scene_id.eq(scene_id))
                    .filter(tokens::owner_user_id.eq(player_id))
                    .filter(tokens::is_primary.eq(true))
                    .filter(tokens::token_id.ne(token_b_id)),
            )
            .set(tokens::is_primary.eq(false))
            .execute(conn)?;

            diesel::update(tokens::table.filter(tokens::token_id.eq(token_b_id)))
                .set(tokens::is_primary.eq(true))
                .execute(conn)?;

            let primary_count: i64 = tokens::table
                .filter(tokens::scene_id.eq(scene_id))
                .filter(tokens::owner_user_id.eq(player_id))
                .filter(tokens::is_primary.eq(true))
                .count()
                .get_result(conn)?;
            assert_eq!(
                primary_count, 1,
                "exactly one primary token must remain for this owner"
            );

            let token_b_is_primary: bool = tokens::table
                .filter(tokens::token_id.eq(token_b_id))
                .select(tokens::is_primary)
                .first(conn)?;
            assert!(token_b_is_primary, "token B must be the surviving primary");

            Ok(())
        });
    }

    /// The three states of `TokenUpdate::photo_url`, against a real
    /// database, because they are a property of Diesel's `AsChangeset`
    /// rather than of any code here: skip, write, and write NULL.
    ///
    /// The clearing case is the one that did not exist before — a plain
    /// `Option<String>` cannot express it, so a GM could replace token art
    /// but never remove it.
    #[test]
    fn token_photo_url_can_be_set_skipped_and_cleared() {
        let Some(mut conn) = try_connect() else {
            eprintln!(
                "skipping token_photo_url_can_be_set_skipped_and_cleared: no DATABASE_URL/dev DB reachable"
            );
            return;
        };

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            use crate::schema::{scenes, tokens, users, worlds};

            let owner_id = uuid::Uuid::now_v7();
            let world_id = uuid::Uuid::now_v7();
            let scene_id = uuid::Uuid::now_v7();
            let token_id = uuid::Uuid::now_v7();
            let now = chrono::Utc::now().naive_utc();

            diesel::insert_into(users::table)
                .values((
                    users::id.eq(owner_id),
                    users::username.eq(format!("token-photo-owner-{owner_id}")),
                    users::password_hash.eq("test-hash"),
                    users::email.eq(format!("token-photo-{owner_id}@example.test")),
                    users::created_at.eq(now),
                    users::updated_at.eq(now),
                ))
                .execute(conn)?;

            diesel::insert_into(worlds::table)
                .values((
                    worlds::id.eq(world_id),
                    worlds::name.eq("Token Photo World"),
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
                    scenes::name.eq("Token Photo Scene"),
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

            let photo_of =
                |conn: &mut PgConnection| -> Result<Option<String>, diesel::result::Error> {
                    tokens::table
                        .filter(tokens::token_id.eq(token_id))
                        .select(tokens::photo_url)
                        .first(conn)
                };

            // Always carries an `x`, both because that is the shape of a
            // real update (the client sends position with every change)
            // and because Diesel rejects a wholly empty changeset at
            // runtime with `EmptyChangeset`.
            let update = |conn: &mut PgConnection, x: f64, photo_url| {
                diesel::update(tokens::table.filter(tokens::token_id.eq(token_id)))
                    .set(crate::models::TokenUpdate {
                        actor_id: None,
                        x: Some(x),
                        y: None,
                        rotation: None,
                        scale: None,
                        metadata: None,
                        owner_user_id: None,
                        is_primary: None,
                        photo_url,
                        health: None,
                        max_health: None,
                        token_type: None,
                    })
                    .execute(conn)
            };

            // Write.
            update(
                conn,
                1.0,
                Some(Some("/api/canvas-assets/abc.webp".to_string())),
            )?;
            assert_eq!(
                photo_of(conn)?,
                Some("/api/canvas-assets/abc.webp".to_string())
            );

            // Skip: a plain move must not disturb the art.
            update(conn, 2.0, None)?;
            assert_eq!(
                photo_of(conn)?,
                Some("/api/canvas-assets/abc.webp".to_string()),
                "an omitted photo_url must leave the column untouched"
            );

            // Clear.
            update(conn, 3.0, Some(None))?;
            assert_eq!(
                photo_of(conn)?,
                None,
                "an explicit null photo_url must clear the column"
            );

            Ok(())
        });
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
