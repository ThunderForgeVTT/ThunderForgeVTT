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
