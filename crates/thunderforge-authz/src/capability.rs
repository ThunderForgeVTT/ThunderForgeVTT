//! What each role may do, as one table instead of a hundred call sites.
//!
//! # The split this encodes
//!
//! Authority over a world divides in two, and the division is the whole
//! point of having three roles rather than two:
//!
//! - A **Game Master** carries every power over a world's *content* —
//!   scenes, tokens, walls, lights, fog. A co-GM invited to help run a
//!   campaign should be able to build the dungeon.
//! - Only an **Owner** may do the things that end or transfer the world
//!   itself. That same co-GM should not be able to delete the campaign.
//!
//! Every caller of an owner-only capability is a door that cannot be reopened
//! once somebody walks through it.
//!
//! # Why a matrix and not a boolean per call site
//!
//! The rules used to live as ad-hoc comparisons wherever they were needed,
//! and three of them were wrong in ways nobody could see by reading any single
//! function:
//!
//! - `deleteWorld` accepted **any world member**, so a Player who had merely
//!   accepted an invite could destroy the whole world.
//! - `createScene` had **no membership check at all** — any signed-in user
//!   could add a scene to any world by id.
//! - `updateFogMask` likewise, and reveal is the dangerous direction: an
//!   attacker could uncover a map the GM was deliberately withholding.
//!
//! None of those was a hard bug to write. Each was a place where somebody had
//! to remember a rule that was written down nowhere. Here the rule is written
//! down once, and [`tests::the_permission_matrix_is_stated_in_full`] prints
//! the whole thing, so adding a capability forces a decision rather than
//! quietly inheriting whatever the nearest call site happened to do.

use crate::role::Role;

/// One thing a person might try to do to a world.
///
/// Deliberately coarse. These are the *kinds* of authority that differ
/// between roles, not an entry per mutation — a list with one variant per
/// GraphQL field would be a list nobody keeps current.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Read the world: its scenes, its content, its members.
    ViewWorld,
    /// Create, change or remove the world's content — scenes, tokens, walls,
    /// lights, actors, items.
    EditContent,
    /// Change what the players can see of a map. Called out separately from
    /// [`Capability::EditContent`] because concealment is the point of fog:
    /// revealing it is not an ordinary content edit, and it was one of the
    /// three holes.
    ChangeFogOfWar,
    /// See scenes the GM has hidden, and other unrevealed prep.
    SeeHiddenContent,
    /// Invite people, remove them, change their roles.
    ManageMembers,
    /// Destroy the world and everything in it. Irreversible.
    DeleteWorld,
    /// Hand the world to somebody else. Irreversible for the current owner.
    TransferOwnership,
}

impl Capability {
    /// Every capability, so a test can enumerate the matrix exhaustively.
    pub const ALL: [Capability; 7] = [
        Capability::ViewWorld,
        Capability::EditContent,
        Capability::ChangeFogOfWar,
        Capability::SeeHiddenContent,
        Capability::ManageMembers,
        Capability::DeleteWorld,
        Capability::TransferOwnership,
    ];

    /// Whether this capability is one of the irreversible, owner-only ones.
    ///
    /// Useful to a caller that wants to be loud about a dangerous action;
    /// the authority decision itself is [`role_allows`].
    pub fn is_irreversible(self) -> bool {
        matches!(
            self,
            Capability::DeleteWorld | Capability::TransferOwnership
        )
    }
}

/// Whether a role, on its own, carries a capability.
///
/// The whole model, in one `match`. Written with every arm spelled out rather
/// than with a catch-all, so adding a [`Capability`] fails to compile until
/// somebody decides what each role may do with it — which is the only
/// mechanism here that actually prevents the class of bug this replaced.
pub fn role_allows(role: Role, capability: Capability) -> bool {
    match capability {
        // Everyone who is in the world at all can see it.
        Capability::ViewWorld => true,

        // Running the world means authority over its content.
        Capability::EditContent
        | Capability::ChangeFogOfWar
        | Capability::SeeHiddenContent
        | Capability::ManageMembers => role.runs_the_world(),

        // Owning the world means authority over its existence.
        Capability::DeleteWorld | Capability::TransferOwnership => role == Role::Owner,
    }
}

/// Who is asking.
///
/// `role` is `None` for somebody with no membership in this world at all —
/// including the case where the stored role string could not be parsed, which
/// must not grant anything (see [`crate::role`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Actor {
    pub role: Option<Role>,
    /// A site administrator, who is treated as the world's owner.
    ///
    /// This matches what the server already did and is stated here rather than
    /// re-derived per call site. It is a real grant of the irreversible
    /// capabilities, which is worth seeing in one place rather than
    /// discovering.
    pub is_site_admin: bool,
}

impl Actor {
    /// A member holding `role`.
    pub fn member(role: Role) -> Self {
        Self {
            role: Some(role),
            is_site_admin: false,
        }
    }

    /// Somebody with no membership in this world.
    pub fn stranger() -> Self {
        Self {
            role: None,
            is_site_admin: false,
        }
    }

