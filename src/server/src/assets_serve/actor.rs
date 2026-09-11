//! Spec 031 (T069, FR-036): reading an actor's portrait and token image back.
//!
//! `mutations_actor_images.rs` writes the bytes into RustFS, which is private,
//! server-credentialled storage (ADR-039) — a client is never handed a RustFS
//! URL. `GET /actor-assets/{asset_id}` and `/thumb` mirror
//! `assets_serve/lore.rs` exactly: authenticated by the same
//! `auth_middleware::require_authenticated_user` layer in `main.rs`, then
//! authorized by the *actor's* own effective permission rather than by world
//! membership, so a picture is readable by precisely the people who may read
//! the actor it belongs to (Constitution Principle III).

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Router};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::actor_permissions::require_actor_permission;
use crate::auth::scene_visibility::asset_scene_visible;
use crate::auth::world_membership::require_world_member;
use crate::auth_middleware::AuthenticatedUser;
use crate::graphql::mutations_actor_images::ROLE_TOKEN;
use crate::graphql::types::ActorPermissionLevel;
use crate::state::AppState;
use crate::storage::rustfs::{RustFsConfig, read_object};

pub fn router() -> Router<AppState> {
    Router::new()
        // `{asset_id}` may end in `.webp` or `.png`, as the canvas route's
        // may: the engine picks an image loader by extension, and a token's
        // art is loaded by the engine (playtest 2026-09-10 P1). Parsed by
        // `canvas::parse_asset_id`, so Bevy's `.meta` probe is a 404 here too.
        // The parameter keeps its name because the `/thumb` route shares the
        // segment, and the router refuses two names for one position.
        .route("/actor-assets/{asset_id}", get(serve_actor_asset))
        .route(
            "/actor-assets/{asset_id}/thumb",
            get(serve_actor_asset_thumbnail),
        )
}

/// RustFS object key for an actor image's full-size rendition — shared with
/// `mutations_actor_images.rs`, which writes this same key on upload.
pub fn actor_image_full_key(asset_id: Uuid) -> String {
    format!("actors/{asset_id}.webp")
}

/// RustFS object key for an actor image's thumbnail rendition.
pub fn actor_image_thumb_key(asset_id: Uuid) -> String {
    format!("actors/{asset_id}-thumb.webp")
}

/// Which actor an asset belongs to, and in which role.
///
/// The asset id alone carries no authority — the row is what says whose
/// picture this is, and therefore who may look at it.
async fn load_asset_owner(state: &AppState, asset_id: Uuid) -> Option<(Uuid, String)> {
    let mut conn = state.db_pool.get().ok()?;
    tokio::task::spawn_blocking(move || {
        use crate::schema::world_actor_images;
        world_actor_images::table
            .filter(world_actor_images::asset_id.eq(asset_id))
            .select((world_actor_images::actor_id, world_actor_images::role))
            .first::<(Uuid, String)>(&mut conn)
            .optional()
    })
    .await
    .ok()?
    .ok()?
}

