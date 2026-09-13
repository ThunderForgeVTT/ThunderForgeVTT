//! The four roles a person can hold in a world, as a type rather than a
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
//! # Why rank, and why the predicates are named
//!
//! ADR-099 put a Trusted Player between Player and Game Master. Rank is what
//! made that safe to do: every rule already written as "at least a Game
//! Master" went on excluding the new role without being touched. What rank
//! does not protect is a rule written as *equality* — "is a Player", or "is
//! not a Game Master, so must be a Player" — and those had to be found by
//! hand. So a rule is asked through a named predicate here
//! ([`Role::runs_the_world`], [`Role::manages_content`]) rather than by
//! comparing variants at the call site, and a fifth role will have one place
//! to be decided in.
//!
//! # Parsing fails closed
//!
//! An unrecognised role string resolves to `None`, and every decision treats
//! `None` as "no role at all". A row this code cannot understand must not be
//! a row that grants anything — the alternative is a typo in the database
//! becoming an authorization bypass.

/// A person's standing in one world.
///
/// Ordered deliberately, lowest first: `Player < TrustedPlayer < GameMaster <
/// Owner`, so rank comparisons read the way the model does. The order of the
/// variants below *is* the ranking, which is why they are not alphabetical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// Plays. Sees what the table shows them and changes only what is theirs.
    Player,
    /// A player the table trusts with its content (ADR-099): a world's book
    /// material, and adopting what players bring. Everywhere else a Player,
    /// and shown exactly what a Player is shown — being trusted with content
    /// is not a licence to see hidden tokens or fog.
    TrustedPlayer,
    /// Runs the world. Full authority over content; none over the world's
    /// existence.
    GameMaster,
    /// Holds the world. Everything a Game Master can do, plus the things that
    /// end or transfer the world itself.
    Owner,
}

impl Role {
    /// Every role, lowest first, for exhaustive iteration in tests and
    /// matrices.
    pub const ALL: [Role; 4] = [
        Role::Player,
        Role::TrustedPlayer,
        Role::GameMaster,
        Role::Owner,
    ];

    /// Parse the string stored in `world_members.role`.
    ///
    /// Returns `None` for anything unrecognised, which every caller must
    /// treat as "not a member". See the module note on failing closed.
    pub fn from_stored(stored: &str) -> Option<Role> {
        match stored {
            "Owner" => Some(Role::Owner),
            "GM" => Some(Role::GameMaster),
            "TrustedPlayer" => Some(Role::TrustedPlayer),
            "Player" => Some(Role::Player),
            _ => None,
        }
    }

    /// The string this role is stored as.
    ///
    /// The inverse of [`Role::from_stored`], and tested to round-trip — the
    /// two must never drift, because one writes the column the other reads.
    /// The column's `CHECK` constraint lists these same strings, so a new one
    /// here needs a migration there.
    pub fn as_stored(self) -> &'static str {
        match self {
            Role::Owner => "Owner",
            Role::GameMaster => "GM",
            Role::TrustedPlayer => "TrustedPlayer",
            Role::Player => "Player",
        }
    }

    /// Whether this role runs the world: Owner or Game Master.
    ///
    /// "DM" in the older call sites. Content authority, not world authority.
    /// A Trusted Player does not run the world, and every rule gated here
    /// goes on refusing them.
    pub fn runs_the_world(self) -> bool {
        self >= Role::GameMaster
    }

    /// Whether this role may manage a world's content material: Owner, Game
    /// Master or Trusted Player.
    ///
    /// Narrower than it sounds. It is the book list and a world's changes to
    /// what it inherited (spec 050 decision 8), and adopting what players
    /// bring (spec 048 decision 4) — not scenes, walls, fog or members, which
    /// stay behind [`Role::runs_the_world`].
    pub fn manages_content(self) -> bool {
        self >= Role::TrustedPlayer
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
        assert_eq!(Role::TrustedPlayer.as_stored(), "TrustedPlayer");
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
            "trustedplayer",
            "TRUSTEDPLAYER",
            "Trusted Player",
            "Trusted",
            "TrustedPlayer ",
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
    fn rank_runs_owner_over_game_master_over_trusted_player_over_player() {
        assert!(Role::Owner > Role::GameMaster);
        assert!(Role::GameMaster > Role::TrustedPlayer);
        assert!(Role::TrustedPlayer > Role::Player);
    }

    /// `ALL` is lowest first, and a test that iterates it to build a matrix
    /// relies on that. Sorting it must change nothing.
    #[test]
    fn every_role_is_listed_once_and_in_rank_order() {
        let mut sorted = Role::ALL;
        sorted.sort();
        assert_eq!(sorted, Role::ALL);
        let mut deduped = Role::ALL.to_vec();
        deduped.dedup();
        assert_eq!(deduped.len(), Role::ALL.len());
    }

    #[test]
    fn owners_and_game_masters_run_the_world_and_players_do_not() {
        assert!(Role::Owner.runs_the_world());
        assert!(Role::GameMaster.runs_the_world());
        assert!(
            !Role::TrustedPlayer.runs_the_world(),
            "a Trusted Player is trusted with content, not with the table"
        );
        assert!(!Role::Player.runs_the_world());
    }

    #[test]
    fn everyone_above_a_player_manages_content() {
        assert!(Role::Owner.manages_content());
        assert!(Role::GameMaster.manages_content());
        assert!(Role::TrustedPlayer.manages_content());
        assert!(!Role::Player.manages_content());
    }

    /// Running the world is the stronger of the two, so it must imply the
    /// weaker. If it ever did not, a Game Master would be refused the book
    /// list a Trusted Player may change.
    #[test]
    fn running_the_world_implies_managing_its_content() {
        for role in Role::ALL {
            if role.runs_the_world() {
                assert!(role.manages_content(), "{role:?}");
            }
        }
    }
}
