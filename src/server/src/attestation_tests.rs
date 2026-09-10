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

    let history = for_content(&state, approved.kind.into(), approved.publishable_id)
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
    let history = for_content(&state, approved.kind.into(), approved.publishable_id)
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

/// SC-003 for everything shared inside a collection: a notice about an item,
/// or about a lore entry that was never published any other way, reaches the
/// collection's agreement — and only the collections it is actually in.
///
/// Drop the membership half of `for_content` and the lore entry comes back
/// with no agreement at all, which is what a notice handler saw before.
#[tokio::test]
async fn a_collections_agreement_covers_what_is_inside_it() {
    use crate::schema::{world_collection_members, world_collections};
    use crate::test_support::{insert_test_item, insert_test_lore_entry, insert_test_world};

    let state = test_app_state();
    let version = archived_version(&state).await;
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, user_id);
    let item_id = insert_test_item(&mut conn, world_id, user_id);
    let lore_id = insert_test_lore_entry(&mut conn, world_id, user_id);

    let now = chrono::Utc::now().naive_utc();
    let mut collection = |name: &str| {
        let id = uuid::Uuid::now_v7();
        diesel::insert_into(world_collections::table)
            .values((
                world_collections::id.eq(id),
                world_collections::world_id.eq(world_id),
                world_collections::name.eq(name),
                world_collections::created_by.eq(user_id),
                world_collections::updated_by.eq(user_id),
                world_collections::created_at.eq(now),
                world_collections::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("collection");
        id
    };
    let containing = collection("holds both");
    let unrelated = collection("holds neither");

    for (member_type, member_id) in [("item", item_id), ("lore", lore_id)] {
        diesel::insert_into(world_collection_members::table)
            .values((
                world_collection_members::id.eq(uuid::Uuid::now_v7()),
                world_collection_members::collection_id.eq(containing),
                world_collection_members::member_type.eq(member_type),
                world_collection_members::member_id.eq(member_id),
                world_collection_members::sort_order.eq(0),
                world_collection_members::added_by.eq(user_id),
                world_collection_members::created_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("member");
    }

    let about = |kind: PublishableKind, publishable_id: uuid::Uuid| PendingAttestation {
        kind,
        publishable_id,
        ..pending(user_id, "someone", &version)
    };
    for (kind, id) in [
        (PublishableKind::Item, item_id),
        (PublishableKind::Collection, containing),
        (PublishableKind::Collection, unrelated),
    ] {
        record_sync(&mut conn, &about(kind, id), uuid::Uuid::now_v7()).expect("record");
    }
    drop(conn);

    let published_as = |records: Vec<Attestation>| {
        let mut ids: Vec<uuid::Uuid> = records.iter().filter_map(|r| r.publishable_id).collect();
        ids.sort();
        ids
    };
    let sorted = |mut ids: Vec<uuid::Uuid>| {
        ids.sort();
        ids
    };

    let for_item = for_content(&state, CoveredKind::Item, item_id)
        .await
        .expect("read");
    assert_eq!(
        published_as(for_item),
        sorted(vec![item_id, containing]),
        "its own agreement and its collection's — not a collection it is not in",
    );

    let for_lore = for_content(&state, CoveredKind::Lore, lore_id)
        .await
        .expect("read");
    assert_eq!(
        published_as(for_lore),
        vec![containing],
        "lore is only ever published inside a collection, so that is its agreement",
    );

    let for_collection = for_content(&state, CoveredKind::Collection, containing)
        .await
        .expect("read");
    assert_eq!(
        published_as(for_collection),
        vec![containing],
        "a collection's own history is its own; its members' do not flow upward",
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

/// The operator rows are global to the database, and one test below clears
/// them all. Held by every test here that writes one, so none of them sees
/// another's rows appear or vanish mid-assertion.
///
/// An async lock, because these tests await while holding it: a blocking
/// `std` guard held across an `.await` can stall the runtime's thread, which
/// is the very flakiness the lock is here to prevent.
async fn operator_rows() -> tokio::sync::MutexGuard<'static, ()> {
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    LOCK.lock().await
}

/// FR-044: one acknowledgement **per version of the words** — so an upgrade
/// that changes them can be acknowledged, and the same words cannot be twice.
#[tokio::test]
async fn each_version_is_acknowledged_once_and_a_new_version_again() {
    use crate::schema::terms_versions;

    let _rows = operator_rows().await;
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let operator = insert_test_user(&mut conn);

    let archive = |conn: &mut PgConnection| -> String {
        let body = format!("## Operator words\n\n{}", uuid::Uuid::now_v7());
        let version = crate::legal::version_id(crate::legal::OPERATOR_RESPONSIBILITIES_SLUG, &body);
        diesel::insert_into(terms_versions::table)
            .values(&crate::models::TermsVersion {
                version_id: version.clone(),
                document_slug: crate::legal::OPERATOR_RESPONSIBILITIES_SLUG.to_string(),
                body,
                first_seen_at: chrono::Utc::now().naive_utc(),
            })
            .execute(conn)
            .expect("archive");
        version
    };
    let before_upgrade = archive(&mut conn);
    let after_upgrade = archive(&mut conn);

    record_operator_sync(
        &mut conn,
        operator,
        Some("operator".into()),
        &before_upgrade,
    )
    .expect("the words setup showed");
    assert!(
        record_operator_sync(
            &mut conn,
            operator,
            Some("operator".into()),
            &before_upgrade
        )
        .is_err(),
        "the same words, twice, is one acknowledgement too many",
    );
    record_operator_sync(&mut conn, operator, Some("operator".into()), &after_upgrade)
        .expect("changed words are acknowledged afresh (FR-044)");
}

/// US8/FR-043: the operator acknowledgement is the same record, and there is
/// one of it for the words this build ships.
#[tokio::test]
async fn an_instance_acknowledges_once_on_the_same_record() {
    let _rows = operator_rows().await;
    let state = test_app_state();
    let version = crate::legal::operator_statement().version_id;
    crate::legal::ensure_terms_versions_recorded(&state)
        .await
        .expect("archive");
    let mut conn = state.db_pool.get().expect("conn");
    let operator = insert_test_user(&mut conn);

    // The table is global and the index allows one row per version, so a
    // previous run of this test may already hold the row for the words this
    // build ships. Clear it first: the assertion is about the constraint, not
    // about test ordering.
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
