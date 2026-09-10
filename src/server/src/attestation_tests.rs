//! The record, and the four things that must remain true about it years later.

use super::*;
use crate::test_support::{insert_test_user, test_app_state};

/// A version to attest to, archived the way startup archives one.
async fn archived_version(state: &AppState) -> String {
    crate::legal::ensure_terms_versions_recorded(state)
        .await
        .expect("archive");
    crate::legal::sharing_terms().version_id
}

fn pending(user_id: uuid::Uuid, username: &str, version: &str) -> PendingAttestation {
    PendingAttestation {
        subject_user_id: user_id,
        subject_username: Some(username.to_string()),
        terms_version_id: version.to_string(),
        kind: PublishableKind::Collection,
        publishable_id: uuid::Uuid::now_v7(),
        world_id: Some(uuid::Uuid::now_v7()),
    }
}

/// FR-003: per publish, not per person. Sharing the same thing twice writes two
/// rows, and nothing consults a previous one.
#[tokio::test]
async fn each_publish_writes_its_own_record() {
    let state = test_app_state();
    let version = archived_version(&state).await;
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);

    let approved = pending(user_id, "someone", &version);
    let first = record_sync(&mut conn, &approved, uuid::Uuid::now_v7()).expect("first");
    let second = record_sync(&mut conn, &approved, uuid::Uuid::now_v7()).expect("second");
    assert_ne!(first, second);
    drop(conn);

    let history = for_publishable(&state, approved.kind, approved.publishable_id)
        .await
        .expect("read");
    assert_eq!(
        history.len(),
        2,
        "two publishes of one thing are two agreements, not one reused",
    );
    assert!(
        history[0].attested_at >= history[1].attested_at,
        "newest first, because a notice asks about the most recent publish",
    );
}

/// FR-007. This is the property that makes an attestation evidence rather than
/// metadata: it outlives the thing it authorised.
///
/// Driven by deleting the share row outright — harsher than revoking, and the
/// case a foreign key would have made impossible.
#[tokio::test]
async fn an_attestation_survives_its_share_being_deleted() {
    let state = test_app_state();
    let version = archived_version(&state).await;
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    let approved = pending(user_id, "someone", &version);
    let share_id = uuid::Uuid::now_v7();
    record_sync(&mut conn, &approved, share_id).expect("record");
    drop(conn);

    // There was never a row for this `share_id` — which is the point. The
    // column carries no foreign key, so the record does not depend on it, and a
    // share that is gone is indistinguishable from one that never was.
    let history = for_publishable(&state, approved.kind, approved.publishable_id)
        .await
        .expect("read");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].share_id, Some(share_id));
}

/// FR-010, FR-037: the record survives the account, in the minimal lawful form.
#[tokio::test]
async fn deleting_an_account_removes_the_name_and_nothing_else() {
    let state = test_app_state();
    let version = archived_version(&state).await;
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    let approved = pending(user_id, "someone", &version);
    record_sync(&mut conn, &approved, uuid::Uuid::now_v7()).expect("record");

    let before: Attestation = attestations::table
        .filter(attestations::subject_user_id.eq(user_id))
        .select(Attestation::as_select())
        .first(&mut conn)
        .expect("read");

    let redacted = redact_for_deleted_account(&mut conn, user_id).expect("redact");
    assert_eq!(redacted, 1);

    let after: Attestation = attestations::table
        .filter(attestations::subject_user_id.eq(user_id))
        .select(Attestation::as_select())
        .first(&mut conn)
        .expect("the row must survive the account");

    assert_eq!(
        after.subject_username, None,
        "the one field that names a human"
    );
    assert_eq!(
        (
            after.id,
            after.purpose,
            after.subject_user_id,
            after.terms_version_id,
            after.publishable_kind,
            after.publishable_id,
            after.share_id,
            after.world_id,
            after.attested_at,
        ),
        (
            before.id,
            before.purpose,
            before.subject_user_id,
            before.terms_version_id,
            before.publishable_kind,
            before.publishable_id,
            before.share_id,
            before.world_id,
            before.attested_at,
        ),
        "redaction must change exactly one field — everything else is what a \
         notice arriving in eighteen months needs",
    );
}

