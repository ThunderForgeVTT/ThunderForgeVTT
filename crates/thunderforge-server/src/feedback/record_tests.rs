//! The record, and the promise FR-018 is: a submission the instance cannot
//! forward is kept, not lost.
//!
//! The database-backed tests need `DATABASE_URL`, like every other integration
//! test in this crate (`test_support`). Nothing here calls a host, which is
//! the property under test rather than a convenience.
//!
//! # Why these serialise on a mutex
//!
//! `feedback_submissions` is one table shared by every test in this binary,
//! and two of the tests below read *sets* of rows — what one account can see,
//! and what is undelivered instance-wide. A row another test wrote lands in
//! both. So the tests that read across the table take [`TABLE`], for the same
//! reason `mail/outbox_tests.rs` takes its own: a `tokio::sync` mutex rather
//! than a `std` one, because these are async and clippy rightly refuses a std
//! guard held across an await.

use uuid::Uuid;

use super::*;
use crate::test_support::{insert_test_user, test_app_state};

/// The one lock for the one table, declared in `feedback/mod.rs` — not a
/// second mutex over the same rows, which is not serialisation at all.
use crate::feedback::{TEST_TABLE as TABLE, clear_for_test};

fn a_submission(user_id: Uuid, kind: Kind) -> NewSubmission {
    NewSubmission {
        user_id,
        kind,
        message: "Tokens vanish when the scene changes".to_string(),
        summary: Some("Tokens vanish".to_string()),
        screen_path: Some("/world/abc/play".to_string()),
        world_id: None,
        client_version: "1.4.2".to_string(),
        browser: "Chrome on Linux".to_string(),
        // No attachments: storing bytes needs RustFS, and what these tests are
        // about is the row and its ordering. The attachment path is exercised
        // by the end-to-end harness, which has a stack.
        attachments: Vec::new(),
    }
}

/// FR-018, as an ordering rather than as an intention: the row exists, in
/// `pending`, with nothing having been asked of any host — and this instance
/// has no destination configured at all, which is FR-030's case.
#[tokio::test]
async fn a_submission_is_recorded_even_with_no_destination_configured() {
    let _guard = TABLE.lock().await;
    let state = test_app_state();
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let mut conn = state.db_pool.get().expect("a connection");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    let row = record(&state, a_submission(user_id, Kind::Issue))
        .await
        .expect("a submission with nowhere to go is still recorded");

    assert_eq!(row.delivery_state(), DeliveryState::Pending);
    assert_eq!(row.kind(), Kind::Issue);
    assert!(row.issue_url.is_none());
    assert!(row.attachments_expire_at > row.created_at);
}

/// FR-002's three, each of which the database itself would refuse a fourth of.
#[tokio::test]
async fn each_of_the_three_kinds_is_recorded_as_itself() {
    let _guard = TABLE.lock().await;
    let state = test_app_state();
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let mut conn = state.db_pool.get().expect("a connection");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    for kind in [Kind::Issue, Kind::FeatureRequest, Kind::General] {
        let row = record(&state, a_submission(user_id, kind))
            .await
            .expect("recorded");
        assert_eq!(row.kind(), kind);
    }
}

/// The retention a person was promised is a column, not a computation, so
/// changing the constant later cannot retroactively shorten it (FR-016).
#[tokio::test]
async fn retention_is_written_onto_the_row_at_submission_time() {
    let _guard = TABLE.lock().await;
    let state = test_app_state();
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let mut conn = state.db_pool.get().expect("a connection");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    let row = record(&state, a_submission(user_id, Kind::General))
        .await
        .expect("recorded");
    let days = (row.attachments_expire_at - row.created_at).num_days();
    assert_eq!(days, RETENTION_DAYS);
    assert!(row.attachments_purged_at.is_none());
}

/// The world a submission names is resolved server-side, and a world the
/// caller is not a member of drops out rather than refusing the submission —
/// a report about a world you have left is still a report.
#[tokio::test]
async fn a_world_the_caller_is_not_in_is_dropped_rather_than_refused() {
    let _guard = TABLE.lock().await;
    let state = test_app_state();
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let mut conn = state.db_pool.get().expect("a connection");
    let owner = insert_test_user(&mut conn);
    let stranger = insert_test_user(&mut conn);
    let world_id = crate::test_support::insert_test_world(&mut conn, owner);
    drop(conn);

    let mut submission = a_submission(stranger, Kind::Issue);
    submission.world_id = Some(world_id);
    let row = record(&state, submission).await.expect("still recorded");
    assert_eq!(row.world_id, None);
    assert_eq!(row.game_system_id, None);

    let mut theirs = a_submission(owner, Kind::Issue);
    theirs.world_id = Some(world_id);
    let row = record(&state, theirs).await.expect("recorded");
    assert_eq!(row.world_id, Some(world_id));
}

