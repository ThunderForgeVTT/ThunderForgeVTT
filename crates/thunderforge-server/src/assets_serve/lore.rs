//! Spec 012: `uploadLoreImage` writes lore image assets to RustFS, but
//! RustFS is private, per-world-scoped storage (mirrors ADR-039) — a raw
//! RustFS URL is never handed to a client. `GET /lore-assets/{asset_id}`
//! and `GET /lore-assets/{asset_id}/thumb` mirror
//! `assets_serve/canvas.rs`'s `/canvas-assets/{asset_id}` exactly:
//! authenticated via the same `auth_middleware::require_authenticated_user`
//! layer, authorized via the entry's effective lore permission
//! (Viewer-or-above), then stream the object's bytes from RustFS using a
//! single-object-scoped, server-held `read_object` credential (never
//! exposed to the client).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Router};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::lore_permissions::require_lore_permission;
use crate::auth_middleware::AuthenticatedUser;
use crate::graphql::types::ActorPermissionLevel;
use crate::state::AppState;
use crate::storage::rustfs::RustFsConfig;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/lore-assets/{asset_id}", get(serve_lore_asset))
        .route(
            "/lore-assets/{asset_id}/thumb",
            get(serve_lore_asset_thumbnail),
        )
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

async fn authorize_and_read(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    asset_id: Uuid,
    key: String,
    headers: &HeaderMap,
) -> Response {
    let Some(lore_entry_id) = load_asset_lore_entry_id(state, asset_id).await else {
        return (StatusCode::NOT_FOUND, "asset not found").into_response();
    };

    if require_lore_permission(
        state,
        user_id,
        is_admin,
        lore_entry_id,
        ActorPermissionLevel::Viewer,
    )
    .await
    .is_err()
    {
        return (
            StatusCode::FORBIDDEN,
            "not permitted to view this lore entry's images",
        )
            .into_response();
    }

    let cfg = RustFsConfig::resolve(state).await;
    let extra = [(header::CONTENT_TYPE, "image/webp")];
    match super::ranged::serve(&cfg, &key, headers, &extra).await {
        Ok(response) => response,
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
    headers: HeaderMap,
    Path(asset_id): Path<Uuid>,
) -> Response {
    authorize_and_read(
        &state,
        auth_user.user_id,
        auth_user.is_admin,
        asset_id,
        full_key(asset_id),
        &headers,
    )
    .await
}

async fn serve_lore_asset_thumbnail(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    headers: HeaderMap,
    Path(asset_id): Path<Uuid>,
) -> Response {
    authorize_and_read(
        &state,
        auth_user.user_id,
        auth_user.is_admin,
        asset_id,
        thumb_key(asset_id),
        &headers,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets_serve::ranged::{assert_refused_without_leak, first_ten_bytes};
    use crate::test_support::*;

    /// Spec 080 T028 (SC-006): a range for an image that is not there is
    /// refused as before, and the refusal says nothing about any file.
    ///
    /// Not tested here with a non-member: `require_lore_permission` defaults
    /// every caller without a grant to Viewer, member or not, so this route
    /// serves a non-member today, with or without a range. That gap predates
    /// spec 080 and is tracked on its own.
    #[tokio::test]
    async fn a_range_for_an_unknown_image_is_refused_without_a_hint() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);
        let unknown = Uuid::now_v7();

        let response = authorize_and_read(
            &state,
            user_id,
            false,
            unknown,
            full_key(unknown),
            &first_ten_bytes(),
        )
        .await;

        assert_refused_without_leak(&response, StatusCode::NOT_FOUND);
    }
}
