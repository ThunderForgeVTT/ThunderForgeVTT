//! Spec 034: mirroring a world's lore into a repository its owner controls.
//!
//! # What is here, and what deliberately is not
//!
//! The first delivery is **export only** — User Story 1 (the mirror) and User
//! Story 2 (failing without harm). User Story 3, accepting edits made in the
//! repository, is separately scheduled and may never be built.
//!
//! That boundary is the most important property of this module and it is
//! visible in the code rather than only asserted: **nothing here writes to a
//! world's lore.** Every lore table is read and none is written, which is what
//! makes "a first delivery cannot damage a world" true by construction rather
//! than by care. A change that adds a write path to this module has left the
//! first delivery, whatever the commit message says.
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
pub mod document;
pub mod git;
pub mod paths;
pub mod plan;
pub mod workspace;

#[cfg(test)]
#[path = "constraints_tests.rs"]
mod constraints_tests;

#[cfg(test)]
#[path = "git_roundtrip_tests.rs"]
mod git_roundtrip_tests;
