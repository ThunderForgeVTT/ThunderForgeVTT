//! Two-factor enrolment, verification, and the two admin switches that make
//! it mandatory.
//!
//! # Why this is a directory and not a file
//!
//! It was one 787-line file against a 1000-line gate, and every task in spec
//! 041 wanted to edit it. Research R11 called for the split; T005 did it as
//! **pure movement** — the same functions, in the same order, with the same
//! bodies — so that the diff which moved them proves nothing changed, and
//! every change after it lands in a file somebody can hold in their head.
//!
//! The seam is the question each file answers:
//!
//! - [`enrolment`] — turning one **on** (US1, US3).
//! - [`verification`] — proving one already held (US2, FR-016).
//! - [`policy`] — the switches that make holding one mandatory (FR-019,
//!   FR-023), as distinct from [`requirement`], which is the *rule* they feed.
//! - [`disable`] — the deliberate way **off** (FR-012, FR-014).
//! - [`recovery`] — the codes for when the authenticator is gone (US2).
//! - [`operator_reset`] — when the codes are gone too (FR-024, FR-025).
//! - [`throttle`] — how many wrong answers an account gets (FR-017).
//! - [`events`] — what happened to a factor, and who did it (FR-015).
//!
//! Everything is re-exported here, so callers outside `auth::two_factor` see
//! the same surface they saw when it was one file.

use super::*;

/// Spec 041 US1/US3: the pending secret, the QR code, and confirmation.
pub(crate) mod enrolment;
pub(crate) use enrolment::*;

/// Spec 041 US2/FR-016: the challenge, and the TOTP step it spends.
pub(crate) mod verification;
pub(crate) use verification::*;

/// Spec 041 FR-019/FR-023: the two admin switches and the instance setting.
pub(crate) mod policy;
pub(crate) use policy::*;

/// Spec 041 US2 (FR-006 … FR-011): the recovery codes issued at confirmation
/// and spent at the challenge.
pub(crate) mod recovery;
pub(crate) use recovery::*;

/// Spec 041 US4 (FR-012, FR-014): the deliberate way off, which did not exist
/// — `setup/start`'s side effect had been doing the job instead.
pub(crate) mod disable;

/// Spec 041 US4 (FR-019 … FR-022): who must hold a factor, what a sign-in does
/// about it, and the ticket that lets an account caught by a requirement enrol
/// at the moment it is asked to.
pub(crate) mod requirement;
pub(crate) use requirement::*;

/// Spec 041 FR-024/FR-025: an operator resets a second factor for somebody who
/// has lost both their authenticator and their recovery codes. Admin-only, and
/// recorded with the operator's own id against it.
pub(crate) mod operator_reset;

/// Spec 041 FR-017: repeated incorrect codes are limited **per account**, not
/// merely per address. See the module header for why the difference matters.
pub(crate) mod throttle;

/// Spec 041 FR-015 / FR-025: what happened to a second factor, and who did
/// it. The act, never the person — see the module header.
pub(crate) mod events;

/// Spec 041 FR-016: a code is not accepted twice, including within its window.
/// Tests the join between the rule in `totp.rs` and the path a sign-in takes.
#[cfg(test)]
mod replay_tests;

/// Spec 041 FR-009: a code is shown once, and no route hands one back.
#[cfg(test)]
#[path = "no_code_readback_tests.rs"]
mod no_code_readback_tests;

/// Spec 041 FR-018: one message and one status for every credential refusal.
#[cfg(test)]
#[path = "refusal_shape_tests.rs"]
mod refusal_shape_tests;
