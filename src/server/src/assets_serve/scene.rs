//! Spec 022 (FR-011/FR-012): serves a scene's generated preview/thumbnail
//! image. Mirrors `assets_serve/lore.rs` exactly (authenticated, then
//! authorized via world membership, then streamed from RustFS via a
//! single-object-scoped, server-held credential) — the one difference is
//! the authorization check itself (world membership, not lore permission,
//! since a scene preview is visible to any world member whose `scenes`
//! query already returned this scene per FR-008/FR-009's hidden-filtering).

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Router};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::require_world_member;
use crate::auth_middleware::AuthenticatedUser;
use crate::state::AppState;
use crate::storage::rustfs::{RustFsConfig, read_object};

pub fn router() -> Router<AppState> {
    Router::new().route("/scene-assets/{asset_id}/thumb", get(serve_scene_preview))
}

/// RustFS object key for a scene preview asset — shared with
/// `map_import/image.rs::save_scene_preview_image`, which writes to this
/// same key when a scene's background image is (re)set.
pub fn preview_key(asset_id: Uuid) -> String {
    format!("scenes/{asset_id}-preview.webp")
}

async fn load_preview_scene_world_id(state: &AppState, asset_id: Uuid) -> Option<Uuid> {
    let mut conn = state.db_pool.get().ok()?;
    tokio::task::spawn_blocking(move || {
        use crate::schema::{scene_preview_images, scenes};
        scene_preview_images::table
            .inner_join(scenes::table.on(scenes::scene_id.eq(scene_preview_images::scene_id)))
            .filter(scene_preview_images::id.eq(asset_id))
            .select(scenes::world_id)
            .first::<Uuid>(&mut conn)
            .optional()
    })
    .await
    .ok()?
    .ok()?
}

async fn serve_scene_preview(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(asset_id): Path<Uuid>,
) -> Response {
    let Some(world_id) = load_preview_scene_world_id(&state, asset_id).await else {
        return (StatusCode::NOT_FOUND, "asset not found").into_response();
    };

    let membership_ok = {
        let user_id = auth_user.user_id;
        let is_admin = auth_user.is_admin;
        let Ok(mut conn) = state.db_pool.get() else {
            return (StatusCode::INTERNAL_SERVER_ERROR, "database unavailable").into_response();
        };
        is_admin
            || tokio::task::spawn_blocking(move || {
                require_world_member(&mut conn, user_id, world_id).is_ok()
            })
            .await
            .unwrap_or(false)
    };
    if !membership_ok {
        return (StatusCode::FORBIDDEN, "not a member of this scene's world").into_response();
    }

    let cfg = RustFsConfig::from_env();
    match read_object(&cfg, &preview_key(asset_id)).await {
        Ok(bytes) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "image/webp")],
            bytes,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "asset object not found in storage").into_response(),
    }
}
