//! Whose turn it is, asked before a player acts (spec 046 research R12, C1).
//!
//! The server's `combat::turn::turn_check` finds the running combat, the
//! acting token's actor and the combatants, and redacts a hidden label; the
//! decision between those is this. A Game Master is never asked.

/// What the turn check reads of a combatant.
pub trait Party {
    type Id: PartialEq + Copy;
    fn party_id(&self) -> Self::Id;
    fn token(&self) -> Option<Self::Id>;
    fn actor(&self) -> Option<Self::Id>;
}

/// The combatant whose turn holds the acting token back, or `None` when it
/// may act.
///
/// Permitted when the token is not in the fight (no combatant is it — by
/// token, or by actor for one added with no token), when one of its own
/// combatants holds the turn, or when the turn names a combatant that is gone.
pub fn held_by<P: Party>(
    combatants: &[P],
    active_id: P::Id,
    acting_token: P::Id,
    acting_actor: Option<P::Id>,
) -> Option<&P> {
    let is_acting = |c: &P| match c.token() {
        Some(token) => token == acting_token,
        None => acting_actor.is_some() && c.actor() == acting_actor,
    };
    let mut mine = combatants.iter().filter(|c| is_acting(c)).peekable();
    if mine.peek().is_none() || mine.any(|c| c.party_id() == active_id) {
        return None;
    }
    combatants.iter().find(|c| c.party_id() == active_id)
}

/// "It is <label>'s turn".
pub fn turn_refusal(label: &str) -> String {
    format!("It is {label}'s turn")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct C(u8, Option<u8>, Option<u8>);
    impl Party for C {
        type Id = u8;
        fn party_id(&self) -> u8 {
            self.0
        }
        fn token(&self) -> Option<u8> {
            self.1
        }
        fn actor(&self) -> Option<u8> {
            self.2
        }
    }

    #[test]
    fn only_someone_elses_turn_holds_a_token_back() {
        let fight = [
            C(1, Some(10), None),
            C(2, Some(20), None),
            C(3, None, Some(30)),
        ];
        assert_eq!(held_by(&fight, 1, 10, None).map(|c| c.0), None);
        assert_eq!(held_by(&fight, 1, 20, None).map(|c| c.0), Some(1));
        assert_eq!(held_by(&fight, 1, 99, None).map(|c| c.0), None);
        assert_eq!(held_by(&fight, 3, 40, Some(30)).map(|c| c.0), None);
        assert_eq!(held_by(&fight, 1, 40, Some(30)).map(|c| c.0), Some(1));
        assert_eq!(held_by(&fight, 9, 20, None).map(|c| c.0), None);
    }
}
