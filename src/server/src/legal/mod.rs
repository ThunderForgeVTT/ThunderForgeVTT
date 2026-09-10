//! The legal prose the server itself serves, and the identity of its words.
//!
//! Spec 039 US1/US4, ADR-076.
//!
//! # Why the server holds the text at all
//!
//! Because the server is what requires agreement to it. A client rendering its
//! own copy of the terms while quoting the server's version identity can attest
//! to words nobody was shown — and the whole value of an attestation is that it
//! records *which words*. So the authoritative copy is here, and the web app
//! reads it from here for the share step. (The policy *pages* keep their Vite
//! glob; nothing attests to those.)
//!
//! `include_str!` rather than a database row or a fetch, on the precedent
//! `admin.rs` set for the realm defaults: the text a person agreed to and the
//! binary that accepted the agreement ship together, so there is no
//! configuration under which the server enforces agreement to words it does not
//! have.
//!
//! # The identity is the hash of the words
//!
//! `<slug>@<first 16 hex of sha256(normalised body)>`, where "normalised" means
//! the HTML comments stripped and the result trimmed — exactly what
//! `apps/web/src/legal/legalDocuments.ts`'s `sectionsOf` already does before
//! rendering.
//!
//! The consequence is the point: **editing a sentence mints a version; editing
//! the explanatory comment does not.** Nobody has to remember to bump anything,
//! and nobody can change the meaning without changing the identity. A
//! hand-maintained version number is wrong the first time somebody fixes a typo
//! without thinking about it, and wrong silently.
//!
//! Sixteen hex characters is 64 bits. Not a collision-resistance problem — an
//! attacker who could forge a version id gains the ability to name a document
//! that already exists — but a legibility one: a full 64 characters in every log
//! line and every refusal buys nothing.

use sha2::{Digest, Sha256};

/// The sharing terms, shown at every publishing path (FR-001).
pub const SHARING_TERMS_SLUG: &str = "sharing-terms";
const SHARING_TERMS_SOURCE: &str = include_str!("../../../../legal/sharing-terms.md");

/// What an operator takes on, shown at first-run setup (US8, FR-042).
pub const OPERATOR_RESPONSIBILITIES_SLUG: &str = "operator-responsibilities";
const OPERATOR_RESPONSIBILITIES_SOURCE: &str =
    include_str!("../../../../legal/operator-responsibilities.md");

/// One block of prose under one heading.
///
/// `heading` is `None` for the text before the first `##`, matching the web
/// app's shape so one dialog can render either side's copy without a second
/// model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalSection {
    pub heading: Option<String>,
    pub body: String,
}

/// A document, its sections, and the identity to attest to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionedDocument {
    pub version_id: String,
    pub slug: String,
    pub sections: Vec<LegalSection>,
}

/// Every document this server serves and archives.
///
/// A list rather than a lookup by string, so `ensure_terms_versions_recorded`
/// cannot archive a subset of what the queries can serve — the failure that
/// would let an attestation name a version that was never written down.
pub fn documents() -> Vec<VersionedDocument> {
    vec![
        document(SHARING_TERMS_SLUG, SHARING_TERMS_SOURCE),
        document(
            OPERATOR_RESPONSIBILITIES_SLUG,
            OPERATOR_RESPONSIBILITIES_SOURCE,
        ),
    ]
}

/// The sharing terms in force right now.
pub fn sharing_terms() -> VersionedDocument {
    document(SHARING_TERMS_SLUG, SHARING_TERMS_SOURCE)
}

/// The operator statement in force right now.
pub fn operator_statement() -> VersionedDocument {
    document(
        OPERATOR_RESPONSIBILITIES_SLUG,
        OPERATOR_RESPONSIBILITIES_SOURCE,
    )
}

fn document(slug: &str, source: &str) -> VersionedDocument {
    let body = normalise(source);
    VersionedDocument {
        version_id: version_id(slug, &body),
        slug: slug.to_string(),
        sections: sections_of(&body),
    }
}

