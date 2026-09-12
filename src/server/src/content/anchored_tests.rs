//! Tests for the anchored reader (spec 049 FR-001a, FR-002, FR-003).
//!
//! The pattern used throughout is a creature anchored on a labelled armour
//! class, because that is the shape measured against 246 real books. No game
//! system is named: the labels come from the pattern, exactly as they do in
//! production.

use super::anchored::entries;
use super::{NameState, ReadValue, SourceLine};
use thunderforge_canvas_core::content_patterns::{
    FieldKind, FieldSpec, NamePosition, NamePreference, NameRule, Pattern, Shape,
};

fn line(text: &str, size: f64, bold: bool) -> SourceLine {
    SourceLine {
        text: text.to_string(),
        size,
        bold,
        page: 1,
        suspect: false,
    }
}

fn pattern() -> Pattern {
    Pattern {
        kind: "creature".into(),
        shape: Shape::Anchored,
        anchor: Some("Armor Class".into()),
        name: NameRule {
            position: Some(NamePosition::Before),
            within_lines: Some(6),
            prefer: Some(NamePreference::Largest),
            style: None,
            ends_at: None,
        },
        fields: vec![
            FieldSpec {
                key: "armorClass".into(),
                label: "Armor Class".into(),
                value_kind: FieldKind::Integer,
            },
            FieldSpec {
                key: "hitPoints".into(),
                label: "Hit Points".into(),
                value_kind: FieldKind::Integer,
            },
            FieldSpec {
                key: "speed".into(),
                label: "Speed".into(),
                value_kind: FieldKind::Text,
            },
        ],
    }
}

#[test]
fn an_entry_is_read_with_its_name_and_declared_fields() {
    let lines = vec![
        line("ADULT RED DRAGON", 18.0, true),
        line("Gargantuan dragon, chaotic evil", 9.0, false),
        line("Armor Class 22", 9.0, false),
        line("Hit Points 546", 9.0, false),
        line("Speed 40 ft., fly 80 ft.", 9.0, false),
    ];
    let found = entries(&lines, &pattern());
    assert_eq!(found.len(), 1);
    let entry = &found[0];
    assert_eq!(entry.name, "ADULT RED DRAGON");
    assert_eq!(entry.name_state, NameState::Clear);
    assert_eq!(entry.kind, "creature");
    assert_eq!(entry.values["armorClass"], ReadValue::Clear("22".into()));
    assert_eq!(entry.values["hitPoints"], ReadValue::Clear("546".into()));
}

#[test]
fn a_field_that_was_not_found_is_unread_and_carries_no_value() {
    // FR-002: absence is recorded as absence. The enum makes "unread with a
    // value anyway" unrepresentable, and this is the test that says so.
    let lines = vec![
        line("GOBLIN", 14.0, true),
        line("Armor Class 15", 9.0, false),
    ];
    let entry = &entries(&lines, &pattern())[0];
    assert_eq!(entry.values["hitPoints"], ReadValue::Unread);
    assert!(entry.values["hitPoints"].text().is_none());
}

#[test]
fn every_declared_field_appears_even_when_absent() {
    // A caller must be able to tell "looked for and not found" from "never
    // asked about". Both are in the map; neither is missing from it.
    let lines = vec![
        line("GOBLIN", 14.0, true),
        line("Armor Class 15", 9.0, false),
    ];
    let entry = &entries(&lines, &pattern())[0];
    assert_eq!(entry.values.len(), 3);
    assert!(entry.values.contains_key("speed"));
}

#[test]
fn a_value_that_will_not_parse_is_uncertain_with_its_text_kept() {
    // Never coerced, never dropped. "natural armor" is what the book says.
    let lines = vec![
        line("OWLBEAR", 14.0, true),
        line("Armor Class thirteen (natural armor)", 9.0, false),
    ];
    let entry = &entries(&lines, &pattern())[0];
    assert_eq!(
        entry.values["armorClass"],
        ReadValue::Uncertain("thirteen (natural armor)".into())
    );
}

#[test]
fn an_integer_may_be_followed_by_prose_and_still_read_cleanly() {
    // `17 (natural armor)` is what a real statblock says far more often than
    // a bare `17`, so a leading number is enough.
    let lines = vec![
        line("KNIGHT", 14.0, true),
        line("Armor Class 18 (plate)", 9.0, false),
    ];
    let entry = &entries(&lines, &pattern())[0];
    assert_eq!(
        entry.values["armorClass"],
        ReadValue::Clear("18 (plate)".into())
    );
}

