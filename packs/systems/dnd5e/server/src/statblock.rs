//! Reading a D&D 5e statblock out of a book.
//!
//! Knows nothing about PDFs. It is handed lines — text with a size and a
//! weight — and finds the creatures among them. That split is the whole point
//! of spec 048 FR-001b: this file could read a statblock out of a web page or
//! a text file, and `thunderforge-pdf` could feed a reader for a system that
//! has never heard of armour class.
//!
//! # What a statblock looks like, from the corpus
//!
//! Measured against real books rather than the SRD's idealised layout:
//!
//! ```text
//! RED DRAGON                          <- name, larger and bold
//! Gargantuan dragon, chaotic evil     <- size, type, alignment, italic
//! Armor Class 22 (natural armor)      <- bold label, roman value
//! Hit Points 546 (28d20 + 252)
//! Speed 40ft., climb 40ft., fly 80ft.
//! STR DEX CON INT WIS CHA             <- a six-column table
//! Challenge 24 (36,500 XP)
//! ACTIONS                             <- a section heading
//! Bite. Melee Weapon Attack: +17 to hit, reach 15ft., one target.
//! ```
//!
//! # Why the anchor is Armor Class
//!
//! Because it is the first line that is unambiguously a statblock and nothing
//! else. A name is just a heading; a size-and-type line is prose-shaped. Every
//! 5e creature has an armour class, it is always labelled, and the label
//! appears nowhere else in a book. Finding it and then reading *backwards* for
//! the name is far more reliable than trying to recognise a name in advance.

use serde::{Deserialize, Serialize};

/// One line of a document, as this reader needs it.
///
/// Mirrors `thunderforge_pdf::layout::Line` without depending on its shape —
/// a statblock is not a PDF concept, and a caller with lines from anywhere
/// else should be able to use this.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceLine {
    pub text: String,
    pub size: f64,
    pub bold: bool,
    /// One-based page number, carried so a reader can say where a creature
    /// came from. Provenance is not decoration here: spec 048's licence
    /// boundary needs to know which book a thing was taken from.
    pub page: u32,
}

/// How confident the reader is about one value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Read {
    /// Found and parsed.
    Clear,
    /// Found, but the text it came from looks damaged — letter-spacing, most
    /// often. Worth showing a person; not worth trusting silently.
    Uncertain,
    /// Not present in the block at all.
    Missing,
}

/// One attack or feature line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub name: String,
    pub text: String,
    /// Reach in feet, for a melee attack that states one.
    ///
    /// **Per attack, not per size.** A tarrasque's bite reaches 10 feet and
    /// its tail 20; an ogre is Large and its greatclub reaches 5. Deriving
    /// reach from a creature's size is the mistake spec 047 records, and this
    /// field exists so the book's own number is what survives.
    pub reach_feet: Option<f64>,
}

/// A creature, as a book describes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Statblock {
    pub name: String,
    /// "Gargantuan dragon, chaotic evil", unsplit — the line as printed.
    pub descriptor: Option<String>,
    pub size_category: Option<String>,
    pub armor_class: Option<i64>,
    pub hit_points: Option<i64>,
    pub hit_dice: Option<String>,
    pub speed: Option<String>,
    pub challenge: Option<String>,
    pub actions: Vec<Action>,
    /// Which page of the source it was found on.
    pub page: u32,
    /// Per-field confidence, for anything a person should look at before
    /// trusting (spec 048 FR-003).
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Confidence {
    pub name: ReadState,
    pub armor_class: ReadState,
    pub hit_points: ReadState,
}

/// `Read`, defaulting to missing, so a `Confidence` can be built up.
pub type ReadState = ReadDefault;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ReadDefault {
    Clear,
    Uncertain,
    #[default]
    Missing,
}

impl From<Read> for ReadDefault {
    fn from(read: Read) -> Self {
        match read {
            Read::Clear => ReadDefault::Clear,
            Read::Uncertain => ReadDefault::Uncertain,
            Read::Missing => ReadDefault::Missing,
        }
    }
}

/// The five 5e size categories, lower-cased.
const SIZES: &[&str] = &["tiny", "small", "medium", "large", "huge", "gargantuan"];

/// The labels that end a statblock's header and begin its prose.
const SECTION_HEADINGS: &[&str] = &[
    "actions",
    "reactions",
    "legendary actions",
    "lair actions",
    "regional effects",
    "bonus actions",
    "villain actions",
];

