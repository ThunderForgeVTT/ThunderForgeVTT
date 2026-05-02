//! Adapters between Diesel models and Core shared models
//! 
//! This layer handles bidirectional conversion between:
//! - Diesel ORM models (database-mapped)
//! - Core shared models (serializable, engine-friendly)
//! 
//! Adapters ensure persistence layer details don't leak into business logic.

use thunderforge_core::models::{
    auth::{User as CoreUser, UserSession as CoreUserSession, OAuthProvider as CoreOAuthProvider},
    world::{World as CoreWorld, WorldEvent as CoreWorldEvent},
};
use crate::models::{User as DbUser, UserSession as DbUserSession, OAuthProvider as DbOAuthProvider, World as DbWorld, WorldEvent as DbWorldEvent};

// NOTE: WorldToken adapters pending - table created via migration 2026-05-02-032300-0000
// Will be uncommented after `diesel migration run` and schema.rs regeneration

/// Convert Diesel User to Core User
impl From<DbUser> for CoreUser {
    fn from(db: DbUser) -> Self {
        CoreUser {
            id: db.id,
            username: db.username,
            email: db.email,
            is_admin: db.is_admin,
            created_at: db.created_at,
            updated_at: db.updated_at,
        }
    }
}

/// Convert Core User back to Diesel User (for updates)
impl From<CoreUser> for DbUser {
    fn from(core: CoreUser) -> Self {
        DbUser {
            id: core.id,
            username: core.username,
            email: core.email,
            is_admin: core.is_admin,
            password_hash: String::new(), // Not exposed in core model (security)
            created_at: core.created_at,
            updated_at: core.updated_at,
            two_factor_enabled: false,
            two_factor_secret_encrypted: None,
            two_factor_confirmed_at: None,
            two_factor_admin_required: false,
        }
    }
}

/// Convert Diesel UserSession to Core UserSession
impl From<DbUserSession> for CoreUserSession {
    fn from(db: DbUserSession) -> Self {
        CoreUserSession {
            id: db.id,
            user_id: db.user_id,
            expires_at: db.expires_at,
            revoked_at: db.revoked_at,
            created_at: db.created_at,
        }
    }
}

/// Convert Core UserSession to Diesel UserSession
impl From<CoreUserSession> for DbUserSession {
    fn from(core: CoreUserSession) -> Self {
        DbUserSession {
            id: core.id,
            user_id: core.user_id,
            expires_at: core.expires_at,
            revoked_at: core.revoked_at,
            created_at: core.created_at,
        }
    }
}

/// Convert Diesel OAuthProvider to Core OAuthProvider
impl From<DbOAuthProvider> for CoreOAuthProvider {
    fn from(db: DbOAuthProvider) -> Self {
        CoreOAuthProvider {
            id: db.id,
            provider_key: db.provider_key,
            display_name: db.display_name,
            authorization_url: db.authorization_url,
            token_url: db.token_url,
            userinfo_url: db.userinfo_url,
            scopes: db.scopes.into_iter().map(Some).collect(),
            enabled: db.enabled,
            configured: db.configured,
            created_at: db.created_at,
            updated_at: db.updated_at,
        }
    }
}

/// Convert Core OAuthProvider to Diesel OAuthProvider
impl From<CoreOAuthProvider> for DbOAuthProvider {
    fn from(core: CoreOAuthProvider) -> Self {
        DbOAuthProvider {
            id: core.id,
            provider_key: core.provider_key,
            display_name: core.display_name,
            authorization_url: core.authorization_url,
            token_url: core.token_url,
            userinfo_url: core.userinfo_url,
            scopes: core.scopes.into_iter().flatten().collect(),
            oauth_client_id: None,
            oauth_client_secret: None,
            enabled: core.enabled,
            configured: core.configured,
            created_at: core.created_at,
            updated_at: core.updated_at,
        }
    }
}

/// Convert Diesel World to Core World
impl From<DbWorld> for CoreWorld {
    fn from(db: DbWorld) -> Self {
        CoreWorld {
            id: db.id,
            name: db.name,
            created_at: db.created_at,
            updated_at: db.updated_at,
        }
    }
}

/// Convert Core World to Diesel World
impl From<CoreWorld> for DbWorld {
    fn from(core: CoreWorld) -> Self {
        DbWorld {
            id: core.id,
            name: core.name,
            created_at: core.created_at,
            updated_at: core.updated_at,
        }
    }
}

/// Convert Diesel WorldEvent to Core WorldEvent
impl From<DbWorldEvent> for CoreWorldEvent {
    fn from(db: DbWorldEvent) -> Self {
        CoreWorldEvent {
            id: db.id,
            world_id: db.world_id,
            event_code: db.event_code,
            token_event: db.token_event,
            created_at: db.created_at,
            // NOTE: schema_version will be available after migration 2026-05-02-032400-0001
            schema_version: 1, // Default version until column is added
        }
    }
}

/// Convert Core WorldEvent to Diesel WorldEvent
impl From<CoreWorldEvent> for DbWorldEvent {
    fn from(core: CoreWorldEvent) -> Self {
        DbWorldEvent {
            id: core.id,
            world_id: core.world_id,
            event_code: core.event_code,
            token_event: core.token_event,
            created_at: core.created_at,
            // schema_version not yet in database - will be added via migration
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_user_roundtrip() {
        let user_id = Uuid::new_v7();
        let db_user = DbUser {
            id: user_id,
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            is_admin: false,
            password_hash: "hash".to_string(),
            created_at: chrono::Local::now().naive_local(),
            updated_at: chrono::Local::now().naive_local(),
            two_factor_enabled: false,
            two_factor_secret_encrypted: None,
            two_factor_confirmed_at: None,
            two_factor_admin_required: false,
        };

        let core_user: CoreUser = db_user.clone().into();
        assert_eq!(core_user.id, user_id);
        assert_eq!(core_user.username, "testuser");
        assert_eq!(core_user.email, "test@example.com");
    }
}
