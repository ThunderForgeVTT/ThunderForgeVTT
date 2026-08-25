//! GraphQL mutations for campaign invites and world membership (Phase 4.10)

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult, SimpleObject};
use chrono::Utc;
use diesel::prelude::*;
use std::str::FromStr;
use uuid::Uuid;

use crate::auth::world_membership::{WorldMembershipError, require_world_member};
use crate::auth_middleware::AuthenticatedUser;
use crate::models::{NewWorldInvite, NewWorldMember, WorldInvite, WorldMember};
use crate::schema::world_events;
use crate::schema::world_invites;
use crate::schema::world_members;
use crate::graphql::share_codes::generate_link_code;
use crate::state::AppState;
use thunderforge_core::models::invites::{WorldInvite as CoreWorldInvite, WorldMemberRole};

// Event codes for world_events audit trail
const EVENT_CODE_INVITE_CREATED: i32 = 2;
const EVENT_CODE_MEMBER_JOINED: i32 = 3;
const EVENT_CODE_MEMBER_ROLE_CHANGED: i32 = 4;
const EVENT_CODE_MEMBER_REMOVED: i32 = 5;

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
}

// ========== Helper Functions ==========

fn get_app_state(ctx: &Context<'_>) -> GraphQLResult<AppState> {
    ctx.data::<AppState>()
        .cloned()
        .map_err(|_| Error::new("Failed to get app state"))
}

fn get_authenticated_user(ctx: &Context<'_>) -> GraphQLResult<AuthenticatedUser> {
    ctx.data::<AuthenticatedUser>()
        .cloned()
        .map_err(|_| Error::new("Authentication required"))
}

/// Record a world event to the audit trail and trigger NOTIFY for real-time sync
fn record_world_event(
    conn: &mut PgConnection,
    world_id: Uuid,
    event_code: i32,
    event_payload: Option<serde_json::Value>,
    user_id: Uuid,
) -> GraphQLResult<i64> {
    let now = Utc::now().naive_utc();

    let event_id = diesel::insert_into(world_events::table)
        .values((
            world_events::world_id.eq(world_id),
            world_events::event_code.eq(event_code),
            world_events::token_event.eq(event_payload),
            world_events::schema_version.eq(1),
            world_events::created_at.eq(now),
            world_events::updated_at.eq(now),
            world_events::created_by.eq(user_id),
            world_events::updated_by.eq(user_id),
        ))
        .returning(world_events::id)
        .get_result::<i64>(conn)
        .map_err(|e| Error::new(format!("Failed to record event: {}", e)))?;

    // Trigger pg_notify for backplane broadcast
    diesel::sql_query("SELECT pg_notify('world_events_channel', $1)")
        .bind::<diesel::sql_types::Text, _>(event_id.to_string())
        .execute(conn)
        .map_err(|e| Error::new(format!("Failed to notify: {}", e)))?;

    Ok(event_id)
}

// ========== Implementation (testable, no GraphQL Context) ==========

