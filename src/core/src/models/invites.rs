//! Campaign invitation and membership models for multiplayer gameplay

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// World membership roles with permission hierarchy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum WorldMemberRole {
    /// Full control: invite players, change roles, delete world, delete members
    Owner,
    /// Game master: invite players, change roles for Player-level members, manage scene content
    GM,
    /// Regular player: can join world and interact with scenes
    Player,
}

impl WorldMemberRole {
    /// Check if this role can perform an action on a target role
    pub fn can_manage(&self, target: WorldMemberRole) -> bool {
        match (self, target) {
            // Owners can manage anyone
            (WorldMemberRole::Owner, _) => true,
            // GMs can manage Players but not Owners or other GMs
            (WorldMemberRole::GM, WorldMemberRole::Player) => true,
            // Players cannot manage anyone
            (WorldMemberRole::Player, _) => false,
            _ => false,
        }
    }

    /// Check if this role can generate invite codes
    pub fn can_invite(&self) -> bool {
        matches!(self, WorldMemberRole::Owner | WorldMemberRole::GM)
    }

    /// Check if this role can change member roles
    pub fn can_change_roles(&self) -> bool {
        matches!(self, WorldMemberRole::Owner | WorldMemberRole::GM)
    }
}

impl std::fmt::Display for WorldMemberRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorldMemberRole::Owner => write!(f, "Owner"),
            WorldMemberRole::GM => write!(f, "GM"),
            WorldMemberRole::Player => write!(f, "Player"),
        }
    }
}

impl std::str::FromStr for WorldMemberRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Owner" => Ok(WorldMemberRole::Owner),
            "GM" => Ok(WorldMemberRole::GM),
            "Player" => Ok(WorldMemberRole::Player),
            _ => Err(format!("Invalid role: {}", s)),
        }
    }
}

/// An invite code for joining a world campaign
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldInvite {
    /// Unique identifier for this invite
    pub id: Uuid,

    /// World this invite belongs to
    pub world_id: Uuid,

    /// URL-safe invite code (e.g., "ABCD1234")
    pub invite_code: String,

    /// Maximum number of times this invite can be used (0 = unlimited)
    pub max_uses: i32,

    /// Current usage count
    pub used_count: i32,

    /// When this invite expires (None = never)
    pub expires_at: Option<chrono::NaiveDateTime>,

    /// User who created this invite
    pub created_by: Uuid,

    /// Creation timestamp
    pub created_at: chrono::NaiveDateTime,

    /// Last update timestamp
    pub updated_at: chrono::NaiveDateTime,

    /// Spec 027 (FR-002): explicitly retired by a GM. Independent of expiry
    /// and of the use cap — this is how a leaked link is killed.
    #[serde(default)]
    pub revoked: bool,
}

impl WorldInvite {
    /// Check if this invite code is still valid.
    ///
    /// # This is not the authorization check
    ///
    /// Spec 027: the authoritative validity test lives in the SQL predicate of
    /// the atomic consume in `join_world_impl`, which evaluates the same three
    /// conditions inside the `UPDATE` that increments `used_count`. Checking
    /// here and writing afterwards is precisely the read-validate-write
    /// sequence that let two concurrent joins claim one remaining use.
    ///
    /// Keep this method for display and for callers reasoning about a link
    /// they already hold — not to gate access.
    pub fn is_valid(&self) -> bool {
        let now = chrono::Utc::now().naive_utc();

        // Spec 027 (FR-002): explicit revocation outranks everything else. A
        // revoked link is dead regardless of remaining uses or expiry.
        if self.revoked {
            return false;
        }

        // Check expiry
        if let Some(expires) = self.expires_at
            && now > expires {
            return false;
        }

        // Check max uses.
        //
        // `max_uses == 0` means unlimited. Note that no API path creates such
        // a row today — `generate_invite_code_impl` rejects `max_uses <= 0` —
        // so this branch is unreachable in practice. It is preserved
        // deliberately: removing it would change behaviour for any row that
        // does have one, which is outside spec 027's scope (research §7).
        if self.max_uses > 0 && self.used_count >= self.max_uses {
            return false;
        }

        true
    }

    /// Increment usage count (if max_uses allows)
    pub fn use_invite(&mut self) -> Result<(), String> {
        if !self.is_valid() {
            return Err("Invite code is no longer valid".to_string());
        }

        if self.max_uses > 0 && self.used_count >= self.max_uses {
            return Err("Invite code has reached max uses".to_string());
        }

        self.used_count += 1;
        Ok(())
    }

