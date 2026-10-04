//! Authentication and user models

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Core user representation (shared across engine, frontend, server)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub is_admin: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// Session token for authenticated connections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub expires_at: chrono::NaiveDateTime,
    pub revoked_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
}

impl UserSession {
    /// Check if session is still valid
    pub fn is_valid(&self) -> bool {
        self.revoked_at.is_none() && chrono::Local::now().naive_local() < self.expires_at
    }
}

/// OAuth provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProvider {
    pub id: Uuid,
    pub provider_key: String,
    pub display_name: String,
    pub authorization_url: String,
    pub token_url: String,
    pub userinfo_url: Option<String>,
    pub scopes: Vec<Option<String>>,
    pub enabled: bool,
    pub configured: bool,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// Two-factor authentication secret (encrypted in storage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoFactorSecret {
    pub user_id: Uuid,
    pub secret_encrypted: String,
    pub confirmed_at: Option<chrono::NaiveDateTime>,
}

/// Security settings (shared across all users)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSecuritySettings {
    pub id: i32,
    pub two_factor_required_for_all_users: bool,
    pub updated_at: chrono::NaiveDateTime,
}
