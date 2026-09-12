//! Tests for the prose reader (spec 049 FR-001b, FR-002).
//!
//! The shapes here are the ones real books actually have, read out of the
//! corpus on 2026-09-12: a magic item is a heading, then an unlabelled type
//! and rarity line, then prose; a feat is a heading, then a prerequisite, then
//! prose. Neither carries an anchor label, which is why this reader exists.

use super::prose::entries;
use super::{NameState, SourceLine};
use thunderforge_canvas_core::content_patterns::{NameRule, NameStyle, Pattern, ProseEnd, Shape};

fn line(text: &str, heading: bool, bold: bool) -> SourceLine {
    SourceLine {
        text: text.to_string(),
        size: if heading { 16.0 } else { 11.0 },
        bold,
        page: 1,
        suspect: false,
        heading,
    }
}

fn pattern(style: NameStyle, ends_at: ProseEnd) -> Pattern {
    Pattern {
        kind: "feat".into(),
        shape: Shape::Prose,
        anchor: None,
        name: NameRule {
            position: None,
            within_lines: None,
            prefer: None,
            style: Some(style),
            ends_at: Some(ends_at),
        },
        fields: Vec::new(),
        confirmed_by: vec!["Prerequisite".into()],
        confirm_within: Some(3),
    }
}

fn feats() -> Pattern {
    pattern(NameStyle::Heading, ProseEnd::NextName)
}

#[test]
fn an_entry_is_its_name_and_the_prose_under_it() {
    let lines = vec![
        line("Cast-Iron Stomach", true, false),
        line("Prerequisite: Constitution 13 or higher", false, false),
        line(
            "Your fondness of spicy and acidic foods has yielded benefits:",
            false,
            false,
        ),
    ];
    let found = entries(&lines, &feats());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "Cast-Iron Stomach");
    assert_eq!(found[0].name_state, NameState::Clear);
    assert!(found[0].text.as_ref().unwrap().contains("Prerequisite"));
}

#[test]
fn a_prose_entry_has_nowhere_to_put_a_mechanical_value() {
    // FR-001b is the point of this reader existing. `values` is empty, and the
    // type gives a future contributor nowhere to quietly add a damage figure.
    let lines = vec![
        line("Breaking Wind", true, false),
        line("Prerequisite: 1st-level fighter", false, false),
        line(
            "Your intestinal fortitude is unmatched when you take a breather.",
            false,
            false,
        ),
    ];
    let entry = &entries(&lines, &feats())[0];
    assert!(entry.values.is_empty());
    assert!(entry.text.is_some());
}

#[test]
fn entries_stop_at_the_next_name() {
    let lines = vec![
        line("Artificer Intelligents", true, false),
        line(
            "Prerequisite: An artificer class feature that grants spells",
            false,
            false,
        ),
        line(
            "You have learned to imbue constructs with a spark of sapience.",
            false,
            false,
        ),
        line("Cast-Iron Stomach", true, false),
        line("Prerequisite: Constitution 13 or higher", false, false),
        line(
            "Your fondness of spicy foods has yielded bizarre benefits here.",
            false,
            false,
        ),
    ];
    let found = entries(&lines, &feats());
    assert_eq!(found.len(), 2);
    assert!(!found[0].text.as_ref().unwrap().contains("Cast-Iron"));
    assert_eq!(found[1].name, "Cast-Iron Stomach");
}

#[test]
fn a_bold_run_in_inside_a_paragraph_does_not_start_an_entry() {
    // The boundary case this reader most has to get right. A line of ordinary
    // prose is not bold at line level even when it opens with a bold word, so
    // it is not a name — and a whole bold paragraph is emphasis, not a name,
    // because a name is short.
    let long_bold = "Amphibious. The dragon can breathe air and water, and it may remain submerged \
         indefinitely without any need to surface for breath at any point.";
    let lines = vec![
        line("Abilities", false, true),
        line("Prerequisite: none at all", false, false),
        line(
            "Some ordinary prose about what these abilities are and do here.",
            false,
            false,
        ),
        line(long_bold, false, true),
        line("Prerequisite: also none", false, false),
        line(
            "More prose that would follow a name if this line were one.",
            false,
            false,
        ),
    ];
    let found = entries(&lines, &pattern(NameStyle::Bold, ProseEnd::NextName));
    assert_eq!(found.len(), 1, "only the short bold line is a name");
    assert_eq!(found[0].name, "Abilities");
}

