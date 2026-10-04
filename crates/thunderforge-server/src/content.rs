//! Reading a system's content out of a document (spec 049).
//!
//! The readers live in `crates/thunderforge-content`, not here, because the
//! reading happens in the Game Master's own browser (FR-020) and the server
//! re-checks what arrives (FR-036) — so the same code has to compile to wasm
//! and to native. Re-exported rather than wrapped: a wrapper is a place for
//! the two sides to drift.

pub use thunderforge_content::{
    Entry, NameState, ReadValue, SourceLine, after_label, anchored, is_mostly_letters,
    looks_damaged, looks_unreadable, prose,
};
