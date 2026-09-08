//! The state machine, and the promise FR-015 is: a message an instance cannot
//! send is recorded, not discarded.
//!
//! The database-backed tests need `DATABASE_URL`, like every other integration
//! test in this crate (`test_support`). The pure ones need nothing, which is
//! why the transition rules were extracted into functions in the first place.

use std::sync::Arc;

use chrono::{Duration, Utc};
use uuid::Uuid;

use super::*;
use crate::mail::capture::CapturingTransport;
use crate::mail::{Availability, DeliveryFailure, MailSeam};
use crate::state::AppState;
use crate::test_support::test_app_state;

const MISSING: [&str; 2] = ["mail.enabled", "mail.host"];

/// One `mail_outbox` table, shared by every test in this binary.
///
/// `release_blocked` moves **every** blocked row to queued — it is a sweep,
/// not a per-message operation, which is right for the tick and fatal for
/// tests that run beside each other: one test configuring mail released
/// another test's message, and the second saw `Queued` where it had just
/// written `Blocked`. The listing tests have the same exposure from the other
/// direction, since they assert over rows the table happens to hold.
///
/// So the tests that care about outbox *state* take this. A `tokio::sync`
/// mutex rather than `std`, because these are async and clippy rightly
/// refuses a std guard held across an await — the same fix
/// `auth/instance_access.rs` uses for the instance policy, which is one row
/// shared the same way.
static OUTBOX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn unconfigured_instance() -> AppState {
    let mut state = test_app_state();
    state.mail = MailSeam::overridden(Arc::new(CapturingTransport::unconfigured(MISSING.to_vec())));
    state
}

fn instance_that_can_send() -> (AppState, Arc<CapturingTransport>) {
    let transport = Arc::new(CapturingTransport::accepting());
    let mut state = test_app_state();
    state.mail = MailSeam::overridden(transport.clone());
    (state, transport)
}

fn a_message() -> NewOutboxMessage {
    NewOutboxMessage {
        purpose: PURPOSE_TEST.to_string(),
        to_address: "operator@example.invalid".to_string(),
        subject: "Does mail work?".to_string(),
        body_text: "If you are reading this, it does.".to_string(),
        created_by: None,
    }
}

fn read(state: &AppState, id: Uuid) -> OutboxRow {
    let mut conn = state.db_pool.get().expect("a connection");
    load(&mut conn, id)
        .expect("the row loads")
        .expect("the row exists")
}

// -- The pure half ----------------------------------------------------------

/// FR-015, as a rule rather than as a code path: an instance that cannot send
/// writes the message down anyway, in a state that says why.
#[test]
fn an_unconfigured_instance_blocks_a_message_and_names_what_is_missing() {
    let (state, reason) = initial_state(&Availability::Unconfigured {
        missing: MISSING.to_vec(),
    });
    assert_eq!(state, OutboxState::Blocked);
    let reason = reason.expect("a blocked message says why");
    assert!(reason.contains("`mail.host`"), "{reason}");
    assert!(reason.contains("`mail.enabled`"), "{reason}");
}

#[test]
fn a_configured_instance_queues_with_nothing_to_explain() {
    let (state, reason) = initial_state(&Availability::Ready);
    assert_eq!(state, OutboxState::Queued);
    assert_eq!(reason, None);
}

/// A refusal that trying again cannot fix goes straight to `failed`. Putting
/// it on the curve would mean nine pointless attempts before an operator is
/// told anything is wrong.
#[test]
fn a_permanent_refusal_fails_immediately_rather_than_walking_the_curve() {
    let now = Utc::now().naive_utc();
    let outcome = outcome_after_failure(1, &DeliveryFailure::permanent("no"), now);
    assert_eq!(outcome.state, OutboxState::Failed);
    assert_eq!(outcome.next_attempt_at, None);
}

#[test]
fn a_transient_refusal_goes_back_to_queued_with_a_later_time() {
    let now = Utc::now().naive_utc();
    let outcome = outcome_after_failure(1, &DeliveryFailure::transient("later"), now);
    assert_eq!(outcome.state, OutboxState::Queued);
    assert_eq!(outcome.next_attempt_at, Some(now + Duration::seconds(30)));
}

