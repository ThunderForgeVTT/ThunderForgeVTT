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
