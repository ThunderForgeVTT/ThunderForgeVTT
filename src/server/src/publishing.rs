//! The publishing gate: what every operation that publishes beyond its world
//! must do before it mints a share code.
//!
//! Spec 039 US1/US3, `contracts/publishing-gate.md`.
//!
//! # Why the gate is here and not in a page
//!
//! Because a policy enforced only in a UI is a policy with a hole in it. The
//! terms were displayed at the collection share step and the moment passed;
//! `create_collection_share_link_impl` never asked whether the person had
//! agreed, so a direct call to the API produced a share link with no agreement
//! at all. This project already holds the line that authorisation belongs at the
//! data boundary (Principle III), and an agreement is authorisation.
//!
//! The gate lives in the **impl**, not the resolver: the impls are what the
//! existing unit tests exercise, and a caller that bypasses GraphQL is refused
//! too.
//!
//! # The order of the checks, and why it is the order
//!
//! 1. **The instance can be notified** — `readiness::may_publish_beyond_world`,
//!    which spec 040 built as its FR-026 and which the four impls already call
//!    first. Asked before ownership so a misconfigured instance answers the same
//!    sentence to every caller instead of leaking which of them owns what.
//! 2. **The version is one this instance knows** — this module.
//! 3. **Ownership**, which each impl checks its own way and keeps.
//!
//! Standing (FR-018 — an account at the suspension rung may not publish) is
//! US5's and is not here yet. When it lands it is one call between 1 and 2, and
//! nothing else in this module moves.
//!
//! # What the client may supply, and what it may not
//!
//! One field: the version identity it was shown. No text, no timestamp, no
//! identity, no record — every one of those is the server's to write (FR-014).
//! A client that could supply them is a client that could write its own
//! evidence.

use async_graphql::{Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;

use crate::attestation::{PendingAttestation, PublishableKind};
use crate::state::AppState;

/// The whole of what a publishing mutation accepts, and its smallness is the
/// point.
#[derive(InputObject, Debug, Clone)]
pub struct AttestationInput {
    /// The `versionId` from `sharingTerms`, echoed back unmodified.
    pub terms_version_id: String,
}

/// The one refusal a wrong or missing version gets.
///
/// FR-013: **a refusal never names a valid version identity**, including the
/// expected one. The identity is obtainable a legitimate request away through
/// `sharingTerms` — that is not the secret. What must not leak is a value
/// somebody could paste in to skip the dialog.
///
/// It says what to do rather than what went wrong, because what went wrong is
/// almost always "the page has been open since before the terms changed" and the
/// answer to that is a reload.
const TERMS_REFUSED: &str =
    "This share needs the current sharing agreement. Reload the page and try again.";

/// May this publish proceed, and what will be recorded if it does?
///
/// On success the caller writes the returned [`PendingAttestation`] **inside the
/// transaction that mints the share row** (`attestation::record_sync`). There is
/// no ordering in which one exists without the other.
///
/// Note what this does not do: it does not write anything. A gate that recorded
/// on approval would leave an attestation behind for a publish that then failed
/// its ownership check.
pub async fn require_attestation(
    state: &AppState,
    subject: uuid::Uuid,
    kind: PublishableKind,
    target: uuid::Uuid,
    world_id: Option<uuid::Uuid>,
    offered: &AttestationInput,
) -> GraphQLResult<PendingAttestation> {
    let offered_version = offered.terms_version_id.trim().to_string();
    if offered_version.is_empty() {
        return Err(Error::new(TERMS_REFUSED));
    }

    // The current version, or a superseded one still in the archive.
    //
    // Accepting a recently-archived version is deliberate (FR-005): somebody who
    // opened the dialog thirty seconds before an operator revised the terms
    // should not lose their work to a race, and what they agreed to is recorded
    // exactly, which is the property that matters. An instance wanting strict
    // currency has the comparison one line away; this is not offered as a
    // setting because two behaviours here is one more than anybody can reason
    // about.
    if !version_is_known(state, &offered_version).await? {
        return Err(Error::new(TERMS_REFUSED));
    }

    // The name as it is right now. A snapshot, not a join: the record has to
    // answer "who agreed" after a rename and after a deletion, and only a
    // snapshot does both.
    let subject_username = username_of(state, subject).await?;

    Ok(PendingAttestation {
        subject_user_id: subject,
        subject_username,
        terms_version_id: offered_version,
        kind,
        publishable_id: target,
        world_id,
    })
}

/// Is this identity in the archive?
///
/// Equality against `terms_versions`, and nothing else. The identity is opaque:
/// nothing here parses it, orders it, or derives a document from it.
async fn version_is_known(state: &AppState, version_id: &str) -> GraphQLResult<bool> {
    use crate::schema::terms_versions;

    let version_id = version_id.to_string();
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        terms_versions::table
            .filter(terms_versions::version_id.eq(&version_id))
            .select(terms_versions::version_id)
            .first::<String>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map(|found| found.is_some())
    .map_err(|_| Error::new("Failed to read the terms archive"))
}

async fn username_of(state: &AppState, user_id: uuid::Uuid) -> GraphQLResult<Option<String>> {
    use crate::schema::users;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::id.eq(user_id))
            .select(users::username)
            .first::<String>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to read the account"))
}

#[cfg(test)]
#[path = "publishing_tests.rs"]
mod publishing_tests;
