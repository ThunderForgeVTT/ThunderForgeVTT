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
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Router};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::{WorldMembershipError, require_world_member};
use crate::auth_middleware::AuthenticatedUser;
use crate::state::AppState;
use crate::storage::rustfs::{RustFsConfig, read_object};

pub fn router() -> Router<AppState> {
    Router::new().route("/canvas-assets/{asset_id}", get(serve_canvas_asset))
}

/// Splits an optional image extension off the `{asset_id}` path segment.
///
/// The segment is accepted bare (`<uuid>`) or with an image extension
/// (`<uuid>.webp`). Every object this route serves is WebP regardless of
/// what was uploaded (`transcode_to_webp`), which is why the response's
/// Content-Type is unconditionally `image/webp` below — the extension only
/// makes the URL self-describing to the engine's asset pipeline.
///
/// Anything else — notably `<uuid>.meta` — is rejected with `None` so the
/// caller 404s. That case is load-bearing, not defensive: Bevy's
/// `AssetServer` runs with `AssetMetaCheck::Always`, so every
/// `asset_server.load("…/<uuid>.webp")` first requests
/// `…/<uuid>.webp.meta`. Serving image bytes for that request would hand
/// Bevy a "meta file" it then fails to parse as RON, failing the load;
/// a 404 is what makes it fall back to the default meta and proceed.
fn parse_asset_id(segment: &str) -> Option<Uuid> {
    const IMAGE_EXTENSIONS: [&str; 2] = ["webp", "png"];

    let id = match segment.split_once('.') {
        None => segment,
        Some((id, ext)) if IMAGE_EXTENSIONS.contains(&ext) => id,
        Some(_) => return None,
    };
    Uuid::parse_str(id).ok()
}

async fn serve_canvas_asset(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(asset_segment): Path<String>,
) -> Response {
    let user_id = auth_user.user_id;

    let Some(asset_id) = parse_asset_id(&asset_segment) else {
        return (StatusCode::NOT_FOUND, "asset not found").into_response();
    };

    let mut conn = match state.db_pool.get() {
        Ok(conn) => conn,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "database unavailable").into_response();
        }
    };

    let lookup = tokio::task::spawn_blocking(
        move || -> Result<Option<(Uuid, String)>, diesel::result::Error> {
            use crate::schema::canvas_image_assets;
            canvas_image_assets::table
                .filter(canvas_image_assets::asset_id.eq(asset_id))
                .select((
                    canvas_image_assets::world_id,
                    canvas_image_assets::storage_path,
                ))
                .first::<(Uuid, String)>(&mut conn)
                .optional()
        },
    )
    .await;

    let Ok(Ok(Some((world_id, storage_path)))) = lookup else {
        return (StatusCode::NOT_FOUND, "asset not found").into_response();
    };

    // FR-014, FR-019: same authorization rule as canvasImageAssetsForScene.
    let mut conn = match state.db_pool.get() {
        Ok(conn) => conn,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "database unavailable").into_response();
        }
    };
    let authz =
        tokio::task::spawn_blocking(move || require_world_member(&mut conn, user_id, world_id))
            .await;
    match authz {
        Ok(Ok(_role)) => {}
        Ok(Err(WorldMembershipError::NotAMember)) => {
            return (StatusCode::FORBIDDEN, "not a member of this world").into_response();
        }
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "authorization check failed",
            )
                .into_response();
        }
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
            (
                StatusCode::BAD_GATEWAY,
                "failed to fetch asset from storage",
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    //! Integration tests against a real Postgres (DATABASE_URL) and a real
    //! RustFS (`docker compose up -d rustfs`) — no mocks, mirrors
    //! `graphql::mutations_assets::tests`'s convention.

    use super::*;
    use crate::graphql::mutations_assets::{GraphQLCanvasImageAssetKind, upload_canvas_image_impl};
    use crate::test_support::*;
    use axum::body::to_bytes;
    use axum::extract::{Extension, Path, State};

    fn fake_auth_user(user_id: Uuid) -> AuthenticatedUser {
        AuthenticatedUser {
            user_id,
            session_id: Uuid::now_v7(),
            expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
            is_admin: false,
            role: "Player".to_string(),
        }
    }

    /// A world member can fetch an asset that belongs to their world; the
    /// bytes served back are exactly what was uploaded (transcoded to WebP).
    #[tokio::test]
    async fn serve_canvas_asset_returns_bytes_for_authorized_world_member() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let asset = upload_canvas_image_impl(
            &state,
            owner_id,
            world_id,
            scene_id,
            GraphQLCanvasImageAssetKind::Pasted,
            tiny_png_bytes(),
        )
        .await
        .expect("upload should succeed");

        let response = serve_canvas_asset(
            State(state),
            Extension(fake_auth_user(owner_id)),
            Path(asset.asset_id.to_string()),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");
        assert!(!body.is_empty(), "served asset bytes should not be empty");
    }

    /// A user who is not a member of the asset's world is rejected, even
    /// though the asset exists (FR-014/FR-019 parity with
    /// `canvasImageAssetsForScene`).
    #[tokio::test]
    async fn serve_canvas_asset_rejects_non_world_member() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let outsider_id = insert_test_user(&mut conn);
        drop(conn);

        let asset = upload_canvas_image_impl(
            &state,
            owner_id,
            world_id,
            scene_id,
            GraphQLCanvasImageAssetKind::Pasted,
            tiny_png_bytes(),
        )
        .await
        .expect("upload should succeed");

        let response = serve_canvas_asset(
            State(state),
            Extension(fake_auth_user(outsider_id)),
            Path(asset.asset_id.to_string()),
        )
        .await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    /// The `{asset_id}` segment parses bare and with an image extension,
    /// but a `.meta` probe (which Bevy issues before every image load) is
    /// rejected so the route 404s and Bevy falls back to its default meta.
    #[test]
    fn parse_asset_id_accepts_image_extensions_and_rejects_meta() {
        let id = Uuid::now_v7();

        assert_eq!(parse_asset_id(&id.to_string()), Some(id));
        assert_eq!(parse_asset_id(&format!("{id}.webp")), Some(id));
        assert_eq!(parse_asset_id(&format!("{id}.png")), Some(id));

        assert_eq!(parse_asset_id(&format!("{id}.webp.meta")), None);
        assert_eq!(parse_asset_id(&format!("{id}.meta")), None);
        assert_eq!(parse_asset_id("not-a-uuid.webp"), None);
    }

    /// A nonexistent asset id returns 404, not a 500 or an authorization
    /// error — the not-found check happens before any authz lookup.
    #[tokio::test]
    async fn serve_canvas_asset_returns_not_found_for_unknown_asset() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        drop(conn);

        let response = serve_canvas_asset(
            State(state),
            Extension(fake_auth_user(user_id)),
            Path(Uuid::now_v7().to_string()),
        )
        .await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