#[test]
fn labels_are_matched_however_the_book_punctuates_them() {
    // Four forms, all learned from real books rather than from a spec.
    for text in [
        "Armor Class 15",
        "ARMOR CLASS 15",
        "Armor Class: 15",
        "Armor Class. 15",
    ] {
        let lines = vec![line("GOBLIN", 14.0, true), line(text, 9.0, false)];
        let found = entries(&lines, &pattern());
        assert_eq!(found.len(), 1, "{text} should be read");
        assert_eq!(found[0].values["armorClass"], ReadValue::Clear("15".into()));
    }
}

#[test]
fn two_entries_in_sequence_do_not_bleed_into_each_other() {
    let lines = vec![
        line("GOBLIN", 14.0, true),
        line("Armor Class 15", 9.0, false),
        line("Hit Points 7", 9.0, false),
        line("ORC", 14.0, true),
        line("Armor Class 13", 9.0, false),
        line("Hit Points 15", 9.0, false),
    ];
    let found = entries(&lines, &pattern());
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].name, "GOBLIN");
    assert_eq!(found[0].values["hitPoints"], ReadValue::Clear("7".into()));
    assert_eq!(found[1].name, "ORC");
    assert_eq!(found[1].values["hitPoints"], ReadValue::Clear("15".into()));
}

#[test]
fn the_name_is_walked_back_past_intervening_lines() {
    // A running head and a page number between the name and the anchor is
    // ordinary in a real book, which is why the lookback is six and not one.
    let lines = vec![
        line("ANCIENT BLACK DRAGON", 18.0, true),
        line("Chapter 3: Monsters", 8.0, false),
        line("47", 8.0, false),
        line("Gargantuan dragon, chaotic evil", 9.0, false),
        line("Armor Class 22", 9.0, false),
    ];
    let entry = &entries(&lines, &pattern())[0];
    assert_eq!(entry.name, "ANCIENT BLACK DRAGON");
}

#[test]
fn an_entry_whose_name_was_never_found_is_kept_but_marked_uncertain() {
    // It still has an armour class, so it is still a creature. Throwing it
    // away loses more than flagging it does.
    let lines = vec![
        line("some ordinary prose running on", 9.0, false),
        line("Armor Class 15", 9.0, false),
        line("Hit Points 7", 9.0, false),
    ];
    let found = entries(&lines, &pattern());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name_state, NameState::Uncertain);
}

#[test]
fn an_anchor_inside_ordinary_prose_does_not_produce_an_entry() {
    // The false-positive guard: a block that matched the anchor but read none
    // of its declared fields is the label appearing in a sentence.
    let lines = vec![
        line("The Rules", 14.0, true),
        line("Armor Class", 9.0, false),
        line("is the number an attack must beat.", 9.0, false),
    ];
    assert!(entries(&lines, &pattern()).is_empty());
}

#[test]
fn a_name_that_is_not_language_is_refused_outright() {
    // A font with no ToUnicode map and no encoding decodes to raw glyph
    // indices, and a whole page arrives as `. / * D D & ! ! $ * D`. There is
    // nothing here for a person to correct, so it is refused rather than
    // flagged.
    //
    // The string matters: `* ROGGUDJRQV` — which is what such a name *looks*
    // like once a few glyphs happen to land on letters — is 11 letters out of
    // 14 characters and passes `is_mostly_letters` honestly. The guard catches
    // symbol soup, not plausible-looking nonsense, and the first version of
    // this test asserted otherwise.
    let lines = vec![
        line(". / * D D & ! ! $ * D", 14.0, true),
        line("Armor Class 15", 9.0, false),
    ];
    assert!(entries(&lines, &pattern()).is_empty());
}

#[test]
fn suspect_lines_make_a_suspect_entry() {
    // FR-004: the layout pass's distrust reaches the review, so a person can
    // see which entries came from pages the reader struggled with.
    let mut lines = vec![
        line("GOBLIN", 14.0, true),
        line("Armor Class 15", 9.0, false),
    ];
    lines[1].suspect = true;
    assert!(entries(&lines, &pattern())[0].suspect);
}

#[test]
fn a_pattern_with_no_anchor_finds_nothing_rather_than_guessing() {
    let mut broken = pattern();
    broken.anchor = None;
    let lines = vec![
        line("GOBLIN", 14.0, true),
        line("Armor Class 15", 9.0, false),
    ];
    assert!(entries(&lines, &broken).is_empty());
}

#[test]
fn the_page_a_thing_was_found_on_is_carried() {
    let mut lines = vec![
        line("GOBLIN", 14.0, true),
        line("Armor Class 15", 9.0, false),
    ];
    lines[0].page = 212;
    lines[1].page = 212;
    assert_eq!(entries(&lines, &pattern())[0].page, 212);
}
