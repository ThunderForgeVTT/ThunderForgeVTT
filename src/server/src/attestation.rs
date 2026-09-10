//! What somebody agreed to, when, and in order to publish what.
//!
//! Spec 039 US1–US4, ADR-076.
//!
//! # Why this is a module and not three lines in each share path
//!
//! Four publishing paths write one kind of record. Four copies of the write
//! would be four places for the shape to drift, and the shape is the evidence —
//! a row missing its version id or its subject is a row that answers nothing.
//!
//! # Written inside the caller's transaction
//!
//! [`record_sync`] takes a connection the caller already holds, so the
//! attestation and the share row it authorised commit together or not at all.
//! There is no interleaving in which a published thing has no agreement behind
//! it, and none in which an agreement exists for a publish that failed.
//!
//! That is the same discipline `two_factor::events` follows, for the same
//! reason: a record written after the fact is a record that can be missing.
//!
//! # What can never happen to a row here
//!
//! No update, no delete, no admin edit. A record somebody can revise is not a
//! record. The single permitted mutation is [`redact_for_deleted_account`],
//! which nulls the one field that names a human and changes nothing else.

use diesel::prelude::*;

use crate::models::{Attestation, NewAttestation};
use crate::schema::attestations;
use crate::state::AppState;

/// What is being published. `str` values are the ones the CHECK constraint and
/// `contracts/attestation.md` both name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishableKind {
    Collection,
    Actor,
    Item,
    Ability,
}

impl PublishableKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Collection => "collection",
            Self::Actor => "actor",
            Self::Item => "item",
            Self::Ability => "ability",
        }
    }
}

/// Anything a notice can name, and so anything an agreement can be asked about.
///
/// Wider than [`PublishableKind`] by one: a lore entry is never published on
/// its own, but it is published inside a collection — and a notice about it has
/// to reach the agreement behind that collection, or SC-003 fails for the only
/// way lore is ever shared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoveredKind {
    Collection,
    Actor,
    Item,
    Ability,
    Lore,
}

impl CoveredKind {
    /// For a kind named by a caller. `None` for anything else, so a typo is
    /// refused rather than answered with an empty history that reads as "nobody
    /// agreed to anything".
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "collection" => Some(Self::Collection),
            "actor" => Some(Self::Actor),
            "item" => Some(Self::Item),
            "ability" => Some(Self::Ability),
            "lore" => Some(Self::Lore),
            _ => None,
        }
    }

    /// What it is attested as when published on its own, if it can be.
    fn published_alone_as(self) -> Option<PublishableKind> {
        match self {
            Self::Collection => Some(PublishableKind::Collection),
            Self::Actor => Some(PublishableKind::Actor),
            Self::Item => Some(PublishableKind::Item),
            Self::Ability => Some(PublishableKind::Ability),
            Self::Lore => None,
        }
    }

    /// Its `world_collection_members.member_type`, if it can sit in a
    /// collection. Collections do not nest.
    fn member_type(self) -> Option<&'static str> {
        match self {
            Self::Collection => None,
            Self::Actor => Some("actor"),
            Self::Item => Some("item"),
            Self::Ability => Some("ability"),
            Self::Lore => Some("lore"),
        }
    }
}

impl From<PublishableKind> for CoveredKind {
    fn from(value: PublishableKind) -> Self {
        match value {
            PublishableKind::Collection => Self::Collection,
            PublishableKind::Actor => Self::Actor,
            PublishableKind::Item => Self::Item,
            PublishableKind::Ability => Self::Ability,
        }
    }
}

/// `purpose`, mirrored by the migration's CHECK constraint.
pub mod purpose {
    /// Somebody publishing something (US1–US4).
    pub const SHARE: &str = "share";
    /// An instance's operator, acknowledging what they have taken on (US8).
    pub const OPERATOR: &str = "operator";
}

/// An attestation the gate has approved and the caller must still write.
///
/// Deliberately not a row and deliberately not writable by the client: every
/// field here was decided by the server. The type exists so the value cannot be
/// constructed anywhere but [`super::publishing::require_attestation`], which
/// is what stops a resolver assembling its own approval.
#[derive(Debug, Clone)]
pub struct PendingAttestation {
    pub subject_user_id: uuid::Uuid,
    pub subject_username: Option<String>,
    pub terms_version_id: String,
    pub kind: PublishableKind,
    pub publishable_id: uuid::Uuid,
    pub world_id: Option<uuid::Uuid>,
}

