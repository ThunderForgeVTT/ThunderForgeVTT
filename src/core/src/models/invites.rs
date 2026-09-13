//! Campaign invitation and membership models for multiplayer gameplay

use serde::{Deserialize, Serialize};
use thunderforge_authz::Role;
use uuid::Uuid;

/// World membership roles, as the invite and membership models carry them.
///
/// A second enum beside [`thunderforge_authz::Role`], kept because this one is
/// serialised under these variant names and the other is not serialised at
/// all. What it must not be is a second *model*: every rule below asks the
/// authz role, through the conversions at the bottom of this block, so the
/// ranking is written down once. When ADR-099 added a role between Player and
/// Game Master, the hand-written `match` this replaced would have let a Game
/// Master manage a Trusted Player only by somebody remembering to add an arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum WorldMemberRole {
    /// Holds the world: invites, changes any role, removes anyone.
    Owner,
    /// Runs the world: invites, and manages members ranked below them.
    GM,
    /// Trusted with the table's content (ADR-099). Manages nobody.
    TrustedPlayer,
    /// Plays. Manages nobody.
    Player,
}

impl WorldMemberRole {
    fn rank(self) -> Role {
        self.into()
    }

    /// Whether this role may change or remove a member holding `target`.
    ///
    /// An Owner may manage anyone. A Game Master may manage whoever ranks
    /// below them — Trusted Players included, which is what lets a Game
    /// Master take the trust back — and not a fellow Game Master, because two
    /// people running a table should not be able to demote each other.
    pub fn can_manage(&self, target: WorldMemberRole) -> bool {
        match self.rank() {
            Role::Owner => true,
            caller if caller.runs_the_world() => target.rank() < caller,
            _ => false,
        }
    }

    /// Whether this role may give a member `new_role`.
    ///
    /// Nobody may hand out more than they hold. Without this a Game Master
    /// could make a Player an Owner, which is a transfer of the world by
    /// another name and is not theirs to make.
    pub fn can_assign(&self, new_role: WorldMemberRole) -> bool {
        self.can_change_roles() && new_role.rank() <= self.rank()
    }

    /// Check if this role can generate invite codes
    pub fn can_invite(&self) -> bool {
        self.rank().runs_the_world()
    }

    /// Check if this role can change member roles
    pub fn can_change_roles(&self) -> bool {
        self.rank().runs_the_world()
    }
}

impl std::fmt::Display for WorldMemberRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.rank().as_stored())
    }
}

impl std::str::FromStr for WorldMemberRole {
    type Err = String;

    /// The stored spelling, parsed by the authz crate so the two enums cannot
    /// disagree about what `world_members.role` contains.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Role::from_stored(s)
            .map(Into::into)
            .ok_or_else(|| format!("Invalid role: {}", s))
    }
}

impl From<WorldMemberRole> for Role {
    fn from(role: WorldMemberRole) -> Self {
        match role {
            WorldMemberRole::Owner => Role::Owner,
            WorldMemberRole::GM => Role::GameMaster,
            WorldMemberRole::TrustedPlayer => Role::TrustedPlayer,
            WorldMemberRole::Player => Role::Player,
        }
    }
}

impl From<Role> for WorldMemberRole {
    fn from(role: Role) -> Self {
        match role {
            Role::Owner => WorldMemberRole::Owner,
            Role::GameMaster => WorldMemberRole::GM,
            Role::TrustedPlayer => WorldMemberRole::TrustedPlayer,
            Role::Player => WorldMemberRole::Player,
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
            && now > expires
        {
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
        assert!(WorldMemberRole::Owner.can_manage(WorldMemberRole::TrustedPlayer));
        assert!(WorldMemberRole::Owner.can_manage(WorldMemberRole::Player));

        // GM manages whoever ranks below them
        assert!(!WorldMemberRole::GM.can_manage(WorldMemberRole::Owner));
        assert!(!WorldMemberRole::GM.can_manage(WorldMemberRole::GM));
        assert!(WorldMemberRole::GM.can_manage(WorldMemberRole::TrustedPlayer));
        assert!(WorldMemberRole::GM.can_manage(WorldMemberRole::Player));

        // Neither kind of player manages anyone
        for caller in [WorldMemberRole::TrustedPlayer, WorldMemberRole::Player] {
            for target in ALL {
                assert!(!caller.can_manage(target), "{caller:?} -> {target:?}");
            }
        }
    }

    #[test]
    fn test_invite_permissions() {
        assert!(WorldMemberRole::Owner.can_invite());
        assert!(WorldMemberRole::GM.can_invite());
        assert!(!WorldMemberRole::TrustedPlayer.can_invite());
        assert!(!WorldMemberRole::Player.can_invite());
    }

    const ALL: [WorldMemberRole; 4] = [
        WorldMemberRole::Owner,
        WorldMemberRole::GM,
        WorldMemberRole::TrustedPlayer,
        WorldMemberRole::Player,
    ];

    /// ADR-099: only an Owner or a Game Master makes somebody a Trusted
    /// Player, and nobody hands out more than they hold.
    #[test]
    fn only_those_who_run_the_world_assign_roles_and_never_above_their_own() {
        use WorldMemberRole::*;
        let expected = [
            (Owner, [true, true, true, true]),
            (GM, [false, true, true, true]),
            (TrustedPlayer, [false, false, false, false]),
            (Player, [false, false, false, false]),
        ];
        for (caller, answers) in expected {
            for (new_role, answer) in ALL.into_iter().zip(answers) {
                assert_eq!(
                    caller.can_assign(new_role),
                    answer,
                    "{caller:?} assigning {new_role:?}"
                );
            }
        }
    }

    /// The two enums, both ways, and through the stored string. Each
    /// conversion is an exhaustive `match`, so a new variant fails to compile
    /// until it is placed; this is what catches it being placed wrongly.
    #[test]
    fn both_role_enums_round_trip_through_each_other_and_the_column() {
        for role in Role::ALL {
            let member: WorldMemberRole = role.into();
            assert_eq!(Role::from(member), role);
            assert_eq!(member.to_string(), role.as_stored());
            assert_eq!(role.as_stored().parse::<WorldMemberRole>(), Ok(member));
        }
        assert_eq!(ALL.len(), Role::ALL.len());
        assert_eq!(
            serde_json::to_string(&WorldMemberRole::TrustedPlayer).unwrap(),
            "\"TrustedPlayer\"",
            "the serialised name must be the stored one"
        );
    }

    #[test]
    fn an_unrecognised_role_string_is_refused_rather_than_defaulted() {
        for wrong in ["trustedplayer", "Trusted Player", "gm", ""] {
            assert!(wrong.parse::<WorldMemberRole>().is_err(), "{wrong:?}");
        }
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
