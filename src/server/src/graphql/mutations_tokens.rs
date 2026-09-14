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

use crate::graphql::mutations_token_links::{
    actor_resource_data, load_placement_actor, placement_for, token_kind_for_actor,
};
use crate::graphql::{
    GraphQLCreateTokenInput, GraphQLToken, GraphQLUpdateTokenInput, app_state, authenticated_user,
};
use crate::play_pause::gate::{refusal_or, refuse_scene_if_paused};
use crate::scene_fingerprint::refresh_scene_fingerprint;
use crate::world_events::{EVENT_CODE_TOKEN_CHANGED, record_world_event, world_id_for_scene};
use async_graphql::MaybeUndefined;
use thunderforge_canvas_core::token_kind::TokenKind;

/// One point of a route a client claims to have walked (spec 045 FR-016).
///
/// World coordinates, in the order they were passed through. Deliberately a
/// plain pair rather than a grid cell: a scene may have no grid, and the
/// geometry that judges the route works in world space either way.
#[derive(async_graphql::InputObject, Debug, Clone, Copy)]
pub struct GraphQLPathPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Default)]

pub struct TokenMutation;

#[async_graphql::Object]
impl TokenMutation {
    /// Create a new token on a scene (scene owner only)
    ///
    /// Spec 046 FR-016: the token is linked to its actor or an unlinked copy
    /// of it, as `input.linked` says or, when it says nothing, as the actor
    /// decides — see `mutations_token_links::placement_for`.
    async fn create_token(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateTokenInput,
    ) -> GraphQLResult<GraphQLToken> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        create_token_impl(state, auth_user.user_id, auth_user.is_admin, input).await
    }

    /// Update an existing token's position/properties (scene owner only)
    ///
    /// **Deliberately not judged against walls** (spec 045 decision 1,
    /// FR-017). This is the Game Master's path — scene owner only — and a
    /// Game Master moves any token anywhere: picking a piece up and setting it
    /// down on the far side of a wall is a normal thing to do at a table.
    /// Refusing it would be the product being wrong about who is in charge.
    ///
    /// This is the rule, not a gap left to tighten later. The judged path is
    /// `move_own_token`, which is where a *player's* move goes.
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
                refuse_scene_if_paused(conn, existing_scene)?;

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
        .map_err(|e| refusal_or(e, "Failed to update token (not found or not owned by you)"))?;

        crate::graphql::token_art::token_with_art(state, updated_token, user_id, is_admin).await
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
            if let Some(scene_id) = scene_id {
                refuse_scene_if_paused(&mut conn, scene_id)?;
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
        .map_err(|e| refusal_or(e, "Failed to delete token"))?;

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
    ///
    /// Spec 045 US2 (ADR-095): this is where a player's move is judged against
    /// the scene's walls. `path` is the route the client claims to have taken
    /// — the cells of a committed route, in order. It is optional because a
    /// drag has no route to send, and when it is absent the straight line from
    /// the token's stored position is judged instead, which is what a drag
    /// actually is.
    ///
    /// The path is a *claim*, never a substitute for judging: it is anchored
    /// to the token's real position and to the requested destination, so a
    /// short innocent route cannot be sent as cover for a move that crossed a
    /// wall. See `crate::movement`.
    async fn move_own_token(
        &self,
        ctx: &Context<'_>,
        token_id: uuid::Uuid,
        x: f64,
        y: f64,
        path: Option<Vec<GraphQLPathPoint>>,
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
            let token = tokens::table
                .filter(tokens::token_id.eq(token_id))
                .select(crate::models::Token::as_select())
                .first::<crate::models::Token>(&mut conn)
                .optional()?;
            // Before the control check, so a Game Master or admin moving a
            // token they do not control still meets the pause.
            if let Some(token) = &token {
                refuse_scene_if_paused(&mut conn, token.scene_id)?;
            }
            Ok::<_, DieselError>(token)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|e| refusal_or(e, "Failed to load token"))?
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

        // Spec 046 C1: the turn gets a say before the walls do. A move made on
        // somebody else's turn is refused whatever route it took, so there is
        // no point judging the route.
        {
            let mut conn = state
                .db_pool
                .get()
                .map_err(|_| Error::new("Failed to get DB connection"))?;
            let scene_id = existing.scene_id;
            let check = tokio::task::spawn_blocking(move || {
                crate::combat::turn::turn_check(&mut conn, scene_id, token_id, user_id, is_admin)
            })
            .await
            .map_err(|_| Error::new("Failed to spawn blocking task"))?
            .map_err(|_| Error::new("Failed to check whose turn it is"))?;
            if let Some(refusal) = check.refusal() {
                return Err(Error::new(refusal));
            }
        }

        // Spec 045 US2: the walls get a say, and this is the only place they
        // get one for a player. The engine refuses the move first so the stop
        // looks like a wall; this is the refusal that counts, because the
        // engine is a program on somebody else's computer (ADR-095).
        judge_against_walls(state, &existing, x, y, path.as_deref()).await?;

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

        crate::graphql::token_art::token_with_art(state, updated_token, user_id, is_admin).await
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

            // Gated before the ownership filter below, so every caller meets
            // the pause, not only the token's owner.
            if let Some(scene_id) = tokens::table
                .filter(tokens::token_id.eq(token_id))
                .select(tokens::scene_id)
                .first::<uuid::Uuid>(&mut conn)
                .optional()?
            {
                refuse_scene_if_paused(&mut conn, scene_id)?;
            }

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
        .map_err(|e| refusal_or(e, "Failed to set photo (not your primary token)"))?;

        crate::graphql::token_art::token_with_art(state, updated_token, user_id, auth_user.is_admin)
            .await
    }

    /// Playtest 2026-09-10 P7: show or hide a token's name from players. A
    /// Game Master always sees it; only a Game Master may change it.
    async fn set_token_name_visibility(
        &self,
        ctx: &Context<'_>,
        token_id: uuid::Uuid,
        visible: bool,
    ) -> GraphQLResult<GraphQLToken> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let token = set_token_name_visibility_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            token_id,
            visible,
        )
        .await?;
        crate::graphql::token_art::token_with_art(
            state,
            token,
            auth_user.user_id,
            auth_user.is_admin,
        )
        .await
    }

    /// Make a token its actor (`linked: true`) or an unlinked copy of it.
    /// Game Master only (spec 046 FR-016, ADR-102).
    ///
    /// Linking a copy discards the copy's own hit points: it takes its
    /// actor's. Unlinking starts the copy from the actor's current values.
    async fn set_token_link(
        &self,
        ctx: &Context<'_>,
        token_id: uuid::Uuid,
        linked: bool,
    ) -> GraphQLResult<GraphQLToken> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        crate::graphql::mutations_token_links::set_token_link_impl(
            state,
            user.user_id,
            user.is_admin,
            token_id,
            linked,
        )
        .await
    }

    /// Mark an NPC as a named individual, whose tokens are placed linked, or
    /// not. Game Master only. Tokens already placed are unchanged.
    async fn set_actor_unique(
        &self,
        ctx: &Context<'_>,
        actor_id: uuid::Uuid,
        unique: bool,
    ) -> GraphQLResult<crate::graphql::types_scene::GraphQLWorldActor> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        crate::graphql::mutations_token_links::set_actor_unique_impl(
            state,
            user.user_id,
            user.is_admin,
            actor_id,
            unique,
        )
        .await
    }
}

