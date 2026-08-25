//! Invite and world membership queries (Phase 4.10)

use async_graphql::Context;
use uuid::Uuid;

use crate::auth::world_membership::{WorldMembershipError, require_world_member};
use crate::graphql::*;
use crate::models::{WorldInvite, WorldMember};
use crate::schema::{world_invites, world_members, worlds};
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

    // Spec 027 (T035, FR-010): payloads are built by `from_row`, which derives
    // `state` and `remainingUses` rather than leaving each call site to
    // recompute them — the panel previously got only a `"3/10 uses"` string,
    // which cannot express revocation, so a revoked link rendered identically
    // to a working one.
    //
    // Revoked links are deliberately **included**: a GM should be able to see
    // what they retired. This listing stays world-scoped and DM-gated, so it
    // is not the cross-world enumeration FR-009 forbids.
    Ok(invites
        .iter()
        .map(crate::graphql::mutations_invites::WorldInvitePayload::from_row)
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

    let mut payloads: Vec<crate::graphql::mutations_invites::WorldMembershipPayload> = members
        .into_iter()
        .map(
            |member| crate::graphql::mutations_invites::WorldMembershipPayload {
                id: member.id,
                world_id: member.world_id,
                user_id: member.user_id,
                role: member.role,
                joined_at: member.joined_at.to_string(),
                created_at: member.created_at.to_string(),
                updated_at: member.updated_at.to_string(),
            },
        )
        .collect();

    // Spec 023 (quickstart.md §1: "Confirm the world's GM/Owner appears
    // in the list"): the same missing-row gap `require_world_member`
    // works around above also means the raw `world_members` load just
    // above can omit the world's own creator entirely, since
    // `create_world` never backfills a row for them. Synthesize an
    // Owner entry from `worlds.created_by`/`created_at` when no real row
    // for them exists, so the roster this feature's Players section
    // relies on actually includes the GM/Owner rather than silently
    // dropping them.
    let owner_already_listed = payloads.iter().any(|p| p.role == "Owner");
    if !owner_already_listed {
        let (created_by, created_at): (Uuid, chrono::NaiveDateTime) = worlds::table
            .filter(worlds::id.eq(world_id))
            .select((worlds::created_by, worlds::created_at))
            .first(&mut conn)
            .map_err(|e| Error::new(format!("Failed to load world: {}", e)))?;
        if !payloads.iter().any(|p| p.user_id == created_by) {
            payloads.insert(
                0,
                crate::graphql::mutations_invites::WorldMembershipPayload {
                    id: world_id,
                    world_id,
                    user_id: created_by,
                    role: "Owner".to_string(),
                    joined_at: created_at.to_string(),
                    created_at: created_at.to_string(),
                    updated_at: created_at.to_string(),
                },
            );
        }
    }

    Ok(payloads)
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

    Ok(member.map(
        |member| crate::graphql::mutations_invites::WorldMembershipPayload {
            id: member.id,
            world_id: member.world_id,
            user_id: member.user_id,
            role: member.role,
            joined_at: member.joined_at.to_string(),
            created_at: member.created_at.to_string(),
            updated_at: member.updated_at.to_string(),
        },
    ))
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

    // Spec 027 (FR-011 / SC-005): this preview resolves a code **without**
    // joining, so its validity check has to be exactly as strict as the join
    // path's. Anything it lets through it discloses — the world's name and
    // description — to whoever holds the code.
    //
    // Revocation was missing here entirely: a revoked code still returned the
    // world, which would have made `joinWorld`'s uniform failure pointless.
    // Someone holding a killed link could still confirm it was real and see
    // what it pointed at.
    if invite.revoked {
        return Ok(None);
    }

    if let Some(expires_at) = invite.expires_at {
        use chrono::Utc;
        if expires_at < Utc::now().naive_utc() {
            return Ok(None); // Invite expired
        }
    }

    // `max_uses == 0` means unlimited, matching the join predicate and
    // `WorldInvite::is_valid`. Without that guard an uncapped link read as
    // exhausted immediately, since `0 >= 0`.
    if invite.max_uses > 0 && invite.used_count >= invite.max_uses {
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

    // Spec 027 (FR-011 / SC-005): an unknown code answers `false`, not an
    // error. You cannot be a member via a code that does not exist, so `false`
    // is both correct and non-disclosing.
    //
    // This previously returned `Error::new("Invalid invite code")`. The join
    // page requests `worldByInviteCode` and `alreadyMember` in one operation,
    // so an unknown code produced a GraphQL error while a revoked one — whose
    // row exists — returned cleanly. The two rendered differently, which made
    // the whole uniform-failure design pointless: a visitor could tell a code
    // that was never real from one that had been killed. Caught by
    // `access-links.spec.ts` comparing the two rendered pages rather than
    // checking each in isolation.
    let Some(invite) = invite else {
        return Ok(false);
    };

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
    async fn already_member(&self, ctx: &Context<'_>, code: String) -> GraphQLResult<bool> {
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
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };
    use diesel::prelude::*;

    fn insert_test_invite(
        conn: &mut diesel::PgConnection,
        world_id: uuid::Uuid,
        created_by: uuid::Uuid,
        max_uses: i32,
        used_count: i32,
        expires_at: Option<chrono::NaiveDateTime>,
    ) -> String {
        insert_test_invite_with_revocation(
            conn, world_id, created_by, max_uses, used_count, expires_at, false,
        )
    }

    fn insert_test_invite_with_revocation(
        conn: &mut diesel::PgConnection,
        world_id: uuid::Uuid,
        created_by: uuid::Uuid,
        max_uses: i32,
        used_count: i32,
        expires_at: Option<chrono::NaiveDateTime>,
        revoked: bool,
    ) -> String {
        let now = chrono::Utc::now().naive_utc();
        // Spec 027: was `format!("T{}", now_v7()...)`. A v7 UUID front-loads a
        // millisecond timestamp, so invites created back-to-back shared a
        // prefix and collided on `world_invites_invite_code_key` — the exact
        // defect spec 005 fixed in production code, still living here. Uses
        // the shared v4-based generator now.
        let code = crate::graphql::share_codes::generate_link_code();
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
            revoked,
            rotated_from: None,
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

        // Spec 023 (quickstart.md §1): with no explicit world_members row
        // of their own, the owner must still be synthesized into the
        // returned list rather than silently omitted.
        assert_eq!(members.len(), 1, "the synthesized owner entry must be present");
        assert_eq!(members[0].user_id, owner_id);
        assert_eq!(members[0].role, "Owner");
    }

    #[tokio::test]
    async fn world_members_does_not_duplicate_an_owner_with_a_real_row() {
        // Spec 023: if the owner *does* have a real `world_members` row
        // (e.g. backfilled some other way), the synthesized fallback
        // entry must not also be added alongside it.
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        insert_test_world_member(&mut conn, world_id, owner_id, "Owner");
        drop(conn);

        let members = world_members_impl(&state, owner_id, world_id)
            .await
            .expect("the world's own owner must be able to list its members");

        assert_eq!(members.len(), 1, "no duplicate synthesized owner entry");
        assert_eq!(members[0].user_id, owner_id);
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

        assert!(
            result.is_err(),
            "a non-member must not be able to list a world's members"
        );
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

        assert!(
            result.is_none(),
            "an expired invite must not resolve a world"
        );
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

        assert!(
            result.is_none(),
            "an exhausted invite (used_count >= max_uses) must not resolve a world"
        );
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
        assert!(
            is_member,
            "an already-accepted member must be reported as already a member"
        );

        let not_yet = already_member_impl(&state, owner_id, &code).await;
        // owner has no world_members row, so is_member is computed purely
        // from world_members — the owner fallback does not apply here.
        assert!(matches!(not_yet, Ok(false)));
    }

    /// Spec 027 (FR-011 / SC-005): the preview must not disclose a world
    /// behind a dead code. Revocation was previously unchecked here, so a
    /// revoked link still returned the world's name — which would have made
    /// `joinWorld`'s uniform failure pointless, since the holder of a killed
    /// link could confirm it was real and see what it pointed at.
    #[tokio::test]
    async fn world_by_invite_code_hides_the_world_behind_every_dead_code() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);

        let past = chrono::Utc::now().naive_utc() - chrono::Duration::days(1);
        let expired = insert_test_invite(&mut conn, world_id, owner_id, 5, 0, Some(past));
        let exhausted = insert_test_invite(&mut conn, world_id, owner_id, 3, 3, None);
        let revoked =
            insert_test_invite_with_revocation(&mut conn, world_id, owner_id, 5, 0, None, true);
        drop(conn);

        for (label, code) in [
            ("expired", expired),
            ("exhausted", exhausted),
            ("revoked", revoked),
        ] {
            let result = world_by_invite_code_impl(&state, &code)
                .await
                .expect("query should not error");
            assert!(
                result.is_none(),
                "a {label} code must not disclose the world it points at"
            );
        }
    }

    /// FR-010: revoked links stay listed for their GM — they need to see what
    /// they retired. This is world-scoped and DM-gated, so it is not the
    /// cross-world enumeration FR-009 forbids.
    #[tokio::test]
    async fn world_invites_lists_revoked_links_with_their_state() {
        use crate::graphql::mutations_invites::WorldAccessLinkState;

        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let active = insert_test_invite(&mut conn, world_id, owner_id, 5, 1, None);
        let revoked =
            insert_test_invite_with_revocation(&mut conn, world_id, owner_id, 5, 0, None, true);
        drop(conn);

        let listed = world_invites_impl(&state, owner_id, world_id)
            .await
            .expect("a DM may list their own world's links");

        let active_row = listed
            .iter()
            .find(|i| i.invite_code == active)
            .expect("the active link must be listed");
        assert_eq!(active_row.state, WorldAccessLinkState::Active);
        assert_eq!(active_row.remaining_uses, Some(4));

        let revoked_row = listed
            .iter()
            .find(|i| i.invite_code == revoked)
            .expect("a revoked link must remain visible to its GM");
        assert_eq!(revoked_row.state, WorldAccessLinkState::Revoked);
    }

    /// FR-011 / SC-005: `alreadyMember` must not become the side channel that
    /// distinguishes a never-issued code from a revoked one. The join page
    /// requests it alongside `worldByInviteCode` in a single operation, so an
    /// error here changes what the visitor sees.
    #[tokio::test]
    async fn already_member_answers_false_for_an_unknown_code_rather_than_erroring() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let revoked =
            insert_test_invite_with_revocation(&mut conn, world_id, owner_id, 5, 0, None, true);
        let visitor = insert_test_user(&mut conn);
        drop(conn);

        let unknown = already_member_impl(&state, visitor, "ZZZZZZZZZZZZZZZZZZZZ")
            .await
            .expect("an unknown code must answer, not error");
        let for_revoked = already_member_impl(&state, visitor, &revoked)
            .await
            .expect("a revoked code must answer, not error");

        assert!(!unknown);
        assert_eq!(
            unknown, for_revoked,
            "an unknown code and a revoked one must be indistinguishable here"
        );
    }
}
