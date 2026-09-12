//! Reading a block of labelled fields (spec 049 FR-001a).
//!
//! An anchored kind is one whose entries begin with a label that appears
//! nowhere else — armour class, for a 5e creature. Find the label, walk to the
//! name, read the declared fields between there and the next one.
//!
//! # Why an anchor rather than a heading
//!
//! Headings were tried first, in effect, and the corpus says no: a real book's
//! creature name, its size-and-type line and its armour class are not reliably
//! adjacent, running heads and page numbers land between them, and plenty of
//! headings are not entries at all. An unambiguous label is the one thing a
//! book is consistent about, and it is why 2155 creatures read out of 246
//! books while the same code finds nothing at all when pointed at prose.
//!
//! This names no game system, and must not: `scripts/check-system-registry.mjs`
//! fails the build for it. Every label it looks for arrives from the pack.

use thunderforge_canvas_core::content_patterns::{
    FieldKind, NamePosition, NamePreference, Pattern,
};

use super::{
    Entry, NameState, ReadValue, SourceLine, after_label, is_mostly_letters, looks_damaged,
    looks_unreadable,
};

/// How far from the anchor a name may be when the pattern does not say.
const DEFAULT_LOOKBACK: u32 = 6;

/// A name is short. Longer than this and it is a sentence that happens to be
/// set large.
const LONGEST_NAME: usize = 60;

/// Every entry of one anchored kind, in the order they appear.
///
/// Lines must be in reading order. An entry runs from its name to the start of
/// the next one, or to the end of the document.
pub fn entries(lines: &[SourceLine], pattern: &Pattern) -> Vec<Entry> {
    let Some(anchor) = pattern.anchor.as_deref().filter(|a| !a.trim().is_empty()) else {
        // A pattern with no anchor cannot find anything. Validation refuses
        // one at install (`pack_system_spec`); this is the belt to that
        // braces, and returning nothing is the only honest answer.
        return Vec::new();
    };

    let anchors: Vec<usize> = (0..lines.len())
        .filter(|index| after_label(&lines[*index].text, anchor).is_some())
        .collect();

    let lookback = pattern.name.within_lines.unwrap_or(DEFAULT_LOOKBACK) as usize;
    let prefer = pattern.name.prefer.unwrap_or(NamePreference::Largest);
    let position = pattern.name.position.unwrap_or(NamePosition::Before);

    let mut out = Vec::new();
    for (position_in_list, at) in anchors.iter().enumerate() {
        // A name may not be searched for past the previous anchor: everything
        // before that belongs to the previous entry. Without this floor a
        // `largest` preference happily reaches back over a whole entry and
        // returns *its* name, which is louder than the one it wanted.
        let floor = position_in_list
            .checked_sub(1)
            .map(|previous| anchors[previous] + 1)
            .unwrap_or(0);
        let next_floor = *at + 1;

        let ends_at = anchors
            .get(position_in_list + 1)
            .map(|next| {
                name_index(lines, *next, lookback, next_floor, prefer, position)
                    .map_or(*next, |(i, _)| i)
            })
            .unwrap_or(lines.len());
        let (starts_at, named) =
            name_index(lines, *at, lookback, floor, prefer, position).unwrap_or((*at, false));
        if starts_at >= ends_at {
            continue;
        }
        if let Some(entry) = read_one(&lines[starts_at..ends_at], named, pattern) {
            out.push(entry);
        }
    }
    out
}