/// Generate a new invite code for a world (Owner/GM only). Extracted as a
/// free function (matching `queries/actor.rs`'s `_impl` convention) so
/// resolver tests can call it directly against `test_app_state()` without
/// constructing a full `async_graphql::Context`.
pub async fn generate_invite_code_impl(
    state: &AppState,
    user_id: Uuid,
    input: GenerateInviteCodeInput,
) -> GraphQLResult<WorldInvitePayload> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let world_id = input.world_id;
    let max_uses = input.max_uses;

    if max_uses <= 0 {
        return Err(Error::new("max_uses must be greater than 0"));
    }

    // Verify user is Owner/GM of the world. `require_world_member` (spec
    // 002, src/server/src/auth/world_membership.rs) falls back to
    // `worlds.created_by` when no `world_members` row exists yet, which
    // is exactly the case for a world's own owner today (`create_world`
    // does not insert an owner row — see that function's own comment).
    // Previously this used a raw `world_members` lookup with no such
    // fallback, so a world's own owner could never generate an invite
    // for their own world (spec 003 found this live; spec 005 US4
    // fixes it here rather than by inserting a row in `create_world`,
    // reusing the already-built, already-tested compensating helper
    // instead of introducing a second authorization path).
    let role = require_world_member(&mut conn, user_id, world_id).map_err(|e| match e {
        WorldMembershipError::NotAMember => Error::new("User is not a member of this world"),
        WorldMembershipError::Database(msg) => Error::new(format!("Database error: {}", msg)),
    })?;

    if role != "Owner" && role != "GM" {
        return Err(Error::new("Only Owners and GMs can generate invite codes"));
    }

    // Generate invite code. `invite_id` (the row's primary key) stays a
    // v7 UUID for index locality, but the human-facing code MUST NOT be
    // derived from it: v7 UUIDs front-load a millisecond timestamp, so
    // taking the first 8 hex characters captures mostly that timestamp
    // — two invites created within the same millisecond (trivially
    // possible under any real concurrent load, and reliably reproduced
    // by this file's own rapid-succession e2e test, spec 005 US4)
    // collide on `world_invites_invite_code_key`. Deriving the code
    // from an independent, fully-random v4 UUID instead removes that
    // collision class entirely.
    // Spec 027 (T023, FR-006): the row's `id` stays v7 for index locality, but
    // the human-facing code comes from the shared generator — 20 characters
    // from an independent v4 UUID, up from the 8 taken here before. The full
    // reasoning, including the v7 collision this must never regress to, lives
    // in `graphql::share_codes`.
    let invite_id = Uuid::now_v7();
    let invite_code = generate_link_code();

    let now = Utc::now().naive_utc();
    let expires_at = input.expires_at.as_ref().and_then(|s| {
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|dt| dt.naive_utc())
    });

    let new_invite = NewWorldInvite {
        id: invite_id,
        world_id,
        invite_code: invite_code.clone(),
        max_uses,
        used_count: 0,
        expires_at,
        created_by: user_id,
        created_at: now,
        updated_at: now,
        revoked: false,
        rotated_from: None,
    };

    diesel::insert_into(world_invites::table)
        .values(&new_invite)
        .execute(&mut conn)
        .map_err(|e| Error::new(format!("Failed to create invite: {}", e)))?;

    // Record event for audit trail and real-time sync
    let event_payload = serde_json::json!({
        "invite_id": new_invite.id,
        "invite_code": new_invite.invite_code,
        "max_uses": new_invite.max_uses,
    });
    record_world_event(
        &mut conn,
        world_id,
        EVENT_CODE_INVITE_CREATED,
        Some(event_payload),
        user_id,
    )?;

    Ok(WorldInvitePayload {
        id: new_invite.id,
        world_id: new_invite.world_id,
        invite_code: new_invite.invite_code,
        max_uses: new_invite.max_uses,
        used_count: new_invite.used_count,
        expires_at: new_invite.expires_at.map(|dt| dt.to_string()),
        created_by: new_invite.created_by,
        created_at: new_invite.created_at.to_string(),
        updated_at: new_invite.updated_at.to_string(),
        state: derive_link_state(false, new_invite.expires_at, max_uses, 0),
        remaining_uses: remaining_uses(max_uses, 0),
        rotated_from: None,
        status: format!("0/{} uses", max_uses),
    })
}

/// Join a world using an invite code. Extracted as a free function for the
/// same reason as `generate_invite_code_impl` above.
pub async fn join_world_impl(
    state: &AppState,
    user_id: Uuid,
    input: JoinWorldInput,
) -> GraphQLResult<WorldMembershipPayload> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    // Look up invite code
    let invite: WorldInvite = world_invites::table
        .filter(world_invites::invite_code.eq(input.invite_code.clone()))
        .select(WorldInvite::as_select())
        .first::<WorldInvite>(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => Error::new("Invalid invite code"),
            _ => Error::new(format!("Database error: {}", e)),
        })?;

    // Convert to core model to use validation
    let mut core_invite: CoreWorldInvite = invite.clone().into();

    // Validate invite
    if !core_invite.is_valid() {
        return Err(Error::new("Invite code is no longer valid"));
    }

    let world_id = invite.world_id;

    // Check if user is already a member
    let existing: Option<WorldMember> = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .filter(world_members::user_id.eq(user_id))
        .select(WorldMember::as_select())
        .first::<WorldMember>(&mut conn)
        .optional()
        .map_err(|e| Error::new(format!("Database error: {}", e)))?;

    if existing.is_some() {
        return Err(Error::new("You are already a member of this world"));
    }

    // Increment usage
    core_invite.use_invite().map_err(Error::new)?;

    // Update invite usage count
    let updated_count = core_invite.used_count;
    diesel::update(world_invites::table.find(invite.id))
        .set(world_invites::used_count.eq(updated_count))
        .execute(&mut conn)
        .map_err(|e| Error::new(format!("Failed to update invite: {}", e)))?;

    // Create membership record
    let membership_id = Uuid::now_v7();
    let now = Utc::now().naive_utc();

    let new_member = NewWorldMember {
        id: membership_id,
        world_id,
        user_id,
        role: "Player".to_string(),
        joined_at: now,
        created_at: now,
        updated_at: now,
    };

    diesel::insert_into(world_members::table)
        .values(&new_member)
        .execute(&mut conn)
        .map_err(|e| Error::new(format!("Failed to create membership: {}", e)))?;

    // Record event for audit trail and real-time sync
    let event_payload = serde_json::json!({
        "user_id": new_member.user_id,
        "role": new_member.role,
        "invite_code": invite.invite_code,
    });
    record_world_event(
        &mut conn,
        world_id,
        EVENT_CODE_MEMBER_JOINED,
        Some(event_payload),
        user_id,
    )?;

    Ok(WorldMembershipPayload {
        id: new_member.id,
        world_id: new_member.world_id,
        user_id: new_member.user_id,
        role: new_member.role,
        joined_at: new_member.joined_at.to_string(),
        created_at: new_member.created_at.to_string(),
        updated_at: new_member.updated_at.to_string(),
    })
}

