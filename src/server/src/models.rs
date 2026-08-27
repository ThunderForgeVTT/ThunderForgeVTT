use crate::schema::{
    admin_bootstrap_oauth_sessions, admin_bootstrap_setup, auth_security_settings,
    canvas_image_assets, content_moderation_actions, fog_masks, game_systems, light_sources,
    login_two_factor_challenges,
    oauth_authorization_sessions, oauth_link_challenges, oauth_providers, players_online, scene_state_fingerprints, scenes,
    shapes, tokens, user_oauth_accounts, user_sessions, users, walls, world_actor_claims,
    world_actor_inventory,
    world_abilities, world_ability_effects, world_ability_permissions, world_ability_shares,
    world_actor_abilities,
    world_actor_permissions, world_actor_shares, world_actor_system_data, world_actors,
    world_chat_messages, world_combatants, world_combats,
    world_events, world_genie_puzzle_clock_rewards, world_genie_puzzle_clocks,
    world_genie_resource_holdings, world_genie_shop_listings,
    world_genie_sessions, world_genie_trade_proposals, world_invites, world_item_effects,
    world_item_permissions, world_item_shares,
    world_items, world_lore_entries, world_lore_image_assets, world_lore_links,
    world_lore_permissions, world_lore_revisions, world_members, world_roll_records,
    world_tokens, worlds,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = admin_bootstrap_setup)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AdminBootstrapSetup {
    pub id: i32,
    pub setup_completed_at: Option<chrono::NaiveDateTime>,
    pub admin_code_hash: Option<String>,
    pub admin_code_generated_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = admin_bootstrap_setup)]
pub struct NewAdminBootstrapSetup {
    pub id: i32,
    pub setup_completed_at: Option<chrono::NaiveDateTime>,
    pub admin_code_hash: Option<String>,
    pub admin_code_generated_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = admin_bootstrap_oauth_sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AdminBootstrapOAuthSession {
    pub id: uuid::Uuid,
    pub provider_id: uuid::Uuid,
    pub oauth_provider_key: String,
    pub oauth_client_id: String,
    pub state: String,
    pub code_verifier: String,
    pub redirect_uri: String,
    pub desired_username: Option<String>,
    pub return_to: Option<String>,
    pub expires_at: chrono::NaiveDateTime,
    pub consumed_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = admin_bootstrap_oauth_sessions)]
pub struct NewAdminBootstrapOAuthSession {
    pub id: uuid::Uuid,
    pub provider_id: uuid::Uuid,
    pub oauth_provider_key: String,
    pub oauth_client_id: String,
    pub state: String,
    pub code_verifier: String,
    pub redirect_uri: String,
    pub desired_username: Option<String>,
    pub return_to: Option<String>,
    pub expires_at: chrono::NaiveDateTime,
    pub consumed_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = game_systems)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GameSystem {
    pub id: uuid::Uuid,
    pub slug: String,
    pub title: String,
    pub manifest_url: String,
    pub version: String,
    pub installed_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = game_systems)]
pub struct NewGameSystem {
    pub slug: String,
    pub title: String,
    pub manifest_url: String,
    pub version: String,
    pub installed_by: uuid::Uuid,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub is_admin: bool,
    pub password_hash: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub two_factor_enabled: bool,
    pub two_factor_secret_encrypted: Option<String>,
    pub two_factor_confirmed_at: Option<chrono::NaiveDateTime>,
    pub two_factor_admin_required: bool,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = auth_security_settings)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AuthSecuritySetting {
    pub id: i32,
    pub two_factor_required_for_all_users: bool,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = auth_security_settings)]
pub struct NewAuthSecuritySetting {
    pub id: i32,
    pub two_factor_required_for_all_users: bool,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = login_two_factor_challenges)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct LoginTwoFactorChallenge {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub expires_at: chrono::NaiveDateTime,
    pub consumed_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = login_two_factor_challenges)]
pub struct NewLoginTwoFactorChallenge {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub expires_at: chrono::NaiveDateTime,
    pub consumed_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = oauth_providers)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OAuthProvider {
    pub id: uuid::Uuid,
    pub provider_key: String,
    pub display_name: String,
    pub authorization_url: String,
    pub token_url: String,
    pub userinfo_url: Option<String>,
    pub scopes: Vec<Option<String>>,
    pub oauth_client_id: Option<String>,
    pub oauth_client_secret: Option<String>,
    pub configured: bool,
    pub enabled: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    /// `"admin"` or `"env"` — see ADR-041. `"env"` rows are materialized and
    /// kept in sync by the startup env-var scan; only `enabled` is writable
    /// on them through the admin GraphQL mutation.
    pub config_source: String,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = oauth_providers)]
pub struct NewOAuthProvider {
    pub id: uuid::Uuid,
    pub provider_key: String,
    pub display_name: String,
    pub authorization_url: String,
    pub token_url: String,
    pub userinfo_url: Option<String>,
    pub scopes: Vec<Option<String>>,
    pub oauth_client_id: Option<String>,
    pub oauth_client_secret: Option<String>,
    pub configured: bool,
    pub enabled: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub config_source: String,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = user_oauth_accounts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserOAuthAccount {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub provider_id: uuid::Uuid,
    pub provider_user_id: String,
    pub provider_email: Option<String>,
    pub access_token_encrypted: Option<String>,
    pub refresh_token_encrypted: Option<String>,
    pub token_expires_at: Option<chrono::NaiveDateTime>,
    pub linked_at: chrono::NaiveDateTime,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = user_sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserSession {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub expires_at: chrono::NaiveDateTime,
    pub revoked_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = user_sessions)]
pub struct NewUserSession {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub expires_at: chrono::NaiveDateTime,
    pub revoked_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = user_oauth_accounts)]
