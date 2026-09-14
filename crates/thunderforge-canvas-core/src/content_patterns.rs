//! How a game system says what its content looks like in a book (spec 049).
//!
//! `thunderforge-pdf` turns a document into positioned, styled lines. This
//! decides what to look for in them: a system declares what anchors a spell,
//! an item or a creature, and which fields to read once one is found.
//!
//! # Why declared rather than built in
//!
//! "Armor Class" is a D&D phrase. Pathfinder says AC, another system says
//! something else again, and shared code that learned all three would have to
//! be edited every time a system it had never heard of arrived — which is
//! exactly what the pack architecture exists to prevent. The build already
//! enforces it: `scripts/check-system-registry.mjs` fails if shared server or
//! web code so much as quotes a system's id.
//!
//! This follows [`crate::vision_declaration`] deliberately, down to the shape:
//! a system declares where its own meaning lives and shared code reads the
//! declaration rather than the words.
//!
//! # Two shapes, because books have two
//!
//! Measured against a real library rather than assumed:
//!
//! - **Anchored** content — spells, magic items, creatures — is a block of
//!   labelled fields introduced by a label that appears nowhere else. Armour
//!   class is the worked example: every 5e creature has one, it is always
//!   labelled, and that is why 717 creatures could be read out of six books.
//! - **Prose** content — class features, feats, subclass options — is a bold
//!   run-in name and paragraphs. There is no label, no fixed field, and no
//!   boundary but the next name.
//!
//! A prose entry therefore has *nowhere to put* a mechanical value, and that
//! is the point rather than a limitation: every parser defect found while
//! building this reader produced text that looked plausible and was wrong, so
//! a shape that cannot invent a damage value is worth more than one that
//! might.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Everything a system declares about finding its content in a document.
///
/// A system that declares nothing cannot have a book read into it, and is
/// told so before a file is opened. That is a refusal rather than a fallback:
/// guessing with another system's vocabulary is how you import a Pathfinder
/// book as badly-parsed D&D.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContentPatterns {
    #[serde(default)]
    pub patterns: Vec<Pattern>,
}

impl ContentPatterns {
    /// Nothing declared, so nothing can be read.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// The pattern for one kind, if this system declares it.
    pub fn for_kind(&self, kind: &str) -> Option<&Pattern> {
        self.patterns.iter().find(|p| p.kind == kind)
    }
}

/// One kind of content, and how to find it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Pattern {
    /// What this finds — `spell`, `creature`, `feat`. Open on purpose: shared
    /// code never switches on the value, it only carries it through, so a
    /// system may name kinds this repository has never heard of.
    pub kind: String,
    pub shape: Shape,
    /// The label that unambiguously begins an entry. Anchored kinds only.
    ///
    /// "Unambiguously" is the whole requirement. A label that also occurs
    /// inside an entry's prose will start entries in the middle of other
    /// entries — which is why the 5e spell pattern anchors on `Casting Time`
    /// and not on `Range`.
    #[serde(default)]
    pub anchor: Option<String>,
    pub name: NameRule,
    /// The labels to read once an anchor is found. Anchored kinds only.
    #[serde(default)]
    pub fields: Vec<FieldSpec>,
    /// What confirms that a prose entry is one of *these*. Prose kinds only,
    /// and required for them.
    ///
    /// Measured on 2026-09-12, and the reason this field exists: a prose kind
    /// declared as "a heading, then paragraphs" describes **every section of
    /// every book**. Without a discriminator the reader returned 72,974 magic
    /// items across 246 books, including `Table of Contents` and `About`, and
    /// returned exactly the same number for feats because the two declarations
    /// were identical.
    ///
    /// At least one of these phrases must appear near the name for the entry
    /// to count. A magic item is confirmed by its type line — `Wondrous item`,
    /// `Weapon (`, `Potion` — and a feat by `Prerequisite`.
    #[serde(default)]
    pub confirmed_by: Vec<String>,
    /// How many lines after the name to look for a confirmation. Three by
    /// default: a type line, a rarity, and one line of slack.
    #[serde(default)]
    pub confirm_within: Option<u32>,
}

/// Whether an entry of this kind carries labelled fields, or only prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Shape {
    Anchored,
    Prose,
}

/// How an entry's name is found.
///
/// Every field is optional and which ones apply depends on the shape, because
/// a manifest is JSON somebody writes by hand and serde cannot key one
/// field's validity on another's value. The combination is checked by
/// `pack_system_spec::validate_system_manifest`, which is where a system
/// author gets told they wrote something that cannot work.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NameRule {
    /// Anchored: which side of the anchor the name sits on.
    #[serde(default)]
    pub position: Option<NamePosition>,
    /// Anchored: how many lines away to look. The 5e creature rule is six —
    /// a statblock's name, its size-and-type line and its armour class are
    /// not always adjacent.
    #[serde(default)]
    pub within_lines: Option<u32>,
    /// Anchored: which candidate line to take when several are in range.
    #[serde(default)]
    pub prefer: Option<NamePreference>,
    /// Prose: what a name looks like.
    #[serde(default)]
    pub style: Option<NameStyle>,
    /// Prose: where the entry stops.
    #[serde(default)]
    pub ends_at: Option<ProseEnd>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NamePosition {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NamePreference {
    /// The largest text in range — a statblock's title is set bigger.
    Largest,
    Bold,
    /// Nearest the anchor.
    Nearest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NameStyle {
    Bold,
    Heading,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ProseEnd {
    NextName,
    NextHeading,
}

/// One labelled field to read out of an anchored entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FieldSpec {
    /// What the read value is called on the entry.
    pub key: String,
    /// The label to look for in the document.
    pub label: String,
    /// `as` in the manifest, because that reads correctly to a system author
    /// writing JSON and is a reserved word to Rust.
    #[serde(rename = "as")]
    pub value_kind: FieldKind,
}

/// What a field's text is meant to be.
///
/// Three, and no more. A value that will not parse as its declared kind is
/// recorded *uncertain* with the text exactly as read — never coerced, never
/// dropped. A richer type language here would be a language, and the thing it
/// would buy is the ability to be confidently wrong about more fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FieldKind {
    Integer,
    Number,
    Text,
}
