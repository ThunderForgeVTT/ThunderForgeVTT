//! Reading a name and the paragraphs under it (spec 049 FR-001b).
//!
//! A prose kind has no labels and no fixed fields. A magic item is a heading
//! followed by `Weapon (whip), uncommon` and then prose; a feat is a heading
//! followed by `Prerequisite: …` and then prose. Neither line is labelled in
//! any way a declaration could anchor on, which is why these are a different
//! shape rather than an anchored kind with optional fields.
//!
//! # What this deliberately does not do
//!
//! It does not extract mechanics. An [`Entry`] produced here has an empty
//! `values` map and **nowhere to put** a damage figure or a range, and that is
//! the requirement rather than a limitation: FR-002 says absence is recorded
//! as absence, and every parser defect found while building this reader
//! produced text that looked plausible and was wrong. A heuristic that guessed
//! a range out of a sentence would be inventing, and marking an invention
//! "uncertain" only launders it.
//!
//! What a Game Master gets is the name, the text, and the page — enough to
//! find it in the real book, which is the honest offer.

use thunderforge_canvas_core::content_patterns::{NameStyle, Pattern, ProseEnd};

use super::{Entry, NameState, SourceLine, is_mostly_letters, looks_damaged, looks_unreadable};

/// A name is short. Longer than this and it is a sentence set large.
const LONGEST_NAME: usize = 60;

/// An entry with less text than this under its name is a heading, not an
/// entry — a chapter title, a running head, a table caption.
///
/// Books are full of headings that introduce nothing. Without a floor, every
/// one of them arrives as an empty "feat".
const LEAST_TEXT: usize = 40;

/// How many lines after a name to look for a confirmation, by default.
const CONFIRM_WITHIN: usize = 3;

/// Every entry of one prose kind, in the order they appear.
pub fn entries(lines: &[SourceLine], pattern: &Pattern) -> Vec<Entry> {
    if pattern.confirmed_by.is_empty() {
        // Validation refuses this at install (`pack_system_spec`). Refusing
        // again here is not belt and braces for its own sake: a prose kind
        // with no discriminator does not return slightly too much, it returns
        // every heading in the book, and doing that silently is how 72,974
        // "magic items" happened.
        return Vec::new();
    }

    let style = pattern.name.style.unwrap_or(NameStyle::Heading);
    let ends_at = pattern.name.ends_at.unwrap_or(ProseEnd::NextName);
    let within = pattern.confirm_within.unwrap_or(CONFIRM_WITHIN as u32) as usize;

    let names: Vec<usize> = (0..lines.len())
        .filter(|index| is_name(&lines[*index], style))
        .filter(|index| confirmed(lines, *index, within, &pattern.confirmed_by))
        .collect();

    let mut out = Vec::new();
    for (position, at) in names.iter().enumerate() {
        let stop = match ends_at {
            ProseEnd::NextName => names.get(position + 1).copied().unwrap_or(lines.len()),
            // Runs past sibling names to the next thing that is structurally a
            // heading — for a kind whose entries nest under one.
            ProseEnd::NextHeading => ((at + 1)..lines.len())
                .find(|index| lines[*index].heading)
                .unwrap_or(lines.len()),
        };
        if let Some(entry) = read_one(&lines[*at..stop], pattern) {
            out.push(entry);
        }
    }
    out
}

/// Whether one of the pattern's confirming phrases appears just under a name.
///
/// Case-insensitive substring, because a book writes `Wondrous item, rare` and
/// `WONDROUS ITEM` and means the same thing both times.
fn confirmed(lines: &[SourceLine], name: usize, within: usize, phrases: &[String]) -> bool {
    let upto = (name + 1 + within).min(lines.len());
    lines[(name + 1)..upto].iter().any(|line| {
        let lowered = line.text.to_lowercase();
        phrases
            .iter()
            .any(|phrase| lowered.contains(&phrase.to_lowercase()))
    })
}

fn is_name(line: &SourceLine, style: NameStyle) -> bool {
    let text = line.text.trim();
    if text.is_empty() || text.chars().count() > LONGEST_NAME {
        return false;
    }
    match style {
        NameStyle::Heading => line.heading,
        // Short and bold. A whole bold paragraph is emphasis, not a name —
        // the same judgement the layout pass makes about headings, applied
        // where a system says its entries are introduced by a bold run-in.
        NameStyle::Bold => line.bold,
    }
}

fn read_one(block: &[SourceLine], pattern: &Pattern) -> Option<Entry> {
    let first = block.first()?;
    let name = first.text.trim().to_string();
    if name.is_empty() || looks_unreadable(&name) || !is_mostly_letters(&name) {
        return None;
    }

    let text = block
        .iter()
        .skip(1)
        .map(|line| line.text.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if text.chars().count() < LEAST_TEXT {
        return None;
    }

    Some(Entry {
        kind: pattern.kind.clone(),
        name: name.clone(),
        name_state: if looks_damaged(&name) {
            NameState::Uncertain
        } else {
            NameState::Clear
        },
        page: first.page,
        // Empty, and there is nowhere to put anything else. FR-001b.
        values: Default::default(),
        text: Some(text),
        suspect: block.iter().any(|line| line.suspect),
        extras: None,
    })
}