pub struct NewUserOAuthAccount {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub provider_id: uuid::Uuid,
    pub provider_user_id: String,
    pub provider_email: Option<String>,
    pub access_token_encrypted: Option<String>,
    pub refresh_token_encrypted: Option<String>,
    pub token_expires_at: Option<chrono::NaiveDateTime>,
    pub linked_at: chrono::NaiveDateTime,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = oauth_authorization_sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OAuthAuthorizationSession {
    pub id: uuid::Uuid,
    pub provider_id: uuid::Uuid,
    pub oauth_provider_key: String,
    pub oauth_client_id: String,
    pub state: String,
    pub code_verifier: String,
    pub redirect_uri: String,
    pub return_to: Option<String>,
    pub expires_at: chrono::NaiveDateTime,
    pub consumed_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = oauth_authorization_sessions)]
pub struct NewOAuthAuthorizationSession {
    pub id: uuid::Uuid,
    pub provider_id: uuid::Uuid,
    pub oauth_provider_key: String,
    pub oauth_client_id: String,
    pub state: String,
    pub code_verifier: String,
    pub redirect_uri: String,
    pub return_to: Option<String>,
    pub expires_at: chrono::NaiveDateTime,
    pub consumed_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = oauth_link_challenges)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OAuthLinkChallenge {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub provider_id: uuid::Uuid,
    pub provider_user_id: String,
    pub provider_email: Option<String>,
    pub challenge_code: String,
    pub expires_at: chrono::NaiveDateTime,
    pub consumed_at: Option<chrono::NaiveDateTime>,
    pub pending_access_token_encrypted: Option<String>,
    pub pending_refresh_token_encrypted: Option<String>,
    pub pending_token_expires_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = oauth_link_challenges)]
pub struct NewOAuthLinkChallenge {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub provider_id: uuid::Uuid,
    pub provider_user_id: String,
    pub provider_email: Option<String>,
    pub challenge_code: String,
    pub expires_at: chrono::NaiveDateTime,
    pub consumed_at: Option<chrono::NaiveDateTime>,
    pub pending_access_token_encrypted: Option<String>,
    pub pending_refresh_token_encrypted: Option<String>,
    pub pending_token_expires_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = worlds)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct World {
    pub id: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    pub game_system_id: Option<String>,
    pub interface_pack_id: Option<String>,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub session_notes: Option<String>,
    /// Spec 017 (FR-007): GM-controlled, defaults to false. Gates whether
    /// the Actor Selection screen offers "create your own character".
    pub allow_player_created_actors: bool,
    /// Spec 020 (FR-003, research.md R1): GM-controlled, defaults to
    /// false. When true, Genie Session Resource holdings carry over into
    /// the next session instead of resetting to 0.
    pub genie_resource_carryover_enabled: bool,
    /// Spec 022 (FR-014/FR-015): GM-controlled default grid type
    /// ("square" | "hex" | "gridless") applied to a newly created scene
    /// when its own `gridType` isn't explicitly set. Never retroactively
    /// changes existing scenes.
    pub default_scene_grid_type: String,
    /// Spec 022 (FR-002a/FR-002b, ADR-046): the world's server-authoritative
    /// "currently launched" scene for Play. `None` = nothing launched yet
    /// (Play shows an empty/unloaded canvas). Set only via `launchScene`.
    pub active_scene_id: Option<uuid::Uuid>,
}

// Policy struct disabled - table not implemented
// #[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
// #[diesel(table_name = policies)]
// #[diesel(check_for_backend(diesel::pg::Pg))]
// pub struct Policy {
//     pub id: uuid::Uuid,
//     pub effect: PolicyEffectEnum,
//     pub resources: Vec<Option<String>>,
//     pub world_id: Option<uuid::Uuid>,
//     pub created_by: uuid::Uuid,
//     pub updated_by: uuid::Uuid,
//     pub created_at: chrono::NaiveDateTime,
//     pub updated_at: chrono::NaiveDateTime,
// }

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_events)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorldEvent {
    pub id: i64,
    pub world_id: uuid::Uuid,
    pub event_code: i32,
    pub token_event: Option<serde_json::Value>,
    pub created_at: chrono::NaiveDateTime,
    pub schema_version: i32,
    pub updated_at: chrono::NaiveDateTime,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorldToken {
    pub id: String,
    pub world_id: uuid::Uuid,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub label: Option<String>,
    pub health: Option<i32>,
    pub max_health: Option<i32>,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
    pub schema_version: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_tokens)]
pub struct NewWorldToken {
    pub id: String,
    pub world_id: uuid::Uuid,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub label: Option<String>,
    pub health: Option<i32>,
    pub max_health: Option<i32>,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
    pub schema_version: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

// ========== Scene Models (Phase 3.5) ==========

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = scenes)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Scene {
    pub scene_id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub type_: String,
    pub grid_size: i32,
    pub grid_type: String,
    pub width: i32,
    pub height: i32,
    pub metadata: Option<serde_json::Value>,
    pub owner_id: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    /// Relative path under `state.directories.asset_directory`, servable
    /// at `/assets/<path>` (native canvas authoring: map import sets
    /// this; `NULL` = no background art). Superseded by
    /// `background_asset_id` (spec 002, FR-018) — kept until every
    /// existing row is backfilled, see the 0006 migration's comment.
    pub background_image_path: Option<String>,
    /// Spec 002 (FR-018): the `canvas_image_assets` row (kind =
    /// `Background`) backing this scene's background image via RustFS,
    /// replacing `background_image_path`'s bare filesystem path.
    pub background_asset_id: Option<uuid::Uuid>,
    /// Spec 022: GM-authored Markdown source for the scene's player-facing
    /// summary (distinct from `description`, which predates this feature
    /// and is treated as plain text elsewhere).
    pub summary_markdown: Option<String>,
    /// Spec 022: sanitized HTML rendered from `summary_markdown` via the
    /// same Markdown pipeline lore entries use (`crate::markdown`), kept in
    /// sync on every write — never rendered client-side.
    pub summary_rendered_html: Option<String>,
    /// Spec 022: player-facing visibility. Defaults to `true` (hidden) at
    /// creation per spec.md's Clarifications — a GM explicitly un-hides a
    /// scene once it's ready to be seen.
    pub hidden: bool,
    /// Spec 022: the `scene_preview_images` row backing this scene's
    /// reduced-size preview/thumbnail image, distinct from the
    /// full-resolution background used in Play. `None` until a background
    /// image has been set and a preview successfully generated.
    pub preview_asset_id: Option<uuid::Uuid>,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = scenes)]
