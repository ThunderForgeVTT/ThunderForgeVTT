//! Spec 041 FR-024/FR-025: the reset, and what it leaves behind.
//!
//! The handler needs an admin cookie, so these drive `clear_second_factor_sync`
//! with the operator event type — the part that is this feature's own, rather
//! than re-testing `verify_admin_request`, which has its own tests and guards
//! every other admin route the same way.

use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use super::super::disable::clear_second_factor_sync;
use super::super::events::{self, event_type};
use crate::schema::{user_recovery_codes, users};
use crate::test_support::{insert_test_user, test_app_state};

fn enrolled(state: &crate::state::AppState, user_id: Uuid) {
    let mut conn = state.db_pool.get().expect("conn");
    diesel::update(users::table.filter(users::id.eq(user_id)))
        .set((
            users::two_factor_enabled.eq(true),
            users::two_factor_secret_encrypted.eq(Some("ciphertext".to_string())),
            users::two_factor_confirmed_at.eq(Some(Utc::now().naive_utc())),
            // Mid-guess and paused, which is the state somebody is actually in
            // when they give up and ask an operator for help.
            users::two_factor_failed_attempts.eq(3),
            users::two_factor_locked_until
                .eq(Some(Utc::now().naive_utc() + chrono::Duration::seconds(60))),
        ))
        .execute(&mut conn)
        .expect("enrolled");
}

/// The act itself, and that it is recorded as an operator's rather than the
/// account holder's — which is the entire reason `two_factor_events` has two
/// user columns.
#[tokio::test]
async fn a_reset_clears_the_factor_and_names_the_operator_who_did_it() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let subject = insert_test_user(&mut conn);
    let operator = insert_test_user(&mut conn);
    drop(conn);
    enrolled(&state, subject);

    let mut conn = state.db_pool.get().expect("conn");
    clear_second_factor_sync(
        &mut conn,
        subject,
        Some(operator),
        event_type::RESET_BY_OPERATOR,
    )
    .expect("reset");
    drop(conn);

    let mut conn = state.db_pool.get().expect("conn");
    let (enabled, secret): (bool, Option<String>) = users::table
        .filter(users::id.eq(subject))
        .select((
            users::two_factor_enabled,
            users::two_factor_secret_encrypted,
        ))
        .first(&mut conn)
        .expect("read");
    assert!(!enabled, "the factor is off");
    assert_eq!(secret, None, "and its secret is gone, not merely disabled");
    drop(conn);

    let listed = events::events_for(&state, subject, 50)
        .await
        .expect("listed");
    let reset = listed
        .iter()
        .find(|e| e.event_type == event_type::RESET_BY_OPERATOR)
        .expect("the reset is recorded");
    assert!(
        reset.by_someone_else,
        "an operator's reset must not read as the account holder's own doing",
    );
}

/// A reset that left the cooling-off running would hand somebody back an
/// account they still could not use — the pause was caused by the very
/// situation that led to the reset.
#[tokio::test]
async fn a_reset_clears_the_attempt_counter_and_any_pause() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let subject = insert_test_user(&mut conn);
    let operator = insert_test_user(&mut conn);
    drop(conn);
    enrolled(&state, subject);

    let mut conn = state.db_pool.get().expect("conn");
    clear_second_factor_sync(
        &mut conn,
        subject,
        Some(operator),
        event_type::RESET_BY_OPERATOR,
    )
    .expect("reset");

    let (attempts, locked): (i32, Option<chrono::NaiveDateTime>) = users::table
        .filter(users::id.eq(subject))
        .select((
            users::two_factor_failed_attempts,
            users::two_factor_locked_until,
        ))
        .first(&mut conn)
        .expect("read");

    assert_eq!(attempts, 0);
    assert_eq!(locked, None, "a reset account is not still serving a pause");
}

/// The codes go too. Leaving them would mean an account with no second factor
/// and ten live credentials that still grant it.
#[tokio::test]
async fn a_reset_takes_the_recovery_codes_with_it() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let subject = insert_test_user(&mut conn);
    let operator = insert_test_user(&mut conn);
    drop(conn);
    enrolled(&state, subject);

    let mut conn = state.db_pool.get().expect("conn");
    let rows: Vec<crate::models::NewUserRecoveryCode> = (0..3)
        .map(|i| crate::models::NewUserRecoveryCode {
            id: Uuid::now_v7(),
            user_id: subject,
            code_hash: format!("$argon2id$not-real-{i}"),
            created_at: Utc::now().naive_utc(),
            used_at: None,
        })
        .collect();
    diesel::insert_into(user_recovery_codes::table)
        .values(&rows)
        .execute(&mut conn)
        .expect("codes issued");

    clear_second_factor_sync(
        &mut conn,
        subject,
        Some(operator),
        event_type::RESET_BY_OPERATOR,
    )
    .expect("reset");

    let remaining: i64 = user_recovery_codes::table
        .filter(user_recovery_codes::user_id.eq(subject))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(
        remaining, 0,
        "a reset must not leave live credentials that still admit the account",
    );
}

