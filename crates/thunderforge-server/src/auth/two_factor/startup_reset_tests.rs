//! The startup reset: who it reaches, and that it leaves everybody else alone.

use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use super::super::events::{self, event_type};
use super::{
    RESET_VAR, StartupReset, apply_startup_second_factor_reset, reset_admin_second_factor,
    startup_reset_decision,
};
use crate::schema::users;
use crate::test_support::{insert_test_user, test_app_state};

/// Make the account an administrator or not, enrolled or not, and hand back
/// its username — which is what the variable names.
fn prepare(
    state: &crate::state::AppState,
    user_id: Uuid,
    is_admin: bool,
    enrolled: bool,
) -> String {
    let mut conn = state.db_pool.get().expect("conn");
    diesel::update(users::table.filter(users::id.eq(user_id)))
        .set((
            users::is_admin.eq(is_admin),
            users::two_factor_enabled.eq(enrolled),
            users::two_factor_secret_encrypted.eq(enrolled.then(|| "ciphertext".to_string())),
            users::two_factor_confirmed_at.eq(enrolled.then(|| Utc::now().naive_utc())),
        ))
        .execute(&mut conn)
        .expect("prepared");
    users::table
        .filter(users::id.eq(user_id))
        .select(users::username)
        .first(&mut conn)
        .expect("username")
}

fn factor(state: &crate::state::AppState, user_id: Uuid) -> (bool, Option<String>) {
    let mut conn = state.db_pool.get().expect("conn");
    users::table
        .filter(users::id.eq(user_id))
        .select((
            users::two_factor_enabled,
            users::two_factor_secret_encrypted,
        ))
        .first(&mut conn)
        .expect("read")
}

#[test]
fn the_decision_reaches_only_an_administrator_with_something_to_clear() {
    assert_eq!(startup_reset_decision(None), StartupReset::NoSuchUser);
    assert_eq!(
        startup_reset_decision(Some((false, true, false))),
        StartupReset::NotAnAdministrator,
        "the variable stripped an ordinary account's second factor"
    );
    assert_eq!(
        startup_reset_decision(Some((true, false, false))),
        StartupReset::NothingToClear
    );
    assert_eq!(
        startup_reset_decision(Some((true, true, false))),
        StartupReset::Cleared
    );
    assert_eq!(
        startup_reset_decision(Some((true, false, true))),
        StartupReset::Cleared,
        "an abandoned pending enrolment is still a secret to clear"
    );
}

/// The case it exists for, and that the record does not name an actor nobody
/// was.
#[tokio::test]
async fn the_named_administrator_is_cleared_and_the_reset_is_recorded() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let admin = insert_test_user(&mut conn);
    drop(conn);
    let username = prepare(&state, admin, true, true);

    let outcome = reset_admin_second_factor(&state, &username)
        .await
        .expect("reset");
    assert_eq!(outcome, StartupReset::Cleared);
    assert_eq!(
        factor(&state, admin),
        (false, None),
        "the factor is off and its secret is gone"
    );

    let listed = events::events_for(&state, admin, 50).await.expect("listed");
    let reset = listed
        .iter()
        .find(|e| e.event_type == event_type::RESET_BY_OPERATOR)
        .expect("the reset is recorded");
    assert!(
        reset.by_someone_else,
        "a reset from the machine must not read as the account holder's own doing"
    );

    // What a restart with the variable left behind does before they re-enrol.
    assert_eq!(
        reset_admin_second_factor(&state, &username)
            .await
            .expect("second pass"),
        StartupReset::NothingToClear
    );
}

#[tokio::test]
async fn an_ordinary_account_and_an_unknown_name_are_left_alone() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let player = insert_test_user(&mut conn);
    drop(conn);
    let username = prepare(&state, player, false, true);

    assert_eq!(
        reset_admin_second_factor(&state, &username)
            .await
            .expect("not an admin"),
        StartupReset::NotAnAdministrator
    );
    assert_eq!(
        factor(&state, player),
        (true, Some("ciphertext".to_string())),
        "an ordinary account's factor was cleared from the environment"
    );

    assert_eq!(
        reset_admin_second_factor(&state, &format!("nobody-{}", Uuid::now_v7()))
            .await
            .expect("unknown"),
        StartupReset::NoSuchUser
    );
}

/// Unset is the ordinary case and must be silent and inert; set, the variable
/// is what names the account.
#[test]
fn the_variable_is_what_asks_for_it() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let admin = insert_test_user(&mut conn);
    drop(conn);
    let username = prepare(&state, admin, true, true);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    crate::settings::test_env::temp_env(&[(RESET_VAR, None)], || {
        assert_eq!(
            runtime.block_on(apply_startup_second_factor_reset(&state)),
            StartupReset::NotRequested
        );
    });
    assert!(
        factor(&state, admin).0,
        "an unset variable cleared a factor"
    );

    crate::settings::test_env::temp_env(&[(RESET_VAR, Some(username.as_str()))], || {
        assert_eq!(
            runtime.block_on(apply_startup_second_factor_reset(&state)),
            StartupReset::Cleared
        );
    });
    assert!(!factor(&state, admin).0);
}