pub struct NewScene {
    pub world_id: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub type_: String,
    pub grid_size: Option<i32>,
    pub grid_type: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub metadata: Option<serde_json::Value>,
    pub owner_id: uuid::Uuid,
}

#[derive(AsChangeset, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = scenes)]
pub struct SceneUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub grid_size: Option<i32>,
    pub grid_type: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub metadata: Option<serde_json::Value>,
    /// Spec 022: `Some(_)` writes both `summary_markdown` and its rendered
    /// HTML together (set by the mutation impl, not passed through
    /// directly) — `None` leaves the summary untouched, matching this
    /// struct's existing "`None` = don't touch column" convention.
    pub summary_markdown: Option<String>,
    pub summary_rendered_html: Option<String>,
    /// Spec 022: set only by `updateSceneHidden`'s impl, not by the
    /// general `updateScene` mutation.
    pub hidden: Option<bool>,
    /// Spec 022: set only by preview-generation code after a background
    /// image is (re)set, not exposed through any GraphQL input.
    pub preview_asset_id: Option<uuid::Uuid>,
}

// ========== Canvas Image Asset Models (Spec 002) ==========

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = canvas_image_assets)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CanvasImageAsset {
    pub asset_id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub scene_id: Option<uuid::Uuid>,
    pub owner_user_id: uuid::Uuid,
    pub storage_path: String,
    pub original_format: String,
    pub width_px: i32,
    pub height_px: i32,
    pub byte_size: i64,
    pub kind: crate::db_types::CanvasImageAssetKindEnum,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    /// Lowercase hex SHA-256 of the STORED WebP bytes (spec 028 FR-005).
    ///
    /// `None` on rows written before the fingerprint backfill reached them.
    /// Callers must read that as "the client must fetch this", never as
    /// "unchanged" — see `thunderforge_cache_core::delta::compute_plan`.
    pub content_hash: Option<String>,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = canvas_image_assets)]
pub struct NewCanvasImageAsset {
    pub asset_id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub scene_id: Option<uuid::Uuid>,
    pub owner_user_id: uuid::Uuid,
    pub storage_path: String,
    pub original_format: String,
    pub width_px: i32,
    pub height_px: i32,
    pub byte_size: i64,
    pub kind: crate::db_types::CanvasImageAssetKindEnum,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    /// Always `Some` on insert — the bytes are in hand at that moment, so
    /// there is no reason to write a row that immediately needs backfilling.
    pub content_hash: Option<String>,
}

// ========== Scene State Fingerprints (Spec 028) ==========

/// A scene's fingerprint over its canonical form.
///
/// Derived data, kept out of `scenes` because it is rewritten on every
/// scene-mutating event and should not contend with ordinary scene updates.
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = scene_state_fingerprints)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct SceneStateFingerprint {
    pub scene_id: uuid::Uuid,
    pub content_hash: String,
    /// The canonical-serialization version this hash was computed under.
    /// Stored so a format change invalidates old rows by comparison rather
    /// than needing a migration to wipe them.
    pub canonical_version: i32,
    pub computed_at: chrono::NaiveDateTime,
    pub updated_by: uuid::Uuid,
}

// ========== Wall Models (Phase 6) ==========

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = walls)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Wall {
    pub wall_id: uuid::Uuid,
    pub scene_id: uuid::Uuid,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub blocks_vision: bool,
    pub blocks_movement: bool,
    pub door_state: String,
    pub metadata: Option<serde_json::Value>,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = walls)]
pub struct NewWall {
    pub scene_id: uuid::Uuid,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub blocks_vision: bool,
    pub blocks_movement: bool,
    pub door_state: String,
    pub metadata: Option<serde_json::Value>,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
}

#[derive(AsChangeset, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = walls)]
pub struct WallUpdate {
    pub x1: Option<f64>,
    pub y1: Option<f64>,
    pub x2: Option<f64>,
    pub y2: Option<f64>,
    pub blocks_vision: Option<bool>,
    pub blocks_movement: Option<bool>,
    pub door_state: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub updated_by: uuid::Uuid,
}

// ========== LightSource Models (native canvas authoring) ==========

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = light_sources)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct LightSource {
    pub light_id: uuid::Uuid,
    pub scene_id: uuid::Uuid,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub intensity: f64,
    pub color: Option<String>,
    pub attached_token_id: Option<uuid::Uuid>,
    pub casts_shadows: bool,
    pub metadata: Option<serde_json::Value>,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = light_sources)]
pub struct NewLightSource {
    pub scene_id: uuid::Uuid,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub intensity: f64,
    pub color: Option<String>,
    pub attached_token_id: Option<uuid::Uuid>,
    pub casts_shadows: bool,
    pub metadata: Option<serde_json::Value>,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
}

#[derive(AsChangeset, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = light_sources)]
pub struct LightSourceUpdate {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub radius: Option<f64>,
    pub intensity: Option<f64>,
    pub color: Option<String>,
    pub attached_token_id: Option<uuid::Uuid>,
    pub casts_shadows: Option<bool>,
    pub metadata: Option<serde_json::Value>,
    pub updated_by: uuid::Uuid,
}

// ========== Shape Models (native canvas authoring) ==========

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = shapes)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Shape {
    pub shape_id: uuid::Uuid,
    pub scene_id: uuid::Uuid,
    pub kind: String,
    pub geometry: serde_json::Value,
    pub text: Option<String>,
    pub style: Option<serde_json::Value>,
    pub visible_to_players: bool,
    pub metadata: Option<serde_json::Value>,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = shapes)]
pub struct NewShape {
    pub scene_id: uuid::Uuid,
    pub kind: String,
    pub geometry: serde_json::Value,
    pub text: Option<String>,
    pub style: Option<serde_json::Value>,
    pub visible_to_players: bool,
    pub metadata: Option<serde_json::Value>,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
}