/// Testable core of `InviteMutation::update_member_role` (spec 023 —
/// extracted from an inline `#[Object]` method so the Owner-fallback fix
/// below has direct test coverage, mirroring `generate_invite_code_impl`'s
/// existing shape).
pub async fn update_member_role_impl(
    state: &AppState,
    user_id: Uuid,
    input: UpdateMemberRoleInput,
) -> GraphQLResult<WorldMembershipPayload> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let world_id = input.world_id;
    let target_user_id = input.user_id;
    let new_role_str = input.role.clone();

    // Parse and validate new role
    match new_role_str.as_str() {
        "Owner" | "GM" | "Player" => {}
        _ => return Err(Error::new("Invalid role. Must be Owner, GM, or Player")),
    };

    // Verify caller is Owner/GM. Spec 023 (research.md §3): uses
    // `require_world_member`'s Owner-fallback (a world's creator may
    // have no `world_members` row of their own — `create_world`
    // never backfills one) instead of a raw row lookup, so the
    // world's actual Owner isn't wrongly rejected here.
    let caller_role_str = require_world_member(&mut conn, user_id, world_id)
        .map_err(|_| Error::new("You are not a member of this world"))?;
    let caller_role = WorldMemberRole::from_str(&caller_role_str).unwrap_or(WorldMemberRole::Player);

    if !caller_role.can_change_roles() {
        return Err(Error::new(
            "You do not have permission to change member roles",
        ));
    }

    // Get target member
    let target_member: WorldMember = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .filter(world_members::user_id.eq(target_user_id))
        .select(WorldMember::as_select())
        .first::<WorldMember>(&mut conn)
        .map_err(|e| match e {
            diesel::result::Error::NotFound => {
                Error::new("Target user is not a member of this world")
            }
            _ => Error::new(format!("Database error: {}", e)),
        })?;

    let target_role =
        WorldMemberRole::from_str(&target_member.role).unwrap_or(WorldMemberRole::Player);

    // Check permission
    if !caller_role.can_manage(target_role) {
        return Err(Error::new(
            "You do not have permission to manage this member's role",
        ));
    }

    // Update role
    let now = Utc::now().naive_utc();
    diesel::update(world_members::table.find(target_member.id))
        .set((
            world_members::role.eq(new_role_str.clone()),
            world_members::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .map_err(|e| Error::new(format!("Failed to update member: {}", e)))?;

    // Record event for audit trail and real-time sync
    let event_payload = serde_json::json!({
        "user_id": target_member.user_id,
        "old_role": target_member.role,
        "new_role": new_role_str.clone(),
    });
    record_world_event(
        &mut conn,
        world_id,
        EVENT_CODE_MEMBER_ROLE_CHANGED,
        Some(event_payload),
        user_id,
    )?;

    Ok(WorldMembershipPayload {
        id: target_member.id,
        world_id: target_member.world_id,
        user_id: target_member.user_id,
        role: new_role_str,
        joined_at: target_member.joined_at.to_string(),
        created_at: target_member.created_at.to_string(),
        updated_at: now.to_string(),
    })
}

/// Testable core of `InviteMutation::remove_member` (spec 023 — same
/// extraction rationale as `update_member_role_impl` above).
pub async fn remove_member_impl(
    state: &AppState,
    caller_id: Uuid,
    world_id: Uuid,
    user_id: Uuid,
) -> GraphQLResult<bool> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    // Prevent self-removal
    if caller_id == user_id {
        return Err(Error::new("You cannot remove yourself from the world"));
    }

    // Get caller's role. Spec 023 (research.md §3): same Owner-fallback
    // fix as `update_member_role_impl` above.
    let caller_role_str = require_world_member(&mut conn, caller_id, world_id)
        .map_err(|_| Error::new("You are not a member of this world"))?;
    let caller_role = WorldMemberRole::from_str(&caller_role_str).unwrap_or(WorldMemberRole::Player);

    // Check permission: Only Owner or GM can remove members
    if caller_role != WorldMemberRole::Owner && caller_role != WorldMemberRole::GM {
        return Err(Error::new("You do not have permission to remove members"));
    }

    // Get target member
    let target_member: Option<WorldMember> = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .filter(world_members::user_id.eq(user_id))
        .select(WorldMember::as_select())
        .first::<WorldMember>(&mut conn)
        .optional()
        .map_err(|e| Error::new(format!("Database error: {}", e)))?;

    let target_member =
        target_member.ok_or_else(|| Error::new("Target user is not a member of this world"))?;

    let target_role =
        WorldMemberRole::from_str(&target_member.role).unwrap_or(WorldMemberRole::Player);

    // Check permission: Can't remove someone of equal or higher rank
    if !caller_role.can_manage(target_role) {
        return Err(Error::new(
            "You cannot remove a member of equal or higher rank",
        ));
    }

    // Delete the membership
    diesel::delete(world_members::table.find(target_member.id))
        .execute(&mut conn)
        .map_err(|e| Error::new(format!("Failed to remove member: {}", e)))?;

    // Spec 010 (research.md §7, FR-022): a removed member's actor
    // ownership-block entries don't get a DB-level cascade (there is
    // no direct FK from `world_members` to `world_actor_permissions`
    // — the relationship is via `world_id` on the joined
    // `world_actors` row), so this is deleted explicitly here,
    // alongside the membership removal.
    {
        use crate::schema::{world_actor_permissions, world_actors};
        diesel::delete(
            world_actor_permissions::table
                .filter(world_actor_permissions::user_id.eq(user_id))
                .filter(
                    world_actor_permissions::actor_id.eq_any(
                        world_actors::table
                            .filter(world_actors::world_id.eq(world_id))
                            .select(world_actors::id),
                    ),
                ),
        )
        .execute(&mut conn)
        .map_err(|e| Error::new(format!("Failed to clean up actor permissions: {}", e)))?;
    }

    // Spec 013: same rationale as the actor-permissions cleanup above,
    // generalized to item ownership-block entries (no direct FK from
    // `world_members` to `world_item_permissions`).
    {
        use crate::schema::{world_item_permissions, world_items};
        diesel::delete(
            world_item_permissions::table
                .filter(world_item_permissions::user_id.eq(user_id))
                .filter(
                    world_item_permissions::item_id.eq_any(
                        world_items::table
                            .filter(world_items::world_id.eq(world_id))
                            .select(world_items::id),
                    ),
                ),
        )
        .execute(&mut conn)
        .map_err(|e| Error::new(format!("Failed to clean up item permissions: {}", e)))?;
    }

    // Spec 012 (data-model.md, mirrors FR-022 from spec 010 verbatim
    // per spec.md's Assumptions): same story for a removed member's
    // lore entry ownership-block entries — no direct FK from
    // `world_members` to `world_lore_permissions`, so cleaned up
    // explicitly here too.
    {
        use crate::schema::{world_lore_entries, world_lore_permissions};
        diesel::delete(
            world_lore_permissions::table
                .filter(world_lore_permissions::world_member_user_id.eq(user_id))
                .filter(
                    world_lore_permissions::lore_entry_id.eq_any(
                        world_lore_entries::table
                            .filter(world_lore_entries::world_id.eq(world_id))
                            .select(world_lore_entries::id),
                    ),
                ),
        )
        .execute(&mut conn)
        .map_err(|e| Error::new(format!("Failed to clean up lore permissions: {}", e)))?;
    }

    // Spec 027 (US2, FR-018): the fourth block, missing until now. Spec 025
    // added `world_ability_permissions` but never extended this path, so a
    // removed member kept their ability ownership-block entries — and
    // re-adding them silently restored Editor/Owner rights on those
    // abilities. Same story as the three above: no direct FK from
    // `world_members` to the grant table, so cleanup is explicit.
    //
    // Written out by hand deliberately. Spec 027 US5 replaces all four of
    // these blocks with a single `purge_member_grants` call derived from the
    // permissioned-entity declaration, so the omission cannot recur — but
    // this fix ships first, independently, because it closes a live
    // privilege leak and must not wait on that refactor.
    {
        use crate::schema::{world_abilities, world_ability_permissions};
        diesel::delete(
            world_ability_permissions::table
                .filter(world_ability_permissions::user_id.eq(user_id))
                .filter(
                    world_ability_permissions::ability_id.eq_any(
                        world_abilities::table
                            .filter(world_abilities::world_id.eq(world_id))
                            .select(world_abilities::id),
                    ),
                ),
        )
        .execute(&mut conn)
        .map_err(|e| Error::new(format!("Failed to clean up ability permissions: {}", e)))?;
    }

    // Record event for audit trail
    let event_payload = serde_json::json!({
        "user_id": target_member.user_id,
        "role": target_member.role,
    });
    record_world_event(
        &mut conn,
        world_id,
        EVENT_CODE_MEMBER_REMOVED,
        Some(event_payload),
        caller_id,
    )?;

    Ok(true)
}

