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

use std::collections::HashMap;

use async_graphql::{Context, Object, SimpleObject};

use crate::attestation::CoveredKind;
use crate::graphql::{Error, GraphQLResult, admin_user, app_state, authenticated_user};
use crate::state::AppState;

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

/// What somebody agreed to, when, and in order to publish what.
///
/// `terms` is filled from `terms_versions` by [`with_archived_terms`] and from
/// nowhere else. There is deliberately no constructor that takes a
/// [`crate::legal::VersionedDocument`]: the compiled-in document is the words
/// as they are *now*, and an old agreement answered with those is the failure
/// FR-008 exists to prevent.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "Attestation")]
pub struct GraphQLAttestation {
    pub id: uuid::Uuid,
    /// `"share"` or `"operator"`.
    pub purpose: String,
    /// Null once the account has been deleted (FR-010, FR-037).
    pub subject_username: Option<String>,
    pub subject_user_id: uuid::Uuid,
    pub attested_at: String,
    pub terms_version_id: String,
    /// The words as they were. Resolved from the archive, never from the
    /// constant.
    pub terms: VersionedLegalDocument,
    pub publishable_kind: Option<String>,
    pub publishable_id: Option<uuid::Uuid>,
    pub world_id: Option<uuid::Uuid>,
}

/// Every archived version among `version_ids`, by id, in one read.
///
/// The one place an old document is read back. `legalDocumentVersion` and
/// `Attestation.terms` both come through here, so there is one definition of
/// "the words as they were" and it is a row, not the binary.
async fn archived_versions(
    state: &AppState,
    version_ids: Vec<String>,
) -> GraphQLResult<HashMap<String, VersionedLegalDocument>> {
    use crate::schema::terms_versions;
    use diesel::prelude::*;

    if version_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let rows: Vec<(String, String, String)> = tokio::task::spawn_blocking(move || {
        terms_versions::table
            .filter(terms_versions::version_id.eq_any(&version_ids))
            .select((
                terms_versions::version_id,
                terms_versions::document_slug,
                terms_versions::body,
            ))
            .load(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to read the terms archive"))?;

    Ok(rows
        .into_iter()
        .map(|(version_id, slug, body)| {
            let document = VersionedLegalDocument {
                version_id: version_id.clone(),
                slug,
                // The archived body, split the same way the live one is. This
                // is the whole point of the archive: the words as they were,
                // not the words as they are.
                sections: crate::legal::sections_of(&body)
                    .into_iter()
                    .map(|section| LegalSection {
                        heading: section.heading,
                        body: section.body,
                    })
                    .collect(),
            };
            (version_id, document)
        })
        .collect())
}

/// Attach to each record the archived words it names.
///
/// A record whose version is missing from the archive is an error rather than
/// something skipped: `attestations.terms_version_id` is a foreign key into an
/// append-only table, so this cannot happen without somebody editing the
/// database, and silently dropping the record would hide exactly that.
pub(crate) async fn with_archived_terms(
    state: &AppState,
    records: Vec<crate::models::Attestation>,
) -> GraphQLResult<Vec<GraphQLAttestation>> {
    let mut wanted: Vec<String> = records.iter().map(|r| r.terms_version_id.clone()).collect();
    wanted.sort();
    wanted.dedup();
    let archive = archived_versions(state, wanted).await?;

    records
        .into_iter()
        .map(|record| {
            let terms = archive
                .get(&record.terms_version_id)
                .cloned()
                .ok_or_else(|| Error::new("An agreement names words the archive does not hold"))?;
            Ok(GraphQLAttestation {
                id: record.id,
                purpose: record.purpose,
                subject_username: record.subject_username,
                subject_user_id: record.subject_user_id,
                attested_at: record.attested_at.and_utc().to_rfc3339(),
                terms_version_id: record.terms_version_id,
                terms,
                publishable_kind: record.publishable_kind,
                publishable_id: record.publishable_id,
                world_id: record.world_id,
            })
        })
        .collect()
}

/// How many of their own agreements a person gets when they do not say.
const MY_ATTESTATIONS_DEFAULT: i32 = 20;
/// And the most they get when they do. Each carries a full copy of the words.
const MY_ATTESTATIONS_MAX: i32 = 100;

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
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let mut found = archived_versions(state, vec![version_id.clone()]).await?;
        Ok(found.remove(&version_id))
    }

    /// Every agreement under which one thing has been published, newest first
    /// — its own, and those of every collection it is currently in.
    ///
    /// The SC-003 surface: a person handling a notice reaches who agreed, when
    /// and the exact words in one query, without a developer. Admin-only for
    /// the reason `moderationCase` is. `publishableKind` also accepts `lore`,
    /// which is never published alone and so is answered entirely through its
    /// collections.
    ///
    /// Returned whether or not the share each one authorised still exists
    /// (FR-007) — there is no join to a share row and no filter on one.
    async fn attestations_for(
        &self,
        ctx: &Context<'_>,
        publishable_kind: String,
        publishable_id: uuid::Uuid,
    ) -> GraphQLResult<Vec<GraphQLAttestation>> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let kind = CoveredKind::parse(&publishable_kind).ok_or_else(|| {
            Error::new("publishableKind must be one of collection, actor, item, ability or lore")
        })?;

        let records = crate::attestation::for_content(state, kind, publishable_id)
            .await
            .map_err(Error::new)?;
        with_archived_terms(state, records).await
    }

    /// The caller's own attestations, newest first. Reachable by a
    /// **disabled** account (`standing-and-termination.md`).
    async fn my_attestations(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
    ) -> GraphQLResult<Vec<GraphQLAttestation>> {
        let state = app_state(ctx)?;
        // On the disabled-account allowlist: somebody shut out should still
        // be able to see what they agreed to.
        let user = crate::graphql::helpers::authenticated_user_even_if_disabled(ctx)?;
        let limit = limit
            .unwrap_or(MY_ATTESTATIONS_DEFAULT)
            .clamp(1, MY_ATTESTATIONS_MAX);

        let records = crate::attestation::for_subject(state, user.user_id, i64::from(limit))
            .await
            .map_err(Error::new)?;
        with_archived_terms(state, records).await
    }
}

#[cfg(test)]
#[path = "legal_tests.rs"]
mod legal_tests;