/// The curve ends. A queue that retries forever never tells anybody anything
/// is wrong, which is the failure mode FR-016 exists against.
#[test]
fn a_transient_refusal_fails_once_the_curve_is_exhausted() {
    let now = Utc::now().naive_utc();
    let outcome = outcome_after_failure(MAX_ATTEMPTS, &DeliveryFailure::transient("still no"), now);
    assert_eq!(outcome.state, OutboxState::Failed);
    assert_eq!(outcome.next_attempt_at, None);
}

/// A sent message is never re-sent, and one in flight is never started twice.
#[test]
fn only_a_message_that_is_not_going_anywhere_may_be_retried() {
    assert!(OutboxState::Blocked.may_be_retried());
    assert!(OutboxState::Failed.may_be_retried());
    assert!(OutboxState::Queued.may_be_retried());
    assert!(!OutboxState::Sent.may_be_retried());
    assert!(!OutboxState::Sending.may_be_retried());
}

#[test]
fn every_state_survives_the_round_trip_the_database_stores_it_by() {
    for state in [
        OutboxState::Queued,
        OutboxState::Blocked,
        OutboxState::Sending,
        OutboxState::Sent,
        OutboxState::Failed,
    ] {
        assert_eq!(OutboxState::parse(state.as_str()), Some(state));
    }
    assert_eq!(OutboxState::parse("posted"), None);
}

// -- The database-backed half -----------------------------------------------

/// The whole of FR-015 in one test: an instance with no mail configured is
/// handed a message, and the message is still there afterwards, in a state
/// that is not `failed`, with a reason naming a setting.
#[tokio::test]
async fn an_unconfigured_instance_records_a_message_rather_than_discarding_it() {
    let _outbox = OUTBOX.lock().await;
    let state = unconfigured_instance();
    let id = enqueue(&state, a_message()).await.expect("it is written");

    let row = read(&state, id);
    assert_eq!(row.state(), Some(OutboxState::Blocked));
    assert_ne!(row.state(), Some(OutboxState::Failed));
    assert_eq!(row.attempts, 0);
    assert_eq!(row.sent_at, None);
    let reason = row.last_failure_reason.expect("it says why");
    assert!(reason.contains("`mail.host`"), "{reason}");
}

/// And the release: configuring mail is what unblocks the queue.
#[tokio::test]
async fn configuring_mail_releases_what_was_held() {
    let _outbox = OUTBOX.lock().await;
    let state = unconfigured_instance();
    let id = enqueue(&state, a_message()).await.expect("it is written");

    let mut conn = state.db_pool.get().expect("a connection");
    let released = release_blocked(&mut conn).expect("the release runs");
    assert!(released >= 1);
    drop(conn);

    let row = read(&state, id);
    assert_eq!(row.state(), Some(OutboxState::Queued));
    assert_eq!(row.next_attempt_at, None);
}

#[tokio::test]
async fn a_configured_instance_queues_and_the_body_is_never_stored_in_the_clear() {
    let _outbox = OUTBOX.lock().await;
    let (state, _) = instance_that_can_send();
    let id = enqueue(&state, a_message()).await.expect("it is written");

    let row = read(&state, id);
    assert_eq!(row.state(), Some(OutboxState::Queued));
    // The columns are ciphertext, in `crypto.rs`'s own format. Asserted on the
    // stored value rather than on the writer, because FR-016 is a claim about
    // what is in the database.
    assert!(
        row.body_encrypted.starts_with("v1."),
        "{}",
        row.body_encrypted
    );
    assert!(!row.body_encrypted.contains("If you are reading this"));
    assert!(!row.subject_encrypted.contains("Does mail work?"));

    // And it reads back, because a retry has to send the message that failed.
    let message = row.decrypt(&state).expect("it decrypts");
    assert_eq!(message.body_text, "If you are reading this, it does.");
    assert_eq!(message.subject, "Does mail work?");
}

