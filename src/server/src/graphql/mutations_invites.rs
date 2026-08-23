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
    pub status: String,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct WorldMembershipPayload {
    pub id: Uuid,
    pub world_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub joined_at: String,
    pub created_at: String,
    pub updated_at: String,
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
        let user_id = auth_user.user_id;
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
        let invite_id = Uuid::now_v7();
        let invite_code = Uuid::new_v4()
            .to_string()
            .replace("-", "")
            .chars()
            .take(8)
            .collect::<String>()
            .to_string()
            .to_uppercase();

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
            status: format!("0/{} uses", max_uses),
        })
    }

    /// Join a world using an invite code
    pub async fn join_world(
        &self,
        ctx: &Context<'_>,
        input: JoinWorldInput,
    ) -> GraphQLResult<WorldMembershipPayload> {
        let state = get_app_state(ctx)?;
        let auth_user = get_authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
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

    /// Update a member's role in a world (Owner/GM only, with permission checks)
    pub async fn update_member_role(
        &self,
        ctx: &Context<'_>,
        input: UpdateMemberRoleInput,
    ) -> GraphQLResult<WorldMembershipPayload> {
        let state = get_app_state(ctx)?;
        let auth_user = get_authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
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

        // Verify caller is Owner/GM
        let caller_member: Option<WorldMember> = world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(user_id))
            .select(WorldMember::as_select())
            .first::<WorldMember>(&mut conn)
            .optional()
            .map_err(|e| Error::new(format!("Database error: {}", e)))?;

        let caller =
            caller_member.ok_or_else(|| Error::new("You are not a member of this world"))?;
        let caller_role =
            WorldMemberRole::from_str(&caller.role).unwrap_or(WorldMemberRole::Player);

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

    /// Remove a member from a world (Owner/GM only)
    async fn remove_member(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        user_id: Uuid,
    ) -> GraphQLResult<bool> {
        let state = get_app_state(ctx)?;
        let auth_user = get_authenticated_user(ctx)?;
        let caller_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        // Prevent self-removal
        if caller_id == user_id {
            return Err(Error::new("You cannot remove yourself from the world"));
        }

        // Get caller's role
        let caller_member: Option<WorldMember> = world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(caller_id))
            .select(WorldMember::as_select())
            .first::<WorldMember>(&mut conn)
            .optional()
            .map_err(|e| Error::new(format!("Database error: {}", e)))?;

        let caller_member =
            caller_member.ok_or_else(|| Error::new("You are not a member of this world"))?;

        let caller_role =
            WorldMemberRole::from_str(&caller_member.role).unwrap_or(WorldMemberRole::Player);

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
}
