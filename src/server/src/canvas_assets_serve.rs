//! Spec 002 gap fix: `uploadCanvasImage`/`save_background_image` write
//! canvas image assets to RustFS, but nothing ever served them back to a
//! browser — RustFS is private, per-campaign-scoped storage (FR-014), so
//! a raw RustFS URL is never handed to a client, and no proxy route
//! existed to authenticate a read and stream the bytes through. Found
//! while wiring `AssetPasteTool` end-to-end for T023/T024/T030 (spec
//! 002's paste-to-canvas e2e coverage) — without this, a pasted image
//! uploads successfully but can never actually render on anyone's
//! canvas, defeating the entire feature.
//!
//! `GET /canvas-assets/{asset_id}` mirrors `map_import.rs`'s
//! `/scenes/{scene_id}/import/uvtt`: authenticated via the same
//! `auth_middleware::require_authenticated_user` layer (see
//! `main.rs`), looks up the asset's owning world, authorizes via
//! `require_world_member` (FR-014, FR-019 — the same rule
//! `canvasImageAssetsForScene` enforces), then streams the object's
//! bytes from RustFS using a single-object-scoped, server-held
//! `read_object` credential (never exposed to the client — the browser
//! only ever talks to this route, never to RustFS directly).

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Router};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::{require_world_member, WorldMembershipError};
use crate::auth_middleware::AuthenticatedUser;
use crate::state::AppState;
use crate::storage::rustfs::{read_object, RustFsConfig};

pub fn router() -> Router<AppState> {
    Router::new().route("/canvas-assets/{asset_id}", get(serve_canvas_asset))
}

async fn serve_canvas_asset(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(asset_id): Path<Uuid>,
) -> Response {
    let user_id = auth_user.user_id;

    let mut conn = match state.db_pool.get() {
        Ok(conn) => conn,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "database unavailable").into_response(),
    };

    let lookup = tokio::task::spawn_blocking(move || -> Result<Option<(Uuid, String)>, diesel::result::Error> {
        use crate::schema::canvas_image_assets;
        canvas_image_assets::table
            .filter(canvas_image_assets::asset_id.eq(asset_id))
            .select((canvas_image_assets::world_id, canvas_image_assets::storage_path))
            .first::<(Uuid, String)>(&mut conn)
            .optional()
    })
    .await;

    let Ok(Ok(Some((world_id, storage_path)))) = lookup else {
        return (StatusCode::NOT_FOUND, "asset not found").into_response();
    };

    // FR-014, FR-019: same authorization rule as canvasImageAssetsForScene.
    let mut conn = match state.db_pool.get() {
        Ok(conn) => conn,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "database unavailable").into_response(),
    };
    let authz = tokio::task::spawn_blocking(move || require_world_member(&mut conn, user_id, world_id)).await;
    match authz {
        Ok(Ok(_role)) => {}
        Ok(Err(WorldMembershipError::NotAMember)) => {
            return (StatusCode::FORBIDDEN, "not a member of this world").into_response();
        }
        _ => return (StatusCode::INTERNAL_SERVER_ERROR, "authorization check failed").into_response(),
    }

    let cfg = RustFsConfig::from_env();
    match read_object(&cfg, &storage_path).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "image/webp"),
                (header::CACHE_CONTROL, "private, max-age=3600"),
            ],
            bytes,
        )
            .into_response(),
        Err(e) => {
            eprintln!("[canvas-assets] failed to read {storage_path}: {e}");
            (StatusCode::BAD_GATEWAY, "failed to fetch asset from storage").into_response()
        }
    }
}