/// Testable core of `TokenMutation::set_token_name_visibility`.
///
/// The authority is the world role, as for every other token change a player
/// cannot make. The event is the usual id-only nudge: every client re-reads
/// its tokens through `token_art`, which decides what each may see — the
/// broadcast itself goes to everyone alike, so it must never carry the name.
pub(crate) async fn set_token_name_visibility_impl(
    state: &crate::state::AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    token_id: uuid::Uuid,
    visible: bool,
) -> GraphQLResult<crate::models::Token> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        use crate::schema::tokens;

        conn.transaction(|conn| {
            let scene_id: uuid::Uuid = tokens::table
                .filter(tokens::token_id.eq(token_id))
                .select(tokens::scene_id)
                .first(conn)?;
            if !crate::auth::world_membership::is_dm_of_scene(conn, user_id, is_admin, scene_id)? {
                return Err(DieselError::NotFound);
            }
            refuse_scene_if_paused(conn, scene_id)?;

            let token = diesel::update(tokens::table.filter(tokens::token_id.eq(token_id)))
                .set(tokens::name_visible_to_players.eq(visible))
                .returning(crate::models::Token::as_returning())
                .get_result(conn)?;

            refresh_scene_fingerprint(conn, scene_id, user_id);
            if let Ok(world_id) = world_id_for_scene(conn, scene_id) {
                let _ = record_world_event(
                    conn,
                    world_id,
                    EVENT_CODE_TOKEN_CHANGED,
                    Some(serde_json::json!({
                        "action": "updated",
                        "token_id": token_id,
                        "scene_id": scene_id,
                    })),
                    user_id,
                );
            }
            Ok::<_, DieselError>(token)
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| {
        refusal_or(
            e,
            "Failed to change the token's name visibility (not found or not yours to change)",
        )
    })
}

