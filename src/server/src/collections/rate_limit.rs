//! FR-009c: the anonymous collection read is rate limited.
//!
//! # Why this is not `auth_middleware::rate_limit_auth_requests`
//!
//! That limiter keys on the request **path** and returns early unless the path
//! contains `/authentication/`. Every GraphQL operation in this product
//! arrives at one path, so extending it to cover `/graphql` would rate-limit
//! the entire application against a threshold written for password attempts.
//! This one sits inside the resolver, where the operation is known.
//!
//! # Why this deliberately ignores `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT`
//!
//! The end-to-end harness sets that variable on **every** run, because
//! registering a table of test users legitimately exceeds a limit written for
//! humans typing passwords. If this limiter honoured it, the limiter would be
//! off during precisely the tests written to prove it works — and it would
//! pass them. A limiter nobody tests is a limiter nobody has.
//!
//! The thing that variable protects is credential stuffing. This limiter
//! protects against walking share codes, which is a different threat with a
//! different threshold and no reason to share a switch.
//!
//! # What it is actually worth
//!
//! A share code is 20 uppercase hex characters — about 80 bits. Guessing one is
//! not feasible at any rate. The limit exists because "not feasible" is a claim
//! about arithmetic, and arithmetic assumes a bounded number of attempts;
//! ADR-069's determination rests on the code being unguessable, so leaving the
//! attempts unbounded would leave that resting on nothing.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use chrono::Utc;

/// Requests per window, per caller. Generous for a person opening a link —
/// including reloads, a preview and then a copy — and nowhere near enough to
/// make walking codes worthwhile.
const MAX_REQUESTS: usize = 30;

/// The sliding window, in seconds.
const WINDOW_SECONDS: i64 = 60;

static COLLECTION_RATE_LIMITER: OnceLock<Mutex<HashMap<String, Vec<i64>>>> = OnceLock::new();

fn limiter_store() -> &'static Mutex<HashMap<String, Vec<i64>>> {
    COLLECTION_RATE_LIMITER.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Whether this caller may make another anonymous collection request.
///
/// Same sliding-window shape as the auth limiter, so the codebase has one idea
/// of what rate limiting looks like rather than two.
pub fn allow_request(caller: &str) -> bool {
    let now = Utc::now().timestamp();
    let Ok(mut store) = limiter_store().lock() else {
        // A poisoned lock must not become an open door. Refusing is the safe
        // way to be broken here: a collection preview failing is a bad page,
        // while an unbounded guessing surface is the thing this exists to stop.
        return false;
    };
    let entry = store.entry(caller.to_string()).or_default();
    entry.retain(|ts| now - *ts < WINDOW_SECONDS);
    if entry.len() >= MAX_REQUESTS {
        return false;
    }
    entry.push(now);
    true
}

/// The sentence a rate-limited caller is shown.
///
/// It says nothing about whether the code they tried was real — FR-009d — and
/// nothing about the limit's shape.
pub fn rate_limited_message() -> &'static str {
    "Too many requests. Please wait a moment and try again."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_caller_is_refused_after_the_limit() {
        let caller = format!("test-{}", uuid::Uuid::new_v4());
        for i in 0..MAX_REQUESTS {
            assert!(allow_request(&caller), "request {i} should be allowed");
        }
        assert!(
            !allow_request(&caller),
            "request {} must be refused",
            MAX_REQUESTS + 1
        );
    }

    #[test]
    fn callers_are_limited_independently() {
        let a = format!("test-a-{}", uuid::Uuid::new_v4());
        let b = format!("test-b-{}", uuid::Uuid::new_v4());
        for _ in 0..MAX_REQUESTS {
            assert!(allow_request(&a));
        }
        assert!(!allow_request(&a), "a is exhausted");
        assert!(allow_request(&b), "b must be unaffected by a");
    }

    /// The mutation this test exists to catch: making this limiter honour the
    /// e2e harness's bypass variable, which would switch it off during every
    /// test written to prove it works.
    #[test]
    fn the_e2e_auth_bypass_does_not_disable_this_limiter() {
        // SAFETY: single-threaded test process for this variable; the auth
        // limiter memoises its own read in a OnceLock and is unaffected.
        unsafe { std::env::set_var("THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT", "1") };

        let caller = format!("test-bypass-{}", uuid::Uuid::new_v4());
        for _ in 0..MAX_REQUESTS {
            assert!(allow_request(&caller));
        }
        assert!(
            !allow_request(&caller),
            "THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT must not reach this limiter — \
             the e2e harness sets it on every run"
        );

        unsafe { std::env::remove_var("THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT") };
    }

    /// FR-009d: the refusal says nothing about whether the code was real.
    #[test]
    fn the_rate_limit_message_reveals_nothing() {
        let message = rate_limited_message().to_lowercase();
        for leak in ["collection", "code", "exist", "revoked", "found"] {
            assert!(
                !message.contains(leak),
                "the rate-limit message must not mention {leak:?}: {message}"
            );
        }
    }
}