#[test]
fn a_heading_that_introduces_nothing_is_not_an_entry() {
    // Books are full of chapter titles and running heads. Without a floor on
    // the text under a name, every one of them arrives as an empty feat.
    let lines = vec![
        line("Feats", true, false),
        line("Chapter 4", true, false),
        line("Cast-Iron Stomach", true, false),
        line("Prerequisite: Constitution 13 or higher", false, false),
        line(
            "Your fondness of spicy and acidic foods has yielded benefits.",
            false,
            false,
        ),
    ];
    let found = entries(&lines, &feats());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "Cast-Iron Stomach");
}

#[test]
fn ends_at_next_heading_runs_past_sibling_names() {
    let lines = vec![
        line("Rage", false, true),
        line("Prerequisite: barbarian", false, false),
        line(
            "In battle you fight with primal ferocity and gain these benefits.",
            false,
            false,
        ),
        line("Reckless Attack", false, true),
        line("Prerequisite: barbarian 2nd level", false, false),
        line(
            "You can throw aside all concern for defence to attack recklessly.",
            false,
            false,
        ),
        line("Chapter 5: Equipment", true, false),
    ];
    let found = entries(&lines, &pattern(NameStyle::Bold, ProseEnd::NextHeading));
    assert_eq!(found.len(), 2);
    // The first runs past the second's name to the heading.
    assert!(found[0].text.as_ref().unwrap().contains("Reckless Attack"));
}

#[test]
fn a_name_that_is_not_language_is_refused() {
    let lines = vec![
        line(". / * D D & ! ! $ * D", true, false),
        line("Prerequisite: something", false, false),
        line(
            "Some perfectly readable prose following the damaged heading.",
            false,
            false,
        ),
    ];
    assert!(entries(&lines, &feats()).is_empty());
}

#[test]
fn suspect_lines_make_a_suspect_entry() {
    let mut lines = vec![
        line("Cast-Iron Stomach", true, false),
        line("Prerequisite: Constitution 13 or higher", false, false),
        line(
            "Your fondness of spicy and acidic foods has yielded benefits.",
            false,
            false,
        ),
    ];
    lines[2].suspect = true;
    assert!(entries(&lines, &feats())[0].suspect);
}

#[test]
fn the_page_a_thing_was_found_on_is_carried() {
    let mut lines = vec![
        line("Cast-Iron Stomach", true, false),
        line("Prerequisite: Constitution 13 or higher", false, false),
        line(
            "Your fondness of spicy and acidic foods has yielded benefits.",
            false,
            false,
        ),
    ];
    lines[0].page = 44;
    lines[1].page = 44;
    lines[2].page = 44;
    assert_eq!(entries(&lines, &feats())[0].page, 44);
}

#[test]
fn a_heading_with_no_confirmation_under_it_is_not_an_entry() {
    // The guard the corpus demanded. Without it, a prose kind declared as
    // "a heading, then paragraphs" matches every section of every book: the
    // reader returned 72,974 magic items across 246 books, including
    // `Table of Contents` and `About`, and returned the identical number for
    // feats because the two declarations were indistinguishable.
    let lines = vec![
        line("Table of Contents", true, false),
        line("Introduction .......... 3", false, false),
        line(
            "Weapons .......... 7 and other assorted things listed here",
            false,
            false,
        ),
    ];
    assert!(entries(&lines, &feats()).is_empty());
}

#[test]
fn a_confirmation_must_be_near_the_name_not_anywhere_in_the_entry() {
    // Otherwise one stray `Prerequisite` deep in a chapter confirms the
    // chapter's title and swallows everything under it.
    let mut lines = vec![line("About This Book", true, false)];
    for _ in 0..8 {
        lines.push(line(
            "Some ordinary introductory prose about the book.",
            false,
            false,
        ));
    }
    lines.push(line("Prerequisite: nothing, this is prose", false, false));
    assert!(entries(&lines, &feats()).is_empty());
}

#[test]
fn a_prose_pattern_with_no_discriminator_finds_nothing() {
    // Validation refuses this at install; the reader refuses it again, because
    // the failure mode is silent and enormous rather than small and noisy.
    let mut undeclared = feats();
    undeclared.confirmed_by = Vec::new();
    let lines = vec![
        line("Cast-Iron Stomach", true, false),
        line("Prerequisite: Constitution 13 or higher", false, false),
        line(
            "Your fondness of spicy and acidic foods has yielded benefits.",
            false,
            false,
        ),
    ];
    assert!(entries(&lines, &undeclared).is_empty());
}