#[derive(AsChangeset, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = shapes)]
pub struct ShapeUpdate {
    pub geometry: Option<serde_json::Value>,
    pub text: Option<String>,
    pub style: Option<serde_json::Value>,
    pub visible_to_players: Option<bool>,
    pub metadata: Option<serde_json::Value>,
    pub updated_by: uuid::Uuid,
}

// ========== Token Models (Phase 3.5) ==========

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Token {
    pub token_id: uuid::Uuid,
    pub scene_id: uuid::Uuid,
    pub actor_id: Option<uuid::Uuid>,
    pub x: f64,
    pub y: f64,
    pub rotation: f64,
    pub scale: f64,
    pub metadata: Option<serde_json::Value>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub owner_user_id: Option<uuid::Uuid>,
    pub is_primary: bool,
    pub photo_url: Option<String>,
    pub health: Option<i32>,
    pub max_health: Option<i32>,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = tokens)]
pub struct NewToken {
    pub scene_id: uuid::Uuid,
    pub actor_id: Option<uuid::Uuid>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub rotation: Option<f64>,
    pub scale: Option<f64>,
    pub metadata: Option<serde_json::Value>,
    pub owner_user_id: Option<uuid::Uuid>,
    pub is_primary: Option<bool>,
    pub photo_url: Option<String>,
    pub health: Option<i32>,
    pub max_health: Option<i32>,
}

#[derive(AsChangeset, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = tokens)]
pub struct TokenUpdate {
    pub actor_id: Option<uuid::Uuid>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub rotation: Option<f64>,
    pub scale: Option<f64>,
    pub metadata: Option<serde_json::Value>,
    pub owner_user_id: Option<uuid::Uuid>,
    pub is_primary: Option<bool>,
    /// Doubly-optional so art can be *removed*, not only replaced.
    ///
    /// `AsChangeset` reads a plain `Option<T>` as "skip this column when
    /// `None`", which is what makes every other field here a partial
    /// update — and which left no way to express "set this column to
    /// NULL". Diesel reads the nested form as: outer `None` skips the
    /// column, `Some(None)` writes NULL, `Some(Some(v))` writes `v`.
    pub photo_url: Option<Option<String>>,
    pub health: Option<i32>,
    pub max_health: Option<i32>,
}

// ========== Fog Mask Models (Phase 3.5) ==========

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = fog_masks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct FogMask {
    pub fog_id: uuid::Uuid,
    pub scene_id: uuid::Uuid,
    #[serde(skip)]
    pub bitmap_data: Vec<u8>,
    pub version: i32,
    pub width: i32,
    pub height: i32,
    pub updated_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = fog_masks)]
pub struct NewFogMask {
    pub scene_id: uuid::Uuid,
    pub bitmap_data: Vec<u8>,
    pub version: i32,
    pub width: i32,
    pub height: i32,
    pub updated_by: uuid::Uuid,
}

#[derive(AsChangeset, Debug, Clone)]
#[diesel(table_name = fog_masks)]
pub struct FogMaskUpdate {
    pub bitmap_data: Option<Vec<u8>>,
    pub version: Option<i32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub updated_by: Option<uuid::Uuid>,
}

impl FogMask {
    /// Get bitmap data as base64 for transmission
    pub fn bitmap_data_base64(&self) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(&self.bitmap_data)
    }
}

impl NewFogMask {
    /// Create from base64 encoded bitmap
    pub fn from_base64(
        scene_id: uuid::Uuid,
        bitmap_data_base64: &str,
        width: i32,
        height: i32,
        updated_by: uuid::Uuid,
    ) -> Result<Self, base64::DecodeError> {
        use base64::Engine;
        let bitmap_data = base64::engine::general_purpose::STANDARD.decode(bitmap_data_base64)?;
        Ok(NewFogMask {
            scene_id,
            bitmap_data,
            version: 1,
            width,
            height,
            updated_by,
        })
    }
}

// ============================================================================
// Phase 4.8.1: System-Agnostic Actor Data Architecture
// ============================================================================

/// Universal actor registry - stores actor identity, ownership, location, type
/// Same schema for D&D 5e characters, Pathfinder NPCs, hazards, props, light sources
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_actors)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorldActor {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub scene_id: uuid::Uuid,
    pub actor_type: String, // 'character', 'npc', 'hazard', 'prop', 'light_source', 'vehicle'
    pub game_system_id: Option<String>, // NULL for non-game objects, 'dnd5e'/'pathfinder2e' for game systems
    pub label: String,
    pub created_by: uuid::Uuid,
    pub owned_by: uuid::Uuid,
    pub is_public: bool,
    pub is_npc: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub description: Option<String>,
    /// Spec 017: GM-set flag offering this (PC-only) actor to a joining
    /// player on the Actor Selection screen. Independent of claim state —
    /// see `world_actor_claims` for who currently has it claimed.
    pub available_for_claim: bool,
}

/// New actor for insertion
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_actors)]
pub struct NewWorldActor {
    pub world_id: uuid::Uuid,
    pub scene_id: uuid::Uuid,
    pub actor_type: String,
    pub game_system_id: Option<String>,
    pub label: String,
    pub created_by: uuid::Uuid,
    pub owned_by: uuid::Uuid,
    pub is_public: bool,
    pub is_npc: bool,
    pub description: Option<String>,
}

/// Spec 010: an actor's "ownership block" entry — one explicit
/// (actor, world member, permission level) grant. Absence of a row means
/// default Viewer access (see `auth::actor_permissions::require_actor_permission`).
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_actor_permissions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ActorPermission {
    pub id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub level: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// New actor permission for insertion/upsert.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_actor_permissions)]
pub struct NewActorPermission {
    pub id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub level: String,
}

/// Spec 010: a revocable, uncapped shareable link for one actor
/// (`createActorShareLink`/`revokeActorShareLink`/`sharedActor`).
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_actor_shares)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ActorShare {
    pub id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub share_code: String,
    pub created_by: uuid::Uuid,
    pub revoked: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// New actor share link for insertion.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_actor_shares)]
pub struct NewActorShare {
    pub id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub share_code: String,
    pub created_by: uuid::Uuid,
}

/// Spec 017: a claimed (PC-only) actor — one active claim per actor and
/// per world member, both enforced by `UNIQUE` constraints on this table
/// (see specs/017-invite-actor-selection/data-model.md, research.md §4).
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_actor_claims)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ActorClaim {
    pub id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub world_member_id: uuid::Uuid,
    pub claimed_at: chrono::DateTime<chrono::Utc>,
}

