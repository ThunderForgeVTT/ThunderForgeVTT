//! Spec 041 FR-009: **no route returns an issued recovery code.**
//!
//! # Why this is two assertions and not one
//!
//! The obvious test — "call every route and check none of them answers with a
//! code" — cannot be written: a route that returns a code returns it in a
//! field of its own response type, and the only way to reach every route from
//! a test is to know what each one needs in order to answer at all.
//!
//! So the guarantee is fenced from both sides instead.
//!
//! **From the route table**: every registered path that could plausibly serve
//! a second factor is enumerated and compared against the set that exists
//! today. Paths, not methods — axum's `Router` debug rendering does not name
//! methods, which a deleted second test in this file discovered the hard way. A route added to this feature — a "show me my codes" convenience,
//! most likely, added by somebody who has not read FR-009 — fails this test by
//! existing, before anybody has to notice what it returns.
//!
//! **From the store**: `recovery.rs` keeps Argon2id hashes and nothing else
//! (ADR-083), so a route that wanted to return a code would have nothing to
//! return. That is asserted in `recovery::tests::what_is_stored_is_a_hash_and_
//! never_the_code`, and it is the assertion that makes this one a fence rather
//! than the whole wall: the codes are unreadable, and the route list says
//! nobody has added a place to read them from.

use axum::Router;

use crate::state::AppState;

fn paths_of(router: &Router<AppState>) -> Vec<String> {
    format!("{router:?}")
        .split('"')
        .filter(|piece| piece.starts_with("/authentication"))
        .map(str::to_string)
        .collect()
}

/// The second-factor surface, named. A change to this list is a deliberate
/// edit to a test rather than something that slides through.
///
/// Exactly two of these hand out codes, and both do it at the moment they
/// *create* them: `setup/confirm` (FR-006) and `recovery-codes` (FR-010).
/// Every other one is a route that acts on a factor without ever reading a
/// code back out.
#[test]
fn the_only_second_factor_routes_are_the_ones_that_are_meant_to_exist() {
    let mut found: Vec<String> = paths_of(&crate::auth::router())
        .into_iter()
        .chain(paths_of(&crate::auth::admin_router()))
        .filter(|path| path.contains("2fa") || path.contains("recovery"))
        .collect();
    found.sort();
    found.dedup();

    let expected = [
        "/authentication/2fa/disable",
        "/authentication/2fa/history",
        "/authentication/2fa/recovery-codes",
        "/authentication/2fa/setup/confirm",
        "/authentication/2fa/setup/start",
        "/authentication/2fa/status",
        "/authentication/2fa/verify",
        "/authentication/admin/2fa/requirement",
        "/authentication/admin/users/{user_id}/2fa/required",
        "/authentication/admin/users/{user_id}/2fa/reset",
    ];

    assert_eq!(
        found, expected,
        "the second-factor route surface changed. If you added a route that \
         returns a recovery code, FR-009 forbids it: a code is shown once, at \
         the moment it is made, and is an Argon2id hash from then on. If you \
         added something else, add it to this list.",
    );
}

// A second test tried to assert that nothing on this surface is registered
// for `GET`, on the reasoning that a reader is the shape a codes-readback
// would take. It passed — and it passed **vacuously**: axum's `Router` debug
// rendering carries paths and route ids and no methods at all, so an
// assertion that the rendering does not say "GET" is an assertion about
// nothing. Its own anti-vacuity guard caught it, which is the only reason
// this comment exists rather than a green test that checked nothing.
//
// Deleted rather than repaired. What it was reaching for is already covered:
// a new reader is a new route, and a new route fails the enumeration above by
// existing, whatever method it is registered for.
