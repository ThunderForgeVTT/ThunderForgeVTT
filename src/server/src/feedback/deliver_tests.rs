//! Delivery, against a host that records what it was asked for instead of
//! calling GitHub.
//!
//! What these prove is the code *around* the host: that an unconfigured
//! instance records a reason rather than losing the submission, that a
//! delivered submission carries the right labels and body, that an **ambiguous**
//! failure makes the next attempt search first and a 4xx does not, and that a
//! failure is recorded in a vocabulary that cannot carry a credential. They
//! prove nothing about GitHub, which is what the end-to-end harness is for.
//!
//! They serialise on [`TABLE`] for the reason `record_tests.rs` gives: the
//! selection tests read across one shared table.

use std::sync::Arc;

use super::*;
use crate::feedback::host::capture::{Call, CapturingHost};
use crate::feedback::{DeliveryState, FeedbackSeam, Kind, NewSubmission, schedule};
use crate::repo_host::scoped::{HostFailure, PostedIssue};
use crate::test_support::{insert_test_user, test_app_state};

use crate::feedback::{TEST_TABLE as TABLE, clear_for_test};

fn instance_with(host: Arc<CapturingHost>) -> crate::state::AppState {
    let mut state = test_app_state();
    state.feedback = FeedbackSeam::overridden(host);
    state
}

async fn a_recorded_submission(
    state: &crate::state::AppState,
    kind: Kind,
) -> crate::feedback::SubmissionRow {
    let mut conn = state.db_pool.get().expect("a connection");
    let user_id = insert_test_user(&mut conn);
    drop(conn);
    crate::feedback::record(
        state,
        NewSubmission {
            user_id,
            kind,
            message: "Tokens vanish when the scene changes".to_string(),
            summary: Some("Tokens vanish".to_string()),
            screen_path: Some("/world/abc/play".to_string()),
            world_id: None,
            client_version: "1.4.2".to_string(),
            browser: "Chrome on Linux".to_string(),
            attachments: Vec::new(),
        },
    )
    .await
    .expect("recorded")
}

// -- The pure half ----------------------------------------------------------

/// FR-021's vocabulary, one arm at a time. Each is a different fix, and
/// collapsing them would make the operator's view a list of things that all
/// say "it did not work".
#[test]
fn each_host_failure_maps_to_a_reason_an_operator_can_act_on() {
    let at = |status: Option<u16>| HostFailure {
        status,
        transport: status.is_none(),
    };
    assert_eq!(
        FailureReason::from_host(&at(None)),
        FailureReason::HostUnavailable
    );
    assert_eq!(
        FailureReason::from_host(&at(Some(401))),
        FailureReason::CredentialsRejected
    );
    assert_eq!(
        FailureReason::from_host(&at(Some(403))),
        FailureReason::PermissionRefused
    );
    assert_eq!(
        FailureReason::from_host(&at(Some(404))),
        FailureReason::DestinationNotFound
    );
    assert_eq!(
        FailureReason::from_host(&at(Some(429))),
        FailureReason::HostRateLimited
    );
    assert_eq!(
        FailureReason::from_host(&at(Some(503))),
        FailureReason::HostUnavailable
    );
    assert_eq!(
        FailureReason::from_host(&at(Some(422))),
        FailureReason::RejectedByHost
    );
}