/// New actor claim for insertion.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_actor_claims)]
pub struct NewActorClaim {
    pub actor_id: uuid::Uuid,
    pub world_member_id: uuid::Uuid,
}

/// System-specific actor data - five semantic JSONB columns
/// Same column names for all systems, different JSON structure per system
/// Example: D&D 5e ability_data = { "strength": 10, "dexterity": 12, ... }
/// Example: Pathfinder ability_data = { "strength_mod": 0, "reflex_mod": 2, ... }
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_actor_system_data)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ActorSystemData {
    pub id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub game_system_id: String, // 'dnd5e', 'pathfinder2e', 'coc7e', etc.
    pub ability_data: Option<serde_json::Value>, // Base ability scores/modifiers
    pub resource_data: Option<serde_json::Value>, // HP, mana, sanity, focus, etc.
    pub proficiency_data: Option<serde_json::Value>, // Skills, weapon/armor proficiencies
    pub trait_data: Option<serde_json::Value>, // Class, subclass, feats, backgrounds
    pub spell_data: Option<serde_json::Value>, // Spellbook, slots, prepared spells
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// New actor system data for insertion
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_actor_system_data)]
pub struct NewActorSystemData {
    pub actor_id: uuid::Uuid,
    pub game_system_id: String,
    pub ability_data: Option<serde_json::Value>,
    pub resource_data: Option<serde_json::Value>,
    pub proficiency_data: Option<serde_json::Value>,
    pub trait_data: Option<serde_json::Value>,
    pub spell_data: Option<serde_json::Value>,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
}

/// Player presence tracking (Phase 4.9.B.1)
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = players_online)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PlayersOnline {
    pub id: i64,
    pub player_id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub scene_id: Option<uuid::Uuid>,
    pub connected_at: chrono::NaiveDateTime,
    pub last_seen: chrono::NaiveDateTime,
    pub idle_duration_secs: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// New player online record for insertion
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = players_online)]
pub struct NewPlayersOnline {
    pub player_id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub scene_id: Option<uuid::Uuid>,
    pub connected_at: chrono::NaiveDateTime,
    pub last_seen: chrono::NaiveDateTime,
    pub idle_duration_secs: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

// ========== Membership Models (Phase 4.10) ==========

// NOTE: WorldInvite models - table created via migration 2026-05-06-120000-0007

/// World invite code record from database
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_invites)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorldInvite {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub invite_code: String,
    pub max_uses: i32,
    pub used_count: i32,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub created_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    /// Spec 027 (FR-002): explicit retirement. Distinct from expiry and from
    /// exhaustion — a GM sets this to kill a leaked link outright.
    pub revoked: bool,
    /// Spec 027 (FR-003): the link this one replaced, when it was created by
    /// rotation. `None` on an original.
    pub rotated_from: Option<uuid::Uuid>,
}

/// New world invite for insertion
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_invites)]
pub struct NewWorldInvite {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub invite_code: String,
    pub max_uses: i32,
    pub used_count: i32,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub created_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    /// Spec 027: a freshly issued link is never born revoked. Present on the
    /// insert struct so a rotation cannot accidentally rely on the column
    /// default while setting `rotated_from` beside it.
    pub revoked: bool,
    /// Spec 027 (FR-003): set to the retired link's id when this row is a
    /// rotation replacement; `None` for a link created from scratch.
    pub rotated_from: Option<uuid::Uuid>,
}

// NOTE: WorldMember models - table created via migration 2026-05-06-120100-0008

