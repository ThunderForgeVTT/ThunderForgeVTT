//! Finding a system's content in a document's text (spec 049).
//!
//! `thunderforge-pdf` turns a book into positioned, styled lines and knows
//! nothing about games. A system's pack declares what its content looks like
//! in those lines ([`thunderforge_canvas_core::content_patterns`]). This is
//! what sits between them, and it names no game system — the build check
//! `scripts/check-system-registry.mjs` fails if shared code ever does.
//!
//! # Why a crate of its own
//!
//! The reading happens on the Game Master's own machine (FR-020) and the
//! server re-checks what arrives (FR-036). So this has to compile to wasm for
//! the browser *and* to native for the server, and be the same code both
//! times — two implementations of "what does this book say" would be the
//! worst of both.
//!
//! It therefore depends on **neither** the PDF layer nor the server. The
//! judgements about whether text is trustworthy live here rather than in the
//! layout pass ([`text`]) precisely so that this crate sits underneath both.
//!
//! # Two readers, because real books have two shapes (ADR-096)
//!
//! - [`anchored`] for labelled blocks — a creature's armour class, a spell's
//!   casting time;
//! - [`prose`] for a name and paragraphs, which is what magic items and feats
//!   actually are. A prose kind must say what confirms it, or it matches every
//!   heading in every book — measured, and the reason `confirmedBy` exists.

pub mod anchored;
pub mod prose;
pub mod text;

pub use text::{after_label, is_mostly_letters, looks_damaged, looks_unreadable};
pub use thunderforge_canvas_core::content_entry::{Entry, NameState, ReadValue, SourceLine};

#[cfg(test)]
#[path = "anchored_tests.rs"]
mod anchored_tests;
#[cfg(test)]
#[path = "prose_tests.rs"]
mod prose_tests;
