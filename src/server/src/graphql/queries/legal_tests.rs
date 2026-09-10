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
