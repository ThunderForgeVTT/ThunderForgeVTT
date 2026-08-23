use crate::admin::user_role;
use crate::auth_middleware::AuthenticatedUser;
use crate::models::{User, World, WorldEvent, WorldToken}; // Policy disabled
use crate::schema::{
    login_two_factor_challenges, oauth_link_challenges, user_oauth_accounts, user_sessions, users,
    world_events, world_tokens, worlds,
};
use crate::state::AppState;
use axum::{
    Json, Router,
    extract::{Extension, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get},
};
use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Write};
use tower_cookies::{Cookie, Cookies};
use tracing::info;
use zip::write::SimpleFileOptions;

#[derive(Debug, Clone, Serialize)]
pub struct PublicUser {
    pub id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub role: String,
    pub is_admin: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    format: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportCounts {
    pub worlds: usize,
    pub world_tokens: usize,
    pub world_events: usize,
    pub policies: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportManifest {
    pub schema_version: &'static str,
    pub exported_at: DateTime<Utc>,
    pub counts: ExportCounts,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaceholderDomainExport {
    pub schema_version: &'static str,
    pub status: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserDataExport {
    pub manifest: ExportManifest,
    pub user: PublicUser,
    pub worlds: Vec<World>,
    pub world_tokens: Vec<WorldToken>,
    pub world_events: Vec<WorldEvent>,
    pub policies: Vec<String>, // Policy disabled
    pub scenes: Vec<PlaceholderDomainExport>,
    pub actors: Vec<PlaceholderDomainExport>,
    pub asset_packs: Vec<PlaceholderDomainExport>,
    pub game_systems: Vec<PlaceholderDomainExport>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UserDataDeleteSummary {
    pub worlds_deleted: i64,
    pub world_tokens_deleted: i64,
    pub world_events_deleted: i64,
    pub policies_deleted: i64,
    pub oauth_links_deleted: i64,
    pub sessions_deleted: i64,
    pub login_challenges_deleted: i64,
    pub oauth_link_challenges_deleted: i64,
    pub users_deleted: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserDataDeleteResponse {
    pub status: &'static str,
    pub message: String,
    pub summary: UserDataDeleteSummary,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/user/data/export", get(export_user_data))
        .route("/user/data", delete(delete_user_data))
}

impl From<User> for PublicUser {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            role: user_role(user.is_admin).to_string(),
            is_admin: user.is_admin,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

pub async fn load_public_user(state: &AppState, user_id: uuid::Uuid) -> Result<PublicUser, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::id.eq(user_id))
            .select(User::as_select())
            .first::<User>(&mut conn)
            .map(PublicUser::from)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to load user".to_string())
}

pub async fn record_auth_audit_event(
    state: &AppState,
    actor_user_id: Option<uuid::Uuid>,
    event_type: &str,
    subject_user_hash: Option<String>,
    metadata: Option<serde_json::Value>,
) -> Result<(), String> {
    let _ = state;
    info!(
        event_type,
        actor_user_id = actor_user_id.map(|id| id.to_string()),
        subject_user_hash,
        metadata = metadata.map(|value| value.to_string()),
        "auth audit event"
    );
    Ok(())
}

pub fn hash_user_identifier_for_audit(secret: &str, user_id: uuid::Uuid) -> String {
    let digest = Sha256::digest(format!("thunderforge:user:{secret}:{user_id}").as_bytes());
    general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

pub async fn export_user_data_payload(
    state: &AppState,
    user_id: uuid::Uuid,
) -> Result<UserDataExport, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    let (user, owned_worlds, owned_tokens, owned_events, owned_policies) =
        tokio::task::spawn_blocking(move || {
            let user = users::table
                .filter(users::id.eq(user_id))
                .select(User::as_select())
                .first::<User>(&mut conn)?;

            let owned_worlds = worlds::table
                .filter(worlds::created_by.eq(user_id))
                .order(worlds::created_at.asc())
                .select(World::as_select())
                .load::<World>(&mut conn)?;

            let owned_tokens = world_tokens::table
                .filter(world_tokens::created_by.eq(user_id))
                .order(world_tokens::created_at.asc())
                .select(WorldToken::as_select())
                .load::<WorldToken>(&mut conn)?;

            let owned_events = world_events::table
                .filter(world_events::created_by.eq(user_id))
                .order(world_events::created_at.asc())
                .select(WorldEvent::as_select())
                .load::<WorldEvent>(&mut conn)?;

            let owned_policies: Vec<String> = vec![]; // Policies disabled

            Ok::<_, diesel::result::Error>((
                user,
                owned_worlds,
                owned_tokens,
                owned_events,
                owned_policies,
            ))
        })
        .await
        .map_err(|_| "Failed to spawn blocking task".to_string())?
        .map_err(|_| "Failed to query export data".to_string())?;

    Ok(UserDataExport {
        manifest: ExportManifest {
            schema_version: "v1",
            exported_at: Utc::now(),
            counts: ExportCounts {
                worlds: owned_worlds.len(),
                world_tokens: owned_tokens.len(),
                world_events: owned_events.len(),
                policies: owned_policies.len(),
            },
        },
        user: PublicUser::from(user),
        worlds: owned_worlds,
        world_tokens: owned_tokens,
        world_events: owned_events,
        policies: owned_policies,
        scenes: Vec::new(),
        actors: Vec::new(),
        asset_packs: Vec::new(),
        game_systems: Vec::new(),
    })
}

pub async fn delete_user_data_owned(
    state: &AppState,
    user_id: uuid::Uuid,
) -> Result<UserDataDeleteSummary, String> {
    let state_for_delete = state.clone();
    tokio::task::spawn_blocking(move || delete_user_data_sync(&state_for_delete, user_id))
        .await
        .map_err(|_| "Failed to spawn blocking task".to_string())?
}

async fn export_user_data(
    Extension(auth_user): Extension<AuthenticatedUser>,
    State(state): State<AppState>,
    Query(query): Query<ExportQuery>,
) -> Response {
    let export_format = match normalize_export_format(query.format.as_deref()) {
        Ok(value) => value,
        Err(message) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "status": "invalid_request",
                    "message": message,
                })),
            )
                .into_response();
        }
    };