/// `<slug>@<16 hex>`. Opaque to every caller: compared for equality against the
/// archive and never parsed, ordered or decomposed.
pub fn version_id(slug: &str, normalised_body: &str) -> String {
    let digest = Sha256::digest(normalised_body.as_bytes());
    let hex: String = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{slug}@{hex}")
}

/// Comments stripped, then trimmed.
///
/// Character-by-character rather than a regex, because the crate has no regex
/// dependency and this is the whole of the grammar: `<!--` to the next `-->`,
/// and an unterminated comment swallows the rest of the file. That last case is
/// a malformed document, and the honest outcome is that its identity changes
/// when somebody fixes it.
pub fn normalise(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        rest = &rest[start + 4..];
        match rest.find("-->") {
            Some(end) => rest = &rest[end + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

/// The normalised body split on `##` headings, in document order.
///
/// Mirrors `sectionsOf`, including the two details that are easy to get subtly
/// different: a leading block with no heading is kept, and a section that is
/// nothing but a heading is kept as well — an empty body under a heading is a
/// document somebody should notice, not one this function should tidy away.
pub fn sections_of(normalised_body: &str) -> Vec<LegalSection> {
    let mut sections: Vec<LegalSection> = Vec::new();
    let mut heading: Option<String> = None;
    let mut body: Vec<&str> = Vec::new();

    let mut flush = |heading: &Option<String>, body: &mut Vec<&str>| {
        let text = body.join("\n").trim().to_string();
        if !text.is_empty() || heading.is_some() {
            sections.push(LegalSection {
                heading: heading.clone(),
                body: text,
            });
        }
        body.clear();
    };

    for line in normalised_body.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            flush(&heading, &mut body);
            heading = Some(rest.trim().to_string());
        } else {
            body.push(line);
        }
    }
    flush(&heading, &mut body);

    sections
}

/// Archive every document this build serves, if it is not already archived.
///
/// # Why at startup, and why this is the whole of FR-016
///
/// FR-016 says an attestation always resolves to the words agreed to. That is
/// not a property of the write path — it is a property of **ordering**: this
/// runs from `main.rs` before the server accepts a request, so a version an
/// attestation could name was archived before the server that would accept the
/// attestation was listening. There is no window in which a publish can name a
/// version that is not yet in the archive, which is why
/// `attestations.terms_version_id` can afford to be a foreign key.
///
/// Insert-if-absent, and **nothing is ever updated**. A row is a historical fact
/// about what a document said; an UPDATE here would rewrite what people agreed
/// to. `ON CONFLICT DO NOTHING` rather than an upsert says that in code, and it
/// also makes this safe to run on every boot and on every replica.
///
/// It walks [`documents()`] rather than a list of its own, because the failure
/// worth designing out is a document the queries can serve and the archive
/// never recorded — see `everything_servable_is_archivable`.
pub async fn ensure_terms_versions_recorded(state: &crate::state::AppState) -> Result<(), String> {
    use crate::schema::terms_versions;
    use diesel::prelude::*;

    let rows: Vec<crate::models::TermsVersion> = documents()
        .into_iter()
        .map(|document| crate::models::TermsVersion {
            version_id: document.version_id,
            document_slug: document.slug,
            // The normalised body, which is what the identity was computed
            // from and therefore what an old attestation must resolve to.
            body: document
                .sections
                .iter()
                .map(|section| match &section.heading {
                    Some(heading) => format!("## {heading}\n\n{}", section.body),
                    None => section.body.clone(),
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
            // Overwritten by the column default; present because the struct is
            // one shape for reading and writing.
            first_seen_at: chrono::Utc::now().naive_utc(),
        })
        .collect();

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(terms_versions::table)
            .values(&rows)
            .on_conflict(terms_versions::version_id)
            .do_nothing()
            .execute(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|e| format!("Failed to archive the legal document versions: {e}"))?;

    Ok(())
}

#[cfg(test)]
#[path = "legal_tests.rs"]
mod legal_tests;
