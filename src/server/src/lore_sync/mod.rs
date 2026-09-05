//! Spec 034: mirroring a world's lore into a repository its owner controls.
//!
//! # What is here, and the guarantee that changed
//!
//! Until 2026-09-04 this module could not damage a world **by construction**:
//! every lore table was read and none was written, so "a first delivery cannot
//! damage a world" was a property of the code's shape rather than of anyone's
//! care. That paragraph is gone because the thing it described is gone —
//! `incoming` writes revisions, and leaving a comfortable claim standing after
//! it stopped being true is worse than never having made it.
//!
//! What replaces a structural guarantee is a narrower one, and it is worth
//! knowing exactly how narrow. **`incoming::IncomingEnabled` is the only key to
//! every write path**: its fields are private, its sole constructor refuses a
//! connection that has not opted in (FR-022) or that has been deactivated
//! (FR-041a), and `detect`, `record`, `accept` and `decline` all demand one.
//! There is no way to spell a lore write for a world that never asked for it.
//!
//! So the guarantee is no longer "this module cannot write" but "this module
//! cannot write **without a value only an opted-in connection can produce**".
//! That is weaker, it is checked by the compiler rather than by review, and a
//! change that widens `IncomingEnabled`'s constructor is the change to look at
//! hardest — not a change that adds another function.
//!
//! Export remains what most of this module does, and User Story 3 remains the
//! only part that can put text into a world its members did not write in the
//! app.
//!
//! # The shape of a synchronisation
//!
//! One pass fetches the remote, diffs the world against a working clone,
//! writes files, commits, pushes, and verifies — reading and writing on the
//! same pass so that divergence detection (FR-031) and write verification
//! (FR-034) are answered from remote state already fetched rather than from an
//! extra round trip.
//!
//! # Where the host lives
//!
//! Nowhere in this module. Arranging access to a repository is
//! `crates/thunderforge-repo-host`, and what crosses back is a credential and
//! an expiry — FR-004c forbids anything past the grant from knowing which host
//! arranged it.
//!
//! The mechanical form of that rule is a case-insensitive grep of this
//! directory for any repository host's name, which must find nothing. It is
//! stated that way round on purpose: naming the vendor here to say "do not
//! name the vendor here" would be the first hit, and a rule that fails its own
//! check teaches the next reader to ignore it.
//!
//! Git over HTTPS is the transport for the same reason: it *is* the
//! host-neutral protocol, where a host's REST API would put one vendor inside
//! this module. See `specs/034-lore-git-sync/research.md` R1.

pub mod apply;
pub mod binding;
pub mod disassociate;
pub mod document;
pub mod git;
pub mod incoming;
pub mod paths;
pub mod plan;
pub mod schedule;
pub mod state;
pub mod takedown_hook;
pub mod workspace;

#[cfg(test)]
#[path = "constraints_tests.rs"]
mod constraints_tests;

#[cfg(test)]
#[path = "scale_tests.rs"]
mod scale_tests;

#[cfg(test)]
#[path = "git_roundtrip_tests.rs"]
mod git_roundtrip_tests;
