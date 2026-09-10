//! What a person has been told about their own account.
//!
//! Spec 039 FR-028, data-model.md § 6.
//!
//! # The durable half, not the delivery
//!
//! There is no mailer behind this. Spec 040 owns delivery; what this module
//! owns is the fact that the telling happened — a row written in the same
//! breath as the thing it is about, readable by the person it is addressed to
//! whenever they look. When mail arrives, it delivers these rows.
//!
//! # Why the words are not stored
//!
//! A notice is `kind` plus `payload`, rendered by the client. The opposite of
//! `attestation`, on purpose: an agreement's value is that its exact words are
//! fixed, and a notice's value is that it was produced — so a wording fix
//! should reach every notice ever written, not require rewriting them.

use diesel::prelude::*;

use crate::models::{AccountNotice, NewAccountNotice};
use crate::schema::account_notices;
use crate::state::AppState;

/// `kind`, mirrored by the migration's CHECK constraint.
pub mod kind {
    /// A strike was recorded: what it was for, that it counts, how many remain,
    /// when it stops counting (FR-028).
    pub const STRIKE_RECORDED: &str = "strike_recorded";
    /// The strike that reached the suspension rung (FR-018).
    pub const PUBLISHING_SUSPENDED: &str = "publishing_suspended";
    /// The third strike: disabled, the window's end date, and — in the first
    /// sentence — that deletion is irreversible (FR-030, FR-036).
    pub const ACCOUNT_DISABLED: &str = "account_disabled";
    /// An appeal was decided, either way (FR-033).
    pub const APPEAL_RESOLVED: &str = "appeal_resolved";
    /// Restored without anybody asking — a strike aged out, or a case was
    /// resolved in the person's favour (FR-035).
    pub const ACCOUNT_RESTORED: &str = "account_restored";
    /// To a player: the world their character lived in was deleted with its
    /// creator's account, and the character was moved to them first.
    pub const ACTOR_RESCUED: &str = "actor_rescued";
}

/// Write one notice, on a connection the caller already holds — so it lands
/// with the event it describes rather than after it.
pub fn record_sync(
    conn: &mut PgConnection,
    account_id: uuid::Uuid,
    kind: &str,
    subject_ref: Option<serde_json::Value>,
    payload: Option<serde_json::Value>,
) -> QueryResult<uuid::Uuid> {
    let id = uuid::Uuid::now_v7();
    diesel::insert_into(account_notices::table)
        .values(&NewAccountNotice {
            id,
            account_id,
            kind: kind.to_string(),
            subject_ref,
            payload,
        })
        .execute(conn)?;
    Ok(id)
}

/// One account's notices, newest first.
pub async fn for_account(
    state: &AppState,
    account_id: uuid::Uuid,
    limit: i64,
) -> Result<Vec<AccountNotice>, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        account_notices::table
            .filter(account_notices::account_id.eq(account_id))
            .order(account_notices::created_at.desc())
            .limit(limit)
            .select(AccountNotice::as_select())
            .load(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to read the notices".to_string())
}
