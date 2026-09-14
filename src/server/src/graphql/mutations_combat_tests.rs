//! The initiative tracker's ordering rules, kept out of `mutations_combat.rs`
//! so that file stays under the length check (spec 046 added `downed_by`).

use super::*;

fn combatant(id: u128, initiative: i32, tiebreak: i32, active: bool) -> Combatant {
    let now = Utc::now().naive_utc();
    Combatant {
        id: Uuid::from_u128(id),
        combat_id: Uuid::nil(),
        actor_id: None,
        token_id: None,
        label: format!("c{id}"),
        initiative,
        tiebreak,
        is_npc: false,
        active,
        created_at: now,
        updated_at: now,
        downed_by: None,
    }
}

#[test]
fn sort_is_initiative_then_tiebreak_then_id() {
    let mut rows = vec![
        combatant(3, 10, 0, true),
        combatant(1, 20, 0, true),
        combatant(2, 10, 5, true),
    ];
    sort_combatants(&mut rows);
    let order: Vec<u128> = rows.iter().map(|c| c.id.as_u128()).collect();
    assert_eq!(order, vec![1, 2, 3]);
}

/// The id tiebreaker must produce a total order even when initiative
/// and tiebreak are identical — otherwise the GM and a player can walk
/// the turn order in different sequences.
#[test]
fn sort_is_deterministic_for_fully_tied_combatants() {
    let mut ascending = vec![combatant(1, 10, 0, true), combatant(2, 10, 0, true)];
    let mut descending = vec![combatant(2, 10, 0, true), combatant(1, 10, 0, true)];

    sort_combatants(&mut ascending);
    sort_combatants(&mut descending);

    let a: Vec<u128> = ascending.iter().map(|c| c.id.as_u128()).collect();
    let d: Vec<u128> = descending.iter().map(|c| c.id.as_u128()).collect();
    assert_eq!(a, d, "input order must not affect the resulting turn order");
}

#[test]
fn next_turn_walks_the_order_and_wraps_into_a_new_round() {
    let ordered = vec![
        combatant(1, 20, 0, true),
        combatant(2, 10, 0, true),
        combatant(3, 5, 0, true),
    ];

    // Entering the order for the first time lands on the top of it, and
    // is not a new round. This asserted combatant 2 until 2026-09-07 —
    // the combatant who rolled highest was skipped in round one, and the
    // test that should have caught it had the same off-by-one written in.
    let (idx, wrapped) = next_turn_index(&ordered, None).unwrap();
    assert_eq!(ordered[idx].id.as_u128(), 1);
    assert!(!wrapped);

    let (idx, wrapped) = next_turn_index(&ordered, Some(Uuid::from_u128(1))).unwrap();
    assert_eq!(ordered[idx].id.as_u128(), 2);
    assert!(!wrapped);

    // Past the end of the order → back to the top, new round.
    let (idx, wrapped) = next_turn_index(&ordered, Some(Uuid::from_u128(3))).unwrap();
    assert_eq!(ordered[idx].id.as_u128(), 1);
    assert!(wrapped);
}

#[test]
fn next_turn_skips_inactive_combatants() {
    let ordered = vec![
        combatant(1, 20, 0, true),
        combatant(2, 10, 0, false),
        combatant(3, 5, 0, true),
    ];
    let (idx, _) = next_turn_index(&ordered, Some(Uuid::from_u128(1))).unwrap();
    assert_eq!(ordered[idx].id.as_u128(), 3);
}

/// A combat where everyone is downed has nowhere to advance to — it
/// must report that rather than looping forever or re-selecting the
/// current combatant.
#[test]
fn next_turn_is_none_when_nobody_is_active() {
    let ordered = vec![combatant(1, 20, 0, false), combatant(2, 10, 0, false)];
    assert!(next_turn_index(&ordered, Some(Uuid::from_u128(1))).is_none());
    assert!(next_turn_index(&ordered, None).is_none());
    assert!(next_turn_index(&[], None).is_none());
}

/// The sole active combatant keeps taking turns, and each pass counts
/// as a new round.
#[test]
fn next_turn_repeats_a_lone_active_combatant_as_new_rounds() {
    let ordered = vec![combatant(1, 20, 0, true), combatant(2, 10, 0, false)];
    let (idx, wrapped) = next_turn_index(&ordered, Some(Uuid::from_u128(1))).unwrap();
    assert_eq!(ordered[idx].id.as_u128(), 1);
    assert!(wrapped);
}