    /// A site administrator.
    pub fn site_admin() -> Self {
        Self {
            role: None,
            is_site_admin: true,
        }
    }

    /// Whether this actor may do `capability` in the world the role came from.
    ///
    /// Fails closed: no role and no admin flag means no.
    pub fn may(self, capability: Capability) -> bool {
        if self.is_site_admin {
            return true;
        }
        self.role.is_some_and(|role| role_allows(role, capability))
    }

    /// Whether this actor runs the world — Owner, Game Master, or admin.
    ///
    /// The old code called this "is DM" and computed it as
    /// `role == "Owner" || role == "GM"` at each site.
    pub fn runs_the_world(self) -> bool {
        self.is_site_admin || self.role.is_some_and(Role::runs_the_world)
    }

    /// Whether this actor owns the world, as distinct from running it.
    pub fn owns_the_world(self) -> bool {
        self.is_site_admin || self.role == Some(Role::Owner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole model, asserted one cell at a time.
    ///
    /// Written out in full on purpose. A reader should be able to answer "may
    /// a Game Master delete a world" by looking here, and a change to any cell
    /// should be a visible diff on a line that says what it means — not an
    /// emergent consequence of an expression somewhere else.
    #[test]
    fn the_permission_matrix_is_stated_in_full() {
        use Capability::*;
        use Role::*;

        // (role, capability, allowed)
        let matrix = [
            (Player, ViewWorld, true),
            (Player, EditContent, false),
            (Player, ChangeFogOfWar, false),
            (Player, SeeHiddenContent, false),
            (Player, ManageMembers, false),
            (Player, DeleteWorld, false),
            (Player, TransferOwnership, false),
            (GameMaster, ViewWorld, true),
            (GameMaster, EditContent, true),
            (GameMaster, ChangeFogOfWar, true),
            (GameMaster, SeeHiddenContent, true),
            (GameMaster, ManageMembers, true),
            // The co-GM line: full authority over content, none over the
            // world's existence.
            (GameMaster, DeleteWorld, false),
            (GameMaster, TransferOwnership, false),
            (Owner, ViewWorld, true),
            (Owner, EditContent, true),
            (Owner, ChangeFogOfWar, true),
            (Owner, SeeHiddenContent, true),
            (Owner, ManageMembers, true),
            (Owner, DeleteWorld, true),
            (Owner, TransferOwnership, true),
        ];

        for (role, capability, expected) in matrix {
            assert_eq!(
                role_allows(role, capability),
                expected,
                "{role:?} × {capability:?}"
            );
        }

        // And the matrix above covers every combination that exists, so a new
        // role or capability cannot slip through untested.
        assert_eq!(
            matrix.len(),
            Role::ALL.len() * Capability::ALL.len(),
            "every role × capability pair must be stated above"
        );
    }

    /// The hole that mattered most: `deleteWorld` used to accept any member.
    #[test]
    fn only_the_owner_may_destroy_or_hand_over_the_world() {
        for capability in [Capability::DeleteWorld, Capability::TransferOwnership] {
            assert!(!Actor::member(Role::Player).may(capability));
            assert!(
                !Actor::member(Role::GameMaster).may(capability),
                "a co-GM builds the dungeon; they do not delete the campaign"
            );
            assert!(Actor::member(Role::Owner).may(capability));
        }
    }

    /// The other two holes: content edits reachable with no membership.
    #[test]
    fn a_stranger_to_the_world_may_do_nothing_at_all() {
        let stranger = Actor::stranger();
        for capability in Capability::ALL {
            assert!(
                !stranger.may(capability),
                "a non-member must not be able to {capability:?}"
            );
        }
    }

    /// An unparseable role string is a stranger, not a member.
    #[test]
    fn a_role_the_database_spelled_wrong_grants_nothing() {
        let actor = Actor {
            role: Role::from_stored("Gm"),
            is_site_admin: false,
        };
        assert_eq!(actor.role, None);
        for capability in Capability::ALL {
            assert!(!actor.may(capability));
        }
    }

    #[test]
    fn a_site_admin_is_treated_as_the_owner() {
        let admin = Actor::site_admin();
        for capability in Capability::ALL {
            assert!(
                admin.may(capability),
                "admin must be able to {capability:?}"
            );
        }
        assert!(admin.owns_the_world());
        assert!(admin.runs_the_world());
    }

    #[test]
    fn running_the_world_and_owning_it_are_different_questions() {
        let gm = Actor::member(Role::GameMaster);
        assert!(gm.runs_the_world());
        assert!(
            !gm.owns_the_world(),
            "the distinction the co-GM model rests on"
        );

        let player = Actor::member(Role::Player);
        assert!(!player.runs_the_world());
        assert!(!player.owns_the_world());
    }

    #[test]
    fn the_irreversible_capabilities_are_exactly_the_owner_only_ones() {
        for capability in Capability::ALL {
            assert_eq!(
                capability.is_irreversible(),
                !role_allows(Role::GameMaster, capability),
                "{capability:?}: irreversible and owner-only must be the same set"
            );
        }
    }
}
