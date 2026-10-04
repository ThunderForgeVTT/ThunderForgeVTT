//! FR-006: submission is rate limited per account.
//!
//! # Why a third limiter and not one of the two that exist
//!
//! `auth_middleware::rate_limit_auth_requests` keys on the request **path**
//! and returns early unless it contains `/authentication/`; every GraphQL
//! operation in this product arrives at one path, so extending it would
//! rate-limit the whole application against a threshold written for password
//! attempts. `graphql/share_rate_limit.rs` exists because of that, and it
//! keys on an anonymous caller for a threshold written for share codes. This
//! one sits in the same place — inside the resolver, where the operation is
//! known — and keys on the **account id**, because FR-006 says per account and
//! the submitter is signed in by assumption.
//!
//! # Why this deliberately ignores `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT`
//!
//! The end-to-end harness sets that variable on every run, because registering
//! a table of test users legitimately exceeds a limit written for humans
//! typing passwords. If this limiter honoured it, the limiter would be off
//! during precisely the tests written to prove it works — and it would pass
//! them. `share_rate_limit.rs` pins that rule with a test named
//! `the_e2e_auth_bypass_does_not_disable_this_limiter`, and this module has
//! its own copy of it below.
//!
//! The variable is in any case `#[cfg(debug_assertions)]`-gated in
//! `auth_middleware.rs` — a release build contains no bypass path at all — so
//! honouring it would be a debug-only hole in a privacy-adjacent control.
//!
//! # The half that is not the limiter
//!
//! FR-006's second clause — a refusal must not discard what the person wrote —
//! is a client property. The refusal arrives as `extensions.code`, the dialog
//! stays open with the draft intact, and it says when they may try again. That
//! is an assertion in the browser suite, not a side effect of this file.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use chrono::Utc;

/// The code the client matches on. Matching on the human-readable message
/// would break the first time somebody reworded it, which the client's own
/// comments about `ALREADY_TAKEN` already say.
pub const RATE_LIMITED: &str = "FEEDBACK_RATE_LIMITED";

/// Submissions per window, per account. Enough for somebody reporting three
/// things they hit in a row and correcting one of them; not enough to make a
/// repository unusable from one account.
pub const MAX_SUBMISSIONS: usize = 5;

/// The sliding window, in seconds.
pub const WINDOW_SECONDS: i64 = 600;

static FEEDBACK_RATE_LIMITER: OnceLock<Mutex<HashMap<String, Vec<i64>>>> = OnceLock::new();

fn limiter_store() -> &'static Mutex<HashMap<String, Vec<i64>>> {
    FEEDBACK_RATE_LIMITER.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Whether this account may submit again, and when it may if not.
///
/// `Ok(())` or the number of seconds to wait. A refusal that cannot say when
/// is a refusal a person reads as "never".
pub fn allow(account: &str) -> Result<(), i64> {
    let now = Utc::now().timestamp();
    let Ok(mut store) = limiter_store().lock() else {
        // A poisoned lock must not become an open door — but it must also not
        // lose somebody's feedback. Refusing with a short wait is the safe way
        // to be broken here: the draft survives on the client and the next
        // attempt succeeds.
        return Err(1);
    };
    let entry = store.entry(account.to_string()).or_default();
    entry.retain(|ts| now - *ts < WINDOW_SECONDS);
    if entry.len() >= MAX_SUBMISSIONS {
        let oldest = entry.iter().copied().min().unwrap_or(now);
        return Err((WINDOW_SECONDS - (now - oldest)).max(1));
    }
    entry.push(now);
    Ok(())
}

/// The sentence a rate-limited person sees. It says when, because "try again
/// later" is not an answer somebody can act on.
pub fn refusal(seconds: i64) -> String {
    let minutes = seconds.div_euclid(60) + i64::from(seconds.rem_euclid(60) > 0);
    if minutes <= 1 {
        "You have sent several pieces of feedback just now. Try again in about \
         a minute — what you have written is kept."
            .to_string()
    } else {
        format!(
            "You have sent several pieces of feedback just now. Try again in about \
             {minutes} minutes — what you have written is kept."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_fresh_account() -> String {
        format!("account-{}", uuid::Uuid::now_v7())
    }

    #[test]
    fn the_sixth_submission_in_the_window_is_refused_and_says_when() {
        let account = a_fresh_account();
        for _ in 0..MAX_SUBMISSIONS {
            assert!(allow(&account).is_ok());
        }
        let wait = allow(&account).expect_err("the sixth is refused");
        assert!(wait > 0);
        assert!(refusal(wait).contains("Try again in about"));
        assert!(refusal(wait).contains("kept"));
    }

    /// The key is the account, so one person going too fast does not refuse
    /// anybody else. This is the difference from the share limiter, which
    /// cannot key on an account because its callers have none.
    #[test]
    fn one_account_going_too_fast_does_not_refuse_another() {
        let busy = a_fresh_account();
        let quiet = a_fresh_account();
        for _ in 0..MAX_SUBMISSIONS {
            let _ = allow(&busy);
        }
        assert!(allow(&busy).is_err());
        assert!(allow(&quiet).is_ok());
    }

    /// The rule this module's header argues for, pinned rather than described.
    /// A limiter that switches itself off in the harness is a limiter nobody
    /// tests, and one nobody tests is one nobody has.
    #[test]
    fn the_e2e_auth_bypass_does_not_disable_this_limiter() {
        // Asserted as a *source* fact rather than by setting the variable:
        // this module reads no environment at all, so there is nothing to
        // toggle, nothing to serialise a test on, and no way for the harness's
        // bypass — or any future one — to reach it. A branch on an environment
        // variable is the only shape this failure could take, and this is the
        // assertion that it does not exist.
        // The needle is assembled at runtime rather than written as a literal:
        // this file reads *itself*, so a literal would be found in the
        // assertion that looks for it and the test would fail on its own text.
        let needle = format!("{}::{}", "env", "var");
        let source = include_str!("rate_limit.rs");
        assert!(
            !source.contains(&needle),
            "this limiter must not read the environment; a bypass it honours is a bypass it has"
        );
        // And it still refuses, which is the behaviour the absent branch is
        // protecting.
        let account = a_fresh_account();
        for _ in 0..MAX_SUBMISSIONS {
            assert!(allow(&account).is_ok());
        }
        assert!(allow(&account).is_err());
    }

    /// A refusal names a wait a person can act on, in whole minutes, and never
    /// says "later".
    #[test]
    fn a_refusal_is_a_time_and_not_a_shrug() {
        assert!(refusal(30).contains("about a minute"));
        assert!(refusal(600).contains("about 10 minutes"));
    }
}