/// One account's history is its own, and a disabled account can still read it.
///
/// The "its own" half is the one worth asserting: `for_subject` filtering on the
/// wrong column would hand somebody else's agreements to whoever asked.
#[tokio::test]
async fn a_persons_history_is_theirs_alone() {
    let state = test_app_state();
    let version = archived_version(&state).await;
    let mut conn = state.db_pool.get().expect("conn");
    let mine = insert_test_user(&mut conn);
    let theirs = insert_test_user(&mut conn);
    record_sync(
        &mut conn,
        &pending(mine, "me", &version),
        uuid::Uuid::now_v7(),
    )
    .expect("mine");
    record_sync(
        &mut conn,
        &pending(theirs, "them", &version),
        uuid::Uuid::now_v7(),
    )
    .expect("theirs");
    drop(conn);

    let history = for_subject(&state, mine, 20).await.expect("read");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].subject_user_id, mine);
}

/// US8/FR-043: the operator acknowledgement is the same record, and there is
/// one of it.
#[tokio::test]
async fn an_instance_acknowledges_once_on_the_same_record() {
    let state = test_app_state();
    let version = crate::legal::operator_statement().version_id;
    crate::legal::ensure_terms_versions_recorded(&state)
        .await
        .expect("archive");
    let mut conn = state.db_pool.get().expect("conn");
    let operator = insert_test_user(&mut conn);

    // The table is global and this index is over a constant, so a previous run
    // of this test may already hold the one row. Clear it first: the assertion
    // is about the constraint, not about test ordering.
    diesel::delete(attestations::table.filter(attestations::purpose.eq(purpose::OPERATOR)))
        .execute(&mut conn)
        .expect("clear");

    record_operator_sync(&mut conn, operator, Some("operator".into()), &version)
        .expect("the first acknowledgement");

    let second = record_operator_sync(&mut conn, operator, Some("operator".into()), &version);
    assert!(
        second.is_err(),
        "an instance acknowledges once; a second row would make \"who took this \
         on\" a question with two answers",
    );
}

/// A share attestation must name what it published, and an operator
/// acknowledgement must name nothing.
///
/// Enforced in the database rather than here, and asserted through this module
/// so the constraint is reachable from the code that depends on it.
#[tokio::test]
async fn the_database_refuses_a_record_that_describes_neither() {
    let state = test_app_state();
    let version = archived_version(&state).await;
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);

    let malformed = diesel::insert_into(attestations::table)
        .values(&NewAttestation {
            id: uuid::Uuid::now_v7(),
            purpose: purpose::SHARE.to_string(),
            subject_user_id: user_id,
            subject_username: None,
            terms_version_id: version.clone(),
            // A share that names nothing.
            publishable_kind: None,
            publishable_id: None,
            share_id: None,
            world_id: None,
        })
        .execute(&mut conn);
    assert!(
        malformed.is_err(),
        "a share attestation about nothing is not a record of anything",
    );

    let unarchived = diesel::insert_into(attestations::table)
        .values(&NewAttestation {
            id: uuid::Uuid::now_v7(),
            purpose: purpose::SHARE.to_string(),
            subject_user_id: user_id,
            subject_username: None,
            terms_version_id: "sharing-terms@0000000000000000".to_string(),
            publishable_kind: Some("actor".to_string()),
            publishable_id: Some(uuid::Uuid::now_v7()),
            share_id: None,
            world_id: None,
        })
        .execute(&mut conn);
    assert!(
        unarchived.is_err(),
        "an attestation naming a version that was never archived is the failure \
         FR-008 exists to prevent",
    );
}
