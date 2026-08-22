//! Adapters between Diesel models and Core shared models
//!
//! This layer handles bidirectional conversion between:
//! - Diesel ORM models (database-mapped)
//! - Core shared models (serializable, engine-friendly)
//!
//! Adapters ensure persistence layer details don't leak into business logic.

use crate::models::{
    OAuthProvider as DbOAuthProvider, User as DbUser, UserSession as DbUserSession,
    World as DbWorld, WorldEvent as DbWorldEvent, WorldToken as DbWorldToken,
};
use thunderforge_core::models::{
    auth::{OAuthProvider as CoreOAuthProvider, User as CoreUser, UserSession as CoreUserSession},
    world::{World as CoreWorld, WorldEvent as CoreWorldEvent, WorldToken as CoreWorldToken},
};

// NOTE: WorldToken adapters - table created via migration 2026-05-02-032300-0000

/// Convert Diesel WorldToken to Core WorldToken
impl From<DbWorldToken> for CoreWorldToken {
    fn from(db: DbWorldToken) -> Self {
        let mut token = CoreWorldToken {
            id: db.id,
            world_id: db.world_id,
            x: db.x,
            y: db.y,
            z: db.z,
            label: db.label,
            health: db.health,
            max_health: db.max_health,
            created_by: db.created_by,
            updated_by: db.updated_by,
            created_at: db.created_at,
            updated_at: db.updated_at,
            schema_version: db.schema_version,
            health_percentage: None,
            is_alive: true,
        };
        // Calculate derived data on conversion
        token.prepare_derived_data();
        token
    }
}

/// Convert Core WorldToken to Diesel WorldToken
impl From<CoreWorldToken> for DbWorldToken {
    fn from(core: CoreWorldToken) -> Self {
        DbWorldToken {
            id: core.id,
            world_id: core.world_id,
            x: core.x,
            y: core.y,
            z: core.z,
            label: core.label,
            health: core.health,
            max_health: core.max_health,
            created_by: core.created_by,
            updated_by: core.updated_by,
            schema_version: core.schema_version,
            created_at: core.created_at,
            updated_at: core.updated_at,
        }
    }
}

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
            scopes: db.scopes,
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
            scopes: core.scopes,
            oauth_client_id: None,
            oauth_client_secret: None,
            enabled: core.enabled,
            configured: core.configured,
            created_at: core.created_at,
            updated_at: core.updated_at,
            // Core's OAuthProvider predates the env-var config source concept
            // (ADR-041) and carries no such field — default to "admin" (the
            // safe, non-privileged default every pre-existing row already
            // has) rather than guessing "env" for a row this conversion path
            // didn't itself materialize from an env-var scan.
            config_source: "admin".to_string(),
        }
    }
}

/// Convert Diesel World to Core World
impl From<DbWorld> for CoreWorld {
    fn from(db: DbWorld) -> Self {
        CoreWorld {
            id: db.id,
            name: db.name,
            description: db.description,
            game_system_id: db.game_system_id,
            interface_pack_id: db.interface_pack_id,
            created_by: db.created_by,
            updated_by: db.updated_by,
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
            description: core.description,
            game_system_id: core.game_system_id,
            interface_pack_id: core.interface_pack_id,
            created_by: core.created_by,
            updated_by: core.updated_by,
            created_at: core.created_at,
            updated_at: core.updated_at,
            session_notes: None,
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
            created_by: db.created_by,
            updated_by: db.updated_by,
            created_at: db.created_at,
            updated_at: db.updated_at,
            schema_version: db.schema_version,
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
            created_by: core.created_by,
            updated_by: core.updated_by,
            created_at: core.created_at,
            updated_at: core.updated_at,
            schema_version: core.schema_version,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::{Timestamp, Uuid};

    #[test]
    fn test_user_roundtrip() {
        let user_id = Uuid::new_v7(Timestamp::now(uuid::NoContext));
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

// NOTE: WorldInvite adapters - convert Diesel ↔ Core models

/// Convert Diesel WorldInvite to Core WorldInvite
impl From<crate::models::WorldInvite> for thunderforge_core::models::invites::WorldInvite {
    fn from(db: crate::models::WorldInvite) -> Self {
        thunderforge_core::models::invites::WorldInvite {
            id: db.id,
            world_id: db.world_id,
            invite_code: db.invite_code,
            max_uses: db.max_uses,
            used_count: db.used_count,
            expires_at: db.expires_at,
            created_by: db.created_by,
            created_at: db.created_at,
            updated_at: db.updated_at,
        }
    }
}

/// Convert Core WorldInvite to Diesel WorldInvite
impl From<thunderforge_core::models::invites::WorldInvite> for crate::models::WorldInvite {
    fn from(core: thunderforge_core::models::invites::WorldInvite) -> Self {
        crate::models::WorldInvite {
            id: core.id,
            world_id: core.world_id,
            invite_code: core.invite_code,
            max_uses: core.max_uses,
            used_count: core.used_count,
            expires_at: core.expires_at,
            created_by: core.created_by,
            created_at: core.created_at,
            updated_at: core.updated_at,
        }
    }
}

// NOTE: WorldMember adapters - convert Diesel ↔ Core models

/// Convert Diesel WorldMember to Core WorldMembership
impl From<crate::models::WorldMember> for thunderforge_core::models::invites::WorldMembership {
    fn from(db: crate::models::WorldMember) -> Self {
        // Parse role string to enum
        let role = match db.role.as_str() {
            "Owner" => thunderforge_core::models::invites::WorldMemberRole::Owner,
            "GM" => thunderforge_core::models::invites::WorldMemberRole::GM,
            _ => thunderforge_core::models::invites::WorldMemberRole::Player, // Default to Player
        };

        thunderforge_core::models::invites::WorldMembership {
            id: db.id,
            world_id: db.world_id,
            user_id: db.user_id,
            role,
            joined_at: db.joined_at,
            created_at: db.created_at,
            updated_at: db.updated_at,
        }
    }
}

/// Convert Core WorldMembership to Diesel WorldMember
impl From<thunderforge_core::models::invites::WorldMembership> for crate::models::WorldMember {
    fn from(core: thunderforge_core::models::invites::WorldMembership) -> Self {
        // Convert role enum to string
        let role = match core.role {
            thunderforge_core::models::invites::WorldMemberRole::Owner => "Owner".to_string(),
            thunderforge_core::models::invites::WorldMemberRole::GM => "GM".to_string(),
            thunderforge_core::models::invites::WorldMemberRole::Player => "Player".to_string(),
        };

        crate::models::WorldMember {
            id: core.id,
            world_id: core.world_id,
            user_id: core.user_id,
            role,
            joined_at: core.joined_at,
            created_at: core.created_at,
            updated_at: core.updated_at,
        }
    }
}
