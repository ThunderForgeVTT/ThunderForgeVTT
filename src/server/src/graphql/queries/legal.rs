//! The legal prose a client must show before it can publish, and the archive an
//! old agreement resolves through.
//!
//! Spec 039 US1/US2/US4, `contracts/attestation.md`.
//!
//! # Why the client reads the words from here
//!
//! Because a client rendering its own copy of the text while quoting the
//! server's version identity can attest to words nobody was shown. The web app
//! keeps its Vite glob for the policy *pages* — nothing attests to those — but
//! the share dialog reads this, so the words shown and the identity recorded
//! come from the same place.

use async_graphql::{Context, Object, SimpleObject};

use crate::graphql::{Error, GraphQLResult, admin_user, app_state, authenticated_user};

/// One block of prose under one heading. `heading` is null for the text before
/// the first one.
#[derive(SimpleObject, Debug, Clone)]
pub struct LegalSection {
    pub heading: Option<String>,
    pub body: String,
}

/// A document, its sections, and the identity to attest to.
#[derive(SimpleObject, Debug, Clone)]
pub struct VersionedLegalDocument {
    /// Opaque, e.g. `"sharing-terms@4f2a9c1e77b03d58"`. **Never parse it.**
    pub version_id: String,
    pub slug: String,
    pub sections: Vec<LegalSection>,
}

impl From<crate::legal::VersionedDocument> for VersionedLegalDocument {
    fn from(value: crate::legal::VersionedDocument) -> Self {
        Self {
            version_id: value.version_id,
            slug: value.slug,
            sections: value
                .sections
                .into_iter()
                .map(|section| LegalSection {
                    heading: section.heading,
                    body: section.body,
                })
                .collect(),
        }
    }
}

#[derive(Default)]
pub struct LegalDocumentQuery;

#[Object]
impl LegalDocumentQuery {
    /// The sharing terms in force right now, and the identity to attest to.
    ///
    /// Requires a session — you cannot publish without one — and is cheap: the
    /// body is a compiled-in constant and the hash is computed from it.
    async fn sharing_terms(&self, ctx: &Context<'_>) -> GraphQLResult<VersionedLegalDocument> {
        let _ = authenticated_user(ctx)?;
        Ok(crate::legal::sharing_terms().into())
    }

    /// What an operator takes on (US8).
    ///
    /// **Readable without a session** (FR-045), unlike the sharing terms: the
    /// person it is shown to is setting an instance up and does not have an
    /// account yet, and it is a statement about responsibility rather than
    /// anything about this instance's contents.
    async fn operator_statement(
        &self,
        _ctx: &Context<'_>,
    ) -> GraphQLResult<VersionedLegalDocument> {
        Ok(crate::legal::operator_statement().into())
    }

    /// One archived version, by id — what an old attestation actually said.
    ///
    /// Admin-only, for the reason `moderationCase` is: this is the
    /// notice-handling surface, not a public archive. Resolved from
    /// `terms_versions` and **never** from the compiled-in constant, which is
    /// the failure FR-008 exists to prevent and the easiest one to write by
    /// accident.
    ///
    /// `None` for an identity that was never archived, rather than an error: an
    /// operator pasting an id from an email should be told there is no such
    /// version, not handed a stack trace.
    async fn legal_document_version(
        &self,
        ctx: &Context<'_>,
        version_id: String,
    ) -> GraphQLResult<Option<VersionedLegalDocument>> {
        use crate::schema::terms_versions;
        use diesel::prelude::*;

        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let row: Option<(String, String, String)> = tokio::task::spawn_blocking(move || {
            terms_versions::table
                .filter(terms_versions::version_id.eq(&version_id))
                .select((
                    terms_versions::version_id,
                    terms_versions::document_slug,
                    terms_versions::body,
                ))
                .first(&mut conn)
                .optional()
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to read the terms archive"))?;

        Ok(row.map(|(version_id, slug, body)| VersionedLegalDocument {
            version_id,
            slug,
            // The archived body, split the same way the live one is. This is
            // the whole point of the archive: the words as they were, not the
            // words as they are.
            sections: crate::legal::sections_of(&body)
                .into_iter()
                .map(|section| LegalSection {
                    heading: section.heading,
                    body: section.body,
                })
                .collect(),
        }))
    }
}

#[cfg(test)]
#[path = "legal_tests.rs"]
mod legal_tests;
