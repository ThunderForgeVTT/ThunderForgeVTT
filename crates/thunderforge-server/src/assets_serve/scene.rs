//! Spec 022 (FR-011/FR-012): serves a scene's generated preview/thumbnail
//! image. Mirrors `assets_serve/lore.rs` exactly (authenticated, then
//! authorized via world membership, then streamed from RustFS via a
//! single-object-scoped, server-held credential) — the one difference is
//! the authorization check itself (world membership, not lore permission,
//! since a scene preview is visible to any world member whose `scenes`
//! query already returned this scene per FR-008/FR-009's hidden-filtering).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Router};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::require_world_member;
use crate::auth_middleware::AuthenticatedUser;
use crate::state::AppState;
use crate::storage::rustfs::RustFsConfig;

pub fn router() -> Router<AppState> {
    Router::new().route("/scene-assets/{asset_id}/thumb", get(serve_scene_preview))
}

/// RustFS object key for a scene preview asset — shared with
/// `map_import/image.rs::save_scene_preview_image`, which writes to this
/// same key when a scene's background image is (re)set.
pub fn preview_key(asset_id: Uuid) -> String {
    format!("scenes/{asset_id}-preview.webp")
}

/// The preview's world, or `None` when there is no such preview — or when its
/// scene has been taken down (spec 015 T042), which is answered exactly like
/// an unknown id: a preview is a picture of the scene, and a takedown against
/// the scene covers it without a claimant naming it separately.
async fn load_preview_scene_world_id(state: &AppState, asset_id: Uuid) -> Option<Uuid> {
    let mut conn = state.db_pool.get().ok()?;
    tokio::task::spawn_blocking(move || {
        use crate::schema::{scene_preview_images, scenes};
        let found = scene_preview_images::table
            .inner_join(scenes::table.on(scenes::scene_id.eq(scene_preview_images::scene_id)))
            .filter(scene_preview_images::id.eq(asset_id))
            .select((scenes::scene_id, scenes::world_id))
            .first::<(Uuid, Uuid)>(&mut conn)
            .optional()?;
        let Some((scene_id, world_id)) = found else {
            return Ok(None);
        };
        if crate::auth::scene_visibility::scene_taken_down(&mut conn, scene_id)? {
            return Ok(None);
        }
        Ok::<_, diesel::result::Error>(Some(world_id))
    })
    .await
    .ok()?
    .ok()?
}

async fn serve_scene_preview(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    headers: HeaderMap,
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

    let cfg = RustFsConfig::resolve(&state).await;
    let extra = [(header::CONTENT_TYPE, "image/webp")];
    match super::ranged::serve(&cfg, &preview_key(asset_id), &headers, &extra).await {
        Ok(response) => response,
        Err(_) => (StatusCode::NOT_FOUND, "asset object not found in storage").into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets_serve::canvas::tests::fake_auth_user;
    use crate::assets_serve::ranged::{assert_refused_without_leak, first_ten_bytes};
    use crate::test_support::*;

    /// Spec 080 T028 (SC-006): a non-member's range is refused as before,
    /// and the refusal says nothing about the preview.
    #[tokio::test]
    async fn a_range_from_a_non_member_is_refused_without_a_hint() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let outsider_id = insert_test_user(&mut conn);
        let preview_id = Uuid::now_v7();
        {
            use crate::schema::scene_preview_images;
            diesel::insert_into(scene_preview_images::table)
                .values((
                    scene_preview_images::id.eq(preview_id),
                    scene_preview_images::scene_id.eq(scene_id),
                    scene_preview_images::byte_size.eq(4096_i64),
                    scene_preview_images::created_at.eq(chrono::Utc::now().naive_utc()),
                ))
                .execute(&mut conn)
                .unwrap();
        }
        drop(conn);

        let response = serve_scene_preview(
            State(state),
            Extension(fake_auth_user(outsider_id)),
            first_ten_bytes(),
            Path(preview_id),
        )
        .await;

        assert_refused_without_leak(&response, StatusCode::FORBIDDEN);
    }
}
