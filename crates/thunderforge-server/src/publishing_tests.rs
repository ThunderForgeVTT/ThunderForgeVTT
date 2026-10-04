//! The gate's refusals, and the two properties they must have.

use super::*;
use crate::test_support::{insert_test_user, test_app_state};

async fn current_version(state: &AppState) -> String {
    crate::legal::ensure_terms_versions_recorded(state)
        .await
        .expect("archive");
    crate::legal::sharing_terms().version_id
}

fn offered(version: &str) -> AttestationInput {
    AttestationInput {
        terms_version_id: version.to_string(),
    }
}

/// The ordinary case: the version the dialog was shown, echoed back.
#[tokio::test]
async fn the_current_version_is_accepted_and_recorded_as_offered() {
    let state = test_app_state();
    let version = current_version(&state).await;
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    let target = uuid::Uuid::now_v7();
    let world = uuid::Uuid::now_v7();
    let approved = require_attestation(
        &state,
        user_id,
        PublishableKind::Collection,
        target,
        Some(world),
        &offered(&version),
    )
    .await
    .expect("the current version must be accepted");

    assert_eq!(approved.terms_version_id, version);
    assert_eq!(approved.subject_user_id, user_id);
    assert_eq!(approved.publishable_id, target);
    assert_eq!(approved.world_id, Some(world));
    assert!(
        approved.subject_username.is_some(),
        "the name is snapshotted at the moment of publishing, so the record can \
         answer \"who agreed\" after a rename",
    );
}

/// A fabricated identity, an empty one, and one that is nearly right.
///
/// All three get the same sentence, and none of them names a valid identity —
/// FR-013. A refusal that echoed the expected version would be a refusal that
/// tells a caller how to skip the dialog.
#[tokio::test]
async fn an_unknown_version_is_refused_without_naming_a_valid_one() {
    let state = test_app_state();
    let version = current_version(&state).await;
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    for candidate in [
        "",
        "   ",
        "sharing-terms@0000000000000000",
        "not-even-shaped-like-one",
        // The right document, a plausible-looking hash, and not in the archive.
        "sharing-terms@deadbeefdeadbeef",
    ] {
        let refusal = require_attestation(
            &state,
            user_id,
            PublishableKind::Actor,
            uuid::Uuid::now_v7(),
            None,
            &offered(candidate),
        )
        .await
        .expect_err(&format!("`{candidate}` must be refused"));

        let message = refusal.message;
        assert_eq!(message, TERMS_REFUSED, "one message for every refusal");
        assert!(
            !message.contains(&version),
            "a refusal must not name a valid version identity: {message}",
        );
        assert!(
            !message.contains('@'),
            "and must not name anything shaped like one: {message}",
        );
    }
}

/// FR-005's race, from the person's side: they opened the dialog, an operator
/// revised the terms, and their publish must still work — recorded against the
/// words they were actually shown.
///
/// The superseded version is in the archive, which is the whole reason the
/// archive is append-only.
#[tokio::test]
async fn a_superseded_version_still_in_the_archive_is_accepted() {
    use crate::schema::terms_versions;

    let state = test_app_state();
    current_version(&state).await;

    // An older version, as `ensure_terms_versions_recorded` would have left it
    // on a build whose prose has since changed.
    let older = "sharing-terms@aaaabbbbccccdddd";
    let mut conn = state.db_pool.get().expect("conn");
    diesel::insert_into(terms_versions::table)
        .values(&crate::models::TermsVersion {
            version_id: older.to_string(),
            document_slug: crate::legal::SHARING_TERMS_SLUG.to_string(),
            body: "## An older wording\n\nWhat it used to say.".to_string(),
            first_seen_at: chrono::Utc::now().naive_utc() - chrono::Duration::days(30),
        })
        .on_conflict(terms_versions::version_id)
        .do_nothing()
        .execute(&mut conn)
        .expect("archive an older version");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    let approved = require_attestation(
        &state,
        user_id,
        PublishableKind::Item,
        uuid::Uuid::now_v7(),
        None,
        &offered(older),
    )
    .await
    .expect("a superseded version in the archive must be accepted");

    assert_eq!(
        approved.terms_version_id, older,
        "and recorded as the version they were actually shown, not silently \
         upgraded to the current one",
    );
}

/// The gate writes nothing.
///
/// A gate that recorded on approval would leave an attestation behind for a
/// publish that then failed its ownership check — an agreement to publish
/// something that was never published.
#[tokio::test]
async fn approving_a_publish_records_nothing_on_its_own() {
    use crate::schema::attestations;

    let state = test_app_state();
    let version = current_version(&state).await;
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    require_attestation(
        &state,
        user_id,
        PublishableKind::Ability,
        uuid::Uuid::now_v7(),
        None,
        &offered(&version),
    )
    .await
    .expect("approved");

    let mut conn = state.db_pool.get().expect("conn");
    let written: i64 = attestations::table
        .filter(attestations::subject_user_id.eq(user_id))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(
        written, 0,
        "the gate approves; the caller records, inside the transaction that \
         mints the share",
    );
}