/// FR-022. There is no argument by which another account's rows could be
/// reached, and this is the assertion that the filter is the one that matters.
#[tokio::test]
async fn a_submitter_sees_their_own_submissions_and_nobody_elses() {
    let _guard = TABLE.lock().await;
    let state = test_app_state();
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let mut conn = state.db_pool.get().expect("a connection");
    let mine = insert_test_user(&mut conn);
    let theirs = insert_test_user(&mut conn);
    drop(conn);

    let ours = record(&state, a_submission(mine, Kind::Issue))
        .await
        .expect("recorded");
    let other = record(&state, a_submission(theirs, Kind::General))
        .await
        .expect("recorded");

    let mut conn = state.db_pool.get().expect("a connection");
    let listed = mine_for(&mut conn, mine);
    assert!(listed.contains(&ours.id));
    assert!(!listed.contains(&other.id));

    let listed = mine_for(&mut conn, theirs);
    assert!(listed.contains(&other.id));
    assert!(!listed.contains(&ours.id));
}

/// The delivery key is generated at record time and never reused: it is the
/// handle a duplicate would be found by, and two submissions sharing one would
/// make a retry adopt somebody else's issue.
#[tokio::test]
async fn every_submission_gets_its_own_delivery_key() {
    let _guard = TABLE.lock().await;
    let state = test_app_state();
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let mut conn = state.db_pool.get().expect("a connection");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    let one = record(&state, a_submission(user_id, Kind::Issue))
        .await
        .expect("recorded");
    let two = record(&state, a_submission(user_id, Kind::Issue))
        .await
        .expect("recorded");
    assert_ne!(one.delivery_key, two.delivery_key);
    assert_ne!(one.delivery_key, one.id);
}

/// An operator's abandon and resume, and the guard that makes each of them a
/// transition rather than a write: abandoning something already abandoned
/// changes nothing and says so.
#[tokio::test]
async fn abandoning_is_a_transition_and_it_is_reversible() {
    let _guard = TABLE.lock().await;
    let state = test_app_state();
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let mut conn = state.db_pool.get().expect("a connection");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    let row = record(&state, a_submission(user_id, Kind::Issue))
        .await
        .expect("recorded");

    let mut conn = state.db_pool.get().expect("a connection");
    assert!(abandon(&mut conn, row.id, user_id).expect("abandons"));
    assert!(!abandon(&mut conn, row.id, user_id).expect("no second time"));
    assert_eq!(
        load(&mut conn, row.id)
            .expect("loads")
            .expect("exists")
            .delivery_state(),
        DeliveryState::Abandoned
    );
    assert!(resume(&mut conn, row.id, user_id).expect("resumes"));
    assert_eq!(
        load(&mut conn, row.id)
            .expect("loads")
            .expect("exists")
            .delivery_state(),
        DeliveryState::Pending
    );
}

/// The version surface FR-009 needs. It is never absent — "unknown build" is
/// an answer, and a missing field is not.
#[test]
fn the_server_records_which_build_it_is() {
    let version = server_version();
    assert!(!version.is_empty());
    assert!(version.contains(env!("CARGO_PKG_VERSION")));
}

/// The storage key is derived from ids this server generated, so a caller
/// cannot choose a path — the rule `rustfs::object_key` already states, and
/// the reason the sweep can delete under this prefix at all.
#[test]
fn an_attachments_key_is_derived_and_lives_under_the_deletable_prefix() {
    let key = object_key(Uuid::nil(), Uuid::nil(), AttachmentKind::Screenshot);
    assert!(key.starts_with(STORAGE_PREFIX));
    assert!(key.ends_with(".webp"));
    let key = object_key(Uuid::nil(), Uuid::nil(), AttachmentKind::Logs);
    assert!(key.ends_with(".log"));
}

fn mine_for(conn: &mut PgConnection, user_id: Uuid) -> Vec<Uuid> {
    mine(conn, user_id)
        .expect("lists")
        .into_iter()
        .map(|row| row.id)
        .collect()
}
