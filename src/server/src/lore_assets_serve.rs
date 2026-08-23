//! Spec 012: `uploadLoreImage` writes lore image assets to RustFS, but
//! RustFS is private, per-world-scoped storage (mirrors ADR-039) — a raw
//! RustFS URL is never handed to a client. `GET /lore-assets/{asset_id}`
//! and `GET /lore-assets/{asset_id}/thumb` mirror
//! `canvas_assets_serve.rs`'s `/canvas-assets/{asset_id}` exactly:
//! authenticated via the same `auth_middleware::require_authenticated_user`
//! layer, authorized via the entry's effective lore permission
//! (Viewer-or-above), then stream the object's bytes from RustFS using a
//! single-object-scoped, server-held `read_object` credential (never
//! exposed to the client).

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Router};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::lore_permissions::require_lore_permission;
use crate::auth_middleware::AuthenticatedUser;
use crate::graphql::types::ActorPermissionLevel;
use crate::state::AppState;
use crate::storage::rustfs::{read_object, RustFsConfig};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/lore-assets/{asset_id}", get(serve_lore_asset))
        .route("/lore-assets/{asset_id}/thumb", get(serve_lore_asset_thumbnail))
}

async fn load_asset_lore_entry_id(state: &AppState, asset_id: Uuid) -> Option<Uuid> {
    let mut conn = state.db_pool.get().ok()?;
    tokio::task::spawn_blocking(move || {
        use crate::schema::world_lore_image_assets;
        world_lore_image_assets::table
            .filter(world_lore_image_assets::id.eq(asset_id))
            .select(world_lore_image_assets::lore_entry_id)
            .first::<Uuid>(&mut conn)
            .optional()
    })
    .await
    .ok()?
    .ok()?
}

async fn authorize_and_read(state: &AppState, user_id: Uuid, is_admin: bool, asset_id: Uuid, key: String) -> Response {
    let Some(lore_entry_id) = load_asset_lore_entry_id(state, asset_id).await else {
        return (StatusCode::NOT_FOUND, "asset not found").into_response();
    };

    if require_lore_permission(state, user_id, is_admin, lore_entry_id, ActorPermissionLevel::Viewer)
        .await
        .is_err()
    {
        return (StatusCode::FORBIDDEN, "not permitted to view this lore entry's images").into_response();
    }

    let cfg = RustFsConfig::from_env();
    match read_object(&cfg, &key).await {
        Ok(bytes) => (StatusCode::OK, [(header::CONTENT_TYPE, "image/webp")], bytes).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "asset object not found in storage").into_response(),
    }
}

/// RustFS object key for a lore image asset's full-size rendition —
/// shared with `mutations_lore_images.rs`, which writes to this same key
/// on upload.
pub fn full_key(asset_id: Uuid) -> String {
    format!("lore/{asset_id}.webp")
}

/// RustFS object key for a lore image asset's thumbnail rendition.
pub fn thumb_key(asset_id: Uuid) -> String {
    format!("lore/{asset_id}-thumb.webp")
}

async fn serve_lore_asset(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(asset_id): Path<Uuid>,
) -> Response {
    authorize_and_read(&state, auth_user.user_id, auth_user.is_admin, asset_id, full_key(asset_id)).await
}

async fn serve_lore_asset_thumbnail(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(asset_id): Path<Uuid>,
) -> Response {
    authorize_and_read(&state, auth_user.user_id, auth_user.is_admin, asset_id, thumb_key(asset_id)).await
}
