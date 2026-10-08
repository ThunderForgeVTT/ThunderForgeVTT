//! Spec 085: the presses `handle_token_drag` leaves to the group.

use super::token_press_yields;
use crate::resources::GroupSelection;

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|id| (*id).to_string()).collect()
}

fn group(tokens: &[&str], walls: &[&str]) -> GroupSelection {
    GroupSelection {
        tokens: ids(tokens),
        walls: ids(walls),
        ..Default::default()
    }
}

#[test]
fn with_shift_held_an_empty_board_press_does_not_deselect() {
    assert!(token_press_yields(true, &[], &group(&["a", "b"], &[])));
    assert!(!token_press_yields(false, &[], &group(&["a", "b"], &[])));
}

#[test]
fn with_shift_held_a_press_on_a_token_is_the_toggle_not_a_pick_up() {
    assert!(token_press_yields(
        true,
        &ids(&["c"]),
        &GroupSelection::default()
    ));
}

#[test]
fn a_press_on_a_member_of_a_group_of_two_or_more_is_the_group_moves() {
    assert!(token_press_yields(
        false,
        &ids(&["a"]),
        &group(&["a"], &["w1"])
    ));
    assert!(token_press_yields(
        false,
        &ids(&["b", "a"]),
        &group(&["a", "b"], &[])
    ));
}

#[test]
fn a_press_on_a_lone_selected_token_or_a_stranger_is_the_drags() {
    assert!(!token_press_yields(
        false,
        &ids(&["a"]),
        &group(&["a"], &[])
    ));
    assert!(!token_press_yields(
        false,
        &ids(&["z"]),
        &group(&["a", "b"], &[])
    ));
}
