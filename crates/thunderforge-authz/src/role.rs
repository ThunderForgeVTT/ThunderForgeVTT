//! The three roles a person can hold in a world, as a type rather than a
//! string.
//!
//! # Why this exists
//!
//! `world_members.role` is a `TEXT` column, and the server compared it by
//! hand at every decision: `role == "Owner"`, `role == "Owner" || role ==
//! "GM"`. Two things go wrong with that, and both are silent.
//!
//! The first is spelling. `"gm"`, `"Gm"` and `"GameMaster"` all compare
//! unequal to `"GM"`, so a mistake in a migration, a seed script or a future
//! call site does not fail — it quietly denies a Game Master their powers, or
//! quietly stops denying a Player theirs, depending on which side of the
//! comparison it lands. Nothing type-checks a string against a column.
//!
//! The second is that a bare string carries no notion of *rank*. Every call
//! site had to re-derive "Owner outranks GM outranks Player" as a boolean
//! expression, which is how the same rule ended up written several different
//! ways, and how one of them ended up wrong.
//!
//! # Parsing fails closed
//!
//! An unrecognised role string resolves to `None`, and every decision treats
//! `None` as "no role at all". A row this code cannot understand must not be
//! a row that grants anything — the alternative is a typo in the database
//! becoming an authorization bypass.

/// A person's standing in one world.
///
/// Ordered deliberately: `Owner > GameMaster > Player`, so rank comparisons
/// read the way the model does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// Holds the world. Everything a Game Master can do, plus the things that
    /// end or transfer the world itself.
    Player,
    /// Runs the world. Full authority over content; none over the world's
    /// existence.
    GameMaster,
    /// The world's owner.
    Owner,
}

impl Role {
    /// Every role, for exhaustive iteration in tests and matrices.
    pub const ALL: [Role; 3] = [Role::Player, Role::GameMaster, Role::Owner];

    /// Parse the string stored in `world_members.role`.
    ///
    /// Returns `None` for anything unrecognised, which every caller must
    /// treat as "not a member". See the module note on failing closed.
    pub fn from_stored(stored: &str) -> Option<Role> {
        match stored {
            "Owner" => Some(Role::Owner),
            "GM" => Some(Role::GameMaster),
            "Player" => Some(Role::Player),
            _ => None,
        }
    }

    /// The string this role is stored as.
    ///
    /// The inverse of [`Role::from_stored`], and tested to round-trip — the
    /// two must never drift, because one writes the column the other reads.
    pub fn as_stored(self) -> &'static str {
        match self {
            Role::Owner => "Owner",
            Role::GameMaster => "GM",
            Role::Player => "Player",
        }
    }

    /// Whether this role runs the world: Owner or Game Master.
    ///
    /// "DM" in the older call sites. Content authority, not world authority.
    pub fn runs_the_world(self) -> bool {
        self >= Role::GameMaster
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_role_round_trips_through_the_stored_string() {
        for role in Role::ALL {
            assert_eq!(
                Role::from_stored(role.as_stored()),
                Some(role),
                "{role:?} must survive a trip through the database column"
            );
        }
    }

    /// The exact strings the database holds. If a migration changes one of
    /// these, this test is the thing that notices.
    #[test]
    fn the_stored_spellings_are_the_ones_the_column_actually_contains() {
        assert_eq!(Role::Owner.as_stored(), "Owner");
        assert_eq!(Role::GameMaster.as_stored(), "GM");
        assert_eq!(Role::Player.as_stored(), "Player");
    }

    /// A role string this code does not understand grants nothing.
    ///
    /// The cases below are not hypothetical: they are the spellings a human
    /// writing a migration or a seed script would plausibly produce.
    #[test]
    fn an_unrecognised_role_is_nobody_rather_than_somebody() {
        for wrong in [
            "gm",
            "Gm",
            "GameMaster",
            "owner",
            "OWNER",
            "player",
            "Admin",
            "",
            " Owner",
            "Owner ",
        ] {
            assert_eq!(
                Role::from_stored(wrong),
                None,
                "{wrong:?} must not resolve to a role"
            );
        }
    }

    #[test]
    fn rank_runs_owner_over_game_master_over_player() {
        assert!(Role::Owner > Role::GameMaster);
        assert!(Role::GameMaster > Role::Player);
    }

    #[test]
    fn owners_and_game_masters_run_the_world_and_players_do_not() {
        assert!(Role::Owner.runs_the_world());
        assert!(Role::GameMaster.runs_the_world());
        assert!(!Role::Player.runs_the_world());
    }
}