/// World membership record from database
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_members)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorldMember {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub role: String,
    pub joined_at: chrono::NaiveDateTime,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// New world member for insertion
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_members)]
pub struct NewWorldMember {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub role: String,
    pub joined_at: chrono::NaiveDateTime,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

// ============================================================================
// Spec 012: World Lore Wiki
// ============================================================================

/// A world-scoped wiki page. `content`/`current_revision_id` are
/// denormalized copies of the latest `world_lore_revisions` row, kept in
/// sync on every save (data-model.md).
#[derive(Queryable, Selectable, Insertable, AsChangeset, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_lore_entries)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct LoreEntry {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub current_revision_id: Option<uuid::Uuid>,
    pub created_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// New lore entry for insertion.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_lore_entries)]
pub struct NewLoreEntry {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub created_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// An immutable snapshot of a lore entry's Markdown content at one point
/// in save time (FR-016). Never updated or deleted after insert.
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_lore_revisions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct LoreRevision {
    pub id: uuid::Uuid,
    pub lore_entry_id: uuid::Uuid,
    pub content_markdown: String,
    pub author_id: uuid::Uuid,
    pub restored_from_revision_id: Option<uuid::Uuid>,
    pub created_at: chrono::NaiveDateTime,
}

/// New lore revision for insertion.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_lore_revisions)]
pub struct NewLoreRevision {
    pub id: uuid::Uuid,
    pub lore_entry_id: uuid::Uuid,
    pub content_markdown: String,
    pub author_id: uuid::Uuid,
    pub restored_from_revision_id: Option<uuid::Uuid>,
    pub created_at: chrono::NaiveDateTime,
}

/// A lore entry's "ownership block" entry — one explicit (lore entry,
/// world member, permission level) grant. Absence of a row means default
/// Viewer access. Direct structural mirror of `ActorPermission` (spec
/// 010), generalized to lore entries (data-model.md).
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_lore_permissions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct LorePermission {
    pub id: uuid::Uuid,
    pub lore_entry_id: uuid::Uuid,
    pub world_member_user_id: uuid::Uuid,
    pub level: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// New lore permission for insertion/upsert.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_lore_permissions)]
pub struct NewLorePermission {
    pub id: uuid::Uuid,
    pub lore_entry_id: uuid::Uuid,
    pub world_member_user_id: uuid::Uuid,
    pub level: String,
}

/// A directional, in-text `[[...]]` reference from one lore entry's
/// content to another lore entry, an actor, or (spec 013 US3) an item.
/// At most one of `target_lore_entry_id`/`target_actor_id`/
/// `target_item_id` is set; a row whose target FK has gone NULL (target
/// deleted) is treated as unresolved by every read path regardless of
/// the stored `target_kind`.
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_lore_links)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct LoreLink {
    pub id: uuid::Uuid,
    pub source_lore_entry_id: uuid::Uuid,
    pub raw_title: String,
    pub target_kind: String,
    pub target_lore_entry_id: Option<uuid::Uuid>,
    pub target_actor_id: Option<uuid::Uuid>,
    pub created_at: chrono::NaiveDateTime,
    // Field order must match schema.rs: both target_item_id (spec 013) and
    // target_ability_id (spec 025) were added by ALTER TABLE, so diesel
    // appends them after created_at rather than beside their siblings.
    pub target_item_id: Option<uuid::Uuid>,
    pub target_ability_id: Option<uuid::Uuid>,
}

/// New lore link for insertion.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_lore_links)]
pub struct NewLoreLink {
    pub id: uuid::Uuid,
    pub source_lore_entry_id: uuid::Uuid,
    pub raw_title: String,
    pub target_kind: String,
    pub target_lore_entry_id: Option<uuid::Uuid>,
    pub target_actor_id: Option<uuid::Uuid>,
    pub target_item_id: Option<uuid::Uuid>,
    pub target_ability_id: Option<uuid::Uuid>,
}

/// An uploaded/pasted image attached to a lore entry (FR-008/009),
/// stored under a UUID-based RustFS object key (never the filename).
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_lore_image_assets)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct LoreImageAsset {
    pub id: uuid::Uuid,
    pub lore_entry_id: uuid::Uuid,
    pub uploaded_by: uuid::Uuid,
    pub original_filename: Option<String>,
    pub content_type: String,
    pub byte_size: i64,
    pub created_at: chrono::NaiveDateTime,
}

/// New lore image asset for insertion.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_lore_image_assets)]
pub struct NewLoreImageAsset {
    pub id: uuid::Uuid,
    pub lore_entry_id: uuid::Uuid,
    pub uploaded_by: uuid::Uuid,
    pub original_filename: Option<String>,
    pub content_type: String,
    pub byte_size: i64,
    pub created_at: chrono::NaiveDateTime,
}

// ============================================================================
// Spec 013: Items & Inventory
// ============================================================================

/// Spec 013: a world-scoped Item — mirrors `WorldActor`'s shape. Name is
/// deliberately NOT unique per world (FR-019); a `suggestItemName` query
/// nudges the DM with "did you mean?" instead of enforcing uniqueness.
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_items)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorldItem {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon_asset_id: Option<uuid::Uuid>,
    pub created_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// New item for insertion.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_items)]
pub struct NewWorldItem {
    pub world_id: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon_asset_id: Option<uuid::Uuid>,
    pub created_by: uuid::Uuid,
}

/// Spec 013: an item's ownership-block entry — direct structural mirror of
/// `ActorPermission` (spec 010), generalized to items.
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_item_permissions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ItemPermission {
    pub id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub level: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// New item permission for insertion/upsert.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_item_permissions)]
pub struct NewItemPermission {
    pub id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub level: String,
}

/// Spec 013: a structured, system-agnostic effect attached to an Item
/// (heal/damage/modifier/attack_roll). `trigger_kind` is scaffolded per
/// FR-004a but not evaluated by any code path in this pass.
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_item_effects)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ItemEffect {
    pub id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub effect_type: String,
    pub formula: String,
    pub target: String,
    pub trigger_kind: Option<String>,
    pub sort_order: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// New item effect for insertion.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_item_effects)]
pub struct NewItemEffect {
    pub item_id: uuid::Uuid,
    pub effect_type: String,
    pub formula: String,
    pub target: String,
    pub trigger_kind: Option<String>,
    pub sort_order: i32,
}

/// Spec 013: a revocable, uncapped shareable link for one Item — direct
/// structural mirror of `ActorShare` (spec 010).
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_item_shares)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ItemShare {
    pub id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub share_code: String,
    pub created_by: uuid::Uuid,
    pub revoked: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// New item share link for insertion.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_item_shares)]
pub struct NewItemShare {
    pub id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub share_code: String,
    pub created_by: uuid::Uuid,
}

/// Spec 013: one (Actor, Item, quantity) inventory row. `item_id` is
/// nullable — nulled via `ON DELETE SET NULL` when the referenced Item is
/// deleted (FR-017), with `item_name_snapshot` retained so the row can
/// still render "X (deleted item)" afterward.
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_actor_inventory)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ActorInventoryEntry {
    pub id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub item_id: Option<uuid::Uuid>,
    pub item_name_snapshot: String,
    pub quantity: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// New inventory entry for insertion.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_actor_inventory)]
pub struct NewActorInventoryEntry {
    pub actor_id: uuid::Uuid,
    pub item_id: Option<uuid::Uuid>,
    pub item_name_snapshot: String,
    pub quantity: i32,
}

// ============================================================================
// Spec 015: DMCA Notice-and-Takedown (content moderation)
// ============================================================================

/// Spec 015: one append-only event in a takedown case's lifecycle
/// (notice received, disabled, counter-notice forwarded, restored, etc.).
/// Deliberately has NO foreign keys to worlds/users/content tables —
/// FR-013 requires this history to survive deletion of any of them.
#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = content_moderation_actions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ContentModerationAction {
    pub id: uuid::Uuid,
    pub case_id: uuid::Uuid,
    pub action_type: String,
    pub entity_type: String,
    pub entity_id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub account_id: Option<uuid::Uuid>,
    pub claimant_name: String,
    pub claimant_contact: String,
    pub copyrighted_work_description: String,
    pub infringing_material_location: String,
    pub good_faith_statement: bool,
    pub accuracy_statement: bool,
    pub signature: String,
    pub validity_result: Option<String>,
    pub missing_elements: Option<Vec<Option<String>>>,
    pub counter_notice_id: Option<uuid::Uuid>,
    pub restoration_due_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub created_by: Option<uuid::Uuid>,
}

/// New moderation event for insertion.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = content_moderation_actions)]
pub struct NewContentModerationAction {
    pub case_id: uuid::Uuid,
    pub action_type: String,
    pub entity_type: String,
    pub entity_id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub account_id: Option<uuid::Uuid>,
    pub claimant_name: String,
    pub claimant_contact: String,
    pub copyrighted_work_description: String,
    pub infringing_material_location: String,
    pub good_faith_statement: bool,
    pub accuracy_statement: bool,
    pub signature: String,
    pub validity_result: Option<String>,
    pub missing_elements: Option<Vec<String>>,
    pub counter_notice_id: Option<uuid::Uuid>,
    pub restoration_due_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_by: Option<uuid::Uuid>,
}

// ============================================================================
// Spec 014: Dice Rolling Engine (world_roll_records)
// ============================================================================

/// Spec 014 (FR-014): one immutable, durable record of a resolved dice
/// roll. `detail` stores the full `thunderforge_dice::RollResolution`
/// (every `DieOutcome`) as JSON; `result_kind`/`result_value` are
/// denormalized out of it for cheap sorting/display in a history list.
#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_roll_records)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct RollRecord {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub triggered_by: uuid::Uuid,
    pub formula: String,
    pub bindings: Option<serde_json::Value>,
    pub detail: serde_json::Value,
    pub result_kind: String,
    pub result_value: f64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_roll_records)]
