//! ADR-076's central claim, as tests: **the identity is the words.**

use super::*;

/// The property the whole design rests on, in both directions.
///
/// A reviewer needs to know that fixing a typo in the explanatory comment is
/// free and that changing a sentence is a version transition. If either half of
/// this were wrong the mistake would be silent — attestations pointing at words
/// that had changed underneath them, or a new version minted every time
/// somebody clarified a comment.
#[test]
fn a_prose_edit_changes_the_identity_and_a_comment_edit_does_not() {
    let prose = "## A heading\n\nSome words.\n";
    let with_comment = format!("<!-- explanatory -->\n{prose}");
    let with_a_different_comment = format!("<!-- rewritten, at length -->\n{prose}");
    let with_edited_prose = "## A heading\n\nSome different words.\n";

    let identity = |source: &str| version_id("doc", &normalise(source));

    assert_eq!(
        identity(&with_comment),
        identity(&with_a_different_comment),
        "editing the leading comment must not mint a version",
    );
    assert_eq!(
        identity(&with_comment),
        identity(prose),
        "and a document with no comment at all is the same document",
    );
    assert_ne!(
        identity(&with_comment),
        identity(with_edited_prose),
        "changing a sentence must mint a version — this is the whole point",
    );
}

/// Trailing and leading whitespace is not meaning.
#[test]
fn whitespace_around_the_body_is_not_part_of_the_identity() {
    let identity = |source: &str| version_id("doc", &normalise(source));
    assert_eq!(
        identity("## H\n\nBody.\n"),
        identity("\n\n## H\n\nBody.\n\n\n")
    );
}

/// A version id is `<slug>@<16 hex>` and nothing in the product parses it.
///
/// Asserted anyway, because it is what a person reads in a log line and in a
/// refusal, and 64 hex characters there would be a legibility regression
/// nobody would notice from a passing test.
#[test]
fn a_version_id_names_its_document_and_is_short_enough_to_read() {
    let id = sharing_terms().version_id;
    let (slug, hex) = id.split_once('@').expect("slug@hex");
    assert_eq!(slug, SHARING_TERMS_SLUG);
    assert_eq!(hex.len(), 16, "16 hex characters: {id}");
    assert!(
        hex.chars().all(|c| c.is_ascii_hexdigit()),
        "the identity must be hex: {id}"
    );
}

/// The two documents are different documents, which sounds obvious and is the
/// thing a copy-paste in `documents()` would break.
#[test]
fn each_document_has_its_own_identity() {
    let sharing = sharing_terms();
    let operator = operator_statement();
    assert_ne!(sharing.version_id, operator.version_id);
    assert_ne!(sharing.slug, operator.slug);
}

/// Every document the queries can serve is a document the archive will record.
///
/// `documents()` is what `ensure_terms_versions_recorded` walks, and
/// `sharing_terms()`/`operator_statement()` are what the queries answer with.
/// If those disagree, an attestation can name a version that was never
/// archived — which is exactly the failure FR-016 forbids, arriving through a
/// missed line in a list rather than through a race.
#[test]
fn everything_servable_is_archivable() {
    let archived: Vec<String> = documents().into_iter().map(|d| d.version_id).collect();
    for servable in [sharing_terms(), operator_statement()] {
        assert!(
            archived.contains(&servable.version_id),
            "`{}` is served but not in `documents()`, so it would never be archived",
            servable.slug,
        );
    }
}

/// The shipped documents are the ones this feature is about, and they say the
/// things the feature claims they say.
///
/// Not a prose review — a check that the section split works on the real files
/// and that neither document arrived empty, which is what a wrong
/// `include_str!` path or a stray comment wrapper would produce.
#[test]
fn the_shipped_documents_parse_into_sections_with_real_headings() {
    for document in documents() {
        assert!(
            document.sections.len() >= 2,
            "`{}` split into {} section(s); the share dialog would render almost nothing",
            document.slug,
            document.sections.len(),
        );
        assert!(
            document
                .sections
                .iter()
                .all(|section| section.heading.is_some() && !section.body.is_empty()),
            "`{}` has a section with no heading or no body: {:#?}",
            document.slug,
            document.sections,
        );
        assert!(
            !document.sections.iter().any(|s| s.body.contains("<!--")),
            "`{}` still carries a comment after normalisation",
            document.slug,
        );
    }
}

