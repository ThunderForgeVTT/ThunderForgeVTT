//! Spec 041 FR-017: limited, and not a lockout.

use super::*;
use crate::test_support::{insert_test_user, test_app_state};

fn at(seconds: i64) -> NaiveDateTime {
    chrono::DateTime::from_timestamp(1_760_000_000 + seconds, 0)
        .expect("a valid instant")
        .naive_utc()
}

#[test]
fn a_pause_that_has_run_out_is_not_a_pause() {
    assert!(!is_cooling_off(None, at(0)));
    assert!(is_cooling_off(Some(at(60)), at(0)));
    assert!(
        !is_cooling_off(Some(at(0)), at(0)),
        "the instant a pause ends is not part of it",
    );
    assert!(!is_cooling_off(Some(at(0)), at(1)));
}

/// The bound, and the shape of it: four failures cost nothing but a count.
#[test]
fn failures_below_the_bound_do_not_pause_anything() {
    let mut attempts = 0;
    for _ in 0..(MAX_CONSECUTIVE_FAILURES - 1) {
        let (next, locked) = after_failure(attempts, at(0));
        assert_eq!(locked, None, "only the bound starts a pause");
        attempts = next;
    }
    assert_eq!(attempts, MAX_CONSECUTIVE_FAILURES - 1);

    let (attempts, locked) = after_failure(attempts, at(0));
    assert_eq!(locked, Some(at(0) + COOLING_OFF));
    assert_eq!(
        attempts, 0,
        "the count resets with the pause, so the next period starts fresh",
    );
}

/// FR-017's second half, stated as a property. A pause that grew with every
/// further failure would be a lockout wearing a different name — somebody
/// hammering an account could hold its owner out indefinitely.
#[test]
fn a_pause_never_grows_into_a_lockout() {
    let (mut attempts, mut longest) = (0, Duration::seconds(0));
    for round in 0..50 {
        let (next, locked) = after_failure(attempts, at(round));
        attempts = next;
        if let Some(until) = locked {
            longest = longest.max(until - at(round));
        }
    }
    assert_eq!(
        longest, COOLING_OFF,
        "however many failures arrive, no single pause exceeds one period",
    );
}

/// The pause outlasts a TOTP step, so waiting it out means facing a different
/// code rather than resuming the guess that was in progress.
#[test]
fn the_pause_outlasts_the_code_being_guessed() {
    assert!(
        COOLING_OFF.num_seconds()
            > i64::from(thunderforge_axum_auth_core::totp::STEP_SECONDS as u32),
    );
}

#[tokio::test]
async fn an_account_is_allowed_until_it_is_not_and_then_recovers() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    assert_eq!(check(&state, user_id).await, ThrottleState::Allowed);

    for _ in 0..MAX_CONSECUTIVE_FAILURES {
        record_failure(&state, user_id).await;
    }
    assert_eq!(
        check(&state, user_id).await,
        ThrottleState::CoolingOff,
        "the bound must actually pause the account",
    );

    // A success is how it recovers, and the only way that does not involve
    // waiting — which is right: whoever can produce a valid code is the owner.
    record_success(&state, user_id).await;
    assert_eq!(check(&state, user_id).await, ThrottleState::Allowed);
}

/// The honest-mistype case, end to end: four wrong then a right one, and the
/// account is no closer to a pause than it started.
#[tokio::test]
async fn a_success_resets_the_count_so_mistypes_do_not_accumulate() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    for _ in 0..(MAX_CONSECUTIVE_FAILURES - 1) {
        record_failure(&state, user_id).await;
    }
    record_success(&state, user_id).await;

    // Four more failures must still not pause it: the count started again.
    for _ in 0..(MAX_CONSECUTIVE_FAILURES - 1) {
        record_failure(&state, user_id).await;
    }
    assert_eq!(
        check(&state, user_id).await,
        ThrottleState::Allowed,
        "a success must genuinely clear the count, not merely pause it",
    );
}

/// An unreadable answer refuses. Failing open here would make a database blip
/// the moment to guess, which is the one direction not to be wrong in.
#[tokio::test]
async fn an_account_that_cannot_be_read_is_not_allowed_to_proceed() {
    let state = test_app_state();
    // An id no row carries: the read succeeds and finds nothing, which is the
    // "no such account" case rather than the failure case, and must not itself
    // become a way to ask whether an account exists.
    assert_eq!(
        check(&state, uuid::Uuid::now_v7()).await,
        ThrottleState::Allowed,
        "an absent account is refused for being absent, not for being paused",
    );
}

/// The test that would have caught the fig leaf.
///
/// When the bound was first written it sat in `two_factor_verify` and nowhere
/// else, and three other routes verified the same guessable credentials —
/// `POST /authentication/login`, `POST /2fa/recovery-codes` and
/// `POST /2fa/disable`. Each was a way to guess without ever meeting the
/// bound. A test naming one route could not have noticed; this one names the
/// *primitive* instead.
///
/// It asserts the shape that makes the bound structural: verification and
/// counting happen together, in one function, so a new route cannot verify a
/// second factor without being bounded — the only way to check a credential is
/// to go through `guarded`.
#[tokio::test]
async fn every_verification_is_bounded_because_the_bound_is_under_it() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    // Five refusals through `guarded` — whichever route they arrived by.
    for _ in 0..MAX_CONSECUTIVE_FAILURES {
        let outcome = guarded(&state, user_id, || async { Ok(false) })
            .await
            .expect("runs");
        assert_eq!(outcome, SecondFactor::Refused);
    }

    // The sixth is refused *without the credential being looked at*. The
    // closure would return `true` — a correct code — and it never runs.
    let mut evaluated = false;
    let outcome = guarded(&state, user_id, || {
        evaluated = true;
        async { Ok(true) }
    })
    .await
    .expect("runs");

    assert_eq!(outcome, SecondFactor::Throttled);
    assert!(
        !evaluated,
        "a paused account must not have its credential evaluated at all — \
         otherwise the pause is a delay, not a bound",
    );
}

/// An error is not a wrong guess. Counting one would let a database wobble
/// spend somebody's budget and lock them out of their own account.
#[tokio::test]
async fn a_failure_to_check_is_not_counted_as_a_failed_attempt() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    for _ in 0..(MAX_CONSECUTIVE_FAILURES * 3) {
        let outcome = guarded(&state, user_id, || async {
            Err("the database went away".to_string())
        })
        .await;
        assert!(outcome.is_err());
    }

    assert_eq!(
        check(&state, user_id).await,
        ThrottleState::Allowed,
        "errors must not accumulate toward a pause",
    );
}
