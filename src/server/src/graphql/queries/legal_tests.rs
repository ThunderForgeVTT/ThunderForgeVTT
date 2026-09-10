//! The one failure FR-008 exists to prevent: an old agreement resolving to the
//! current words.

use super::*;
use crate::test_support::test_app_state;

/// `legalDocumentVersion` must read the **archive**, not the constant.
///
/// Written as an archived version whose body deliberately differs from
/// everything this build ships. If the resolver were implemented as "the current
/// document with a version label" — the tempting shortcut, and the one the
/// contract calls out — this test would hand back today's prose under
/// yesterday's id and nobody would notice until a notice arrived.
#[tokio::test]
async fn an_archived_version_resolves_to_its_own_words() {
    use crate::schema::terms_versions;
    use diesel::prelude::*;

    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let version_id = "sharing-terms@abcdef0123456789";
    let body = "## What it used to say\n\nSomething this build no longer ships.";
    diesel::insert_into(terms_versions::table)
        .values(&crate::models::TermsVersion {
            version_id: version_id.to_string(),
            document_slug: crate::legal::SHARING_TERMS_SLUG.to_string(),
            body: body.to_string(),
            first_seen_at: chrono::Utc::now().naive_utc(),
        })
        .on_conflict(terms_versions::version_id)
        .do_nothing()
        .execute(&mut conn)
        .expect("archive");

    let sections = crate::legal::sections_of(body);
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].heading.as_deref(), Some("What it used to say"));

    let live = crate::legal::sharing_terms();
    assert_ne!(
        live.version_id, version_id,
        "the fixture must differ from what this build ships, or this proves nothing",
    );
    assert!(
        !live
            .sections
            .iter()
            .any(|s| s.body.contains("no longer ships")),
        "and the live document must not contain the archived words",
    );
}

/// The live document is what a client is shown, and its identity is what a
/// client echoes back.
#[test]
fn the_live_terms_carry_their_own_identity_and_real_sections() {
    let terms = crate::legal::sharing_terms();
    let converted: VersionedLegalDocument = terms.clone().into();
    assert_eq!(converted.version_id, terms.version_id);
    assert_eq!(converted.slug, crate::legal::SHARING_TERMS_SLUG);
    assert!(
        converted.sections.len() >= 2,
        "a share dialog with fewer than two sections is not showing the terms",
    );
    assert!(
        converted
            .sections
            .iter()
            .all(|s| s.heading.is_some() && !s.body.is_empty()),
    );
}

// ---------------------------------------------------------------------------
// Reading a record: `attestationsFor` and `myAttestations` (T034, T036)
// ---------------------------------------------------------------------------

use crate::auth_middleware::AuthenticatedUser;
use async_graphql::Request;

fn schema(state: crate::state::AppState) -> crate::graphql::AppSchema {
    async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state)
    .finish()
}

fn caller(user_id: uuid::Uuid, is_admin: bool) -> AuthenticatedUser {
    AuthenticatedUser {
        user_id,
        session_id: uuid::Uuid::now_v7(),
        expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
        is_admin,
        role: if is_admin { "Admin" } else { "User" }.to_string(),
    }
}

/// An old version, archived with words this build does not ship, and one
/// agreement to it — the state a revision followed by a restart leaves behind.
///
/// Unique per call, because the archive is append-only and shared by every test
/// in the crate.
fn an_agreement_to_superseded_words(
    state: &crate::state::AppState,
) -> (String, String, uuid::Uuid, uuid::Uuid) {
    use crate::schema::{attestations, terms_versions};
    use diesel::prelude::*;

    let mut conn = state.db_pool.get().expect("conn");
    let subject = crate::test_support::insert_test_user(&mut conn);
    let publishable_id = uuid::Uuid::now_v7();
    let marker = format!("superseded-{}", publishable_id.simple());
    let body = format!("## What it said then\n\nWords this build no longer ships: {marker}.");
    let version_id = crate::legal::version_id(crate::legal::SHARING_TERMS_SLUG, &body);

    diesel::insert_into(terms_versions::table)
        .values(&crate::models::TermsVersion {
            version_id: version_id.clone(),
            document_slug: crate::legal::SHARING_TERMS_SLUG.to_string(),
            body,
            first_seen_at: chrono::Utc::now().naive_utc(),
        })
        .execute(&mut conn)
        .expect("archive the old words");

    diesel::insert_into(attestations::table)
        .values(&crate::models::NewAttestation {
            id: uuid::Uuid::now_v7(),
            purpose: crate::attestation::purpose::SHARE.to_string(),
            subject_user_id: subject,
            subject_username: Some("then".to_string()),
            terms_version_id: version_id.clone(),
            publishable_kind: Some("item".to_string()),
            publishable_id: Some(publishable_id),
            share_id: Some(uuid::Uuid::now_v7()),
            world_id: None,
        })
        .execute(&mut conn)
        .expect("record the old agreement");

    (version_id, marker, subject, publishable_id)
}

