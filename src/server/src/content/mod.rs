//! Reading a system's content out of a document (spec 049).
//!
//! `thunderforge-pdf` turns a book into positioned, styled lines and knows
//! nothing about games. A system's pack declares what its content looks like
//! in those lines ([`thunderforge_canvas_core::content_patterns`]). This is
//! what sits between them, and it names no game system — the build check
//! `scripts/check-system-registry.mjs` fails if it ever does.
//!
//! Two readers, because real books have two shapes (ADR-096):
//!
//! - [`anchored`] for labelled blocks — a creature's armour class, a spell's
//!   casting time;
//! - `prose` for a name and paragraphs, which is what magic items, feats and
//!   class features actually are.

pub mod anchored;
#[cfg(test)]
mod anchored_tests;
pub mod prose;
#[cfg(test)]
#[path = "prose_tests.rs"]
mod prose_tests;

pub use thunderforge_canvas_core::content_entry::{Entry, NameState, ReadValue, SourceLine};

/// The text after a label, if the line starts with it.
///
/// Case- and punctuation-insensitive, because books genuinely disagree:
/// `Armor Class 15`, `ARMOR CLASS 15`, `Armor Class: 15`, and — in a book
/// that emboldens its labels as a run of their own — `Armor Class. 15`, where
/// the full stop is styling rather than a sentence.
///
/// Lifted verbatim in behaviour from the 5e reader, which learned each of
/// those forms from a real book rather than from a specification.
pub fn after_label<'a>(text: &'a str, label: &str) -> Option<&'a str> {
    let trimmed = text.trim_start();
    let label_len = label.chars().count();
    let candidate: String = trimmed.chars().take(label_len).collect();
    if !candidate.eq_ignore_ascii_case(label) {
        return None;
    }
    let rest = &trimmed[candidate.len()..];
    let rest = rest.trim_start_matches([':', '.', '—', '-', ' ', '\t']);
    Some(rest.trim())
}

/// Whether a string is made of letters rather than symbols.
///
/// One bestiary embeds a font with no `ToUnicode` map *and* no encoding — its
/// codes are raw glyph indices, meaningful only to the font program beside
/// them — and its pages decode to `. / * D D & ! ! $ * D`. A name read from
/// one arrives as `* ROGGUDJRQV`. Not recoverable without reading the
/// embedded font, and refused outright rather than flagged, because there is
/// nothing there for a person to correct.
pub fn is_mostly_letters(text: &str) -> bool {
    let considered = text.chars().filter(|c| !c.is_whitespace()).count();
    if considered == 0 {
        return false;
    }
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    letters * 2 >= considered
}

/// Whether text is too damaged to present as read.
pub fn looks_damaged(text: &str) -> bool {
    thunderforge_pdf::layout::looks_letter_spaced(text)
}

/// Whether text is not language at all.
pub fn looks_unreadable(text: &str) -> bool {
    thunderforge_pdf::layout::looks_unreadable(text)
}