/// A section that is a heading with nothing under it is kept, not tidied away.
///
/// Mirrors `sectionsOf`, and it matters: a document that has lost its body
/// under a heading is a document somebody should see is wrong, and silently
/// dropping the heading hides it.
#[test]
fn a_heading_with_no_body_survives_the_split() {
    let sections = sections_of("## Empty\n\n## Full\n\nWords.");
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].heading.as_deref(), Some("Empty"));
    assert_eq!(sections[0].body, "");
}

/// The text before the first heading is a section with no heading, exactly as
/// the web app treats it.
#[test]
fn a_preamble_before_the_first_heading_is_its_own_section() {
    let sections = sections_of("Opening words.\n\n## A heading\n\nMore.");
    assert_eq!(sections[0].heading, None);
    assert_eq!(sections[0].body, "Opening words.");
    assert_eq!(sections[1].heading.as_deref(), Some("A heading"));
}

/// FR-016 as an ordering claim, driven against the database.
///
/// The property is not "the write path checks the archive" — it is that the
/// archive is written **before anything can name a version**. So: run the
/// archive, then assert every version this build serves is already there.
///
/// If somebody later moves this call after the server starts listening, this
/// test still passes and the guarantee is gone; what it catches is the more
/// likely mistake — a document served but not archived, or an archive write
/// that silently did nothing.
#[tokio::test]
async fn every_servable_version_is_in_the_archive_after_the_startup_write() {
    use crate::schema::terms_versions;
    use diesel::prelude::*;

    let state = crate::test_support::test_app_state();
    ensure_terms_versions_recorded(&state)
        .await
        .expect("the archive write must succeed");

    let mut conn = state.db_pool.get().expect("conn");
    for document in documents() {
        let archived: Option<String> = terms_versions::table
            .filter(terms_versions::version_id.eq(&document.version_id))
            .select(terms_versions::version_id)
            .first(&mut conn)
            .optional()
            .expect("read");
        assert_eq!(
            archived.as_deref(),
            Some(document.version_id.as_str()),
            "`{}` is servable but not archived — an attestation naming it would \
             point at words this instance has no record of",
            document.slug,
        );
    }
}

/// Running it twice writes one row, and does not touch the one already there.
///
/// `ON CONFLICT DO NOTHING` rather than an upsert, because an upsert would
/// rewrite `first_seen_at` — and "when did this version first appear" is the
/// only history this table carries.
#[tokio::test]
async fn archiving_twice_leaves_the_first_row_exactly_as_it_was() {
    use crate::schema::terms_versions;
    use diesel::prelude::*;

    let state = crate::test_support::test_app_state();
    ensure_terms_versions_recorded(&state).await.expect("first");

    let mut conn = state.db_pool.get().expect("conn");
    let slug = SHARING_TERMS_SLUG;
    let before: (String, chrono::NaiveDateTime) = terms_versions::table
        .filter(terms_versions::document_slug.eq(slug))
        .select((terms_versions::body, terms_versions::first_seen_at))
        .order(terms_versions::first_seen_at.asc())
        .first(&mut conn)
        .expect("archived once");
    drop(conn);

    ensure_terms_versions_recorded(&state)
        .await
        .expect("second");

    let mut conn = state.db_pool.get().expect("conn");
    let after: (String, chrono::NaiveDateTime) = terms_versions::table
        .filter(terms_versions::document_slug.eq(slug))
        .select((terms_versions::body, terms_versions::first_seen_at))
        .order(terms_versions::first_seen_at.asc())
        .first(&mut conn)
        .expect("still archived");

    assert_eq!(
        before, after,
        "a second startup must not rewrite an archived version — `first_seen_at` \
         is the only history this table carries",
    );
}

/// The archived body is the words, not the file.
///
/// A row carrying the raw file — comments and all — would mean an old
/// attestation resolves to text whose identity does not match its own version
/// id, which is the failure in the shape hardest to notice.
#[tokio::test]
async fn the_archived_body_hashes_back_to_its_own_version_id() {
    use crate::schema::terms_versions;
    use diesel::prelude::*;

    let state = crate::test_support::test_app_state();
    ensure_terms_versions_recorded(&state)
        .await
        .expect("archive");

    let mut conn = state.db_pool.get().expect("conn");
    for document in documents() {
        let body: String = terms_versions::table
            .filter(terms_versions::version_id.eq(&document.version_id))
            .select(terms_versions::body)
            .first(&mut conn)
            .expect("archived");
        assert!(
            !body.contains("<!--"),
            "`{}` was archived with its comments, so its stored body is not what \
             its identity was computed from",
            document.slug,
        );
        assert_eq!(
            version_id(&document.slug, &normalise(&body)),
            document.version_id,
            "the archived body of `{}` does not hash back to its own version id",
            document.slug,
        );
    }
}
