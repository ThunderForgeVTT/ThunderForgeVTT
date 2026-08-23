//! Invite and world membership queries (Phase 4.10)

use async_graphql::Context;
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::{require_world_member, WorldMembershipError};
use crate::graphql::*;
use crate::models::{WorldInvite, WorldMember};
use crate::schema::{world_invites, world_members};
use crate::state::AppState;

/// Testable core of `InviteQuery::world_invites` (see `actor.rs`'s
/// `_impl` convention).
pub async fn world_invites_impl(
    state: &AppState,
    user_id: Uuid,
    world_id: Uuid,
) -> GraphQLResult<Vec<crate::graphql::mutations_invites::WorldInvitePayload>> {
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

/// Testable core of `InviteQuery::world_members`.
pub async fn world_members_impl(
    state: &AppState,
    user_id: Uuid,
    world_id: Uuid,
) -> GraphQLResult<Vec<crate::graphql::mutations_invites::WorldMembershipPayload>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    // Verify user is a member of the world (any role can view members).
    // Spec 010 fix: this used to be an inline `world_members` lookup
    // with no fallback for a world's own owner — who has no
    // `world_members` row (`create_world` doesn't insert one; see
    // `require_world_member`'s doc comment) — so a world's own DM
    // could never list their own world's membership (discovered while
    // building the actor ownership block, which depends on this query
    // via `useWorldMembers`'s RxDB replication). Routing through
    // `require_world_member` applies the same owner fallback every
    // other world-scoped query/mutation already uses (e.g.
    // `generate_invite_code`).
    require_world_member(&mut conn, user_id, world_id).map_err(|e| match e {
        WorldMembershipError::NotAMember => Error::new("User is not a member of this world"),
        WorldMembershipError::Database(msg) => Error::new(format!("Database error: {}", msg)),
    })?;

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

/// Testable core of `InviteQuery::world_member`.
pub async fn world_member_impl(
    state: &AppState,
    caller_id: Uuid,
    world_id: Uuid,
    user_id: Uuid,
) -> GraphQLResult<Option<crate::graphql::mutations_invites::WorldMembershipPayload>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    // Verify caller is a member of the world (spec 010 fix — see
    // `world_members_impl`'s own comment above on the missing owner
    // fallback this inline check used to have).
    require_world_member(&mut conn, caller_id, world_id).map_err(|e| match e {
        WorldMembershipError::NotAMember => Error::new("User is not a member of this world"),
        WorldMembershipError::Database(msg) => Error::new(format!("Database error: {}", msg)),
    })?;

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

/// Testable core of `InviteQuery::world_by_invite_code`.
pub async fn world_by_invite_code_impl(
    state: &AppState,
    code: &str,
) -> GraphQLResult<Option<WorldPreviewPayload>> {
    use crate::schema::worlds;

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

/// Testable core of `InviteQuery::already_member`.
pub async fn already_member_impl(
    state: &AppState,
    user_id: Uuid,
    code: &str,
) -> GraphQLResult<bool> {
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
        world_invites_impl(state, auth_user.user_id, world_id).await
    }

    /// Get all members of a world
    async fn world_members(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Vec<crate::graphql::mutations_invites::WorldMembershipPayload>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        world_members_impl(state, auth_user.user_id, world_id).await
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
        world_member_impl(state, auth_user.user_id, world_id, user_id).await
    }

    /// Get world info by invite code (for /join/:code landing page)
    async fn world_by_invite_code(
        &self,
        ctx: &Context<'_>,
        code: String,
    ) -> GraphQLResult<Option<WorldPreviewPayload>> {
        let state = app_state(ctx)?;
        world_by_invite_code_impl(state, &code).await
    }

    /// Check if current user is already a member of world via invite code
    async fn already_member(
        &self,
        ctx: &Context<'_>,
        code: String,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        already_member_impl(state, auth_user.user_id, &code).await
    }
}

#[derive(async_graphql::SimpleObject)]
pub struct WorldPreviewPayload {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{
        already_member_impl, world_by_invite_code_impl, world_invites_impl, world_member_impl,
        world_members_impl,
    };
    use crate::models::NewWorldInvite;
    use crate::schema::world_invites;
    use crate::test_support::{insert_test_user, insert_test_world, insert_test_world_member, test_app_state};
    use diesel::prelude::*;

    fn insert_test_invite(
        conn: &mut diesel::PgConnection,
        world_id: uuid::Uuid,
        created_by: uuid::Uuid,
        max_uses: i32,
        used_count: i32,
        expires_at: Option<chrono::NaiveDateTime>,
    ) -> String {
        let now = chrono::Utc::now().naive_utc();
        let code = format!("T{}", &uuid::Uuid::now_v7().simple().to_string()[..20]);
        let invite = NewWorldInvite {
            id: uuid::Uuid::now_v7(),
            world_id,
            invite_code: code.clone(),
            max_uses,
            used_count,
            expires_at,
            created_by,
            created_at: now,
            updated_at: now,
        };
        diesel::insert_into(world_invites::table)
            .values(&invite)
            .execute(conn)
            .expect("failed to insert test invite");
        code
    }

    #[tokio::test]
    async fn world_invites_rejects_a_plain_player() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let player_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let result = world_invites_impl(&state, player_id, world_id).await;

        assert!(
            result.is_err(),
            "a plain Player must not be able to view invite codes"
        );
    }

    #[tokio::test]
    async fn world_invites_allows_the_owner_via_created_by_fallback() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_invite(&mut conn, world_id, owner_id, 5, 0, None);
        drop(conn);

        let invites = world_invites_impl(&state, owner_id, world_id)
            .await
            .expect("the world's own owner must be able to list its invites, even with no world_members row");

        assert_eq!(invites.len(), 1);
    }

    #[tokio::test]
    async fn world_members_allows_the_owner_via_created_by_fallback() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let members = world_members_impl(&state, owner_id, world_id)
            .await
            .expect("the world's own owner must be able to list its members, even with no world_members row");

        assert!(members.is_empty(), "owner has no explicit world_members row of their own");
    }

    #[tokio::test]
    async fn world_members_rejects_non_member() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let outsider_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let result = world_members_impl(&state, outsider_id, world_id).await;

        assert!(result.is_err(), "a non-member must not be able to list a world's members");
    }

    #[tokio::test]
    async fn world_member_returns_the_specific_row() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let player_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let member = world_member_impl(&state, owner_id, world_id, player_id)
            .await
            .expect("owner should be able to look up a member")
            .expect("the player's row should exist");

        assert_eq!(member.role, "Player");
    }

    #[tokio::test]
    async fn world_by_invite_code_returns_none_for_an_expired_invite() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let past = chrono::Utc::now().naive_utc() - chrono::Duration::hours(1);
        let code = insert_test_invite(&mut conn, world_id, owner_id, 5, 0, Some(past));
        drop(conn);

        let result = world_by_invite_code_impl(&state, &code)
            .await
            .expect("query itself should not error");

        assert!(result.is_none(), "an expired invite must not resolve a world");
    }

    #[tokio::test]
    async fn world_by_invite_code_returns_none_for_an_exhausted_invite() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let code = insert_test_invite(&mut conn, world_id, owner_id, 3, 3, None);
        drop(conn);

        let result = world_by_invite_code_impl(&state, &code)
            .await
            .expect("query itself should not error");

        assert!(result.is_none(), "an exhausted invite (used_count >= max_uses) must not resolve a world");
    }

    #[tokio::test]
    async fn world_by_invite_code_returns_world_for_a_valid_invite() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let code = insert_test_invite(&mut conn, world_id, owner_id, 5, 1, None);
        drop(conn);

        let result = world_by_invite_code_impl(&state, &code)
            .await
            .expect("query itself should not error")
            .expect("a valid, non-exhausted invite must resolve its world");

        assert_eq!(result.id, world_id.to_string());
    }

    #[tokio::test]
    async fn already_member_reflects_membership_state() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let player_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let code = insert_test_invite(&mut conn, world_id, owner_id, 5, 0, None);
        drop(conn);

        let is_member = already_member_impl(&state, player_id, &code)
            .await
            .expect("query should not error");
        assert!(is_member, "an already-accepted member must be reported as already a member");

        let not_yet = already_member_impl(&state, owner_id, &code).await;
        // owner has no world_members row, so is_member is computed purely
        // from world_members — the owner fallback does not apply here.
        assert!(matches!(not_yet, Ok(false)));
    }
}
