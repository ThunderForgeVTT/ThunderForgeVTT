//! The admin surface is guarded by where a route lives, not by what its author
//! remembered.
//!
//! # Why this file exists
//!
//! An audit on 2026-09-09 found every operator-scoped route and resolver
//! correctly guarded — and guarded by **three different idioms**, applied one
//! handler at a time:
//!
//! - `verify_admin_request(&state, &cookies)` inside the handler;
//! - `admin_user(ctx)?` inside a GraphQL resolver;
//! - `authenticated_user(ctx)?` and then `is_admin` passed into an `_impl`
//!   that refuses (`resolve_moderation_case`).
//!
//! Nothing was wrong. But "correct because three authors each remembered" is
//! not a property, it is a run of luck, and it is the same shape as the
//! second-factor bound that guarded one route of four earlier the same day.
//! The fourth route is always the one nobody notices.
//!
//! So the admin authentication routes moved into `auth::admin_router()`, which
//! `main.rs` wraps in `require_admin_user`. The layer refuses before a handler
//! is entered, which means a route added to that router is guarded **by having
//! been added to it**. This test is the other half: it fails if an
//! admin-looking route is registered anywhere else.

use axum::Router;

use crate::state::AppState;

/// Every path in a router, as the router itself reports them.
///
/// Read from the debug rendering rather than a list maintained by hand: a list
/// maintained by hand is a list that disagrees with the code the moment
/// somebody adds a route, which is exactly the moment this test exists for.
fn paths_of(router: &Router<AppState>) -> String {
    format!("{router:?}")
}

/// Nothing under `/authentication/admin/` may be registered in the ordinary
/// router, because the ordinary router carries no admin layer.
///
/// If this fails, the route you just added is reachable by any signed-in
/// account. Move it into `admin_router()`.
#[test]
fn no_admin_route_hides_in_the_unguarded_router() {
    let ordinary = paths_of(&super::router());

    assert!(
        !ordinary.contains("/authentication/admin"),
        "an `/authentication/admin/` route is registered in `router()`, which \
         has no admin layer — move it into `admin_router()`:\n{ordinary}",
    );
}

/// And the admin router contains only admin paths, so "is it guarded?" is
/// answerable by reading the path.
///
/// The inverse matters as much: a non-admin route quietly added to the guarded
/// router would become administrator-only, and would fail for ordinary people
/// in a way whose cause is nowhere near the symptom.
#[test]
fn the_admin_router_holds_admin_paths_and_nothing_else() {
    let admin = paths_of(&super::admin_router());

    assert!(
        admin.contains("/authentication/admin"),
        "the admin router should hold the admin routes:\n{admin}",
    );

    // Every registered path in that router, pulled out of the rendering.
    for path in admin
        .split('"')
        .filter(|piece| piece.starts_with("/authentication"))
    {
        assert!(
            path.starts_with("/authentication/admin/"),
            "`{path}` is in the admin router but is not an admin path — it \
             would silently become administrator-only",
        );
    }
}

/// The three routes that are there today, named.
///
/// Deliberately a list, unlike `paths_of` above: these are the routes that
/// change somebody *else's* second factor, and a change to this set should be
/// a deliberate edit to a test rather than something that slides through.
#[test]
fn the_admin_routes_are_the_ones_we_think_they_are() {
    let admin = paths_of(&super::admin_router());

    for expected in [
        "/authentication/admin/2fa/requirement",
        "/authentication/admin/users/{user_id}/2fa/required",
        "/authentication/admin/users/{user_id}/2fa/reset",
    ] {
        assert!(
            admin.contains(expected),
            "`{expected}` is missing from the admin router:\n{admin}",
        );
    }
}
