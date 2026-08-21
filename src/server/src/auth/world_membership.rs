//! Spec 002 (FR-014, FR-015, FR-016, FR-019): shared world-membership
//! authorization guard for canvas image asset reads/writes.
//!
//! Generalizes the inline `world_members` lookup pattern already used in
//! `graphql/mutations_invites.rs` (e.g. `generate_invite_code`'s
//! "Verify user is Owner/GM of the world" check) into one function so
//! both `uploadCanvasImage` (write) and `canvasImageAssetsForScene`
//! (read) enforce the identical rule: a `world_members` row for
//! `(world_id, user_id)` — present only for the world's owner (created
//! at world-creation time) or an invite that has been accepted
//! (`mutations_invites::join_world` is the only other writer of this
//! table) — is required, or the caller is rejected before any other
//! work happens (FR-016).
//!
//! This is a synchronous function so it can run inside the
//! `tokio::task::spawn_blocking` closures both call sites already use
//! for Diesel access (Diesel's `PgConnection` is not `Send`-across-await
//! safe in this codebase's usage pattern), rather than being `async`
//! itself.

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::{world_members, worlds};

#[derive(Debug, Clone, thiserror::Error)]
pub enum WorldMembershipError {
    #[error("user is not a member of this world")]
    NotAMember,
    #[error("database error: {0}")]
    Database(String),
}

impl From<diesel::result::Error> for WorldMembershipError {
    fn from(e: diesel::result::Error) -> Self {
        WorldMembershipError::Database(e.to_string())
    }
}

/// Returns the caller's role string (e.g. `"Owner"`, `"GM"`, `"Player"`)
/// if `user_id` has an accepted `world_members` row for `world_id`, or
/// `WorldMembershipError::NotAMember` otherwise. Callers MUST treat any
/// error from this function as "reject the request" — it is the single
/// authorization gate for canvas asset reads and writes (FR-014..FR-016,
/// FR-019).
///
/// Falls back to `worlds.created_by == user_id` (returning `"Owner"`)
/// when no `world_members` row exists: `create_world`
/// (`src/server/src/graphql.rs`) does not currently insert a
/// `world_members` row for the creator — its RBAC auto-assignment is
/// explicitly commented out as "disabled pending schema" — so
/// `world_members` alone would incorrectly reject a world's own owner.
/// `worlds.created_by` is this codebase's actual, already-enforced
/// ownership source of truth (used directly by every existing
/// scene/wall/shape/token mutation's `scenes::owner_id` chain back to
/// it); this fallback makes `require_world_member` consistent with that
/// existing model rather than silently depending on the disabled
/// auto-assignment being re-enabled first.
pub fn require_world_member(
    conn: &mut PgConnection,
    user_id: Uuid,
    world_id: Uuid,
) -> Result<String, WorldMembershipError> {
    let member_role = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .filter(world_members::user_id.eq(user_id))
        .select(world_members::role)
        .first::<String>(conn)
        .optional()?;

    if let Some(role) = member_role {
        return Ok(role);
    }

    let is_creator = worlds::table
        .filter(worlds::id.eq(world_id))
        .filter(worlds::created_by.eq(user_id))
        .select(worlds::id)
        .first::<Uuid>(conn)
        .optional()?
        .is_some();

    if is_creator {
        Ok("Owner".to_string())
    } else {
        Err(WorldMembershipError::NotAMember)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_a_member_error_message_is_stable() {
        // Regression guard: callers pattern-match/convert this error into
        // a GraphQL FORBIDDEN — the message shouldn't silently change
        // shape (e.g. via a #[error] format tweak) without a conscious
        // review of every call site.
        let err = WorldMembershipError::NotAMember;
        assert_eq!(err.to_string(), "user is not a member of this world");
    }
}