/// Find every statblock in a run of lines.
///
/// Lines must be in reading order. A block runs from its name to the start of
/// the next one, or to the end.
pub fn statblocks(lines: &[SourceLine]) -> Vec<Statblock> {
    let mut out = Vec::new();
    let anchors: Vec<usize> = (0..lines.len())
        .filter(|index| armor_class_of(&lines[*index]).is_some())
        .collect();

    for (position, anchor) in anchors.iter().enumerate() {
        let ends_at = anchors
            .get(position + 1)
            .map(|next| name_index(lines, *next).map_or(*next, |(index, _)| index))
            .unwrap_or(lines.len());
        let (starts_at, named) = name_index(lines, *anchor).unwrap_or((*anchor, false));
        if starts_at >= ends_at {
            continue;
        }
        if let Some(block) = read_one(&lines[starts_at..ends_at], named) {
            out.push(block);
        }
    }
    out
}

/// How far back from an armour class a creature's name may be.
///
/// A name, a descriptor, and sometimes a stray line of running head or a
/// page number between them. Six is enough for every book measured and short
/// enough not to reach the previous creature's prose.
const NAME_LOOKBACK: usize = 6;

/// Walk back from an armour-class line to the creature's name.
///
/// Returns the name's index and whether it is really a name. A block whose
/// name could not be found still has an armour class and hit points worth
/// keeping — it is a creature — so the descriptor stands in and the caller
/// marks it uncertain rather than throwing the creature away or presenting
/// "Medium humanoid (aarakocra), neutral good" as a monster's name, which is
/// what the first version of this did.
fn name_index(lines: &[SourceLine], anchor: usize) -> Option<(usize, bool)> {
    let lowest = anchor.saturating_sub(NAME_LOOKBACK);
    let mut descriptor_at = None;
    for index in (lowest..anchor).rev() {
        let line = &lines[index];
        if line.text.trim().is_empty() {
            continue;
        }
        if descriptor_of(&line.text).is_some() {
            descriptor_at = Some(index);
            continue;
        }
        // A name is short, and set larger or bolder than what follows.
        if line.text.chars().count() <= 60 && (line.bold || line.size > lines[anchor].size) {
            return Some((index, true));
        }
    }
    descriptor_at.map(|index| (index, false))
}

fn read_one(block: &[SourceLine], named: bool) -> Option<Statblock> {
    let first = block.first()?;
    let name = first.text.trim().to_string();
    if name.is_empty() {
        return None;
    }
    // A creature nobody can name is not worth importing. Some books use a
    // subsetted font with no usable encoding, and their text decodes into
    // something that looks like words and is not — a block read from one
    // would arrive called `* ROGGUDJRQV`. Refused outright rather than
    // flagged, because there is nothing here for a person to correct.
    if thunderforge_pdf::layout::looks_unreadable(&name) {
        return None;
    }

    let mut statblock = Statblock {
        name: name.clone(),
        descriptor: None,
        size_category: None,
        armor_class: None,
        hit_points: None,
        hit_dice: None,
        speed: None,
        challenge: None,
        actions: Vec::new(),
        page: first.page,
        confidence: Confidence {
            // Uncertain covers both ways a name can be wrong: damaged text,
            // and a name that was never found at all — in which case what is
            // standing in for it is the creature's descriptor.
            name: if named && !looks_damaged(&name) {
                ReadDefault::Clear
            } else {
                ReadDefault::Uncertain
            },
            ..Confidence::default()
        },
    };

    let mut in_actions = false;
    for line in block {
        let text = line.text.trim();
        let lowered = text.to_lowercase();

        if SECTION_HEADINGS.contains(&lowered.trim_end_matches(':')) {
            in_actions = lowered.starts_with("action")
                || lowered.starts_with("legendary")
                || lowered.starts_with("bonus")
                || lowered.starts_with("reaction")
                || lowered.starts_with("villain");
            continue;
        }

        if statblock.descriptor.is_none() {
            if let Some((size, descriptor)) = descriptor_of(text) {
                statblock.size_category = Some(size);
                statblock.descriptor = Some(descriptor);
                continue;
            }
        }
        if statblock.armor_class.is_none() {
            if let Some(value) = armor_class_of(line) {
                statblock.armor_class = Some(value);
                statblock.confidence.armor_class = read_state(text);
                continue;
            }
        }
        if statblock.hit_points.is_none() {
            if let Some((points, dice)) = hit_points_of(text) {
                statblock.hit_points = Some(points);
                statblock.hit_dice = dice;
                statblock.confidence.hit_points = read_state(text);
                continue;
            }
        }
        if statblock.speed.is_none() {
            if let Some(rest) = after_label(text, "speed") {
                statblock.speed = Some(rest.to_string());
                continue;
            }
        }
        if statblock.challenge.is_none() {
            if let Some(rest) = after_label(text, "challenge") {
                statblock.challenge = Some(rest.to_string());
                continue;
            }
        }
        if in_actions {
            if let Some(action) = action_of(text) {
                statblock.actions.push(action);
            }
        }
    }

    // Without an armour class this is not a statblock, whatever else matched.
    statblock.armor_class?;
    Some(statblock)
}