/// Whether this caller can see a scene on which `actor_id` has a token.
///
/// Playtest 2026-09-10 P1. A token's art is on the map for everybody who can
/// see the token, and who can see the map is exactly what `scene_visibility`
/// decides — so a character's **token** image (never its portrait, never
/// anything else) is readable by anyone to whom a scene carrying that token is
/// visible, by the same rule the scene's own art follows. Without it, a player
/// looking at an NPC's token was refused the image and saw nothing at all where
/// the token stood.
///
/// A membership refusal or a database error on one world is a `false` for
/// that world, not an error: the safe way to be wrong is to show a swatch.
pub(crate) fn token_art_visible_sync(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> Result<bool, diesel::result::Error> {
    use crate::schema::{scenes, tokens};

    let placed: Vec<(Uuid, Uuid)> = tokens::table
        .inner_join(scenes::table.on(scenes::scene_id.eq(tokens::scene_id)))
        .filter(tokens::actor_id.eq(actor_id))
        .select((tokens::scene_id, scenes::world_id))
        .distinct()
        .load(conn)?;

    for (scene_id, world_id) in placed {
        let Ok(role) = require_world_member(conn, user_id, world_id) else {
            continue;
        };
        let is_dm = thunderforge_authz::Actor {
            role: thunderforge_authz::Role::from_stored(&role),
            is_site_admin: is_admin,
        }
        .runs_the_world();
        if asset_scene_visible(conn, is_dm, Some(scene_id))? {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn token_art_visible(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> bool {
    let Ok(mut conn) = state.db_pool.get() else {
        return false;
    };
    tokio::task::spawn_blocking(move || {
        token_art_visible_sync(&mut conn, user_id, is_admin, actor_id)
    })
    .await
    .ok()
    .and_then(Result::ok)
    .unwrap_or(false)
}

async fn authorize_and_read(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    asset_id: Uuid,
    key: String,
) -> Response {
    let Some((actor_id, role)) = load_asset_owner(state, asset_id).await else {
        return (StatusCode::NOT_FOUND, "asset not found").into_response();
    };

    let may_view_actor = require_actor_permission(
        state,
        user_id,
        is_admin,
        actor_id,
        ActorPermissionLevel::Viewer,
    )
    .await
    .is_ok();
    // The token image, and only it, follows the map — see
    // `token_art_visible_sync`.
    let may_view_token = !may_view_actor
        && role == ROLE_TOKEN
        && token_art_visible(state, user_id, is_admin, actor_id).await;

    if !(may_view_actor || may_view_token) {
        return (
            StatusCode::FORBIDDEN,
            "not permitted to view this actor's imagery",
        )
            .into_response();
    }

    let cfg = RustFsConfig::from_env();
    match read_object(&cfg, &key).await {
        Ok(bytes) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "image/webp")],
            bytes,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "asset object not found in storage").into_response(),
    }
}

async fn serve_actor_asset(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(asset_segment): Path<String>,
) -> Response {
    // `<uuid>`, `<uuid>.webp` or `<uuid>.png`; anything else — Bevy's
    // `<uuid>.webp.meta` probe in particular — is a 404, as on the canvas
    // route and for the same reason.
    let Some(asset_id) = super::canvas::parse_asset_id(&asset_segment) else {
        return (StatusCode::NOT_FOUND, "asset not found").into_response();
    };
    authorize_and_read(
        &state,
        auth_user.user_id,
        auth_user.is_admin,
        asset_id,
        actor_image_full_key(asset_id),
    )
    .await
}

async fn serve_actor_asset_thumbnail(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(asset_id): Path<Uuid>,
) -> Response {
    authorize_and_read(
        &state,
        auth_user.user_id,
        auth_user.is_admin,
        asset_id,
        actor_image_thumb_key(asset_id),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        insert_test_actor, insert_test_scene, insert_test_user, insert_test_world,
        insert_test_world_member, test_app_state,
    };

    /// A GM's world with one scene and one NPC, and a player who is a member.
    struct Table {
        gm: Uuid,
        player: Uuid,
        scene: Uuid,
        npc: Uuid,
    }

    fn table(conn: &mut PgConnection) -> Table {
        let gm = insert_test_user(conn);
        let player = insert_test_user(conn);
        let world = insert_test_world(conn, gm);
        insert_test_world_member(conn, world, player, "Player");
        let scene = insert_test_scene(conn, world, gm);
        let npc = insert_test_actor(conn, world, scene, gm);
        Table {
            gm,
            player,
            scene,
            npc,
        }
    }

    fn place_token(conn: &mut PgConnection, scene: Uuid, actor: Uuid) {
        use crate::schema::tokens;
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(tokens::table)
            .values((
                tokens::token_id.eq(Uuid::now_v7()),
                tokens::scene_id.eq(scene),
                tokens::actor_id.eq(Some(actor)),
                tokens::x.eq(0.0),
                tokens::y.eq(0.0),
                tokens::rotation.eq(0.0),
                tokens::scale.eq(1.0),
                tokens::token_type.eq("npc"),
                tokens::created_at.eq(now),
                tokens::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("token placed");
    }

    fn set_hidden(conn: &mut PgConnection, scene: Uuid, hidden: bool) {
        use crate::schema::scenes;
        diesel::update(scenes::table.filter(scenes::scene_id.eq(scene)))
            .set(scenes::hidden.eq(hidden))
            .execute(conn)
            .expect("scene visibility set");
    }

    /// Playtest 2026-09-10 P1: a player looking at an NPC's token may load its
    /// art, though they may not view the NPC itself.
    #[test]
    fn a_player_may_see_the_art_of_a_token_on_a_scene_they_can_see() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("connection");
        let t = table(&mut conn);
        place_token(&mut conn, t.scene, t.npc);
        set_hidden(&mut conn, t.scene, false);

        assert!(token_art_visible_sync(&mut conn, t.player, false, t.npc).expect("answered"));
    }

    #[test]
    fn a_hidden_scene_lends_its_tokens_no_art() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("connection");
        let t = table(&mut conn);
        place_token(&mut conn, t.scene, t.npc);
        set_hidden(&mut conn, t.scene, true);

        assert!(
            !token_art_visible_sync(&mut conn, t.player, false, t.npc).expect("answered"),
            "a token on a scene the player cannot see reveals nothing",
        );
        // The Game Master sees their own hidden scene, and so its tokens.
        assert!(token_art_visible_sync(&mut conn, t.gm, false, t.npc).expect("answered"));
    }

    #[test]
    fn a_stranger_sees_no_token_art() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("connection");
        let t = table(&mut conn);
        place_token(&mut conn, t.scene, t.npc);
        set_hidden(&mut conn, t.scene, false);
        let stranger = insert_test_user(&mut conn);

        assert!(!token_art_visible_sync(&mut conn, stranger, false, t.npc).expect("answered"));
    }

    #[test]
    fn an_actor_with_no_token_on_the_map_lends_nothing() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("connection");
        let t = table(&mut conn);
        set_hidden(&mut conn, t.scene, false);

        assert!(
            !token_art_visible_sync(&mut conn, t.player, false, t.npc).expect("answered"),
            "only a token on the map lends its character's art",
        );
    }
}
