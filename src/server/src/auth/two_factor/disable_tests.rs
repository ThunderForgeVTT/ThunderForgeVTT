//! Spec 041 US4 (T038): what removal costs, and who is refused it.

use chrono::Utc;
use uuid::Uuid;

use super::*;
use crate::models::NewUserRecoveryCode;
use crate::schema::{user_recovery_codes, users};
use crate::test_support::{insert_test_user, test_app_state};

fn enrol_with_codes(state: &crate::state::AppState, user_id: Uuid) {
    let mut conn = state.db_pool.get().expect("conn");
    diesel::update(users::table.filter(users::id.eq(user_id)))
        .set((
            users::two_factor_enabled.eq(true),
            users::two_factor_secret_encrypted.eq(Some("ciphertext".to_string())),
            users::two_factor_confirmed_at.eq(Some(Utc::now().naive_utc())),
            // A half-started enrolment sitting alongside a live factor. Left
            // here on purpose: it is the thing that must not survive removal.
            users::two_factor_pending_secret_encrypted.eq(Some("pending".to_string())),
            users::two_factor_pending_started_at.eq(Some(Utc::now().naive_utc())),
        ))
        .execute(&mut conn)
        .expect("enrolled");

    let rows: Vec<NewUserRecoveryCode> = (0..3)
        .map(|i| NewUserRecoveryCode {
            id: Uuid::now_v7(),
            user_id,
            code_hash: format!("$argon2id$not-a-real-hash-{i}"),
            created_at: Utc::now().naive_utc(),
            used_at: None,
        })
        .collect();
    diesel::insert_into(user_recovery_codes::table)
        .values(&rows)
        .execute(&mut conn)
        .expect("codes issued");
}

fn factor_state(
    state: &crate::state::AppState,
    user_id: Uuid,
) -> (bool, Option<String>, Option<String>, i64) {
    let mut conn = state.db_pool.get().expect("conn");
    let (enabled, secret, pending) = users::table
        .filter(users::id.eq(user_id))
        .select((
            users::two_factor_enabled,
            users::two_factor_secret_encrypted,
            users::two_factor_pending_secret_encrypted,
        ))
        .first::<(bool, Option<String>, Option<String>)>(&mut conn)
        .expect("read the account");
    let codes: i64 = user_recovery_codes::table
        .filter(user_recovery_codes::user_id.eq(user_id))
        .count()
        .get_result(&mut conn)
        .expect("count codes");
    (enabled, secret, pending, codes)
}

/// FR-027, FR-019, FR-023 — the three refusals, exhaustively, and the one case
/// that is allowed.
///
/// Stated as a table rather than three tests because the interesting property
/// is that these four rows are *all* the rows: a fourth reason appearing would
/// be a policy nobody wrote down.
#[test]
fn who_may_turn_it_off_and_who_is_told_why_not() {
    // (is_admin, instance_required, admin_required) -> refusal
    let cases = [
        ((false, false, false), None),
        ((false, false, true), Some(DisableRefusal::AdminRequired)),
        ((false, true, false), Some(DisableRefusal::InstancePolicy)),
        ((false, true, true), Some(DisableRefusal::InstancePolicy)),
        ((true, false, false), Some(DisableRefusal::AdminRole)),
        ((true, true, true), Some(DisableRefusal::AdminRole)),
    ];
    for ((is_admin, instance, admin_required), expected) in cases {
        assert_eq!(
            refusal_for(is_admin, instance, admin_required),
            expected,
            "is_admin={is_admin} instance={instance} admin_required={admin_required}"
        );
    }
}

/// The ordering is load-bearing, not incidental.
///
/// An administrator under an instance-wide policy is told about the *role*,
/// because that is the thing they can act on: they would go hunting for a
/// setting otherwise, and the setting would not help.
#[test]
fn an_administrator_is_told_about_the_role_rather_than_the_policy() {
    assert_eq!(
        refusal_for(true, true, true),
        Some(DisableRefusal::AdminRole)
    );
}

/// Every refusal names something the caller can do about it.
///
/// This route is the one place a refusal may be specific: by the time it is
/// reached the caller has proved the password *and* possession of the factor,
/// so there is no attacker left to keep in the dark.
#[test]
fn every_refusal_says_what_to_do_about_it() {
    for refusal in [
        DisableRefusal::AdminRole,
        DisableRefusal::InstancePolicy,
        DisableRefusal::AdminRequired,
    ] {
        let message = refusal.message();
        assert!(message.len() > 40, "{refusal:?}: {message}");
        assert!(
            message.contains("administrator") || message.contains("instance"),
            "{refusal:?} does not name who requires it: {message}"
        );
    }
}

/// FR-014, against the database: the factor, its pending enrolment and every
/// recovery code go together.
///
/// The recovery codes are the half worth writing a test for. A factor cleared
/// while its codes survive is an account that can still be admitted by a
/// credential nothing now protects — and the codes live in a different table,
/// which is exactly how that gets forgotten.
#[tokio::test]
async fn removal_takes_the_secret_the_pending_enrolment_and_every_recovery_code() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    enrol_with_codes(&state, user_id);
    let before = factor_state(&state, user_id);
    assert_eq!(
        before,
        (
            true,
            Some("ciphertext".to_string()),
            Some("pending".to_string()),
            3
        ),
        "the fixture did not set up an enrolled account, so this would prove nothing"
    );

    let mut conn = state.db_pool.get().expect("conn");
    clear_second_factor_sync(&mut conn, user_id).expect("cleared");

    assert_eq!(factor_state(&state, user_id), (false, None, None, 0));
}

/// Idempotent: clearing an account that has nothing is not an error.
#[tokio::test]
async fn clearing_an_account_with_no_factor_is_not_an_error() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);

    clear_second_factor_sync(&mut conn, user_id).expect("cleared");
    drop(conn);
    assert_eq!(factor_state(&state, user_id), (false, None, None, 0));
}

/// One account's removal does not reach another's.
///
/// `delete_recovery_codes_sync` deletes by `user_id`; a missing filter there
/// would empty the table for everybody, and every other test in this file
/// would still pass.
#[tokio::test]
async fn removal_reaches_only_the_account_that_asked() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let mine = insert_test_user(&mut conn);
    let theirs = insert_test_user(&mut conn);
    drop(conn);

    enrol_with_codes(&state, mine);
    enrol_with_codes(&state, theirs);

    let mut conn = state.db_pool.get().expect("conn");
    clear_second_factor_sync(&mut conn, mine).expect("cleared");
    drop(conn);

    assert_eq!(factor_state(&state, mine), (false, None, None, 0));
    assert_eq!(
        factor_state(&state, theirs),
        (
            true,
            Some("ciphertext".to_string()),
            Some("pending".to_string()),
            3
        ),
        "another account's factor and codes were taken with it"
    );
}