// ========== Mutations ==========

#[derive(Default)]
pub struct InviteMutation;

#[async_graphql::Object]
impl InviteMutation {
    /// Generate a new invite code for a world (Owner/GM only)
    pub async fn generate_invite_code(
        &self,
        ctx: &Context<'_>,
        input: GenerateInviteCodeInput,
    ) -> GraphQLResult<WorldInvitePayload> {
        let state = get_app_state(ctx)?;
        let auth_user = get_authenticated_user(ctx)?;
        generate_invite_code_impl(&state, auth_user.user_id, input).await
    }

    /// Join a world using an invite code
    pub async fn join_world(
        &self,
        ctx: &Context<'_>,
        input: JoinWorldInput,
    ) -> GraphQLResult<WorldMembershipPayload> {
        let state = get_app_state(ctx)?;
        let auth_user = get_authenticated_user(ctx)?;
        join_world_impl(&state, auth_user.user_id, input).await
    }

    /// Update a member's role in a world (Owner/GM only, with permission checks)
    pub async fn update_member_role(
        &self,
        ctx: &Context<'_>,
        input: UpdateMemberRoleInput,
    ) -> GraphQLResult<WorldMembershipPayload> {
        let state = get_app_state(ctx)?;
        let auth_user = get_authenticated_user(ctx)?;
        update_member_role_impl(&state, auth_user.user_id, input).await
    }

