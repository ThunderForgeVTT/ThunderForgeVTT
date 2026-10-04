//! Spec 041 FR-015 / FR-025: the record, and its limits.

use super::*;
use crate::test_support::{insert_test_user, test_app_state};

#[tokio::test]
async fn an_account_reads_its_own_history_most_recent_first() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    for kind in [
        event_type::ENROLLED,
        event_type::RECOVERY_CODES_ISSUED,
        event_type::RECOVERY_CODE_USED,
    ] {
        record_sync(
            &mut state.db_pool.get().expect("conn"),
            user_id,
            Some(user_id),
            kind,
        )
        .expect("recorded");
    }

    let listed = events_for(&state, user_id, 50).await.expect("listed");
    assert_eq!(listed.len(), 3);
    // `occurred_at` defaults to CURRENT_TIMESTAMP, which inside one
    // transaction-less burst can tie to the microsecond; assert the set rather
    // than an order the database never promised.
    let kinds: std::collections::HashSet<&str> =
        listed.iter().map(|e| e.event_type.as_str()).collect();
    assert_eq!(
        kinds,
        ["enrolled", "recovery_codes_issued", "recovery_code_used"]
            .into_iter()
            .collect(),
    );
    assert!(
        listed.iter().all(|e| !e.by_someone_else),
        "self-service events are not somebody else's doing",
    );
}

/// The row that is the entire reason for two columns.
#[tokio::test]
async fn an_operator_reset_is_marked_as_somebody_elses_doing() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let subject = insert_test_user(&mut conn);
    let operator = insert_test_user(&mut conn);
    drop(conn);

    record_sync(
        &mut state.db_pool.get().expect("conn"),
        subject,
        Some(operator),
        event_type::RESET_BY_OPERATOR,
    )
    .expect("recorded");

    let listed = events_for(&state, subject, 50).await.expect("listed");
    assert_eq!(listed.len(), 1);
    assert!(
        listed[0].by_someone_else,
        "a reset performed by an operator must not read as self-service",
    );
}

/// One account cannot read another's history. Scoped by the query, not by a
/// check a caller has to remember.
#[tokio::test]
async fn one_account_cannot_read_anothers_history() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let subject = insert_test_user(&mut conn);
    let stranger = insert_test_user(&mut conn);
    drop(conn);

    record_sync(
        &mut state.db_pool.get().expect("conn"),
        subject,
        Some(subject),
        event_type::ENROLLED,
    )
    .expect("recorded");

    assert!(
        events_for(&state, stranger, 50)
            .await
            .expect("listed")
            .is_empty(),
    );
}

/// The CHECK constraint is the vocabulary's enforcement, and this is what
/// proves the constants and the constraint have not drifted apart. A value
/// added to `event_type` without adding it to the migration fails here rather
/// than at a table somebody is reading months later.
#[tokio::test]
async fn every_declared_event_type_is_accepted_and_an_invented_one_is_not() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);

    for kind in [
        event_type::ENROLLED,
        event_type::REMOVED,
        event_type::RECOVERY_CODE_USED,
        event_type::RECOVERY_CODES_ISSUED,
        event_type::RESET_BY_OPERATOR,
        event_type::REQUIREMENT_SET,
        event_type::REQUIREMENT_CLEARED,
    ] {
        record_sync(&mut conn, user_id, Some(user_id), kind)
            .unwrap_or_else(|e| panic!("{kind} must satisfy the CHECK constraint: {e}"));
    }

    assert!(
        record_sync(&mut conn, user_id, Some(user_id), "adopted_a_cat").is_err(),
        "the constraint must refuse a type the migration does not know",
    );
}

/// FR-025's limit. A row carries two account ids, a time and a kind — and
/// nothing else, because there is nothing else it is allowed to carry. This
/// asserts over the table's actual columns, so adding one that could hold an
/// address or a code fails here.
#[tokio::test]
async fn a_row_can_hold_nothing_that_describes_a_person() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");

    let columns: Vec<String> = diesel::sql_query(
        "SELECT column_name FROM information_schema.columns \
         WHERE table_name = 'two_factor_events'",
    )
    .load::<ColumnName>(&mut conn)
    .expect("read the table's columns")
    .into_iter()
    .map(|c| c.column_name)
    .collect();

    let mut sorted = columns.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        vec![
            "actor_user_id".to_string(),
            "event_type".to_string(),
            "id".to_string(),
            "occurred_at".to_string(),
            "subject_user_id".to_string(),
        ],
        "a column added here is a place to put something this table must not \
         keep — see the migration's header",
    );
}

#[derive(diesel::QueryableByName)]
struct ColumnName {
    #[diesel(sql_type = diesel::sql_types::Text)]
    column_name: String,
}