/// Testable core of `TokenMutation::create_token`.
pub(crate) async fn create_token_impl(
    state: &crate::AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    input: GraphQLCreateTokenInput,
) -> GraphQLResult<GraphQLToken> {
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
    // that is cheap to say. `None` here means the caller named no kind,
    // and the actor's kind is used below (spec 046 research R4).
    let requested_type = match input.token_type.as_deref() {
        None => None,
        Some(raw) => Some(parse_token_kind(Some(raw))?),
    };
    let requested_link = input.linked;
    if requested_link == Some(true) && actor_id.is_none() {
        return Err(Error::new("A token with no actor cannot be linked"));
    }

    let inserted_token = tokio::task::spawn_blocking(move || {
        use crate::schema::tokens;

        // 🔐 Authority to author content on a scene follows the world
        // role — the Owner and any GM, never a Player — not who happened
        // to create the scene. See `world_membership::is_dm_of_scene`.
        if !crate::auth::world_membership::is_dm_of_scene(&mut conn, user_id, is_admin, scene_id)? {
            return Err(DieselError::NotFound);
        }
        refuse_scene_if_paused(&mut conn, scene_id)?;

        // Spec 046 FR-016 (ADR-102): linked or a copy, decided here from
        // the actor so every client that places a token gets the same
        // answer. The actor must be of this scene's world: a copy is
        // seeded from its sheet, and an actor from elsewhere would carry
        // another world's data onto this board.
        let placement_actor = match actor_id {
            None => None,
            Some(id) => {
                let actor = load_placement_actor(&mut conn, id)?.ok_or(DieselError::NotFound)?;
                let scene_world: uuid::Uuid = crate::schema::scenes::table
                    .filter(crate::schema::scenes::scene_id.eq(scene_id))
                    .select(crate::schema::scenes::world_id)
                    .first(&mut conn)?;
                if scene_world != actor.world_id {
                    return Err(DieselError::NotFound);
                }
                Some(actor)
            }
        };
        let placement = placement_for(placement_actor.as_ref(), requested_link)
            .map_err(|_| DieselError::NotFound)?;
        let system_data = match (placement.seeds_from_actor, actor_id) {
            (true, Some(id)) => actor_resource_data(&mut conn, id)?,
            _ => None,
        };
        let token_type = requested_type
            .or_else(|| placement_actor.as_ref().map(token_kind_for_actor))
            .unwrap_or_default();

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
                tokens::linked.eq(placement.linked),
                tokens::system_data.eq(&system_data),
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

        // With its character's art, as every token read now carries it —
        // and its name in full: only a Game Master may create a token.
        let mut with_art =
            crate::graphql::token_art::tokens_with_art(&mut conn, vec![token], true)?;
        Ok(with_art.remove(0))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| {
        refusal_or(
            e,
            "Failed to create token (scene not found or not owned by you)",
        )
    })?;

    Ok(inserted_token)
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

/// Refuse a player's move if a wall is in the way (spec 045 FR-012, ADR-095).
///
/// Loads the scene's walls and asks `crate::movement` — the same geometry the
/// engine used to stop the move before it was sent. Only reached from
/// `move_own_token`: a Game Master's `update_token` is deliberately unjudged
/// (FR-017, spec decision 1).
///
/// A scene with no walls is the common case and costs one empty query. It is
/// not skipped on a hunch — "this scene probably has no walls" is exactly the
/// assumption a modified client would like the server to make.
async fn judge_against_walls(
    state: &crate::AppState,
    existing: &crate::models::Token,
    x: f64,
    y: f64,
    path: Option<&[GraphQLPathPoint]>,
) -> GraphQLResult<()> {
    use crate::movement::{BLOCKED_MESSAGE, Refusal, Verdict, judge, wall_set_from_rows};
    use thunderforge_canvas_core::Vec2;

    let scene_id = existing.scene_id;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let walls = tokio::task::spawn_blocking(move || {
        use crate::schema::walls;
        walls::table
            .filter(walls::scene_id.eq(scene_id))
            .select(crate::models::Wall::as_select())
            .load::<crate::models::Wall>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load walls"))?;

    if walls.is_empty() {
        return Ok(());
    }

    let claimed: Option<Vec<Vec2>> = path.map(|points| {
        points
            .iter()
            .map(|p| Vec2::new(p.x as f32, p.y as f32))
            .collect()
    });

    let verdict = judge(
        Vec2::new(existing.x as f32, existing.y as f32),
        Vec2::new(x as f32, y as f32),
        claimed.as_deref(),
        &wall_set_from_rows(&walls),
    );

    match verdict {
        Verdict::Allowed => Ok(()),
        // The player is told one thing, whatever kind of wall it was: a closed
        // secret door stops them like any other, and this refusal must not be
        // how they learn there is a door there (FR-019).
        Verdict::Refused(Refusal::Wall { .. }) => Err(Error::new(BLOCKED_MESSAGE)),
        Verdict::Refused(Refusal::MalformedPath) => Err(Error::new("That route could not be read")),
    }
}

#[cfg(test)]
#[path = "mutations_tokens_tests.rs"]
mod tests;
