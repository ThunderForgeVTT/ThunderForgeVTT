//! Turn order: who goes first, and who goes next.
//!
//! Ordering is defined in exactly one place and is a total order: initiative
//! descending, then `tiebreak` descending, then id. That last id term is not
//! decoration. Without a deterministic final key, two combatants tied on both
//! initiative and tiebreak could come back in whatever order storage happened
//! to return, and "next turn" would step through a different sequence on one
//! screen than on another — precisely the disagreement a shared tracker exists
//! to prevent.
//!
//! The server's rows and the demo's in-memory seats are different types; each
//! says how it is read through [`Seat`].

use std::cmp::Ordering;

/// What turn order reads of a combatant.
pub trait Seat {
    type Id: Ord + Copy;
    fn seat_id(&self) -> Self::Id;
    fn initiative(&self) -> i32;
    fn tiebreak(&self) -> i32;
    /// False for a combatant that is downed or out of the fight.
    fn is_active(&self) -> bool;
}

/// The single comparison turn order is.
pub fn turn_order<S: Seat>(a: &S, b: &S) -> Ordering {
    b.initiative()
        .cmp(&a.initiative())
        .then(b.tiebreak().cmp(&a.tiebreak()))
        .then(a.seat_id().cmp(&b.seat_id()))
}

/// Sort into turn order.
pub fn sort_seats<S: Seat>(seats: &mut [S]) {
    seats.sort_by(turn_order);
}

/// Index of the combatant that takes the turn after `active_id`.
///
/// Returns `(index, wrapped)` — `wrapped` is true when the turn passed the
/// end of the order, which is what increments the round. Skips inactive
/// (downed/removed) combatants. Returns `None` when nobody is eligible, so
/// a combat whose combatants are all inactive stops rather than spinning.
pub fn next_turn<S: Seat>(ordered: &[S], active_id: Option<S::Id>) -> Option<(usize, bool)> {
    if ordered.is_empty() {
        return None;
    }

    let current = active_id.and_then(|id| ordered.iter().position(|c| c.seat_id() == id));

    // Walk the whole ring exactly once, and where it starts depends on
    // whether anybody is holding the turn.
    //
    // Moving on from a combatant starts *just past* them — offset 1 — which
    // is what makes "next" skip the current one rather than landing back on
    // it when it is the only active combatant.
    //
    // Entering the order starts *at* the top — offset 0. Starting past it
    // meant the combatant who rolled highest never acted in the first round:
    // a new combat holds no active combatant, so the first advance arrived
    // here with `current` as `None` and opened the encounter on second place.
    let len = ordered.len();
    let (start, first_offset) = match current {
        Some(index) => (index, 1),
        None => (0, 0),
    };

    for offset in first_offset..(first_offset + len) {
        let idx = (start + offset) % len;
        if ordered[idx].is_active() {
            // A wrap happened if we passed index 0 on the way. With no
            // current combatant we are entering the order for the first
            // time, which is not a new round.
            let wrapped = current.is_some() && start + offset >= len;
            return Some((idx, wrapped));
        }
    }
    None
}
