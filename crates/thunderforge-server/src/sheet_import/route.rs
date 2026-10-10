//! `GET /api/sheet-imports/{versionId}/file`: an uploaded sheet, back to the
//! people who may have it (FR-043b).
//!
//! Declared without the prefix and merged into `api_router` in
//! `apps/thunderforge/src/main.rs`, behind the authenticated-user layer, as the
//! asset routes are. Not behind `feature.sheet_import`: switching imports off
//! does not take a player's own files away.
//!
//! The player who brought the character may download it, and so may the GM of
//! a world where that version was applied. Anyone else gets the 404 a version
//! that does not exist gets: the id is a key, not a capability, and a refusal
//! must not confirm that a file is there.

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Router};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::actor_in_world;
use crate::auth_middleware::AuthenticatedUser;
use crate::state::AppState;
use crate::storage::rustfs::{RustFsConfig, read_object};

pub fn router() -> Router<AppState> {
    Router::new().route("/sheet-imports/{version_id}/file", get(serve_sheet_file))
}

async fn serve_sheet_file(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(version_id): Path<Uuid>,
) -> Response {
    sheet_file(&state, user.user_id, user.is_admin, version_id).await
}

/// The file's key and version number, when `user_id` may have it.
fn may_download(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    version_id: Uuid,
) -> QueryResult<Option<(String, i32)>> {
    use crate::schema::{actor_imports, brought_characters, sheet_import_versions as versions};
    let Some((key, number, owner)) = versions::table
        .inner_join(brought_characters::table)
        .filter(versions::id.eq(version_id))
        .select((
            versions::file_key,
            versions::version_no,
            brought_characters::owner_user_id,
        ))
        .first::<(String, i32, Uuid)>(conn)
        .optional()?
    else {
        return Ok(None);
    };
    if owner == user_id {
        return Ok(Some((key, number)));
    }
    let worlds: Vec<Uuid> = actor_imports::table
        .filter(actor_imports::version_id.eq(version_id))
        .select(actor_imports::world_id)
        .distinct()
        .load(conn)?;
    let runs_one = worlds
        .into_iter()
        .any(|world| actor_in_world(conn, user_id, is_admin, world).runs_the_world());
    Ok(runs_one.then_some((key, number)))
}

pub async fn sheet_file(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    version_id: Uuid,
) -> Response {
    let not_found = || (StatusCode::NOT_FOUND, "sheet not found").into_response();
    let pool = state.db_pool.clone();
    let allowed = tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().ok()?;
        may_download(&mut conn, user_id, is_admin, version_id)
            .ok()
            .flatten()
    })
    .await
    .ok()
    .flatten();
    let Some((key, number)) = allowed else {
        return not_found();
    };
    let cfg = RustFsConfig::resolve(state).await;
    match read_object(&cfg, &key).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/pdf".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"sheet-v{number}.pdf\""),
                ),
                // A character sheet is somebody's: never kept by a shared
                // cache, and never rendered with the page's privileges.
                (header::CACHE_CONTROL, "private, no-store".to_string()),
                (
                    header::CONTENT_SECURITY_POLICY,
                    "default-src 'none'; sandbox".to_string(),
                ),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => not_found(),
    }
}
