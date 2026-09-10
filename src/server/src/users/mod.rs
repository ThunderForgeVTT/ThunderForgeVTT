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

pub mod export_content;

pub use export_content::{
    ExportedAbility, ExportedActor, ExportedCollection, ExportedItem, ExportedLoreEntry,
    ExportedScene,
};

#[derive(Debug, Clone, Serialize)]
pub struct ExportCounts {
    pub worlds: usize,
    pub world_tokens: usize,
    pub world_events: usize,
    pub policies: usize,
    pub scenes: usize,
    pub actors: usize,
    pub items: usize,
    pub abilities: usize,
    pub lore_entries: usize,
    pub collections: usize,
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
    /// Spec 039 T076 (ADR-011 as amended): what the person made, as the
    /// export's own shapes rather than table rows.
    pub scenes: Vec<ExportedScene>,
    pub actors: Vec<ExportedActor>,
    pub items: Vec<ExportedItem>,
    pub abilities: Vec<ExportedAbility>,
    pub lore_entries: Vec<ExportedLoreEntry>,
    pub collections: Vec<ExportedCollection>,
    /// Still reserved: neither is something a person makes.
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
    Router::new().route("/user/data", delete(delete_user_data))
}

/// The download, on a router of its own because it is one of the two things a
/// **disabled** account may still do (spec 039 FR-031). `main.rs` layers it
/// with `require_authenticated_user_even_if_disabled`; everything in
/// [`router`] refuses a disabled account like every other route.
pub fn export_router() -> Router<AppState> {
    Router::new().route("/user/data/export", get(export_user_data))
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

    let (user, owned_worlds, owned_tokens, owned_events, owned_policies, content) =
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

            let content = export_content::load_content_sync(&mut conn, user_id)?;

            Ok::<_, diesel::result::Error>((
                user,
                owned_worlds,
                owned_tokens,
                owned_events,
                owned_policies,
                content,
            ))
        })
        .await
        .map_err(|_| "Failed to spawn blocking task".to_string())?
        .map_err(|_| "Failed to query export data".to_string())?;

    Ok(UserDataExport {
        manifest: ExportManifest {
            // v2: the person's own content, as shapes (ADR-011 as amended).
            schema_version: "v2",
            exported_at: Utc::now(),
            counts: ExportCounts {
                worlds: owned_worlds.len(),
                world_tokens: owned_tokens.len(),
                world_events: owned_events.len(),
                policies: owned_policies.len(),
                scenes: content.scenes.len(),
                actors: content.actors.len(),
                items: content.items.len(),
                abilities: content.abilities.len(),
                lore_entries: content.lore_entries.len(),
                collections: content.collections.len(),
            },
        },
        user: PublicUser::from(user),
        worlds: owned_worlds,
        world_tokens: owned_tokens,
        world_events: owned_events,
        policies: owned_policies,
        scenes: content.scenes,
        actors: content.actors,
        items: content.items,
        abilities: content.abilities,
        lore_entries: content.lore_entries,
        collections: content.collections,
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

    delete_user_data_on(&mut conn, user_id).map_err(|_| "Failed to delete user data".to_string())
}

/// The deletion itself, on a connection the caller holds — so the termination
/// sweep (spec 039 US7) runs the same code a person deleting their own account
/// does, inside the transaction that closes the window.
pub(crate) fn delete_user_data_on(
    conn: &mut PgConnection,
    user_id: uuid::Uuid,
) -> Result<UserDataDeleteSummary, diesel::result::Error> {
    conn.transaction(|conn| {
        let mut summary = UserDataDeleteSummary::default();

        let owned_world_ids = worlds::table
            .filter(worlds::created_by.eq(user_id))
            .select(worlds::id)
            .load::<uuid::Uuid>(conn)?;

        // Decided 2026-09-08 (spec 039's contract records it): the account's
        // worlds go, including ones other people play in — but every
        // character owned by somebody else is moved to its player first. In
        // this transaction, so a rescue that failed means nothing was deleted.
        crate::collections::rescue::rescue_characters_sync(conn, user_id, &owned_world_ids)?;

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

        // Spec 039 FR-010/FR-037: the agreements survive the account, with the
        // name removed. Inside this transaction so the redaction and the
        // deletion are one act — never a deleted account whose name is still
        // on its records, and never a redaction for a deletion that rolled back.
        crate::attestation::redact_for_deleted_account(conn, user_id)?;

        // Spec 039 FR-037: a notice is addressed to a person and says nothing
        // once they are gone, so unlike an agreement it does not survive them.
        // The moderation cases it described are untouched.
        diesel::delete(
            crate::schema::account_notices::table
                .filter(crate::schema::account_notices::account_id.eq(user_id)),
        )
        .execute(conn)?;

        summary.users_deleted +=
            diesel::delete(users::table.filter(users::id.eq(user_id))).execute(conn)? as i64;

        Ok::<UserDataDeleteSummary, diesel::result::Error>(summary)
    })
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

    /// Spec 039 FR-010/FR-037, through the real deletion path: the account
    /// goes, its agreements stay, and the name is the only thing that leaves
    /// with it.
    ///
    /// `attestation_tests` proves `redact_for_deleted_account` does the right
    /// thing. This proves somebody calls it — remove the call from
    /// `delete_user_data_sync` and the name survives the account.
    #[tokio::test]
    async fn an_agreement_outlives_the_account_that_made_it() {
        use crate::attestation::{PendingAttestation, PublishableKind, record_sync};
        use crate::schema::{attestations, users};
        use crate::test_support::{insert_test_user, test_app_state};
        use diesel::prelude::*;

        let state = test_app_state();
        crate::legal::ensure_terms_versions_recorded(&state)
            .await
            .expect("archive");
        let version = crate::legal::sharing_terms().version_id;

        let mut conn = state.db_pool.get().expect("conn");
        let user_id = insert_test_user(&mut conn);
        let publishable_id = uuid::Uuid::now_v7();
        record_sync(
            &mut conn,
            &PendingAttestation {
                subject_user_id: user_id,
                subject_username: Some("leaving".to_string()),
                terms_version_id: version.clone(),
                kind: PublishableKind::Item,
                publishable_id,
                world_id: None,
            },
            uuid::Uuid::now_v7(),
        )
        .expect("record");
        drop(conn);

        let summary = super::delete_user_data_sync(&state, user_id).expect("delete");
        assert_eq!(summary.users_deleted, 1);

        let mut conn = state.db_pool.get().expect("conn");
        let still_there: i64 = users::table
            .filter(users::id.eq(user_id))
            .count()
            .get_result(&mut conn)
            .expect("count");
        assert_eq!(still_there, 0, "the account itself is gone");

        let (username, kept_version, kept_publishable): (
            Option<String>,
            String,
            Option<uuid::Uuid>,
        ) = attestations::table
            .filter(attestations::subject_user_id.eq(user_id))
            .select((
                attestations::subject_username,
                attestations::terms_version_id,
                attestations::publishable_id,
            ))
            .first(&mut conn)
            .expect("the agreement must survive the account that made it");
        assert_eq!(username, None, "and the name must not");
        assert_eq!(kept_version, version);
        assert_eq!(kept_publishable, Some(publishable_id));
    }

    /// Where each of `player`'s characters now lives: `(actor, world, world name)`.
    fn characters_of(
        conn: &mut diesel::PgConnection,
        player: uuid::Uuid,
    ) -> Vec<(uuid::Uuid, uuid::Uuid, String)> {
        use crate::schema::{world_actors, worlds};
        use diesel::prelude::*;

        let placed: Vec<(uuid::Uuid, uuid::Uuid)> = world_actors::table
            .filter(world_actors::owned_by.eq(player))
            .select((world_actors::id, world_actors::world_id))
            .load(conn)
            .expect("characters");
        placed
            .into_iter()
            .map(|(actor, world)| {
                let name: String = worlds::table
                    .filter(worlds::id.eq(world))
                    .select(worlds::name)
                    .first(conn)
                    .expect("world");
                (actor, world, name)
            })
            .collect()
    }

    /// Decided 2026-09-08, and spec 039's T068 as amended by it: deleting an
    /// account deletes its worlds — even one somebody else plays in — and
    /// moves each player's character to that player first, filed as a
    /// collection named for the world it came from. The GM's own NPC goes
    /// with the world.
    #[tokio::test]
    async fn a_players_character_outlives_the_world_it_was_played_in() {
        use crate::schema::{account_notices, world_collection_members, world_collections, worlds};
        use crate::test_support::{
            insert_test_actor, insert_test_scene, insert_test_user, insert_test_world,
            test_app_state,
        };
        use diesel::prelude::*;

        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("conn");
        let gm = insert_test_user(&mut conn);
        let campaign = insert_test_world(&mut conn, gm);
        let scene = insert_test_scene(&mut conn, campaign, gm);
        let player = insert_test_user(&mut conn);
        let character = insert_test_actor(&mut conn, campaign, scene, player);
        insert_test_actor(&mut conn, campaign, scene, gm);
        let campaign_name: String = worlds::table
            .filter(worlds::id.eq(campaign))
            .select(worlds::name)
            .first(&mut conn)
            .expect("name");
        drop(conn);

        super::delete_user_data_sync(&state, gm).expect("delete");

        let mut conn = state.db_pool.get().expect("conn");
        let campaign_left: i64 = worlds::table
            .filter(worlds::id.eq(campaign))
            .count()
            .get_result(&mut conn)
            .expect("count");
        assert_eq!(
            campaign_left, 0,
            "the world goes with its creator — decided, not a defect"
        );

        let rescued = characters_of(&mut conn, player);
        assert_eq!(
            rescued.len(),
            1,
            "the player's character, and only theirs: {rescued:?}"
        );
        let (copy, home, home_name) = &rescued[0];
        assert_ne!(*copy, character, "a copy, in a world that survives");
        assert!(
            home_name.ends_with("'s characters"),
            "a world of the player's own: {home_name}",
        );

        let filed_under: String = world_collection_members::table
            .inner_join(
                world_collections::table
                    .on(world_collections::id.eq(world_collection_members::collection_id)),
            )
            .filter(world_collection_members::member_id.eq(*copy))
            .filter(world_collections::world_id.eq(*home))
            .select(world_collections::name)
            .first(&mut conn)
            .expect("filed in a collection");
        assert_eq!(
            filed_under, campaign_name,
            "named for the world it came from"
        );

        let told: i64 = account_notices::table
            .filter(account_notices::account_id.eq(player))
            .filter(account_notices::kind.eq(crate::notices::kind::ACTOR_RESCUED))
            .count()
            .get_result(&mut conn)
            .expect("count");
        assert_eq!(told, 1, "and the player is told where it went");
    }

    /// Spec 039 T076: the download carries what the person made — the
    /// character they own (with what it carries), and the items, abilities,
    /// lore and collections they wrote. A download missing them is not the
    /// remedy a disabled account is offered.
    #[tokio::test]
    async fn the_export_carries_everything_a_person_made() {
        use crate::schema::{world_actor_inventory, world_collections};
        use crate::test_support::{
            insert_test_ability, insert_test_actor, insert_test_item, insert_test_lore_entry,
            insert_test_scene, insert_test_user, insert_test_world, test_app_state,
        };
        use diesel::prelude::*;

        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("conn");
        let author = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, author);
        let scene = insert_test_scene(&mut conn, world, author);
        let actor = insert_test_actor(&mut conn, world, scene, author);
        let item = insert_test_item(&mut conn, world, author);
        let ability = insert_test_ability(&mut conn, world, author);
        let lore = insert_test_lore_entry(&mut conn, world, author);
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(world_actor_inventory::table)
            .values((
                world_actor_inventory::id.eq(uuid::Uuid::now_v7()),
                world_actor_inventory::actor_id.eq(actor),
                world_actor_inventory::item_id.eq(Some(item)),
                world_actor_inventory::item_name_snapshot.eq("Lantern"),
                world_actor_inventory::quantity.eq(2),
                world_actor_inventory::created_at.eq(now),
                world_actor_inventory::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("inventory");
        let collection = uuid::Uuid::now_v7();
        diesel::insert_into(world_collections::table)
            .values((
                world_collections::id.eq(collection),
                world_collections::world_id.eq(world),
                world_collections::name.eq("Mine"),
                world_collections::created_by.eq(author),
                world_collections::updated_by.eq(author),
                world_collections::created_at.eq(now),
                world_collections::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("collection");
        drop(conn);

        let export = super::export_user_data_payload(&state, author)
            .await
            .expect("export");

        assert_eq!(export.manifest.schema_version, "v2");
        let exported_actor = export
            .actors
            .iter()
            .find(|a| a.id == actor)
            .expect("the character");
        assert_eq!(
            exported_actor.inventory,
            vec![super::export_content::ExportedInventoryLine {
                name: "Lantern".to_string(),
                quantity: 2,
            }],
            "with what it carries",
        );
        assert!(export.items.iter().any(|i| i.id == item));
        assert!(export.abilities.iter().any(|a| a.id == ability));
        assert!(export.lore_entries.iter().any(|l| l.id == lore));
        assert!(export.collections.iter().any(|c| c.id == collection));
        assert!(export.scenes.iter().any(|s| s.id == scene));
        assert_eq!(export.manifest.counts.actors, export.actors.len());
    }

    /// A player who already has a world on the same system gets the character
    /// there, rather than a new world every time a campaign ends.
    #[tokio::test]
    async fn a_rescued_character_goes_to_a_world_the_player_already_has() {
        use crate::test_support::{
            insert_test_actor, insert_test_scene, insert_test_user, insert_test_world,
            test_app_state,
        };

        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("conn");
        let gm = insert_test_user(&mut conn);
        let campaign = insert_test_world(&mut conn, gm);
        let scene = insert_test_scene(&mut conn, campaign, gm);
        let player = insert_test_user(&mut conn);
        let their_own = insert_test_world(&mut conn, player);
        insert_test_scene(&mut conn, their_own, player);
        insert_test_actor(&mut conn, campaign, scene, player);
        drop(conn);

        super::delete_user_data_sync(&state, gm).expect("delete");

        let mut conn = state.db_pool.get().expect("conn");
        let rescued = characters_of(&mut conn, player);
        assert_eq!(rescued.len(), 1);
        assert_eq!(
            rescued[0].1, their_own,
            "the world they already had on this system"
        );
    }
}
