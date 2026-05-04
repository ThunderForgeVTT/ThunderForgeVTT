use crate::schema::{
    admin_bootstrap_oauth_sessions, admin_bootstrap_setup, auth_security_settings,
    login_two_factor_challenges, oauth_authorization_sessions, oauth_link_challenges,
    oauth_providers, policies, user_oauth_accounts, user_sessions, users, world_events,
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
    pub created_by: uuid::Uuid,
    pub updated_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub schema_version: i32,
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