pub struct NewRollRecord {
    pub world_id: uuid::Uuid,
    pub triggered_by: uuid::Uuid,
    pub formula: String,
    pub bindings: Option<serde_json::Value>,
    pub detail: serde_json::Value,
    pub result_kind: String,
    pub result_value: f64,
}

// ============================================================================
// Spec 018 (User Story 7): Genie Session Loop — Session Wish Pool, Doom
// Clock, Puzzle Clocks, Session Resource holdings, and trade proposals.
// data-model.md "Session Wish Pool + Doom Clock", "world_genie_puzzle_clocks",
// "world_genie_resource_holdings".
// ============================================================================

// ============================================================================
// Play-view Chat + Combat (world_chat_messages, world_combats,
// world_combatants). Both ride the existing `world_events` bus rather than
// introducing a transport of their own — see `world_events.rs`'s
// EVENT_CODE_CHAT_MESSAGE / EVENT_CODE_COMBAT_CHANGED.
// ============================================================================

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_chat_messages)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ChatMessage {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub scene_id: Option<uuid::Uuid>,
    pub author_user_id: uuid::Uuid,
    pub author_label: String,
    pub body: String,
    pub gm_only: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_chat_messages)]
pub struct NewChatMessage {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub scene_id: Option<uuid::Uuid>,
    pub author_user_id: uuid::Uuid,
    pub author_label: String,
    pub body: String,
    pub gm_only: bool,
}

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_combats)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Combat {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub scene_id: Option<uuid::Uuid>,
    pub round: i32,
    pub active_combatant_id: Option<uuid::Uuid>,
    pub ended_at: Option<chrono::NaiveDateTime>,
    pub created_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_combats)]
pub struct NewCombat {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub scene_id: Option<uuid::Uuid>,
    pub created_by: uuid::Uuid,
}

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_combatants)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Combatant {
    pub id: uuid::Uuid,
    pub combat_id: uuid::Uuid,
    pub actor_id: Option<uuid::Uuid>,
    pub token_id: Option<uuid::Uuid>,
    pub label: String,
    pub initiative: i32,
    pub tiebreak: i32,
    pub is_npc: bool,
    pub active: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_combatants)]
pub struct NewCombatant {
    pub id: uuid::Uuid,
    pub combat_id: uuid::Uuid,
    pub actor_id: Option<uuid::Uuid>,
    pub token_id: Option<uuid::Uuid>,
    pub label: String,
    pub initiative: i32,
    pub tiebreak: i32,
    pub is_npc: bool,
}

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GenieSession {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub wishes_remaining: i32,
    pub doom_clock_current: i32,
    pub doom_clock_max: i32,
    pub status: String,
    pub created_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_sessions)]
pub struct NewGenieSession {
    pub world_id: uuid::Uuid,
    pub doom_clock_max: i32,
    pub created_by: uuid::Uuid,
}

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_puzzle_clocks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GeniePuzzleClock {
    pub id: uuid::Uuid,
    pub session_id: uuid::Uuid,
    pub label: String,
    pub segments_current: i32,
    pub segments_max: i32,
    pub resolved_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_puzzle_clocks)]
pub struct NewGeniePuzzleClock {
    pub session_id: uuid::Uuid,
    pub label: String,
    pub segments_max: i32,
}

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_resource_holdings)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GenieResourceHolding {
    pub id: uuid::Uuid,
    pub session_id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub resource_type: String,
    pub quantity: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_trade_proposals)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GenieTradeProposal {
    pub id: uuid::Uuid,
    pub session_id: uuid::Uuid,
    pub from_actor_id: uuid::Uuid,
    pub from_resource_type: String,
    pub from_quantity: i32,
    pub to_actor_id: uuid::Uuid,
    pub to_resource_type: String,
    pub to_quantity: i32,
    pub status: String,
    pub created_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_trade_proposals)]
pub struct NewGenieTradeProposal {
    pub session_id: uuid::Uuid,
    pub from_actor_id: uuid::Uuid,
    pub from_resource_type: String,
    pub from_quantity: i32,
    pub to_actor_id: uuid::Uuid,
    pub to_resource_type: String,
    pub to_quantity: i32,
    pub created_by: uuid::Uuid,
}

