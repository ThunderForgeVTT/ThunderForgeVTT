//! Spec 041 FR-017: repeated incorrect codes are limited, per **account**.
//!
//! # The gap this closes
//!
//! `rate_limit_auth_requests` caps `/authentication/` at 40 requests a minute
//! per address. That bounds one attacker from one place. It does not bound
//! attempts against one *account*, and here that distinction is the whole
//! thing: a TOTP code is six digits and stays valid for 30-90 seconds, so a
//! per-address cap spread over rotating addresses bounds nothing.
//!
//! The replay guard in `totp.rs` stops a code being *reused*. This stops one
//! being *found*. They are different holes and both were open.
//!
//! # A cooling-off, not a lockout
//!
//! FR-017's second half — an honest mistype must not cost somebody their
//! account. So failures buy a short pause that clears itself, never a state an
//! administrator has to undo. [`MAX_CONSECUTIVE_FAILURES`] is well past
//! fumbling a code and far short of a search, and **any success resets the
//! count**, so somebody who mistypes twice and then succeeds is no closer to a
//! pause than somebody who never mistyped.
//!
//! # Why the count is in the database
//!
//! The existing limiter is a process-local `HashMap`: a second server process
//! is a second budget and a restart is a fresh one. Neither is acceptable in
//! front of a second factor.

use chrono::{Duration, NaiveDateTime, Utc};
use diesel::prelude::*;

use crate::schema::users;
use crate::state::AppState;

/// Consecutive failures that buy a pause.
///
/// Five. Enough that somebody reading a code off a phone in bad light does not
/// meet it; few enough that a search is throttled to roughly five guesses per
/// cooling-off period, which against a million possibilities is not a search
/// at all.
pub(crate) const MAX_CONSECUTIVE_FAILURES: i32 = 5;

/// How long the pause lasts.
///
/// Sixty seconds — longer than a TOTP step, so a paused attacker cannot simply
/// wait out the code they were guessing and keep the same target; short enough
/// that a person who genuinely mistyped five times is inconvenienced rather
/// than locked out.
pub(crate) const COOLING_OFF: Duration = Duration::seconds(60);

/// Why verification was refused before the code was even looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThrottleState {
    /// Go ahead and evaluate the code.
    Allowed,
    /// In a cooling-off period. The code is not evaluated at all — which is
    /// also what makes this cheap enough to be worth having.
    CoolingOff,
}

/// Whether `locked_until` still holds at `now`.
///
/// Pure, so the boundary can be asserted without waiting a minute. A lock
/// exactly at `now` has expired: the pause is a duration, and the instant it
/// ends is not part of it.
pub(crate) fn is_cooling_off(locked_until: Option<NaiveDateTime>, now: NaiveDateTime) -> bool {
    locked_until.is_some_and(|until| until > now)
}

/// The count and pause after a failure.
///
/// Returns the new `(attempts, locked_until)`. Reaching the bound starts a
/// pause **and resets the count**, so the next period starts fresh rather than
/// every subsequent failure extending an ever-growing lock — which is how a
/// cooling-off quietly becomes the lockout FR-017 forbids.
pub(crate) fn after_failure(attempts: i32, now: NaiveDateTime) -> (i32, Option<NaiveDateTime>) {
    let attempts = attempts.saturating_add(1);
    if attempts >= MAX_CONSECUTIVE_FAILURES {
        (0, Some(now + COOLING_OFF))
    } else {
        (attempts, None)
    }
}

/// May this account attempt a second factor right now?
pub(crate) async fn check(state: &AppState, user_id: uuid::Uuid) -> ThrottleState {
    let now = Utc::now().naive_utc();
    let Ok(mut conn) = state.db_pool.get() else {
        // Unreadable state refuses. This is the one direction to be wrong in:
        // failing open here would make a database blip the moment to guess.
        return ThrottleState::CoolingOff;
    };

    let locked_until = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::id.eq(user_id))
            .select(users::two_factor_locked_until)
            .first::<Option<NaiveDateTime>>(&mut conn)
            .optional()
    })
    .await;

    match locked_until {
        Ok(Ok(Some(until))) if is_cooling_off(until, now) => ThrottleState::CoolingOff,
        // An account that does not exist is not throttled *here* — the caller
        // refuses it for being absent, and answering differently would make
        // this a way to ask whether an account exists.
        Ok(Ok(_)) => ThrottleState::Allowed,
        _ => ThrottleState::CoolingOff,
    }
}