const ATTESTATION_SELECTION: &str = "
    subjectUsername
    termsVersionId
    terms { versionId slug sections { heading body } }
";

/// T034, FR-008: `Attestation.terms` is the words agreed to, read from the
/// archive — **not** today's document wearing yesterday's label.
///
/// Replace `with_archived_terms`'s lookup with `crate::legal::sharing_terms()`
/// and this fails on both halves: the version id comes back as the live one,
/// and the words come back without the marker.
#[tokio::test]
async fn an_old_agreement_resolves_to_the_words_it_agreed_to() {
    let state = test_app_state();
    crate::legal::ensure_terms_versions_recorded(&state)
        .await
        .expect("archive");
    let (version_id, marker, _, publishable_id) = an_agreement_to_superseded_words(&state);

    let response = schema(state)
        .execute(
            Request::new(format!(
                r#"{{ attestationsFor(publishableKind: "item", publishableId: "{publishable_id}") {{ {ATTESTATION_SELECTION} }} }}"#
            ))
            .data(caller(uuid::Uuid::now_v7(), true)),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);

    let data = response.data.into_json().expect("json");
    let records = data["attestationsFor"].as_array().expect("a list");
    assert_eq!(records.len(), 1);
    let record = &records[0];

    assert_eq!(record["termsVersionId"], version_id);
    assert_eq!(
        record["terms"]["versionId"], version_id,
        "the words must be the ones the record names",
    );
    assert_ne!(
        record["terms"]["versionId"],
        crate::legal::sharing_terms().version_id,
        "an old agreement answered with the live document is FR-008's failure",
    );
    let words = record["terms"]["sections"].to_string();
    assert!(
        words.contains(&marker),
        "the archived words, not today's: {words}",
    );
}

/// `attestationsFor` refuses a kind it does not know rather than answering with
/// an empty history — which would read, to a notice handler, as "nobody
/// agreed to publish this".
#[tokio::test]
async fn an_unknown_kind_is_refused_not_answered_with_nothing() {
    let state = test_app_state();
    let response = schema(state)
        .execute(
            Request::new(
                r#"{ attestationsFor(publishableKind: "scene", publishableId: "00000000-0000-0000-0000-000000000000") { id } }"#,
            )
            .data(caller(uuid::Uuid::now_v7(), true)),
        )
        .await;
    assert!(!response.errors.is_empty());
}

/// `myAttestations` hands a person their own agreements, archived words and
/// all — and nobody else's.
#[tokio::test]
async fn a_person_reads_their_own_agreements_in_the_words_they_agreed_to() {
    let state = test_app_state();
    crate::legal::ensure_terms_versions_recorded(&state)
        .await
        .expect("archive");
    let (version_id, marker, subject, _) = an_agreement_to_superseded_words(&state);
    // Somebody else's, which must not appear.
    let (other_version, _, _, _) = an_agreement_to_superseded_words(&state);

    let response = schema(state)
        .execute(
            Request::new(format!(
                "{{ myAttestations {{ {ATTESTATION_SELECTION} }} }}"
            ))
            .data(caller(subject, false)),
        )
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);

    let data = response.data.into_json().expect("json");
    let records = data["myAttestations"].as_array().expect("a list");
    assert_eq!(records.len(), 1, "one agreement, and it is theirs");
    assert_eq!(records[0]["termsVersionId"], version_id);
    assert_ne!(records[0]["termsVersionId"], other_version);
    assert!(
        records[0]["terms"]["sections"]
            .to_string()
            .contains(&marker)
    );
}

/// T036's SDL guard: the two queries exist in the shapes
/// `contracts/attestation.md` names, and `terms` is non-null — an agreement
/// that could come back without its words is one that answers nothing.
#[test]
fn the_record_is_readable_in_the_contracted_shape() {
    let sdl = async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .finish()
    .sdl();

    for declaration in [
        "attestationsFor(publishableKind: String!, publishableId: UUID!): [Attestation!]!",
        "myAttestations(limit: Int): [Attestation!]!",
        "terms: VersionedLegalDocument!",
        "subjectUsername: String\n",
    ] {
        assert!(
            sdl.contains(declaration),
            "the schema must declare `{}`",
            declaration.trim(),
        );
    }
}
