//! Spec 041 FR-028 / spec 040 FR-002a: an instance is not set up until its
//! first administrator holds a confirmed second factor.
//!
//! # The two halves, and why one of them stays as it is
//!
//! `setup_status` used to read completion as
//! `admin_exists || setup_completed_at.is_some()`, and the task that produced
//! this file asked for **both** halves to move.
//!
//! Only one of them could. The first — `setup_completed_at` — is now written
//! at `/authentication/setup/complete` and nowhere else, and that route
//! refuses with `409 second_factor_required` while the first administrator has
//! no confirmed factor. That is the gate, and it is what FR-028 is asking for:
//! a fresh instance cannot finish setup without one.
//!
//! The second half — `admin_exists && the bootstrap code was consumed` —
//! stays, and adding a second-factor condition to it would be a serious
//! regression rather than a stricter reading of FR-028. Every instance that
//! completed setup before this feature existed has an administrator with no
//! second factor. Requiring one there would put every one of them back into
//! the first-run wizard, on an instance with live worlds and real accounts,
//! with a bootstrap code that was consumed long ago and cannot be presented
//! again. That is a lockout, which is the class of failure this whole spec was
//! written to remove.
//!
//! So the rule is: **a fresh instance is gated, an upgraded one is not
//! reopened**, and administrators on an upgraded instance meet the requirement
//! at their next sign-in instead (FR-031), where the answer is an enrolment
//! rather than a locked door.

use crate::auth::admin_bootstrap::setup_is_completed;
use crate::auth::setup_requirements::SetupCompletion;
use crate::models::AdminBootstrapSetup;

fn setup_row(
    completed_at: Option<chrono::NaiveDateTime>,
    code_hash: Option<&str>,
) -> AdminBootstrapSetup {
    let now = chrono::Utc::now().naive_utc();
    AdminBootstrapSetup {
        id: 1,
        setup_completed_at: completed_at,
        admin_code_hash: code_hash.map(str::to_string),
        admin_code_generated_at: code_hash.map(|_| now),
        created_at: now,
        updated_at: now,
    }
}

/// A fresh instance, mid-wizard: the administrator exists and the bootstrap
/// code has not been consumed. Setup is **not** finished.
///
/// This is the state FR-028 is about, and it is the state the account step
/// leaves behind now that it no longer marks completion (ADR-093). Reading it
/// as complete is what let an instance finish with an administrator who had
/// nothing but a password.
#[test]
fn an_administrator_mid_wizard_does_not_mean_setup_is_finished() {
    assert!(
        !setup_is_completed(true, Some(&setup_row(None, Some("still-unconsumed")))),
        "the account step is the middle of the wizard, not the end of it",
    );
}

/// And the gate itself: completion is refused while that administrator has no
/// confirmed factor, whatever else is in order.
#[test]
fn completion_is_refused_until_the_first_administrator_confirms_one() {
    let unconfirmed = SetupCompletion {
        missing: Vec::new(),
        second_factor_confirmed: false,
    };
    assert!(
        unconfirmed.refusal().is_some(),
        "every required setting can be answered and setup still must not \
         finish without a second factor",
    );

    let confirmed = SetupCompletion {
        missing: Vec::new(),
        second_factor_confirmed: true,
    };
    assert!(
        confirmed.refusal().is_none(),
        "and with one confirmed, nothing here stands in the way",
    );
}

/// The back-compatibility arm, asserted deliberately rather than left to be
/// discovered.
///
/// An instance whose bootstrap code was consumed and whose administrator has
/// no second factor reads as **set up**. That is not an oversight: see this
/// module's header. If somebody tightens this, every existing deployment
/// reopens its first-run wizard with a code it cannot produce.
#[test]
fn an_upgraded_instance_is_not_dragged_back_into_setup() {
    assert!(
        setup_is_completed(true, Some(&setup_row(None, None))),
        "an instance that finished setup before this feature existed is still \
         finished; its administrators meet the requirement at their next \
         sign-in (FR-031), not by being locked out",
    );
    assert!(
        setup_is_completed(true, None),
        "and neither is one that has no bootstrap row at all",
    );
}