fn read_state(text: &str) -> ReadDefault {
    if looks_damaged(text) {
        ReadDefault::Uncertain
    } else {
        ReadDefault::Clear
    }
}

/// Whether a string is too letter-spaced to trust — the same judgement the
/// PDF layer makes, applied to a value rather than a heading.
fn looks_damaged(text: &str) -> bool {
    thunderforge_pdf::layout::looks_letter_spaced(text)
}

/// The text after a label, if the line starts with it.
///
/// Case- and punctuation-insensitive, because books genuinely disagree:
/// "Armor Class 15", "ARMOR CLASS 15", "Armor Class: 15", and — in a book
/// that emboldens its labels as a run of their own — "Armor Class. 15", where
/// the full stop is part of the label's styling rather than a sentence.
fn after_label<'a>(text: &'a str, label: &str) -> Option<&'a str> {
    let lowered = text.to_lowercase();
    if !lowered.starts_with(label) {
        return None;
    }
    let rest = text[label.len()..].trim_start();
    Some(rest.trim_start_matches([':', '.', '-', '\u{2014}']).trim())
}

fn first_number(text: &str) -> Option<i64> {
    let digits: String = text
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

fn armor_class_of(line: &SourceLine) -> Option<i64> {
    let rest = after_label(line.text.trim(), "armor class")
        .or_else(|| after_label(line.text.trim(), "armour class"))?;
    first_number(rest)
}

fn hit_points_of(text: &str) -> Option<(i64, Option<String>)> {
    let rest = after_label(text, "hit points")?;
    let points = first_number(rest)?;
    // The dice are in brackets: "546 (28d20 + 252)".
    let dice = rest
        .split_once('(')
        .and_then(|(_, tail)| tail.split_once(')'))
        .map(|(inside, _)| inside.trim().to_string())
        .filter(|inside| inside.contains('d'));
    Some((points, dice))
}

/// "Gargantuan dragon, chaotic evil" → the size and the whole line.
fn descriptor_of(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim();
    let first_word = trimmed.split_whitespace().next()?.to_lowercase();
    let size = SIZES.iter().find(|size| first_word.starts_with(**size))?;
    // A descriptor names a kind after the size. Without that, "Large" alone
    // is a heading in a table of sizes, not a creature.
    if trimmed.split_whitespace().count() < 2 {
        return None;
    }
    Some(((*size).to_string(), trimmed.to_string()))
}

/// `Bite. Melee Weapon Attack: +17 to hit, reach 15 ft., one target.`
fn action_of(text: &str) -> Option<Action> {
    let (name, rest) = text.split_once('.')?;
    let name = name.trim();
    // An action's name is short and titled. A sentence that merely contains a
    // full stop is prose continuing from the line before.
    if name.is_empty() || name.chars().count() > 48 {
        return None;
    }
    if !name.chars().next()?.is_uppercase() {
        return None;
    }
    Some(Action {
        name: name.to_string(),
        text: rest.trim().to_string(),
        reach_feet: reach_of(text),
    })
}

/// The reach an attack states, in feet.
///
/// Read from the attack's own words. Books write "reach 15 ft.", "reach 15
/// feet" and "reach 15ft." — the last of which is what the Monster Manual's
/// own text extraction produces, with the space lost.
fn reach_of(text: &str) -> Option<f64> {
    let lowered = text.to_lowercase();
    let at = lowered.find("reach")? + "reach".len();
    let rest = lowered[at..].trim_start();
    let number: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let number = number.trim_end_matches('.');
    let value: f64 = number.parse().ok()?;
    // Guard against "reach" used in prose followed by an unrelated number.
    let tail = rest[number.len()..].trim_start();
    (tail.starts_with("ft") || tail.starts_with("feet") || tail.starts_with("'")).then_some(value)
}

#[cfg(test)]
#[path = "statblock_tests.rs"]
mod tests;