    /// Remove a member from a world (Owner/GM only)
    async fn remove_member(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        user_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = get_app_state(ctx)?;
        let auth_user = get_authenticated_user(ctx)?;
        remove_member_impl(&state, auth_user.user_id, world_id, user_id).await
    }
}

#[cfg(test)]
mod tests {
    use diesel::{Connection, PgConnection};

    /// Establishes a connection to the dev database configured via
    /// DATABASE_URL (same source main.rs uses). Skips (rather than fails)
    /// when no dev database is reachable, since this is a real-DB
    /// integration test, not a unit test.
    fn try_connect() -> Option<PgConnection> {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL").ok()?;
        diesel::Connection::establish(&url).ok()
    }

    /// Spec 005 US4 regression test (T020): before this fix,
    /// `generate_invite_code`'s own inline `world_members` lookup had no
    /// fallback, so a world's own owner — who has no `world_members` row
    /// today (`create_world` doesn't insert one; see
    /// `auth::world_membership::require_world_member`'s doc comment) —
    /// could never generate an invite for their own world. The fix routes
    /// `generate_invite_code` (and `world_invites`, the query
    /// `CampaignSettingsPanel.tsx` calls on mount) through
    /// `require_world_member` instead, which already falls back to
    /// `worlds.created_by`. This test exercises that shared primitive
    /// directly against a freshly created world with no `world_members`
    /// row, which is exactly the state `generate_invite_code` now sees.
    #[test]
    fn owner_can_be_authorized_for_invites_immediately_after_world_creation() {
        let Some(mut conn) = try_connect() else {
            eprintln!(
                "skipping owner_can_be_authorized_for_invites_immediately_after_world_creation: no DATABASE_URL/dev DB reachable"
            );
            return;
        };

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            let owner_id = crate::test_support::insert_test_user(conn);
            let world_id = crate::test_support::insert_test_world(conn, owner_id);

            // No insert_test_world_member call here — deliberately, since
            // this is exactly the state `create_world` leaves a fresh
            // world in today.
            let role =
                crate::auth::world_membership::require_world_member(conn, owner_id, world_id)
                    .expect(
                        "owner must be authorized immediately, with no separate membership step",
                    );
            assert_eq!(role, "Owner");

            // A non-owner, non-member user must still be rejected — this
            // fix must not have loosened the check for anyone else.
            let intruder_id = crate::test_support::insert_test_user(conn);
            let intruder_result =
                crate::auth::world_membership::require_world_member(conn, intruder_id, world_id);
            assert!(
                intruder_result.is_err(),
                "a non-member/non-owner must still be rejected"
            );

            Ok(())
        });
    }

    // ===== Resolver-level tests for generate_invite_code_impl / join_world_impl =====
    //
    // These call the `_impl` free functions directly against
    // `test_support::test_app_state()` (a real DB pool, no transaction
    // wrapper — matching `mutations_actor_claims.rs`'s established
    // convention), rather than the `require_world_member`/core-model unit
    // tests above, which exercise the shared primitives in isolation but
    // never actually call these two mutations end-to-end.

    use super::*;
    use crate::test_support::{insert_test_user, insert_test_world, insert_test_world_member, test_app_state};

    /// Inserts an invite row. The **8-character** code is deliberate: it is
    /// exactly the shape codes had before spec 027, so every test built on
    /// this helper doubles as coverage that pre-existing links still work
    /// (FR-007 / SC-006).
    fn insert_test_invite(
        conn: &mut PgConnection,
        world_id: Uuid,
        created_by: Uuid,
        max_uses: i32,
        used_count: i32,
        expires_at: Option<chrono::NaiveDateTime>,
    ) -> (Uuid, String) {
        insert_test_invite_with_revocation(
            conn, world_id, created_by, max_uses, used_count, expires_at, false,
        )
    }

    /// As above, but lets a test build an already-retired link.
    fn insert_test_invite_with_revocation(
        conn: &mut PgConnection,
        world_id: Uuid,
        created_by: Uuid,
        max_uses: i32,
        used_count: i32,
        expires_at: Option<chrono::NaiveDateTime>,
        revoked: bool,
    ) -> (Uuid, String) {
        let id = Uuid::now_v7();
        let code = Uuid::new_v4()
            .to_string()
            .replace('-', "")
            .chars()
            .take(8)
            .collect::<String>()
            .to_uppercase();
        let now = Utc::now().naive_utc();
        diesel::insert_into(world_invites::table)
            .values(NewWorldInvite {
                id,
                world_id,
                invite_code: code.clone(),
                max_uses,
                used_count,
                expires_at,
                created_by,
                created_at: now,
                updated_at: now,
                revoked,
                rotated_from: None,
            })
            .execute(conn)
            .expect("failed to insert test invite");
        (id, code)
    }

    #[tokio::test]
    async fn join_world_rejects_invalid_code() {
        let state = test_app_state();
        let joiner_id = {
            let mut conn = state.db_pool.get().unwrap();
            insert_test_user(&mut conn)
        };

        let result = join_world_impl(
            &state,
            joiner_id,
            JoinWorldInput {
                invite_code: "NONEXIST".to_string(),
            },
        )
        .await;
        assert!(result.is_err(), "an unknown invite code must be rejected");
    }

    #[tokio::test]
    async fn join_world_rejects_expired_invite() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let past = Utc::now().naive_utc() - chrono::Duration::days(1);
        let (_id, code) = insert_test_invite(&mut conn, world_id, owner_id, 10, 0, Some(past));
        let joiner_id = insert_test_user(&mut conn);
        drop(conn);

        let result = join_world_impl(&state, joiner_id, JoinWorldInput { invite_code: code }).await;
        assert!(result.is_err(), "an expired invite must be rejected");
    }

    #[tokio::test]
    async fn join_world_rejects_exhausted_invite() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        // max_uses == used_count: no uses remaining.
        let (_id, code) = insert_test_invite(&mut conn, world_id, owner_id, 3, 3, None);
        let joiner_id = insert_test_user(&mut conn);
        drop(conn);

        let result = join_world_impl(&state, joiner_id, JoinWorldInput { invite_code: code }).await;
        assert!(result.is_err(), "an exhausted invite must be rejected");
    }

    #[tokio::test]
    async fn join_world_rejects_existing_member() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let (_id, code) = insert_test_invite(&mut conn, world_id, owner_id, 10, 0, None);
        let existing_member_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, existing_member_id, "Player");
        drop(conn);

        let result = join_world_impl(
            &state,
            existing_member_id,
            JoinWorldInput { invite_code: code },
        )
        .await;
        assert!(
            result.is_err(),
            "a user who is already a member must not be able to join again"
        );
    }

    #[tokio::test]
    async fn join_world_success_creates_player_membership_and_increments_usage() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let (invite_id, code) = insert_test_invite(&mut conn, world_id, owner_id, 5, 0, None);
        let joiner_id = insert_test_user(&mut conn);
        drop(conn);

        let payload = join_world_impl(&state, joiner_id, JoinWorldInput { invite_code: code })
            .await
            .expect("a valid, unused invite must allow joining");
        assert_eq!(payload.world_id, world_id);
        assert_eq!(payload.user_id, joiner_id);
        assert_eq!(payload.role, "Player");

        let mut conn = state.db_pool.get().unwrap();
        let member: WorldMember = world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(joiner_id))
            .select(WorldMember::as_select())
            .first(&mut conn)
            .expect("membership row must have been created");
        assert_eq!(member.role, "Player");

        let updated_invite: WorldInvite = world_invites::table
            .find(invite_id)
            .select(WorldInvite::as_select())
            .first(&mut conn)
            .expect("invite row must still exist");
        assert_eq!(
            updated_invite.used_count, 1,
            "used_count must be incremented on a successful join"
        );
    }

    #[tokio::test]
    async fn generate_invite_code_rejects_non_member() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let outsider_id = insert_test_user(&mut conn);
        drop(conn);

        let result = generate_invite_code_impl(
            &state,
            outsider_id,
            GenerateInviteCodeInput {
                world_id,
                max_uses: 5,
                expires_at: None,
            },
        )
        .await;
        assert!(
            result.is_err(),
            "a non-member/non-owner must not be able to generate an invite"
        );
    }

    #[tokio::test]
    async fn generate_invite_code_success_path() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let payload = generate_invite_code_impl(
            &state,
            owner_id,
            GenerateInviteCodeInput {
                world_id,
                max_uses: 7,
                expires_at: None,
            },
        )
        .await
        .expect("the world's own owner must be able to generate an invite");
        assert_eq!(payload.world_id, world_id);
        assert_eq!(payload.max_uses, 7);
        assert_eq!(payload.used_count, 0);

        // Spec 027 (FR-006): this assertion previously expected 8 characters.
        // Raising it to 20 is a **deliberate behaviour change**, not a test
        // relaxed to fit an accident: an invite code grants membership in a
        // world, and ~32 bits did not meet ADR-049's unguessable-code
        // invariant while content share links already used ~80.
        assert_eq!(
            payload.invite_code.len(),
            20,
            "invite codes must match content-share-link strength"
        );

        // A freshly issued link is usable, with its whole cap intact.
        assert_eq!(payload.state, WorldAccessLinkState::Active);
        assert_eq!(payload.remaining_uses, Some(7));
        assert_eq!(payload.rotated_from, None);
    }

    // ===== Spec 027 (T012, FR-010): link-state derivation =====
    //
    // Pure functions over a row's fields, so these need no database.

    fn in_the_past() -> Option<chrono::NaiveDateTime> {
        Some(Utc::now().naive_utc() - chrono::Duration::hours(1))
    }

    fn in_the_future() -> Option<chrono::NaiveDateTime> {
        Some(Utc::now().naive_utc() + chrono::Duration::hours(1))
    }

    #[test]
    fn a_fresh_capped_link_is_active() {
        assert_eq!(
            derive_link_state(false, None, 10, 0),
            WorldAccessLinkState::Active
        );
        assert_eq!(
            derive_link_state(false, in_the_future(), 10, 3),
            WorldAccessLinkState::Active
        );
    }

    #[test]
    fn a_past_expiry_reads_expired() {
        assert_eq!(
            derive_link_state(false, in_the_past(), 10, 0),
            WorldAccessLinkState::Expired
        );
    }

    #[test]
    fn a_spent_cap_reads_exhausted() {
        assert_eq!(
            derive_link_state(false, None, 5, 5),
            WorldAccessLinkState::Exhausted
        );
        // Over-consumption still reads exhausted rather than active.
        assert_eq!(
            derive_link_state(false, None, 5, 7),
            WorldAccessLinkState::Exhausted
        );
    }

    #[test]
    fn revocation_reads_revoked() {
        assert_eq!(
            derive_link_state(true, None, 10, 0),
            WorldAccessLinkState::Revoked
        );
    }

    /// The precedence case from data-model.md §2: a link can be revoked *and*
    /// expired *and* exhausted at once. The GM should see the most decisive
    /// reason, which is revocation — it is the one a human deliberately did.
    #[test]
    fn revoked_outranks_expired_and_exhausted() {
        assert_eq!(
            derive_link_state(true, in_the_past(), 5, 5),
            WorldAccessLinkState::Revoked,
            "revocation must outrank every other reason"
        );
        assert_eq!(
            derive_link_state(false, in_the_past(), 5, 5),
            WorldAccessLinkState::Expired,
            "expiry must outrank exhaustion"
        );
    }

    /// `max_uses == 0` means unlimited, so it can never be exhausted and has
    /// no remaining count to report. Unreachable via the API today, but the
    /// model still branches on it — see `WorldInvite::is_valid`.
    #[test]
    fn an_uncapped_link_never_exhausts_and_reports_no_remainder() {
        assert_eq!(
            derive_link_state(false, None, 0, 9_999),
            WorldAccessLinkState::Active
        );
        assert_eq!(remaining_uses(0, 9_999), None);
    }

    #[test]
    fn remaining_uses_counts_down_and_saturates_at_zero() {
        assert_eq!(remaining_uses(10, 0), Some(10));
        assert_eq!(remaining_uses(10, 4), Some(6));
        assert_eq!(remaining_uses(10, 10), Some(0));
        // Never negative, even if a row somehow over-consumed.
        assert_eq!(remaining_uses(10, 12), Some(0));
    }

    // ===== Spec 027 US2: member removal must clear grants on EVERY type =====
    //
    // `remove_member_impl` cleans up actor, item and lore grants in three
    // hand-written blocks, each commented to explain there is no FK cascade
    // from `world_members`. Spec 025 added `world_ability_permissions` and
    // never added a fourth block, so a removed member kept their ability
    // grants — and re-adding them silently restored Editor/Owner rights.
    //
    // These fail on the code as it stood before spec 027.

    /// Sets up a member holding an explicit grant on one row of each of the
    /// four permissioned content types.
    /// Returns `(world_id, owner_id, member_id)`.
    fn world_with_a_fully_granted_member(
        conn: &mut PgConnection,
        level: &str,
    ) -> (Uuid, Uuid, Uuid) {
        use crate::test_support::{
            grant_all_content_permissions, insert_test_ability, insert_test_actor,
            insert_test_item, insert_test_lore_entry, insert_test_scene,
        };

        let owner_id = insert_test_user(conn);
        let world_id = insert_test_world(conn, owner_id);
        let scene_id = insert_test_scene(conn, world_id, owner_id);

        let actor_id = insert_test_actor(conn, world_id, scene_id, owner_id);
        let item_id = insert_test_item(conn, world_id, owner_id);
        let lore_id = insert_test_lore_entry(conn, world_id, owner_id);
        let ability_id = insert_test_ability(conn, world_id, owner_id);

        let member_id = insert_test_user(conn);
        insert_test_world_member(conn, world_id, member_id, "Player");
        grant_all_content_permissions(
            conn, member_id, actor_id, item_id, lore_id, ability_id, level,
        );

        (world_id, owner_id, member_id)
    }

    /// FR-018 / US2-1: all four grant types are cleared on removal.
    /// Before the fix this failed on the ability count alone.
    #[tokio::test]
    async fn removing_a_member_clears_grants_on_every_content_type() {
        use crate::test_support::count_content_permissions;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let (world_id, owner_id, member_id) =
            world_with_a_fully_granted_member(&mut conn, "Editor");

        // Precondition: the member really does hold all four.
        let before = count_content_permissions(&mut conn, world_id, member_id);
        assert_eq!(
            before,
            (1, 1, 1, 1),
            "setup failed — member should hold one grant of each type"
        );
        drop(conn);

        remove_member_impl(&state, owner_id, world_id, member_id)
            .await
            .expect("owner must be able to remove a player");

        let mut conn = state.db_pool.get().unwrap();
        let after = count_content_permissions(&mut conn, world_id, member_id);
        assert_eq!(
            after,
            (0, 0, 0, 0),
            "every grant must be cleared on removal; \
             a non-zero fourth element is the ability-cleanup gap (FR-018)"
        );
    }

    /// SC-008 / US2-2: readmission grants nothing back.
    #[tokio::test]
    async fn a_readmitted_member_holds_no_elevated_rights() {
        use crate::test_support::count_content_permissions;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let (world_id, owner_id, member_id) =
            world_with_a_fully_granted_member(&mut conn, "Owner");
        drop(conn);

        remove_member_impl(&state, owner_id, world_id, member_id)
            .await
            .expect("removal should succeed");

        // Re-invite: they come back as an ordinary Player.
        let mut conn = state.db_pool.get().unwrap();
        insert_test_world_member(&mut conn, world_id, member_id, "Player");

        let after = count_content_permissions(&mut conn, world_id, member_id);
        assert_eq!(
            after,
            (0, 0, 0, 0),
            "a readmitted member must not silently regain any prior grant"
        );
    }

    /// US2-3: removal is scoped to one world, and an empty grant set is fine.
    #[tokio::test]
    async fn removal_is_world_scoped_and_tolerates_no_grants() {
        use crate::test_support::count_content_permissions;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();

        let (world_a, owner_a, member_id) =
            world_with_a_fully_granted_member(&mut conn, "Editor");

        // The same user holds grants in an unrelated world.
        let world_b = {
            use crate::test_support::{
                grant_all_content_permissions, insert_test_ability, insert_test_actor,
                insert_test_item, insert_test_lore_entry, insert_test_scene,
            };
            let owner_b = insert_test_user(&mut conn);
            let world_b = insert_test_world(&mut conn, owner_b);
            let scene_b = insert_test_scene(&mut conn, world_b, owner_b);
            let actor_b = insert_test_actor(&mut conn, world_b, scene_b, owner_b);
            let item_b = insert_test_item(&mut conn, world_b, owner_b);
            let lore_b = insert_test_lore_entry(&mut conn, world_b, owner_b);
            let ability_b = insert_test_ability(&mut conn, world_b, owner_b);
            insert_test_world_member(&mut conn, world_b, member_id, "Player");
            grant_all_content_permissions(
                &mut conn, member_id, actor_b, item_b, lore_b, ability_b, "Editor",
            );
            world_b
        };
        drop(conn);

        remove_member_impl(&state, owner_a, world_a, member_id)
            .await
            .expect("removal from world A should succeed");

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            count_content_permissions(&mut conn, world_b, member_id),
            (1, 1, 1, 1),
            "removal from one world must not touch grants in another"
        );

        // A member with no grants at all removes cleanly rather than erroring.
        let bare_member = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_a, bare_member, "Player");
        drop(conn);

        remove_member_impl(&state, owner_a, world_a, bare_member)
            .await
            .expect("removing a member holding zero grants must succeed quietly");
    }

    #[tokio::test]
    async fn owner_with_no_membership_row_can_change_roles_and_remove_members() {
        // Spec 023 (research.md §3): identical bug class to
        // `owner_can_be_authorized_for_invites_immediately_after_world_creation`
        // above, now fixed in `update_member_role_impl`/`remove_member_impl`
        // via `require_world_member`'s Owner-fallback.
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        // No insert_test_world_member for the owner — deliberately, matching
        // what `create_world` actually leaves behind.
        let target_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, target_id, "Player");
        drop(conn);

        let payload = update_member_role_impl(
            &state,
            owner_id,
            UpdateMemberRoleInput {
                world_id,
                user_id: target_id,
                role: "GM".to_string(),
            },
        )
        .await
        .expect("the world's own owner, with no world_members row, must be able to change roles");
        assert_eq!(payload.role, "GM");

        let removed = remove_member_impl(&state, owner_id, world_id, target_id)
            .await
            .expect("the world's own owner, with no world_members row, must be able to remove members");
        assert!(removed);

        let mut conn = state.db_pool.get().unwrap();
        let remaining: Option<WorldMember> = world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(target_id))
            .select(WorldMember::as_select())
            .first(&mut conn)
            .optional()
            .unwrap();
        assert!(remaining.is_none(), "removed member's row must be gone");
    }
}
