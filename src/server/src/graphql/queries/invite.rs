//! Invite and world membership queries (Phase 4.10)

use async_graphql::Context;
use diesel::prelude::*;
use uuid::Uuid;

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

        // Verify user is Owner/GM of the world
        let member: Option<WorldMember> = world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(user_id))
            .select(WorldMember::as_select())
            .first::<WorldMember>(&mut conn)
            .optional()
            .map_err(|e| Error::new(format!("Database error: {}", e)))?;

        let member = member.ok_or_else(|| Error::new("User is not a member of this world"))?;

        if member.role != "Owner" && member.role != "GM" {
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
}