/// FR-021's other half, and the reason the reason is an enum: there is no
/// value in this vocabulary that a host could have written.
#[test]
fn no_failure_reason_can_carry_anything_from_a_host() {
    for reason in [
        FailureReason::NotConfigured,
        FailureReason::CredentialsRejected,
        FailureReason::DestinationNotFound,
        FailureReason::PermissionRefused,
        FailureReason::HostUnavailable,
        FailureReason::HostRateLimited,
        FailureReason::RejectedByHost,
        FailureReason::AttachmentsExpired,
    ] {
        let text = reason.as_str();
        assert_eq!(FailureReason::parse(text), Some(reason));
        assert!(text.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
    }
}

/// The whole of "search before create, but only after an ambiguous failure",
/// as a function rather than as a branch inside a delivery attempt.
#[test]
fn only_a_lost_answer_makes_the_next_attempt_search() {
    assert!(is_ambiguous(Some("failed"), None));
    assert!(is_ambiguous(Some("failed"), Some(500)));
    assert!(is_ambiguous(Some("failed"), Some(502)));
    // A 4xx means the host declined and created nothing.
    assert!(!is_ambiguous(Some("failed"), Some(422)));
    assert!(!is_ambiguous(Some("failed"), Some(403)));
    // Nothing reached the host at all, so there is nothing to find.
    assert!(!is_ambiguous(Some("not_configured"), None));
    assert!(!is_ambiguous(Some("created"), Some(201)));
    assert!(!is_ambiguous(None, None));
}

// -- Against a host ---------------------------------------------------------

/// FR-030: an unconfigured instance still collects feedback, it just cannot
/// forward it yet — and it says which state that is, in a reason an operator
/// can act on, without touching the submission.
#[tokio::test]
async fn an_unconfigured_instance_records_a_reason_and_keeps_the_submission() {
    let _guard = TABLE.lock().await;
    // No seam override and no destination row: the ordinary state of an
    // instance nobody has configured.
    let state = test_app_state();
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let row = a_recorded_submission(&state, Kind::Issue).await;

    let outcome = attempt(&state, row.id)
        .await
        .expect("an attempt is recorded");
    assert_eq!(outcome, Outcome::NotConfigured);

    let mut conn = state.db_pool.get().expect("a connection");
    let after = crate::feedback::load(&mut conn, row.id)
        .expect("loads")
        .expect("exists");
    assert_eq!(after.delivery_state(), DeliveryState::Pending);
    assert!(after.issue_url.is_none());

    let attempts = schedule::attempts_of(&mut conn, row.id).expect("attempts");
    assert_eq!(attempts.len(), 1);
    assert_eq!(
        attempts[0].2.as_deref(),
        Some(FailureReason::NotConfigured.as_str())
    );
}

/// FR-017 and FR-020 together: the issue carries the message and the kind, and
/// the kind is on it as a label so a maintainer does not have to read the body
/// to know what it is.
#[tokio::test]
async fn a_delivered_submission_arrives_with_its_message_and_its_kind() {
    let _guard = TABLE.lock().await;
    let host = Arc::new(CapturingHost::accepting());
    let state = instance_with(host.clone());
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let row = a_recorded_submission(&state, Kind::FeatureRequest).await;

    assert_eq!(
        attempt(&state, row.id).await.expect("attempted"),
        Outcome::Created
    );

    let Some(Call::Created {
        title,
        body,
        labels,
    }) = host.created()
    else {
        panic!("the host was never asked to create an issue");
    };
    assert!(title.starts_with("[Feature] "));
    assert_eq!(
        labels,
        vec![
            "feedback".to_string(),
            "feedback:feature-request".to_string()
        ]
    );
    assert!(body.contains("Tokens vanish when the scene changes"));
    // FR-013: a reference, never an address.
    assert!(body.contains(&row.id.to_string()));
    assert!(!body.contains('@'));
    // FR-019's handle.
    assert!(body.contains(&row.delivery_key.to_string()));

    let mut conn = state.db_pool.get().expect("a connection");
    let after = crate::feedback::load(&mut conn, row.id)
        .expect("loads")
        .expect("exists");
    assert_eq!(after.delivery_state(), DeliveryState::Delivered);
    assert_eq!(
        after.issue_url.as_deref(),
        Some("https://github.com/o/n/issues/7")
    );
    assert_eq!(after.issue_state.as_deref(), Some("open"));
}

/// The first attempt does not search. A search that finds nothing costs a
/// request and a rate-limit unit for no information, and there is nothing to
/// find before anything has been created.
#[tokio::test]
async fn a_first_attempt_creates_without_searching() {
    let _guard = TABLE.lock().await;
    let host = Arc::new(CapturingHost::accepting());
    let state = instance_with(host.clone());
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let row = a_recorded_submission(&state, Kind::Issue).await;

    attempt(&state, row.id).await.expect("attempted");
    assert!(
        !host
            .calls()
            .iter()
            .any(|c| matches!(c, Call::Searched { .. })),
        "{:?}",
        host.calls()
    );
}

/// FR-019, the case that could break it: the host may have created the issue
/// and the answer was lost. The next attempt searches for the key this product
/// wrote, finds it, and **adopts** it rather than creating a second one.
#[tokio::test]
async fn an_ambiguous_failure_makes_the_next_attempt_adopt_rather_than_duplicate() {
    let _guard = TABLE.lock().await;
    let refusing = Arc::new(CapturingHost::refusing(HostFailure {
        status: None,
        transport: true,
    }));
    let state = instance_with(refusing.clone());
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let row = a_recorded_submission(&state, Kind::Issue).await;

    assert_eq!(
        attempt(&state, row.id).await.expect("attempted"),
        Outcome::Failed
    );

    // The same instance, now able to reach a host that already holds the issue
    // the lost answer belonged to.
    let holding = Arc::new(CapturingHost::holding(PostedIssue {
        number: 12,
        html_url: "https://github.com/o/n/issues/12".to_string(),
    }));
    let mut state = state;
    state.feedback = FeedbackSeam::overridden(holding.clone());

    assert_eq!(
        attempt(&state, row.id).await.expect("attempted"),
        Outcome::Adopted
    );
    assert_eq!(
        holding.calls(),
        vec![Call::Searched {
            delivery_key: row.delivery_key.to_string()
        }],
        "an adopted submission must not also be created"
    );

    let mut conn = state.db_pool.get().expect("a connection");
    let after = crate::feedback::load(&mut conn, row.id)
        .expect("loads")
        .expect("exists");
    assert_eq!(after.delivery_state(), DeliveryState::Delivered);
    assert_eq!(after.issue_number, Some(12));
}

/// The other side of the same rule: a 4xx means the host declined and created
/// nothing, so the next attempt creates directly. Searching there would be a
/// request for an issue nobody made.
#[tokio::test]
async fn a_refusal_by_the_host_does_not_make_the_next_attempt_search() {
    let _guard = TABLE.lock().await;
    let refusing = Arc::new(CapturingHost::refusing(HostFailure {
        status: Some(422),
        transport: false,
    }));
    let state = instance_with(refusing);
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let row = a_recorded_submission(&state, Kind::Issue).await;
    attempt(&state, row.id).await.expect("attempted");

    let accepting = Arc::new(CapturingHost::accepting());
    let mut state = state;
    state.feedback = FeedbackSeam::overridden(accepting.clone());
    attempt(&state, row.id).await.expect("attempted");

    assert!(
        !accepting
            .calls()
            .iter()
            .any(|c| matches!(c, Call::Searched { .. })),
        "{:?}",
        accepting.calls()
    );
    assert!(accepting.created().is_some());
}

/// FR-021: an operator can see what has not been delivered, and why, and the
/// why is a vocabulary entry rather than anything the host said.
#[tokio::test]
async fn an_operator_sees_what_has_not_been_delivered_and_why() {
    let _guard = TABLE.lock().await;
    let refusing = Arc::new(CapturingHost::refusing(HostFailure {
        status: Some(403),
        transport: false,
    }));
    let state = instance_with(refusing);
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let row = a_recorded_submission(&state, Kind::General).await;
    attempt(&state, row.id).await.expect("attempted");

    let mut conn = state.db_pool.get().expect("a connection");
    let waiting = undelivered(&mut conn).expect("lists");
    assert!(waiting.iter().any(|r| r.id == row.id));

    let attempts = schedule::attempts_of(&mut conn, row.id).expect("attempts");
    assert_eq!(
        attempts[0].2.as_deref(),
        Some(FailureReason::PermissionRefused.as_str())
    );
}

/// FR-019's backoff, and the single-flight guard, expressed as the selection
/// rather than as an `if` in a loop: a submission just attempted is not due
/// again yet, and one whose attempt has not finished is not due at all.
#[tokio::test]
async fn a_submission_just_attempted_is_not_due_again_until_the_backoff_elapses() {
    let _guard = TABLE.lock().await;
    let state = test_app_state();
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let row = a_recorded_submission(&state, Kind::Issue).await;
    let mut conn = state.db_pool.get().expect("a connection");
    let now = chrono::Utc::now().naive_utc();

    let due = schedule::due_now(&mut conn, now).expect("selects");
    assert!(due.iter().any(|d| d.submission_id == row.id));

    // An attempt that has not finished: in flight, and not due.
    let attempt_id = begin(&mut conn, row.id, 1, now).expect("begins");
    let due = schedule::due_now(&mut conn, now).expect("selects");
    assert!(!due.iter().any(|d| d.submission_id == row.id));

    // Finished and failed: due again, but only after the first backoff step.
    finish(
        &mut conn,
        attempt_id,
        Outcome::Failed,
        Some(FailureReason::HostUnavailable),
        Some(503),
        now,
    )
    .expect("finishes");
    let due = schedule::due_now(&mut conn, now).expect("selects");
    assert!(!due.iter().any(|d| d.submission_id == row.id));
    let later = now + chrono::Duration::seconds(schedule::BACKOFF_SECONDS[0] + 1);
    let due = schedule::due_now(&mut conn, later).expect("selects");
    assert!(due.iter().any(|d| d.submission_id == row.id));
}

/// Delivering a submission whose evidence has expired would produce a report
/// without the thing it was for, so it is excluded from the selection and
/// recorded honestly if it is attempted anyway.
#[tokio::test]
async fn a_submission_whose_evidence_has_expired_is_not_delivered() {
    let _guard = TABLE.lock().await;
    let host = Arc::new(CapturingHost::accepting());
    let state = instance_with(host.clone());
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let row = a_recorded_submission(&state, Kind::Issue).await;

    let mut conn = state.db_pool.get().expect("a connection");
    crate::feedback::record_purged(&mut conn, row.id, chrono::Utc::now().naive_utc())
        .expect("purges");
    let due = schedule::due_now(&mut conn, chrono::Utc::now().naive_utc()).expect("selects");
    assert!(!due.iter().any(|d| d.submission_id == row.id));
    drop(conn);

    assert_eq!(
        attempt(&state, row.id).await.expect("attempted"),
        Outcome::Failed
    );
    assert!(host.created().is_none());

    let mut conn = state.db_pool.get().expect("a connection");
    let attempts = schedule::attempts_of(&mut conn, row.id).expect("attempts");
    assert_eq!(
        attempts[0].2.as_deref(),
        Some(FailureReason::AttachmentsExpired.as_str())
    );
}

/// An abandoned submission is not retried, which is what "abandoned by an
/// operator" has to mean for the button to be worth pressing.
#[tokio::test]
async fn an_abandoned_submission_is_not_selected() {
    let _guard = TABLE.lock().await;
    let state = test_app_state();
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let row = a_recorded_submission(&state, Kind::Issue).await;
    let mut conn = state.db_pool.get().expect("a connection");
    crate::feedback::abandon(&mut conn, row.id, row.user_id).expect("abandons");

    let due = schedule::due_now(&mut conn, chrono::Utc::now().naive_utc()).expect("selects");
    assert!(!due.iter().any(|d| d.submission_id == row.id));

    crate::feedback::resume(&mut conn, row.id, row.user_id).expect("resumes");
    let due = schedule::due_now(&mut conn, chrono::Utc::now().naive_utc()).expect("selects");
    assert!(due.iter().any(|d| d.submission_id == row.id));
}

/// US5: what a maintainer does with the issue comes back, with the time it was
/// observed — never one without the other.
#[tokio::test]
async fn an_issue_a_maintainer_closes_is_reflected_to_the_submitter() {
    let _guard = TABLE.lock().await;
    let host = Arc::new(CapturingHost::accepting());
    let state = instance_with(host);
    clear_for_test(&mut state.db_pool.get().expect("a connection"));
    let row = a_recorded_submission(&state, Kind::Issue).await;
    attempt(&state, row.id).await.expect("attempted");

    let mut conn = state.db_pool.get().expect("a connection");
    crate::feedback::record_issue_state(
        &mut conn,
        row.id,
        "closed",
        chrono::Utc::now().naive_utc(),
    )
    .expect("records");

    let after = crate::feedback::load(&mut conn, row.id)
        .expect("loads")
        .expect("exists");
    assert_eq!(after.issue_state.as_deref(), Some("closed"));
    assert!(after.issue_state_checked_at.is_some());
}
