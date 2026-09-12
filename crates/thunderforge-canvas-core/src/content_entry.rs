//! What a reader got out of a document (spec 049).
//!
//! The result types, beside [`crate::content_patterns`]'s declaration of what
//! to look for. They live here rather than in the server for one reason: a
//! system pack contributes a **refinement** over a generic read
//! ([`crate::system_contribution`]), and a function pointer collected in this
//! crate has to be able to name the types it operates on.
//!
//! Deliberately free of any dependency on the PDF layer. The guards that
//! decide whether text is trustworthy need it and stay in the server; these
//! are plain data, and dragging a document parser into a crate the engine
//! compiles to wasm would cost every player who never imports anything.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One line of a document, as a reader sees it.
///
/// The same shape the 5e statblock reader has always taken, lifted into
/// shared code so there is one of it. Deliberately not
/// `thunderforge_pdf::layout::Line`: that carries the geometry the layout
/// pass needed and a reader does not, and taking it here would tie every
/// reader to the PDF crate's internals.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceLine {
    pub text: String,
    pub size: f64,
    pub bold: bool,
    /// One-based, as a person would cite it.
    pub page: u32,
    /// The layout pass did not trust the text on this line — letter-spaced,
    /// or from a font this build cannot decode (049 FR-004).
    pub suspect: bool,
}

/// What was read for one field, and how much to trust it.
///
/// An enum rather than a `{ state, value }` pair so that **unread carries no
/// value by construction**. Spec 049 FR-002 says absence must be recorded as
/// absence and no value invented to fill it; a struct with an optional value
/// beside a state can express "unread, and here is the value anyway", and
/// anything expressible eventually gets written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "state", content = "value")]
pub enum ReadValue {
    /// Found, and the reader has no reason to doubt it.
    Clear(String),
    /// Found, and worth a person's eye: the text was damaged, or it would not
    /// parse as the kind the system declared. The text is kept **exactly as
    /// read** — never coerced, never blanked — because a value somebody can
    /// check is worth more than one quietly presented as fact.
    Uncertain(String),
    /// Looked for, not found.
    Unread,
}

impl ReadValue {
    /// The text, if anything was read at all.
    pub fn text(&self) -> Option<&str> {
        match self {
            ReadValue::Clear(value) | ReadValue::Uncertain(value) => Some(value),
            ReadValue::Unread => None,
        }
    }

    pub fn is_unread(&self) -> bool {
        matches!(self, ReadValue::Unread)
    }
}

/// One thing read out of a book: a spell, an item, a creature, a feat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// The system's own word for what this is. Carried, never switched on.
    pub kind: String,
    pub name: String,
    /// How much to trust the name. A block whose name could not be found is
    /// still worth keeping when its declared fields read cleanly.
    pub name_state: NameState,
    /// One-based page, so a Game Master can look it up in the real book.
    pub page: u32,
    /// The declared fields. Anchored kinds only; every declared field appears,
    /// including the ones that were not found.
    #[serde(default)]
    pub values: BTreeMap<String, ReadValue>,
    /// The entry's text. Prose kinds only.
    #[serde(default)]
    pub text: Option<String>,
    /// Built from lines the layout pass did not trust (049 FR-004).
    pub suspect: bool,
    /// Whatever the system's own pack added that a declaration could not
    /// express — a 5e creature's per-attack reach, for instance.
    ///
    /// Opaque on purpose. Shared code carries this and never looks inside it,
    /// which is what lets a pack own a reading only it understands without
    /// shared code learning that system's vocabulary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<serde_json::Value>,
}

/// Whether an entry's name is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NameState {
    Clear,
    Uncertain,
}