/// The single-flight guard. Two senders, one message, one attempt — enforced
/// by one conditional statement rather than by a read followed by a write.
#[tokio::test]
async fn a_message_can_only_be_claimed_once() {
    let _outbox = OUTBOX.lock().await;
    let (state, _) = instance_that_can_send();
    let id = enqueue(&state, a_message()).await.expect("it is written");
    let now = Utc::now().naive_utc();

    let mut conn = state.db_pool.get().expect("a connection");
    assert!(mark_sending(&mut conn, id, now).expect("the first claim runs"));
    assert!(!mark_sending(&mut conn, id, now).expect("the second claim runs"));

    let row = load(&mut conn, id).expect("loads").expect("exists");
    assert_eq!(row.state(), Some(OutboxState::Sending));
    assert_eq!(row.attempts, 1);
}

/// `sent_at` is the idempotence guard, and a retry checks it rather than the
/// state string (contracts/mail.md rule 6).
#[tokio::test]
async fn a_sent_message_is_never_sent_again_by_a_retry() {
    let _outbox = OUTBOX.lock().await;
    let (state, _) = instance_that_can_send();
    let id = enqueue(&state, a_message()).await.expect("it is written");
    let now = Utc::now().naive_utc();

    let mut conn = state.db_pool.get().expect("a connection");
    record_success(&mut conn, id, now).expect("it is recorded");
    let row = load(&mut conn, id).expect("loads").expect("exists");
    assert_eq!(row.state(), Some(OutboxState::Sent));
    // Present, not compared exactly: Postgres stores microseconds and
    // `Utc::now()` has nanoseconds, so an equality here fails about half the
    // time on a number nothing depends on.
    assert!(row.sent_at.is_some());

    let refusal = requeue_now(&mut conn, &row).expect_err("a sent message is not requeued");
    assert!(refusal.contains("already been sent"), "{refusal}");
}

/// A failure is recorded where an operator will look for it, in prose, with
/// the row still present.
#[tokio::test]
async fn a_failure_is_recorded_on_the_message_rather_than_lost_with_the_attempt() {
    let _outbox = OUTBOX.lock().await;
    let (state, _) = instance_that_can_send();
    let id = enqueue(&state, a_message()).await.expect("it is written");
    let now = Utc::now().naive_utc();

    let mut conn = state.db_pool.get().expect("a connection");
    mark_sending(&mut conn, id, now).expect("claimed");
    let row = load(&mut conn, id).expect("loads").expect("exists");
    let failure = DeliveryFailure::transient("Check `mail.host` and `mail.port`.");
    let outcome = record_failure(&mut conn, &row, &failure, now).expect("recorded");

    assert_eq!(outcome.state, OutboxState::Queued);
    let row = load(&mut conn, id).expect("loads").expect("exists");
    assert_eq!(
        row.last_failure_reason.as_deref(),
        Some("Check `mail.host` and `mail.port`.")
    );
    assert!(row.next_attempt_at.is_some());
}

/// A subject is content unless the instance wrote it about itself. The
/// disclosure rule is asked of the row, so no surface has to remember it.
#[tokio::test]
async fn a_subject_is_disclosed_only_for_the_instances_own_test_message() {
    let _outbox = OUTBOX.lock().await;
    let (state, _) = instance_that_can_send();
    let test_id = enqueue(&state, a_message()).await.expect("written");
    let about_a_person = enqueue(
        &state,
        NewOutboxMessage {
            purpose: "invitation".to_string(),
            ..a_message()
        },
    )
    .await
    .expect("written");

    assert_eq!(
        read(&state, test_id)
            .subject_if_disclosable(&state)
            .as_deref(),
        Some("Does mail work?")
    );
    assert_eq!(
        read(&state, about_a_person).subject_if_disclosable(&state),
        None
    );
}

#[tokio::test]
async fn the_listing_is_newest_first_and_can_be_narrowed_to_one_state() {
    let _outbox = OUTBOX.lock().await;
    let state = unconfigured_instance();
    let id = enqueue(&state, a_message()).await.expect("written");

    let mut conn = state.db_pool.get().expect("a connection");
    let blocked = list(&mut conn, Some(OutboxState::Blocked), 50).expect("lists");
    assert!(blocked.iter().any(|row| row.id == id));
    assert!(blocked.iter().all(|row| row.state == "blocked"));

    let sent = list(&mut conn, Some(OutboxState::Sent), 50).expect("lists");
    assert!(!sent.iter().any(|row| row.id == id));
}