    let export = match export_user_data_payload(&state, auth_user.user_id).await {
        Ok(value) => value,
        Err(message) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "status": "export_failed",
                    "message": message,
                })),
            )
                .into_response();
        }
    };

    match export_format {
        "json" => match serde_json::to_vec_pretty(&export) {
            Ok(body) => {
                build_download_response(body, "application/json", "thunderforge-user-export.json")
            }
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "status": "export_failed",
                    "message": "Failed to serialize export payload",
                })),
            )
                .into_response(),
        },
        "zip" => match build_zip_export(&export) {
            Ok(body) => {
                build_download_response(body, "application/zip", "thunderforge-user-export.zip")
            }
            Err(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "status": "export_failed",
                    "message": message,
                })),
            )
                .into_response(),
        },
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn delete_user_data(
    Extension(auth_user): Extension<AuthenticatedUser>,
    State(state): State<AppState>,
    cookies: Cookies,
) -> (StatusCode, Json<UserDataDeleteResponse>) {
    let user_id = auth_user.user_id;
    let summary = match delete_user_data_owned(&state, user_id).await {
        Ok(summary) => summary,
        Err(message) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UserDataDeleteResponse {
                    status: "deletion_failed",
                    message,
                    summary: UserDataDeleteSummary::default(),
                }),
            );
        }
    };

    let subject_user_hash = hash_user_identifier_for_audit(&state.config.secret, user_id);
    let _ = record_auth_audit_event(
        &state,
        None,
        "user_data_deleted",
        Some(subject_user_hash.clone()),
        Some(serde_json::json!({
            "worlds_deleted": summary.worlds_deleted,
            "world_tokens_deleted": summary.world_tokens_deleted,
            "world_events_deleted": summary.world_events_deleted,
            "policies_deleted": summary.policies_deleted,
        })),
    )
    .await;

    cookies
        .private(&state.key)
        .remove(Cookie::new("session", ""));
    cookies.remove(Cookie::new("csrf_token", ""));

    info!(subject_user_hash, "user data permanently deleted");

    (
        StatusCode::OK,
        Json(UserDataDeleteResponse {
            status: "deleted",
            message: "User profile and owned data were permanently deleted".to_string(),
            summary,
        }),
    )
}