    /// Human-readable status of this invite
    pub fn status(&self) -> String {
        if let Some(expires) = self.expires_at {
            let now = chrono::Utc::now().naive_utc();
            if now > expires {
                return format!("Expired ({})", expires.format("%Y-%m-%d"));
            }
        }

        if self.max_uses > 0 {
            format!("{}/{} uses", self.used_count, self.max_uses)
        } else {
            "Unlimited uses".to_string()
        }
    }
}

/// Membership record: tracks which users belong to which worlds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldMembership {
    /// Unique identifier for this membership
    pub id: Uuid,

    /// World this user belongs to
    pub world_id: Uuid,

    /// User in this world
    pub user_id: Uuid,

    /// User's role and permissions in this world
    pub role: WorldMemberRole,

    /// When the user joined
    pub joined_at: chrono::NaiveDateTime,

    /// Creation timestamp
    pub created_at: chrono::NaiveDateTime,

    /// Last update timestamp (when role changed, etc.)
    pub updated_at: chrono::NaiveDateTime,
}

impl WorldMembership {
    /// Check if this member can invite other players
    pub fn can_invite(&self) -> bool {
        self.role.can_invite()
    }

    /// Check if this member can change another member's role
    pub fn can_manage_member(&self, other_role: WorldMemberRole) -> bool {
        self.role.can_manage(other_role)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_hierarchy() {
        // Owner can manage anyone
        assert!(WorldMemberRole::Owner.can_manage(WorldMemberRole::GM));
        assert!(WorldMemberRole::Owner.can_manage(WorldMemberRole::Player));

        // GM can only manage Players
        assert!(!WorldMemberRole::GM.can_manage(WorldMemberRole::Owner));
        assert!(!WorldMemberRole::GM.can_manage(WorldMemberRole::GM));
        assert!(WorldMemberRole::GM.can_manage(WorldMemberRole::Player));

        // Player can't manage anyone
        assert!(!WorldMemberRole::Player.can_manage(WorldMemberRole::Owner));
        assert!(!WorldMemberRole::Player.can_manage(WorldMemberRole::GM));
        assert!(!WorldMemberRole::Player.can_manage(WorldMemberRole::Player));
    }

    #[test]
    fn test_invite_permissions() {
        assert!(WorldMemberRole::Owner.can_invite());
        assert!(WorldMemberRole::GM.can_invite());
        assert!(!WorldMemberRole::Player.can_invite());
    }

    #[test]
    fn test_invite_validity() {
        let future = chrono::Utc::now().naive_utc() + chrono::Duration::hours(1);
        let past = chrono::Utc::now().naive_utc() - chrono::Duration::hours(1);

        let mut invite = WorldInvite {
            id: Uuid::new_v4(),
            world_id: Uuid::new_v4(),
            invite_code: "TEST1234".to_string(),
            max_uses: 5,
            used_count: 3,
            expires_at: Some(future),
            created_by: Uuid::new_v4(),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
            revoked: false,
        };

        assert!(invite.is_valid());

        // Test expiry
        invite.expires_at = Some(past);
        assert!(!invite.is_valid());

        // Test max uses
        invite.expires_at = Some(future);
        invite.used_count = 5;
        assert!(!invite.is_valid());

        // Spec 027 (FR-002): revocation invalidates a link that is otherwise
        // perfectly usable — unexpired, with uses to spare.
        invite.used_count = 0;
        assert!(invite.is_valid());
        invite.revoked = true;
        assert!(
            !invite.is_valid(),
            "a revoked link must be invalid regardless of expiry or remaining uses"
        );
    }

    #[test]
    fn test_use_invite() {
        let mut invite = WorldInvite {
            id: Uuid::new_v4(),
            world_id: Uuid::new_v4(),
            invite_code: "TEST1234".to_string(),
            max_uses: 2,
            used_count: 0,
            expires_at: None,
            created_by: Uuid::new_v4(),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
            revoked: false,
        };

        assert!(invite.use_invite().is_ok());
        assert_eq!(invite.used_count, 1);

        assert!(invite.use_invite().is_ok());
        assert_eq!(invite.used_count, 2);

        // Should fail on third attempt (max_uses = 2)
        assert!(invite.use_invite().is_err());
    }
}