/// Walk from an anchor to the entry's name.
///
/// Returns the name's index and whether it is really a name. An entry whose
/// name could not be found is still worth keeping when its declared fields
/// read cleanly — so the nearest line stands in and the caller marks it
/// uncertain, rather than throwing the entry away.
fn name_index(
    lines: &[SourceLine],
    anchor: usize,
    lookback: usize,
    floor: usize,
    prefer: NamePreference,
    position: NamePosition,
) -> Option<(usize, bool)> {
    let range: Vec<usize> = match position {
        NamePosition::Before => (anchor.saturating_sub(lookback).max(floor)..anchor)
            .rev()
            .collect(),
        NamePosition::After => ((anchor + 1)..(anchor + 1 + lookback).min(lines.len())).collect(),
    };

    let candidates: Vec<usize> = range
        .into_iter()
        .filter(|index| {
            let line = &lines[*index];
            !line.text.trim().is_empty()
                && line.text.chars().count() <= LONGEST_NAME
                && (line.bold || line.size > lines[anchor].size)
        })
        .collect();

    let chosen = match prefer {
        // Nearest first, because `range` is already ordered away from the
        // anchor.
        NamePreference::Nearest => candidates.first().copied(),
        NamePreference::Bold => candidates
            .iter()
            .copied()
            .find(|index| lines[*index].bold)
            .or_else(|| candidates.first().copied()),
        // Strictly greater, so that equal sizes leave the first candidate —
        // which `range` has already ordered nearest-to-the-anchor — in place.
        NamePreference::Largest => candidates.iter().copied().reduce(|best, next| {
            if lines[next].size > lines[best].size {
                next
            } else {
                best
            }
        }),
    };

    match chosen {
        Some(index) => Some((index, true)),
        // Nothing that looks like a name. Fall back to the nearest non-empty
        // line so the entry still has something to be called.
        None => match position {
            NamePosition::Before => (anchor.saturating_sub(lookback).max(floor)..anchor)
                .rev()
                .find(|index| !lines[*index].text.trim().is_empty())
                .map(|index| (index, false)),
            NamePosition::After => None,
        },
    }
}

fn read_one(block: &[SourceLine], named: bool, pattern: &Pattern) -> Option<Entry> {
    let first = block.first()?;
    let name = first.text.trim().to_string();
    if name.is_empty() || looks_unreadable(&name) || !is_mostly_letters(&name) {
        return None;
    }

    // Every declared field appears, whether or not it was found. A caller
    // reading this map can tell "looked for and absent" from "never asked
    // about", which is what FR-003's three states are for.
    let mut values = std::collections::BTreeMap::new();
    for field in &pattern.fields {
        values.insert(field.key.clone(), ReadValue::Unread);
    }

    for line in block {
        let text = line.text.trim();
        for field in &pattern.fields {
            if !values
                .get(&field.key)
                .is_some_and(super::ReadValue::is_unread)
            {
                continue;
            }
            let Some(rest) = after_label(text, &field.label) else {
                continue;
            };
            if rest.is_empty() {
                continue;
            }
            values.insert(field.key.clone(), read_as(rest, field.value_kind));
        }
    }

    // An entry that matched the anchor but read none of its declared fields is
    // a false positive — the anchor's label appearing in ordinary prose.
    if values.values().all(super::ReadValue::is_unread) {
        return None;
    }

    Some(Entry {
        kind: pattern.kind.clone(),
        name: name.clone(),
        // Uncertain covers both ways a name can be wrong: damaged text, and a
        // name that was never found at all, in which case a neighbouring line
        // is standing in for it.
        name_state: if named && !looks_damaged(&name) {
            NameState::Clear
        } else {
            NameState::Uncertain
        },
        page: first.page,
        values,
        text: None,
        suspect: block.iter().any(|line| line.suspect),
        // Filled by the system pack's refinement, if it has one (FR-016).
        extras: None,
    })
}

/// Read one value as the kind its system declared it to be.
///
/// A value that will not parse is **uncertain with the text exactly as read**,
/// never coerced and never dropped. `Speed 30 ft., fly 60 ft.` is not an
/// integer and is also exactly what the book says; turning it into `30` would
/// be inventing, and dropping it would be losing.
fn read_as(text: &str, kind: FieldKind) -> ReadValue {
    if looks_damaged(text) {
        return ReadValue::Uncertain(text.to_string());
    }
    let parses = match kind {
        FieldKind::Text => true,
        FieldKind::Integer => leading_number(text).is_some_and(|n| n.fract() == 0.0),
        FieldKind::Number => leading_number(text).is_some(),
    };
    if parses {
        ReadValue::Clear(text.to_string())
    } else {
        ReadValue::Uncertain(text.to_string())
    }
}

/// The number a value starts with, if it starts with one.
///
/// Leading rather than whole-string, because a declared integer in a real book
/// is `17 (natural armor)` far more often than it is `17`.
fn leading_number(text: &str) -> Option<f64> {
    let digits: String = text
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    digits.parse::<f64>().ok()
}
