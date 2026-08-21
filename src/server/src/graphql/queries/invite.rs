//! Invite and world membership queries (Phase 4.10)

use async_graphql::Context;
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::{require_world_member, WorldMembershipError};
use crate::graphql::*;
use crate::models::{WorldInvite, WorldMember};
use crate::schema::{world_invites, world_members};

#[derive(Default)]
pub struct InviteQuery;

#[async_graphql::Object]
impl InviteQuery {
    /// Get all invites for a world (Owner/GM only)
    async fn world_invites(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Vec<crate::graphql::mutations_invites::WorldInvitePayload>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        // Verify user is Owner/GM of the world. Uses `require_world_member`
        // (falls back to `worlds.created_by` when no `world_members` row
        // exists) rather than a raw lookup, matching the fix applied to
        // `generate_invite_code` (spec 005 US4) — otherwise a world's own
        // owner could not even list their own world's invites, which broke
        // `CampaignSettingsPanel.tsx`'s invite list on mount.
        let role = require_world_member(&mut conn, user_id, world_id).map_err(|e| match e {
            WorldMembershipError::NotAMember => Error::new("User is not a member of this world"),
            WorldMembershipError::Database(msg) => Error::new(format!("Database error: {}", msg)),
        })?;

        if role != "Owner" && role != "GM" {
            return Err(Error::new("Only Owners and GMs can view invite codes"));
        }

        // Load all invites for the world
        let invites: Vec<WorldInvite> = world_invites::table
            .filter(world_invites::world_id.eq(world_id))
            .select(WorldInvite::as_select())
            .load::<WorldInvite>(&mut conn)
            .map_err(|e| Error::new(format!("Failed to load invites: {}", e)))?;

        Ok(invites
            .into_iter()
            .map(|invite| crate::graphql::mutations_invites::WorldInvitePayload {
                id: invite.id,
                world_id: invite.world_id,
                invite_code: invite.invite_code,
                max_uses: invite.max_uses,
                used_count: invite.used_count,
                expires_at: invite.expires_at.map(|dt| dt.to_string()),
                created_by: invite.created_by,
                created_at: invite.created_at.to_string(),
                updated_at: invite.updated_at.to_string(),
                status: format!("{}/{} uses", invite.used_count, invite.max_uses),
            })
            .collect())
    }

    /// Get all members of a world
    async fn world_members(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Vec<crate::graphql::mutations_invites::WorldMembershipPayload>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        // Verify user is a member of the world (any role can view members)
        let _member: Option<WorldMember> = world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(user_id))
            .select(WorldMember::as_select())
            .first::<WorldMember>(&mut conn)
            .optional()
            .map_err(|e| Error::new(format!("Database error: {}", e)))?;

        if _member.is_none() {
            return Err(Error::new("User is not a member of this world"));
        }

        // Load all members for the world
        let members: Vec<WorldMember> = world_members::table
            .filter(world_members::world_id.eq(world_id))
            .select(WorldMember::as_select())
            .load::<WorldMember>(&mut conn)
            .map_err(|e| Error::new(format!("Failed to load members: {}", e)))?;

        Ok(members
            .into_iter()
            .map(|member| crate::graphql::mutations_invites::WorldMembershipPayload {
                id: member.id,
                world_id: member.world_id,
                user_id: member.user_id,
                role: member.role,
                joined_at: member.joined_at.to_string(),
                created_at: member.created_at.to_string(),
                updated_at: member.updated_at.to_string(),
            })
            .collect())
    }

    /// Get a specific member's info
    async fn world_member(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        user_id: Uuid,
    ) -> GraphQLResult<Option<crate::graphql::mutations_invites::WorldMembershipPayload>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let caller_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        // Verify caller is a member of the world
        let _caller: Option<WorldMember> = world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(caller_id))
            .select(WorldMember::as_select())
            .first::<WorldMember>(&mut conn)
            .optional()
            .map_err(|e| Error::new(format!("Database error: {}", e)))?;

        if _caller.is_none() {
            return Err(Error::new("User is not a member of this world"));
        }

        // Load the specific member
        let member: Option<WorldMember> = world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(user_id))
            .select(WorldMember::as_select())
            .first::<WorldMember>(&mut conn)
            .optional()
            .map_err(|e| Error::new(format!("Database error: {}", e)))?;

        Ok(member.map(|member| crate::graphql::mutations_invites::WorldMembershipPayload {
            id: member.id,
            world_id: member.world_id,
            user_id: member.user_id,
            role: member.role,
            joined_at: member.joined_at.to_string(),
            created_at: member.created_at.to_string(),
            updated_at: member.updated_at.to_string(),
        }))
    }

    /// Get world info by invite code (for /join/:code landing page)
    async fn world_by_invite_code(
        &self,
        ctx: &Context<'_>,
        code: String,
    ) -> GraphQLResult<Option<WorldPreviewPayload>> {
        use crate::schema::worlds;

        let state = app_state(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        // Find the invite by code
        let invite: Option<WorldInvite> = world_invites::table
            .filter(world_invites::invite_code.eq(code))
            .select(WorldInvite::as_select())
            .first::<WorldInvite>(&mut conn)
            .optional()
            .map_err(|e| Error::new(format!("Database error: {}", e)))?;

        let invite = match invite {
            Some(inv) => inv,
            None => return Ok(None), // Code not found
        };

        // Check if invite is still valid
        if let Some(expires_at) = invite.expires_at {
            use chrono::Utc;
            if expires_at < Utc::now().naive_utc() {
                return Ok(None); // Invite expired
            }
        }

        if invite.used_count >= invite.max_uses {
            return Ok(None); // Invite exhausted
        }

        // Load the world info
        let world = worlds::table
            .find(invite.world_id)
            .select((worlds::id, worlds::name, worlds::description))
            .first::<(Uuid, String, Option<String>)>(&mut conn)
            .optional()
            .map_err(|e| Error::new(format!("Failed to load world: {}", e)))?;

        Ok(world.map(|(id, name, description)| WorldPreviewPayload {
            id: id.to_string(),
            name,
            description,
        }))
    }

    /// Check if current user is already a member of world via invite code
    async fn already_member(
        &self,
        ctx: &Context<'_>,
        code: String,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        // Find the invite by code
        let invite: Option<WorldInvite> = world_invites::table
            .filter(world_invites::invite_code.eq(code))
            .select(WorldInvite::as_select())
            .first::<WorldInvite>(&mut conn)
            .optional()
            .map_err(|e| Error::new(format!("Database error: {}", e)))?;

        let invite = invite.ok_or_else(|| Error::new("Invalid invite code"))?;

        // Check if user is already a member
        let is_member: bool = world_members::table
            .filter(world_members::world_id.eq(invite.world_id))
            .filter(world_members::user_id.eq(user_id))
            .count()
            .get_result::<i64>(&mut conn)
            .map(|count| count > 0)
            .map_err(|e| Error::new(format!("Database error: {}", e)))?;

        Ok(is_member)
    }
}

#[derive(async_graphql::SimpleObject)]
pub struct WorldPreviewPayload {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}
