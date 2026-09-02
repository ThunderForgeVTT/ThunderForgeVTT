//! Spec 031 (T069, FR-036): reading an actor's portrait and token image back.
//!
//! `mutations_actor_images.rs` writes the bytes into RustFS, which is private,
//! server-credentialled storage (ADR-039) — a client is never handed a RustFS
//! URL. `GET /actor-assets/{asset_id}` and `/thumb` mirror
//! `lore_assets_serve.rs` exactly: authenticated by the same
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
use crate::auth_middleware::AuthenticatedUser;
use crate::graphql::types::ActorPermissionLevel;
use crate::state::AppState;
use crate::storage::rustfs::{RustFsConfig, read_object};

pub fn router() -> Router<AppState> {
    Router::new()
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

/// Which actor an asset belongs to.
///
/// The asset id alone carries no authority — the row is what says whose
/// picture this is, and therefore who may look at it.
async fn load_asset_actor_id(state: &AppState, asset_id: Uuid) -> Option<Uuid> {
    let mut conn = state.db_pool.get().ok()?;
    tokio::task::spawn_blocking(move || {
        use crate::schema::world_actor_images;
        world_actor_images::table
            .filter(world_actor_images::asset_id.eq(asset_id))
            .select(world_actor_images::actor_id)
            .first::<Uuid>(&mut conn)
            .optional()
    })
    .await
    .ok()?
    .ok()?
}

async fn authorize_and_read(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    asset_id: Uuid,
    key: String,
) -> Response {
    let Some(actor_id) = load_asset_actor_id(state, asset_id).await else {
        return (StatusCode::NOT_FOUND, "asset not found").into_response();
    };

    if require_actor_permission(
        state,
        user_id,
        is_admin,
        actor_id,
        ActorPermissionLevel::Viewer,
    )
    .await
    .is_err()
    {
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
    Path(asset_id): Path<Uuid>,
) -> Response {
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
