//! Spec 041 FR-021 / SC-007: what an operator can see about second-factor
//! coverage, and what they deliberately cannot.
//!
//! # Why these are invariants rather than exact figures
//!
//! The counts are instance-wide, and this suite runs its tests in parallel
//! against one database — every other test inserting a user moves them. An
//! assertion like "enrolled is 3" would be a test that fails according to what
//! else happened to be running, which is worse than no test at all. What holds
//! regardless of who else is inserting rows is the *arithmetic between* the
//! figures, and that is what would break if the query were wrong.

use diesel::prelude::*;

use crate::admin::load_two_factor_coverage;
use crate::schema::users;
use crate::test_support::{insert_test_user, test_app_state};

/// The three figures have to describe one population.
///
/// `required_not_enrolled` is a subset of `not_enrolled` — the name says so,
/// and an operator reading "12 not enrolled, 15 of them required" would
/// rightly stop trusting the panel.
///
/// Deliberately *not* compared against a separately-read `COUNT(*)`. The first
/// version of this test did exactly that and failed with 184330 against
/// 184317: thirteen accounts were inserted by other tests in the gap between
/// the two reads. That is the race the query itself now takes a
/// repeatable-read snapshot to avoid — and a test that reads the total in a
/// second statement re-introduces it on its own side, where no snapshot can
/// help.
#[tokio::test]
async fn the_counts_describe_one_population() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    insert_test_user(&mut conn);
    drop(conn);

    let coverage = load_two_factor_coverage(&state).await.expect("coverage");

    assert!(
        coverage.required_not_enrolled <= coverage.not_enrolled,
        "accounts required and not enrolled are a subset of accounts not \
         enrolled: {} of {}",
        coverage.required_not_enrolled,
        coverage.not_enrolled,
    );
    assert!(
        coverage.enrolled >= 0 && coverage.not_enrolled >= 0,
        "a negative account count is a query that went wrong",
    );
}

/// FR-027 reaches this figure without a column, which is the point of
/// computing the rule (ADR-094).
///
/// An administrator who has not enrolled is *required and not enrolled*, and
/// nothing was written to their row to make that true — so the operator's
/// panel is right about an instance that upgraded into the rule with no
/// backfill. Asserted as a delta, because the absolute figure belongs to
/// whatever else is in the database.
#[tokio::test]
async fn promoting_an_account_moves_it_into_required_and_not_enrolled() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    let before = load_two_factor_coverage(&state)
        .await
        .expect("coverage")
        .required_not_enrolled;

    let mut conn = state.db_pool.get().expect("conn");
    diesel::update(users::table.filter(users::id.eq(user_id)))
        .set(users::is_admin.eq(true))
        .execute(&mut conn)
        .expect("promote");
    drop(conn);

    let after = load_two_factor_coverage(&state)
        .await
        .expect("coverage")
        .required_not_enrolled;

    assert!(
        after > before,
        "promoting an unenrolled account to administrator must show up as one \
         more account required and not enrolled ({before} → {after})",
    );
}

/// SC-007's other half: the field answers "how much", never "who".
///
/// A roster of accounts without a second factor is a target list — the
/// accounts on it are exactly the ones a stolen password is sufficient for —
/// and it would be handed to anybody who takes over an operator's session. So
/// the type carries three integers and no way to reach an account from them.
#[test]
fn coverage_is_counts_and_offers_no_way_to_name_an_account() {
    let schema = async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .finish();
    let sdl = schema.sdl();

    let start = sdl
        .find("type TwoFactorCoverage")
        .expect("the coverage type must be in the schema under its contract name");
    let block = &sdl[start..];
    let end = block.find('}').expect("a closed type block");
    let block = &block[..end];

    for field in ["Int!", "enrolled", "notEnrolled", "requiredNotEnrolled"] {
        assert!(block.contains(field), "expected `{field}` in:\n{block}");
    }
    for leak in ["User", "[", "username", "email", "id"] {
        assert!(
            !block.contains(leak),
            "`TwoFactorCoverage` names `{leak}`, which is a way to a roster \
             rather than a count:\n{block}",
        );
    }
}
