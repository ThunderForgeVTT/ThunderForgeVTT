//! Spec 041 US6 (FR-023, FR-025): requiring a second factor of one account.
//!
//! # Why "and no other" is a test rather than an obvious property
//!
//! Because the instance-wide switch next door is one row that reaches
//! everybody, and these two live in the same file and read almost the same.
//! A `set` that lost its `WHERE` would be an instance-wide requirement wearing
//! a per-account name — every account on the instance required to enrol, from
//! a control whose label says it affects one person.

use diesel::prelude::*;
use uuid::Uuid;

use super::events::{self, event_type};
use super::policy::set_user_requirement_sync;
use crate::schema::users;
use crate::test_support::{insert_test_user, test_app_state};

/// FR-023 and FR-025 in one act: the column moves on the named account, and
/// the operator who moved it is on the record.
#[tokio::test]
async fn requiring_a_factor_of_one_account_names_who_required_it() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let subject = insert_test_user(&mut conn);
    let bystander = insert_test_user(&mut conn);
    let operator = insert_test_user(&mut conn);

    let updated = set_user_requirement_sync(&mut conn, subject, Some(operator), true).expect("set");
    assert_eq!(updated, 1, "exactly the account named, and no other");

    let (required, untouched): (bool, bool) = (
        users::table
            .filter(users::id.eq(subject))
            .select(users::two_factor_admin_required)
            .first(&mut conn)
            .expect("read"),
        users::table
            .filter(users::id.eq(bystander))
            .select(users::two_factor_admin_required)
            .first(&mut conn)
            .expect("read"),
    );
    assert!(required, "the requirement lands on the account it names");
    assert!(
        !untouched,
        "a per-account requirement that reached a second account would be the \
         instance-wide switch wearing the wrong label",
    );
    drop(conn);

    let history = events::events_for(&state, subject, 20)
        .await
        .expect("events");
    let latest = history.first().expect("the act is on the record");
    assert_eq!(latest.event_type, event_type::REQUIREMENT_SET);
    assert!(
        latest.by_someone_else,
        "an administrator requiring this of somebody else is not that person's \
         own doing, and the record has to say so",
    );

    let bystander_history = events::events_for(&state, bystander, 20)
        .await
        .expect("events");
    assert!(
        bystander_history.is_empty(),
        "nothing happened to this account, so nothing is written about it",
    );
}

/// Clearing it is the same act in the other direction, and is recorded as its
/// own event rather than as the absence of one.
///
/// "It used to be required and now is not" is a question somebody will ask
/// after the fact, and a log that only records impositions cannot answer it.
#[tokio::test]
async fn clearing_the_requirement_is_recorded_as_its_own_act() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let subject = insert_test_user(&mut conn);
    let operator = insert_test_user(&mut conn);

    set_user_requirement_sync(&mut conn, subject, Some(operator), true).expect("set");
    set_user_requirement_sync(&mut conn, subject, Some(operator), false).expect("clear");

    let required: bool = users::table
        .filter(users::id.eq(subject))
        .select(users::two_factor_admin_required)
        .first(&mut conn)
        .expect("read");
    assert!(!required);
    drop(conn);

    let history = events::events_for(&state, subject, 20)
        .await
        .expect("events");
    let types: Vec<&str> = history.iter().map(|e| e.event_type.as_str()).collect();
    assert_eq!(
        types,
        vec![event_type::REQUIREMENT_CLEARED, event_type::REQUIREMENT_SET],
        "most recent first, and both acts are there",
    );
}

/// An account that does not exist is nought rows and no event — not a silent
/// success, and not a row written about a subject that is not there.
#[tokio::test]
async fn a_requirement_placed_on_nobody_writes_nothing() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let operator = insert_test_user(&mut conn);
    let nobody = Uuid::now_v7();

    let updated = set_user_requirement_sync(&mut conn, nobody, Some(operator), true).expect("set");
    assert_eq!(
        updated, 0,
        "the handler answers 404 on the strength of this"
    );
    drop(conn);

    let history = events::events_for(&state, nobody, 20)
        .await
        .expect("events");
    assert!(
        history.is_empty(),
        "an event about an account that does not exist is a record of \
         something that did not happen",
    );
}