/// Record a failed attempt, starting a cooling-off period if it is the fifth.
pub(crate) async fn record_failure(state: &AppState, user_id: uuid::Uuid) {
    let now = Utc::now().naive_utc();
    let Ok(mut conn) = state.db_pool.get() else {
        return;
    };

    let _ = tokio::task::spawn_blocking(move || {
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            // Both read in one go, and the row is locked for the rest of the
            // transaction: two failures arriving together must not both read
            // the same count and both write "one more than it".
            let (attempts, existing_lock) = users::table
                .filter(users::id.eq(user_id))
                .select((
                    users::two_factor_failed_attempts,
                    users::two_factor_locked_until,
                ))
                .for_update()
                .first::<(i32, Option<NaiveDateTime>)>(conn)?;

            let (attempts, started_lock) = after_failure(attempts, now);
            diesel::update(users::table.filter(users::id.eq(user_id)))
                .set((
                    users::two_factor_failed_attempts.eq(attempts),
                    // A failure that does not reach the bound must leave a
                    // pause already running exactly as it was, rather than
                    // clearing it — otherwise a steady trickle of guesses
                    // would keep resetting its own punishment.
                    users::two_factor_locked_until.eq(started_lock.or(existing_lock)),
                ))
                .execute(conn)?;
            Ok(())
        })
    })
    .await;
}

/// Clear the count and any pause. Called on every success.
pub(crate) async fn record_success(state: &AppState, user_id: uuid::Uuid) {
    let Ok(mut conn) = state.db_pool.get() else {
        return;
    };
    let _ = tokio::task::spawn_blocking(move || {
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set((
                users::two_factor_failed_attempts.eq(0),
                users::two_factor_locked_until.eq(None::<NaiveDateTime>),
            ))
            .execute(&mut conn)
    })
    .await;
}

/// What a guarded second-factor check concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SecondFactor {
    /// The credential was good.
    Held,
    /// It was not. Indistinguishable, to the caller, from a replayed code or
    /// a spent recovery code — all three are "that did not work".
    Refused,
    /// The account is pausing. The credential was **not evaluated**, so this
    /// costs nothing and cannot itself be used to test one.
    Throttled,
}

/// Verify a second factor with the per-account bound applied.
///
/// # Why this exists rather than a check in each handler
///
/// Because a check in each handler is a check somebody forgets. When the bound
/// was first added it guarded `POST /2fa/verify` and nothing else — and three
/// other routes verify the same guessable credentials:
/// `POST /authentication/login` (which takes `two_factor_code` directly),
/// `POST /2fa/recovery-codes`, and `POST /2fa/disable`. An attacker would
/// simply have used one of those, and the bound would have been decoration.
///
/// So the bound lives *under* the verification rather than in front of it, and
/// every route is covered by construction — including routes not written yet.
pub(crate) async fn guarded<F, Fut>(
    state: &AppState,
    user_id: uuid::Uuid,
    verify: F,
) -> Result<SecondFactor, String>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<bool, String>>,
{
    if check(state, user_id).await == ThrottleState::CoolingOff {
        return Ok(SecondFactor::Throttled);
    }

    match verify().await {
        Ok(true) => {
            record_success(state, user_id).await;
            Ok(SecondFactor::Held)
        }
        Ok(false) => {
            record_failure(state, user_id).await;
            Ok(SecondFactor::Refused)
        }
        // An error is not a wrong guess. Counting it would let a database
        // wobble lock somebody out of their own account.
        Err(message) => Err(message),
    }
}

#[cfg(test)]
#[path = "throttle_tests.rs"]
mod throttle_tests;
