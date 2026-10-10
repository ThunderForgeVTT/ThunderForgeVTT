//! Spec 081: the one rule for who sees a roll (research R2).
//!
//! The fetch, the feed, the subscription and the catch-up all ask this module,
//! so the rule cannot drift between them. Nothing here touches the database:
//! the caller says who is looking, and this says what they get.

use uuid::Uuid;

/// Who a roll is for (data-model.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    /// The whole table sees the whole roll.
    Everyone,
    /// A player's roll for the GM: the other players see that it happened,
    /// and nothing of what it was.
    GmEyes,
    /// The GM's roll behind the screen: the players do not see it at all.
    GmOnly,
}

impl Visibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Visibility::Everyone => "everyone",
            Visibility::GmEyes => "gm_eyes",
            Visibility::GmOnly => "gm_only",
        }
    }

    /// A stored value. Anything unrecognised reads as the most hidden, so a
    /// value from a later build can never make a roll more public.
    pub fn parse(value: &str) -> Visibility {
        match value {
            "everyone" => Visibility::Everyone,
            "gm_eyes" => Visibility::GmEyes,
            _ => Visibility::GmOnly,
        }
    }
}

/// The one asking.
#[derive(Debug, Clone, Copy)]
pub struct Viewer {
    pub user_id: Uuid,
    /// Runs the world: its owner or a GM.
    pub is_gm: bool,
    pub is_admin: bool,
}

impl Viewer {
    fn sees_behind_the_screen(&self) -> bool {
        self.is_gm || self.is_admin
    }
}

/// What a viewer gets of one roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollView {
    Whole,
    Masked,
    Hidden,
}

/// The facts about a roll the rule needs.
#[derive(Debug, Clone, Copy)]
pub struct RollFacts {
    pub roller: Uuid,
    pub visibility: Visibility,
    pub revealed: bool,
}

/// FR-007: a player may roll for the GM's eyes and the GM may roll for their
/// own; neither may borrow the other's. An admin who does not run the world
/// rolls as a player.
pub fn may_roll(visibility: Visibility, is_gm: bool) -> Result<(), &'static str> {
    match (visibility, is_gm) {
        (Visibility::Everyone, _) => Ok(()),
        (Visibility::GmEyes, false) => Ok(()),
        (Visibility::GmEyes, true) => Err("The GM rolls GM only, not for the GM's eyes"),
        (Visibility::GmOnly, true) => Ok(()),
        (Visibility::GmOnly, false) => Err("Only the GM can roll for their eyes only"),
    }
}

/// FR-003: the roller, the GM and an admin see the whole roll; another player
/// sees a GM's eyes roll masked and a GM only roll not at all, until it is
/// revealed.
pub fn view_of(roll: RollFacts, viewer: Viewer) -> RollView {
    if roll.revealed
        || roll.visibility == Visibility::Everyone
        || roll.roller == viewer.user_id
        || viewer.sees_behind_the_screen()
    {
        return RollView::Whole;
    }
    match roll.visibility {
        Visibility::GmEyes => RollView::Masked,
        _ => RollView::Hidden,
    }
}

/// FR-005a: whether a roll event of this visibility may be delivered to this
/// viewer at all. Only a GM only roll is withheld, and only from players; the
/// roller of one is the GM, who receives it.
pub fn event_reaches(visibility: Visibility, is_gm_or_admin: bool) -> bool {
    visibility != Visibility::GmOnly || is_gm_or_admin
}

/// Spec 088 FR-040: whether a roll made at `created_at` was cleared by the
/// GM's last clear at `rolls_cleared_at`. A cleared roll reaches no one, GM
/// included, so this is asked before any visibility rule. A roll made in the
/// same instant as the clear counts as cleared: the GM saw it when they chose.
pub fn cleared(
    created_at: chrono::DateTime<chrono::Utc>,
    rolls_cleared_at: Option<chrono::DateTime<chrono::Utc>>,
) -> bool {
    rolls_cleared_at.is_some_and(|cleared_at| created_at <= cleared_at)
}

#[cfg(test)]
#[path = "visibility_tests.rs"]
mod tests;