fn delete_user_data_sync(
    state: &AppState,
    user_id: uuid::Uuid,
) -> Result<UserDataDeleteSummary, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    conn.transaction(|conn| {
        let mut summary = UserDataDeleteSummary::default();

        let owned_world_ids = worlds::table
            .filter(worlds::created_by.eq(user_id))
            .select(worlds::id)
            .load::<uuid::Uuid>(conn)?;

        if !owned_world_ids.is_empty() {
            summary.world_events_deleted += diesel::delete(
                world_events::table.filter(world_events::world_id.eq_any(&owned_world_ids)),
            )
            .execute(conn)? as i64;

            summary.world_tokens_deleted += diesel::delete(
                world_tokens::table.filter(world_tokens::world_id.eq_any(&owned_world_ids)),
            )
            .execute(conn)? as i64;

            summary.worlds_deleted +=
                diesel::delete(worlds::table.filter(worlds::id.eq_any(&owned_world_ids)))
                    .execute(conn)? as i64;
        }

        summary.world_events_deleted +=
            diesel::delete(world_events::table.filter(world_events::created_by.eq(user_id)))
                .execute(conn)? as i64;

        summary.world_tokens_deleted +=
            diesel::delete(world_tokens::table.filter(world_tokens::created_by.eq(user_id)))
                .execute(conn)? as i64;

        //         summary.policies_deleted +=
        //             diesel::delete(policies::table.filter(policies::created_by.eq(user_id)))
        //                 .execute(conn)? as i64;

        summary.oauth_link_challenges_deleted += diesel::delete(
            oauth_link_challenges::table.filter(oauth_link_challenges::user_id.eq(user_id)),
        )
        .execute(conn)? as i64;

        summary.login_challenges_deleted += diesel::delete(
            login_two_factor_challenges::table
                .filter(login_two_factor_challenges::user_id.eq(user_id)),
        )
        .execute(conn)? as i64;

        summary.oauth_links_deleted += diesel::delete(
            user_oauth_accounts::table.filter(user_oauth_accounts::user_id.eq(user_id)),
        )
        .execute(conn)? as i64;

        summary.sessions_deleted +=
            diesel::delete(user_sessions::table.filter(user_sessions::user_id.eq(user_id)))
                .execute(conn)? as i64;

        summary.users_deleted +=
            diesel::delete(users::table.filter(users::id.eq(user_id))).execute(conn)? as i64;

        Ok::<UserDataDeleteSummary, diesel::result::Error>(summary)
    })
    .map_err(|_| "Failed to delete user data".to_string())
}

fn normalize_export_format(format: Option<&str>) -> Result<&'static str, &'static str> {
    match format
        .unwrap_or("json")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "json" => Ok("json"),
        "zip" => Ok("zip"),
        _ => Err("Unsupported export format. Use 'json' or 'zip'."),
    }
}

fn build_zip_export(export: &UserDataExport) -> Result<Vec<u8>, String> {
    let export_json =
        serde_json::to_vec_pretty(export).map_err(|_| "Failed to serialize export payload")?;
    let manifest_json = serde_json::to_vec_pretty(&export.manifest)
        .map_err(|_| "Failed to serialize export manifest")?;

    let cursor = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("manifest.json", options)
        .map_err(|e| format!("Failed to create manifest entry: {e}"))?;
    zip.write_all(&manifest_json)
        .map_err(|e| format!("Failed to write manifest entry: {e}"))?;

    zip.start_file("export.json", options)
        .map_err(|e| format!("Failed to create export entry: {e}"))?;
    zip.write_all(&export_json)
        .map_err(|e| format!("Failed to write export entry: {e}"))?;

    zip.finish()
        .map(|cursor| cursor.into_inner())
        .map_err(|e| format!("Failed to finalize zip export: {e}"))
}

fn build_download_response(body: Vec<u8>, content_type: &str, filename: &str) -> Response {
    let mut response = body.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type).expect("valid content type"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .expect("valid content disposition"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::{hash_user_identifier_for_audit, normalize_export_format};

    #[test]
    fn export_format_defaults_to_json() {
        assert_eq!(normalize_export_format(None), Ok("json"));
    }

    #[test]
    fn export_format_rejects_unknown_value() {
        assert_eq!(
            normalize_export_format(Some("tar")),
            Err("Unsupported export format. Use 'json' or 'zip'.")
        );
    }

    #[test]
    fn audit_hash_is_deterministic() {
        let user_id = uuid::Uuid::nil();
        let first = hash_user_identifier_for_audit("test-secret", user_id);
        let second = hash_user_identifier_for_audit("test-secret", user_id);

        assert_eq!(first, second);
        assert!(!first.is_empty());
    }
}