// ============================================================================
// Spec 020: Genie Session Resource Economy — NPC shop listings and
// configurable Puzzle Clock rewards. data-model.md
// "world_genie_shop_listings", "world_genie_puzzle_clock_rewards".
// ============================================================================

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_shop_listings)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GenieShopListing {
    pub id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub price_kind: String,
    pub price_resource_type: Option<String>,
    pub price_resource_amount: Option<i32>,
    pub price_item_id: Option<uuid::Uuid>,
    pub price_item_quantity: Option<i32>,
    pub created_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_shop_listings)]
pub struct NewGenieShopListing {
    pub actor_id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub price_kind: String,
    pub price_resource_type: Option<String>,
    pub price_resource_amount: Option<i32>,
    pub price_item_id: Option<uuid::Uuid>,
    pub price_item_quantity: Option<i32>,
    pub created_by: uuid::Uuid,
}

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_puzzle_clock_rewards)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GeniePuzzleClockReward {
    pub id: uuid::Uuid,
    pub clock_id: uuid::Uuid,
    pub trigger_segment: i32,
    pub reward_resource_type: Option<String>,
    pub reward_resource_amount: Option<i32>,
    pub reward_item_id: Option<uuid::Uuid>,
    pub reward_item_quantity: Option<i32>,
    pub recipient_mode: String,
    pub granted_at: Option<chrono::NaiveDateTime>,
    pub created_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_puzzle_clock_rewards)]
pub struct NewGeniePuzzleClockReward {
    pub clock_id: uuid::Uuid,
    pub trigger_segment: i32,
    pub reward_resource_type: Option<String>,
    pub reward_resource_amount: Option<i32>,
    pub reward_item_id: Option<uuid::Uuid>,
    pub reward_item_quantity: Option<i32>,
    pub recipient_mode: String,
    pub created_by: uuid::Uuid,
}

// ============================================================================
// Spec 025: World Abilities Compendium
// ============================================================================

/// Spec 025: a world-scoped Ability (spell / feat / power / talent). Mirrors
/// `WorldItem`'s shape with two deliberate differences:
///
/// * `updated_by` is present — `WorldItem` carries only `created_by`, but
///   spec 025 FR-027 requires both per Constitution Principle III.
/// * `gm_only` is the visibility control (FR-024a). It is deliberately NOT a
///   level in the ownership block: `ActorPermissionLevel`'s lowest value
///   (`Viewer`) is also its default for a member with no row, so the permission
///   model structurally cannot express "hidden". Mirrors `scenes.hidden`.
///
/// Name is NOT unique per world (FR-006); `suggest_ability_name` nudges with
/// "did you mean?" rather than enforcing uniqueness.
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_abilities)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorldAbility {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    pub classification: String,
    pub gm_only: bool,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// New ability for insertion. `id`/timestamps come from DB defaults.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_abilities)]
pub struct NewWorldAbility {
    pub world_id: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    pub classification: String,
    pub gm_only: bool,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
}

/// Spec 025: an ability's ownership-block entry — structural mirror of
/// `ItemPermission` (spec 013). Governs EDIT RIGHTS ONLY; absence of a row
/// means Viewer (read-only), never hidden — see `WorldAbility::gm_only`.
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_ability_permissions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AbilityPermission {
    pub id: uuid::Uuid,
    pub ability_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub level: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// New ability-permission row. Unlike `NewWorldAbility`, this carries `id`
/// explicitly — `world_ability_permissions.id` has no DB default, matching
/// `world_item_permissions`; callers supply `Uuid::now_v7()`.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_ability_permissions)]
pub struct NewAbilityPermission {
    pub id: uuid::Uuid,
    pub ability_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub level: String,
}

/// Spec 025 (FR-015): one authored effect on an ability. Structurally
/// identical to `ItemEffect` so a future resolution engine can consume both
/// through one path. Inert data — FR-019 forbids this spec from resolving,
/// rolling, or applying it.
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_ability_effects)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AbilityEffect {
    pub id: uuid::Uuid,
    pub ability_id: uuid::Uuid,
    pub effect_type: String,
    pub formula: String,
    pub target: String,
    /// Scaffolded per FR-020; evaluated by nothing in this pass.
    pub trigger_kind: Option<String>,
    pub sort_order: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// New ability effect for insertion. `id`/timestamps come from DB defaults.
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_ability_effects)]
pub struct NewAbilityEffect {
    pub ability_id: uuid::Uuid,
    pub effect_type: String,
    pub formula: String,
    pub target: String,
    pub trigger_kind: Option<String>,
    pub sort_order: i32,
}

/// Spec 025 (FR-021): one ability an actor knows.
///
/// `ability_id` is nullable and `ON DELETE SET NULL` — deleting an ability
/// never blocks on actors knowing it (FR-023). `ability_name_snapshot` keeps a
/// tombstoned row identifiable. No quantity: an actor either knows an ability
/// or does not.
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_actor_abilities)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ActorAbilityEntry {
    pub id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub ability_id: Option<uuid::Uuid>,
    pub ability_name_snapshot: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_actor_abilities)]
pub struct NewActorAbilityEntry {
    pub actor_id: uuid::Uuid,
    pub ability_id: Option<uuid::Uuid>,
    pub ability_name_snapshot: String,
}

/// Spec 025 (FR-032): a share link for one ability. `revoked` is a soft flag,
/// never a row delete — FR-036 needs a revoked link to render a distinct "no
/// longer available" state, which a deleted row could not distinguish from a
/// code that never existed.
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_ability_shares)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AbilityShare {
    pub id: uuid::Uuid,
    pub ability_id: uuid::Uuid,
    pub share_code: String,
    pub created_by: uuid::Uuid,
    pub revoked: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_ability_shares)]
pub struct NewAbilityShare {
    pub id: uuid::Uuid,
    pub ability_id: uuid::Uuid,
    pub share_code: String,
    pub created_by: uuid::Uuid,
}
