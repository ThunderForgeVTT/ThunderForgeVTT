use crate::schema::{
    admin_bootstrap_oauth_sessions, admin_bootstrap_setup, audit_logs, auth_security_settings, fog_masks,
    game_systems, login_two_factor_challenges, oauth_authorization_sessions, oauth_link_challenges,
    oauth_providers, permission_grants, policies, players_online, scenes, tokens, user_oauth_accounts, user_sessions,
    users, world_actors, world_actor_system_data, world_collaborators, world_events, world_tokens, worlds,
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
}

use crate::db_types::PolicyEffectEnum;

#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = policies)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Policy {
    pub id: uuid::Uuid,
    pub effect: PolicyEffectEnum,
    pub resources: Vec<Option<String>>,
    pub world_id: Option<uuid::Uuid>,
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

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
    pub actor_type: String,  // 'character', 'npc', 'hazard', 'prop', 'light_source', 'vehicle'
    pub game_system_id: Option<String>,  // NULL for non-game objects, 'dnd5e'/'pathfinder2e' for game systems
    pub label: String,
    pub created_by: uuid::Uuid,
    pub owned_by: uuid::Uuid,
    pub is_public: bool,
    pub is_npc: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
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
    pub game_system_id: String,  // 'dnd5e', 'pathfinder2e', 'coc7e', etc.
    pub ability_data: Option<serde_json::Value>,  // Base ability scores/modifiers
    pub resource_data: Option<serde_json::Value>,  // HP, mana, sanity, focus, etc.
    pub proficiency_data: Option<serde_json::Value>,  // Skills, weapon/armor proficiencies
    pub trait_data: Option<serde_json::Value>,  // Class, subclass, feats, backgrounds
    pub spell_data: Option<serde_json::Value>,  // Spellbook, slots, prepared spells
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

// ========== Audit Log Models (Security Audit Phase 1.2) ==========

/// Audit log entry for tracking sensitive operations
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = audit_logs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AuditLog {
    pub id: uuid::Uuid,
    pub event_type: String,
    pub actor_id: uuid::Uuid,
    pub resource_type: Option<String>,
    pub resource_id: Option<uuid::Uuid>,
    pub action: Option<String>,
    pub details: Option<serde_json::Value>,
    pub created_at: chrono::NaiveDateTime,
}

/// New audit log entry for insertion
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = audit_logs)]
pub struct NewAuditLog {
    pub id: uuid::Uuid,
    pub event_type: String,
    pub actor_id: uuid::Uuid,
    pub resource_type: Option<String>,
    pub resource_id: Option<uuid::Uuid>,
    pub action: Option<String>,
    pub details: Option<serde_json::Value>,
    pub created_at: chrono::NaiveDateTime,
}

// ========== RBAC Models (Security Audit Phase 2.1) ==========

/// World collaborator (user with access to a world)
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_collaborators)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorldCollaborator {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub role: String, // OWNER, EDITOR, VIEWER
    pub created_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// New world collaborator for insertion
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_collaborators)]
pub struct NewWorldCollaborator {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub role: String,
    pub created_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// Permission grant for a collaborator
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = permission_grants)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PermissionGrant {
    pub id: uuid::Uuid,
    pub collaborator_id: uuid::Uuid,
    pub permission: String, // view, edit, delete
    pub granted_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub expires_at: Option<chrono::NaiveDateTime>,
}

/// New permission grant for insertion
#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = permission_grants)]
pub struct NewPermissionGrant {
    pub id: uuid::Uuid,
    pub collaborator_id: uuid::Uuid,
    pub permission: String,
    pub granted_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub expires_at: Option<chrono::NaiveDateTime>,
}