/// Write the record, on a connection the caller already holds.
///
/// `share_id` is the row this publish just minted. It carries no foreign key
/// (see the migration): revoking or deleting the share leaves the record
/// standing, which is FR-007.
pub fn record_sync(
    conn: &mut PgConnection,
    pending: &PendingAttestation,
    share_id: uuid::Uuid,
) -> Result<uuid::Uuid, diesel::result::Error> {
    let id = uuid::Uuid::now_v7();
    diesel::insert_into(attestations::table)
        .values(&NewAttestation {
            id,
            purpose: purpose::SHARE.to_string(),
            subject_user_id: pending.subject_user_id,
            subject_username: pending.subject_username.clone(),
            terms_version_id: pending.terms_version_id.clone(),
            publishable_kind: Some(pending.kind.as_str().to_string()),
            publishable_id: Some(pending.publishable_id),
            share_id: Some(share_id),
            world_id: pending.world_id,
        })
        .execute(conn)?;
    Ok(id)
}

/// The operator acknowledgement (US8, FR-043).
///
/// The same table and the same rules, which is the only way FR-043's "on the
/// same terms as a sharing attestation" stays true as either side changes. The
/// migration's partial unique index allows one, so a second call is a
/// `unique_violation` rather than a second row — which is the right answer to
/// "setup ran twice".
pub fn record_operator_sync(
    conn: &mut PgConnection,
    subject_user_id: uuid::Uuid,
    subject_username: Option<String>,
    terms_version_id: &str,
) -> Result<uuid::Uuid, diesel::result::Error> {
    let id = uuid::Uuid::now_v7();
    diesel::insert_into(attestations::table)
        .values(&NewAttestation {
            id,
            purpose: purpose::OPERATOR.to_string(),
            subject_user_id,
            subject_username,
            terms_version_id: terms_version_id.to_string(),
            publishable_kind: None,
            publishable_id: None,
            share_id: None,
            world_id: None,
        })
        .execute(conn)?;
    Ok(id)
}

/// Every agreement under which one thing has been published, newest first:
/// its own, and those of every collection it is in.
///
/// The lookup a person handling a notice makes (FR-009, SC-003). There is **no
/// join to a share row and no filter on one**: an attestation is returned
/// whether or not the share it authorised still exists, because a revoked share
/// does not un-agree anything (FR-007).
///
/// # Why a collection's agreements count, and why by current membership
///
/// A shared collection serves its members live (`shared_collection_impl` reads
/// them on every request), so whatever is in a collection now is being
/// published under that collection's agreement — including something added
/// after the agreement was made, which is exactly what a notice about it needs
/// to see. What current membership cannot reach is a thing taken *out* of a
/// collection after somebody adopted a copy; following copies is ADR-079's
/// question, not this one's.
pub async fn for_content(
    state: &AppState,
    kind: CoveredKind,
    id: uuid::Uuid,
) -> Result<Vec<Attestation>, String> {
    use crate::schema::world_collection_members as members;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        let mut query = attestations::table.into_boxed();
        if let Some(alone) = kind.published_alone_as() {
            query = query.or_filter(
                attestations::publishable_kind
                    .eq(alone.as_str())
                    .and(attestations::publishable_id.eq(id)),
            );
        }
        if let Some(member_type) = kind.member_type() {
            let containing = members::table
                .filter(members::member_type.eq(member_type))
                .filter(members::member_id.eq(id))
                .select(members::collection_id.nullable());
            query = query.or_filter(
                attestations::publishable_kind
                    .eq(PublishableKind::Collection.as_str())
                    .and(attestations::publishable_id.eq_any(containing)),
            );
        }
        query
            .order(attestations::attested_at.desc())
            .select(Attestation::as_select())
            .load(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to read the attestations".to_string())
}

/// One account's own history, newest first.
///
/// Available to a **disabled** account: somebody shut out should still be able
/// to see what they agreed to.
pub async fn for_subject(
    state: &AppState,
    subject_user_id: uuid::Uuid,
    limit: i64,
) -> Result<Vec<Attestation>, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        attestations::table
            .filter(attestations::subject_user_id.eq(subject_user_id))
            .order(attestations::attested_at.desc())
            .limit(limit)
            .select(Attestation::as_select())
            .load(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to read the attestations".to_string())
}

/// The minimal lawful form, on account deletion (FR-010, FR-037).
///
/// Nulls `subject_username` and changes nothing else. What remains is:
/// somebody, identified only by an id that no longer resolves to a person,
/// agreed to this exact text at this exact time in order to publish this exact
/// thing.
///
/// **Not a delete.** An account deletion must not be able to destroy the
/// evidence for a claim already filed against it — which is also why the table
/// has no foreign key to `users` to force the question.
///
/// Sync, and called from `delete_user_data_sync`'s transaction: the redaction
/// and the deletion are one act.
pub fn redact_for_deleted_account(
    conn: &mut PgConnection,
    subject_user_id: uuid::Uuid,
) -> Result<usize, diesel::result::Error> {
    diesel::update(attestations::table.filter(attestations::subject_user_id.eq(subject_user_id)))
        .set(attestations::subject_username.eq::<Option<String>>(None))
        .execute(conn)
}

#[cfg(test)]
#[path = "attestation_tests.rs"]
mod attestation_tests;
