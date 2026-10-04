//! What the person who becomes an operator is told, and the record that they
//! were.
//!
//! Spec 039 US8, `contracts/operator-acknowledgement.md`, FR-041 to FR-045.
//!
//! # It is an attestation
//!
//! Not a parallel mechanism. FR-043 says the operator record is made "on the
//! same terms as a sharing attestation", and the only way to keep that true as
//! either side changes is for it to be the same table, the same version
//! archive and the same code: `attestations` with `purpose = 'operator'`,
//! through [`crate::attestation::record_operator_sync`].
//!
//! # Required, not optional
//!
//! Both first-run paths — local credentials and a trusted OAuth provider —
//! carry an [`OperatorAcknowledgementRequest`] as a **required** field. A
//! request without one does not parse, so there is no path through setup that
//! produces an administrator and no acknowledgement (FR-041).
//!
//! # A new version asks again
//!
//! The statement is `include_str!`'d and hashed like the sharing terms, so an
//! upgrade that changes its words mints a new version (FR-044). The index on
//! `attestations` allows one operator acknowledgement **per version**; the
//! administrator is shown what changed and acknowledges again, rather than the
//! new words being applied to them silently.

use diesel::prelude::*;
use serde::Deserialize;

use crate::models::Attestation;
use crate::schema::{attestations, terms_versions, users};

/// The version of the operator statement the person was shown, echoed back.
/// The whole of what setup accepts about it; everything else is the server's
/// to write.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct OperatorAcknowledgementRequest {
    pub(crate) terms_version_id: String,
}

/// The refusal a missing or unknown version gets. Says what to do, and names
/// no valid version — the same rule as the sharing gate (FR-013).
pub(crate) const ACKNOWLEDGEMENT_REFUSED: &str = "Setup needs your acknowledgement of the operator \
     responsibilities shown on this page. Reload the page and try again.";

/// Is this a version of the operator statement this instance has archived?
///
/// Equality against the archive, scoped to the operator document: a sharing
/// terms version is a real version and still the wrong words.
pub(crate) fn is_operator_version_sync(
    conn: &mut PgConnection,
    version_id: &str,
) -> QueryResult<bool> {
    diesel::select(diesel::dsl::exists(
        terms_versions::table
            .filter(terms_versions::version_id.eq(version_id))
            .filter(terms_versions::document_slug.eq(crate::legal::OPERATOR_RESPONSIBILITIES_SLUG)),
    ))
    .get_result(conn)
}

/// Record the acknowledgement, on the caller's connection — so the
/// administrator and the record of what they took on commit together.
pub(crate) fn record_sync(
    conn: &mut PgConnection,
    administrator: uuid::Uuid,
    version_id: &str,
) -> QueryResult<uuid::Uuid> {
    let username = users::table
        .filter(users::id.eq(administrator))
        .select(users::username)
        .first::<String>(conn)
        .optional()?;
    crate::attestation::record_operator_sync(conn, administrator, username, version_id)
}

/// The most recent acknowledgement this instance holds, of any version.
pub(crate) fn latest_sync(conn: &mut PgConnection) -> QueryResult<Option<Attestation>> {
    attestations::table
        .filter(attestations::purpose.eq(crate::attestation::purpose::OPERATOR))
        .order(attestations::attested_at.desc())
        .select(Attestation::as_select())
        .first(conn)
        .optional()
}

#[cfg(test)]
#[path = "operator_acknowledgement_tests.rs"]
mod operator_acknowledgement_tests;
