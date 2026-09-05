//! Changing and ending a membership, once someone is already in a world.
//!
//! Split out of `mutations_invites.rs`, which is otherwise about invite codes
//! — issuing them, spending them, rotating and revoking them. These two are
//! the other side: they act on a `world_members` row that already exists and
//! never read an invite at all.
//!
//! `remove_member_impl` carries the load-bearing comment of the pair: there is
//! no FK cascade from `world_members` to the permission tables, so every
//! content type's grants are cleared by hand, and spec 027 exists because a
//! fifth table was added without a fifth block.

use async_graphql::{Error, Result as GraphQLResult};
use chrono::Utc;
use diesel::prelude::*;
use std::str::FromStr;
use uuid::Uuid;

use super::{
    EVENT_CODE_MEMBER_REMOVED, EVENT_CODE_MEMBER_ROLE_CHANGED, UpdateMemberRoleInput,
    WorldMembershipPayload, record_world_event,
};
use crate::auth::world_membership::require_world_member;
use crate::models::WorldMember;
use crate::schema::world_members;
use crate::state::AppState;
use thunderforge_core::models::invites::WorldMemberRole;

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
    let caller_role =
        WorldMemberRole::from_str(&caller_role_str).unwrap_or(WorldMemberRole::Player);

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
    let caller_role =
        WorldMemberRole::from_str(&caller_role_str).unwrap_or(WorldMemberRole::Player);

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

    // Spec 027 (T058, FR-018): one call replaces four hand-written cleanup
    // blocks — actors, items, lore entries and abilities.
    //
    // There is no FK from `world_members` to the grant tables (the
    // relationship runs through `world_id` on the parent content table), so a
    // removed member's grants do not cascade and must be deleted explicitly.
    // That was previously written out once per content type, and spec 025
    // added a fourth type without adding a fourth block — a removed member
    // kept their ability grants and silently regained them on readmission.
    //
    // The set of types walked is now the declaration in
    // `auth::permissioned_entities` itself, so a content type cannot be
    // declared and then forgotten here.
    crate::auth::permissioned_entities::purge_member_grants(&mut conn, world_id, user_id)
        .map_err(|e| Error::new(format!("Failed to clean up content permissions: {}", e)))?;

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

#[cfg(test)]
#[path = "mutations_invites_membership_tests.rs"]
mod tests;
