//! Spec 048 T045/T046: what has no field lands in the notes, under its
//! heading, and a re-import replaces that block instead of stacking another.

use super::{NOTES_HEADING, notes_with_unmapped};

fn lines(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(label, value)| (label.to_string(), value.to_string()))
        .collect()
}

#[test]
fn unmapped_values_go_under_the_heading_after_what_the_player_wrote() {
    let notes = notes_with_unmapped(
        Some("Owes the guild 40 gp."),
        &lines(&[("Defenses", "Disease - Immunity")]),
    );
    assert_eq!(
        notes,
        format!("Owes the guild 40 gp.\n\n{NOTES_HEADING}\n- Defenses: Disease - Immunity")
    );
}

#[test]
fn a_reimport_replaces_the_block_and_keeps_the_players_words() {
    let first = notes_with_unmapped(Some("Mine."), &lines(&[("Defenses", "Disease - Immunity")]));
    let again = notes_with_unmapped(Some(&first), &lines(&[("Encumbrance", "Light")]));
    assert_eq!(
        again,
        format!("Mine.\n\n{NOTES_HEADING}\n- Encumbrance: Light")
    );
    assert_eq!(again.matches(NOTES_HEADING).count(), 1);
}

#[test]
fn nothing_unmapped_leaves_no_heading() {
    assert_eq!(notes_with_unmapped(None, &[]), "");
    assert_eq!(notes_with_unmapped(Some("Mine.\n"), &[]), "Mine.");
}