/// One account's reset reaches one account.
#[tokio::test]
async fn a_reset_reaches_only_the_account_it_names() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let subject = insert_test_user(&mut conn);
    let bystander = insert_test_user(&mut conn);
    let operator = insert_test_user(&mut conn);
    drop(conn);
    enrolled(&state, subject);
    enrolled(&state, bystander);

    let mut conn = state.db_pool.get().expect("conn");
    clear_second_factor_sync(
        &mut conn,
        subject,
        Some(operator),
        event_type::RESET_BY_OPERATOR,
    )
    .expect("reset");

    let still_on: bool = users::table
        .filter(users::id.eq(bystander))
        .select(users::two_factor_enabled)
        .first(&mut conn)
        .expect("read");
    assert!(still_on, "a bystander's factor is untouched");
}

/// FR-024 as a property an operator can rely on: doing it twice is doing it
/// once.
///
/// An operator who is not sure whether the reset went through will press it
/// again — that is not a mistake, it is the only information they have — and a
/// route that errored, or that half-cleared on the second pass, would turn a
/// stuck person into a stuck person plus an incident. The account with no
/// factor is already in the state a reset produces, so a reset of it is a
/// no-op that still succeeds.
#[tokio::test]
async fn resetting_an_account_that_holds_no_factor_succeeds_and_changes_nothing() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let subject = insert_test_user(&mut conn);
    let operator = insert_test_user(&mut conn);

    let before: (bool, Option<String>) = users::table
        .filter(users::id.eq(subject))
        .select((
            users::two_factor_enabled,
            users::two_factor_secret_encrypted,
        ))
        .first(&mut conn)
        .expect("read");

    for _ in 0..2 {
        clear_second_factor_sync(
            &mut conn,
            subject,
            Some(operator),
            event_type::RESET_BY_OPERATOR,
        )
        .expect("a reset of an unenrolled account must succeed");
    }

    let after: (bool, Option<String>) = users::table
        .filter(users::id.eq(subject))
        .select((
            users::two_factor_enabled,
            users::two_factor_secret_encrypted,
        ))
        .first(&mut conn)
        .expect("read");

    assert_eq!(
        before, after,
        "resetting an account with nothing to reset must leave it exactly as it was"
    );
}

/// FR-025's other half, and the one that would be a privilege escalation if it
/// were wrong: a reset **issues nothing**.
///
/// An operator who could hand out a working second factor — a secret, a set of
/// recovery codes, a session — could sign in as the account holder, which is
/// precisely the authority this feature exists to keep out of an operator's
/// hands. What a reset leaves behind is an account with no factor, which the
/// person then enrols from their own screen with a secret only they see.
#[tokio::test]
async fn a_reset_issues_no_secret_no_codes_and_no_session() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let subject = insert_test_user(&mut conn);
    let operator = insert_test_user(&mut conn);
    drop(conn);
    enrolled(&state, subject);

    let mut conn = state.db_pool.get().expect("conn");
    let sessions_before: i64 = crate::schema::user_sessions::table
        .filter(crate::schema::user_sessions::user_id.eq(subject))
        .count()
        .get_result(&mut conn)
        .expect("count");

    clear_second_factor_sync(
        &mut conn,
        subject,
        Some(operator),
        event_type::RESET_BY_OPERATOR,
    )
    .expect("reset");

    let (enabled, secret, pending): (bool, Option<String>, Option<String>) = users::table
        .filter(users::id.eq(subject))
        .select((
            users::two_factor_enabled,
            users::two_factor_secret_encrypted,
            users::two_factor_pending_secret_encrypted,
        ))
        .first(&mut conn)
        .expect("read");
    assert!(!enabled, "the account must hold no factor after a reset");
    assert_eq!(secret, None, "a reset must not leave a secret behind");
    assert_eq!(
        pending, None,
        "a reset must not start an enrolment on somebody else's behalf",
    );

    let codes: i64 = user_recovery_codes::table
        .filter(user_recovery_codes::user_id.eq(subject))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(codes, 0, "a reset must issue no recovery codes");

    let sessions_after: i64 = crate::schema::user_sessions::table
        .filter(crate::schema::user_sessions::user_id.eq(subject))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(
        sessions_after, sessions_before,
        "a reset must sign nobody in",
    );
}
