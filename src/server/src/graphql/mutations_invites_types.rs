//! The shapes an invite is asked for and answered with.
//!
//! Split out of `mutations_invites.rs`: these are data declarations plus the
//! two pure functions that read a link's state off its own columns. Nothing
//! here touches the database, which is why it reads as one piece.

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult, SimpleObject};
use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use super::get_app_state;
use crate::models::WorldInvite;

// ========== Input Types ==========

#[derive(InputObject, Debug, Clone)]
pub struct GenerateInviteCodeInput {
    /// World ID for the campaign
    pub world_id: Uuid,
    /// Maximum number of times this invite can be used (0 = unlimited)
    pub max_uses: i32,
    /// Optional expiry time (ISO 8601 format)
    pub expires_at: Option<String>,
}

#[derive(InputObject, Debug, Clone)]
pub struct JoinWorldInput {
    /// The invite code from the URL
    pub invite_code: String,
}

#[derive(InputObject, Debug, Clone)]
pub struct UpdateMemberRoleInput {
    /// World ID where the member belongs
    pub world_id: Uuid,
    /// User ID of the member to update
    pub user_id: Uuid,
    /// New role: Owner, GM, or Player
    pub role: String,
}

// ========== Output Types ==========

/// Spec 027 (T010, FR-010): whether a link works right now, and if not, why.
///
/// **Derived, never stored.** Computing this from the row means it cannot
/// drift from the facts that produce it — there is no column to forget to
/// update when a link expires or its last use is consumed.
#[derive(async_graphql::Enum, Copy, Clone, Debug, Eq, PartialEq)]
pub enum WorldAccessLinkState {
    /// Usable right now.
    Active,
    /// Past its expiry time.
    Expired,
    /// Its use cap is spent.
    Exhausted,
    /// Explicitly retired by a GM, by revocation or by rotation.
    Revoked,
}

/// Derives a link's state from its row.
///
/// The precedence order (revoked → expired → exhausted → active) matters for
/// **display only**: a link can be simultaneously revoked and expired, and the
/// GM should see the most decisive reason.
///
/// For **enforcement** these collapse to a single boolean, and the caller is
/// never told which applied — see FR-011. Do not reach for this function to
/// gate a join; the authoritative check is the SQL predicate in
/// `join_world_impl`, which evaluates the same conditions atomically with the
/// use increment.
pub fn derive_link_state(
    revoked: bool,
    expires_at: Option<chrono::NaiveDateTime>,
    max_uses: i32,
    used_count: i32,
) -> WorldAccessLinkState {
    if revoked {
        return WorldAccessLinkState::Revoked;
    }
    if let Some(expires) = expires_at
        && Utc::now().naive_utc() >= expires
    {
        return WorldAccessLinkState::Expired;
    }
    // `max_uses == 0` is unlimited, so it can never be exhausted. See
    // `WorldInvite::is_valid` in src/core for why that branch still exists.
    if max_uses > 0 && used_count >= max_uses {
        return WorldAccessLinkState::Exhausted;
    }
    WorldAccessLinkState::Active
}

/// Uses left on a link, or `None` when it is uncapped (`max_uses == 0`).
///
/// Saturates at zero rather than reporting a negative remainder, so a row that
/// somehow over-consumed reads as spent instead of nonsensical.
pub fn remaining_uses(max_uses: i32, used_count: i32) -> Option<i32> {
    if max_uses <= 0 {
        None
    } else {
        Some((max_uses - used_count).max(0))
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct WorldInvitePayload {
    pub id: Uuid,
    pub world_id: Uuid,
    pub invite_code: String,
    pub max_uses: i32,
    pub used_count: i32,
    pub expires_at: Option<String>,
    pub created_by: Uuid,
    pub created_at: String,
    pub updated_at: String,

    /// Spec 027 (FR-010): whether this link currently works, and why not.
    pub state: WorldAccessLinkState,
    /// Spec 027 (FR-010): uses left, or `null` when uncapped.
    pub remaining_uses: Option<i32>,
    /// Spec 027 (FR-003): the link this one replaced, if created by rotation.
    pub rotated_from: Option<Uuid>,

    /// **Deprecated** — retained for one release. A free-form string like
    /// `"3/10 uses"` cannot express revocation, so a revoked link rendered
    /// identically to a working one. Prefer `state` and `remainingUses`.
    #[graphql(deprecation = "Use `state` and `remainingUses`; this cannot express revocation.")]
    pub status: String,
}

impl WorldInvitePayload {
    /// Builds a payload from a stored row, deriving state rather than trusting
    /// a caller to compute it consistently at each call site.
    pub fn from_row(invite: &WorldInvite) -> Self {
        Self {
            id: invite.id,
            world_id: invite.world_id,
            invite_code: invite.invite_code.clone(),
            max_uses: invite.max_uses,
            used_count: invite.used_count,
            expires_at: invite.expires_at.map(|dt| dt.to_string()),
            created_by: invite.created_by,
            created_at: invite.created_at.to_string(),
            updated_at: invite.updated_at.to_string(),
            state: derive_link_state(
                invite.revoked,
                invite.expires_at,
                invite.max_uses,
                invite.used_count,
            ),
            remaining_uses: remaining_uses(invite.max_uses, invite.used_count),
            rotated_from: invite.rotated_from,
            status: format!("{}/{} uses", invite.used_count, invite.max_uses),
        }
    }
}

// Spec 023 (FR-004): `claimedActor` is a per-request-computed field (a
// `world_actor_claims` lookup, not a stored column on this payload), so
// this type is `#[graphql(complex)]` — mirrors `GraphQLWorldActor`'s own
// `claimed_by` computed field (graphql.rs), just resolved from the other
// direction.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex)]
pub struct WorldMembershipPayload {
    pub id: Uuid,
    pub world_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub joined_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[async_graphql::ComplexObject]
impl WorldMembershipPayload {
    /// Spec 023 (FR-004/FR-005): the character this member has claimed,
    /// if any — `None` when they haven't claimed one yet.
    async fn claimed_actor(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<Option<crate::graphql::GraphQLWorldActor>> {
        let state = get_app_state(ctx)?;
        crate::graphql::mutations_actor_claims::claimed_actor_impl(&state, self.id).await
    }

    /// Spec 031 (FR-033): the name to put on a player's card.
    ///
    /// Computed here rather than stored on the payload for the same reason
    /// `claimed_actor` is: it lives in `users`, not in `world_members`, and
    /// every existing caller of this type builds it from a membership row
    /// alone. A card headed by a UUID is not a roster anyone can search.
    async fn username(&self, ctx: &Context<'_>) -> GraphQLResult<String> {
        let state = get_app_state(ctx)?;
        let user_id = self.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        tokio::task::spawn_blocking(move || {
            crate::schema::users::table
                .filter(crate::schema::users::id.eq(user_id))
                .select(crate::schema::users::username)
                .first::<String>(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|e| Error::new(format!("Failed to look up username: {e}")))
    }
}
