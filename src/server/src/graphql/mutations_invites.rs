//! GraphQL mutations for campaign invites and world membership (Phase 4.10)

use async_graphql::{Context, Error, ErrorExtensions, Result as GraphQLResult};
use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::{WorldMembershipError, require_world_member};
use crate::auth_middleware::AuthenticatedUser;
use crate::graphql::share_codes::generate_link_code;
use crate::models::{NewWorldInvite, NewWorldMember, WorldInvite};
use crate::schema::world_events;
use crate::schema::world_invites;
use crate::schema::world_members;
use crate::state::AppState;

// Event codes for world_events audit trail
const EVENT_CODE_INVITE_CREATED: i32 = 2;
const EVENT_CODE_MEMBER_JOINED: i32 = 3;
const EVENT_CODE_MEMBER_ROLE_CHANGED: i32 = 4;
const EVENT_CODE_MEMBER_REMOVED: i32 = 5;

#[path = "mutations_invites_types.rs"]
pub mod types;
pub use types::*;

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

    let submitted_code = input.invite_code.clone();

    // Spec 027 (T042, US4-2): the already-a-member check runs FIRST, before any
    // use is consumed. A player who clicks a link twice must not burn a use of
    // their GM's cap on the second click. It is also not a failure — it needs
    // its own message, and it requires a *valid* code, so it reveals nothing a
    // uniform response would have protected.
    let already_a_member = {
        let code = submitted_code.clone();
        let mut probe_conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        tokio::task::spawn_blocking(move || {
            world_invites::table
                .inner_join(
                    world_members::table.on(world_members::world_id.eq(world_invites::world_id)),
                )
                .filter(world_invites::invite_code.eq(code))
                .filter(world_members::user_id.eq(user_id))
                .select(world_members::id)
                .first::<Uuid>(&mut probe_conn)
                .optional()
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|e| Error::new(format!("Database error: {}", e)))?
    };

    if already_a_member.is_some() {
        return Err(Error::new(ALREADY_A_MEMBER_MESSAGE));
    }

    // Spec 027 (T019/T020, FR-011/FR-012): validate-and-consume atomically,
    // then create the membership in the same transaction.
    //
    // This replaces a read, an in-memory `is_valid()`, and a write-back of a
    // computed count. That sequence lost updates: two joins racing for the
    // last use both read `used_count = N`, both computed `N + 1`, and both
    // wrote it — admitting two members against one remaining use. Carrying the
    // whole validity predicate in the UPDATE's WHERE clause makes the check
    // and the increment one indivisible step.
    //
    // It also delivers FR-011's uniform failure for free: zero rows updated
    // means unusable, and the reason — unknown, revoked, expired, or
    // exhausted — is never distinguished, here or to the caller.
    let mut txn_conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let code_for_txn = submitted_code.clone();

    let new_member = tokio::task::spawn_blocking(move || {
        txn_conn.transaction::<NewWorldMember, diesel::result::Error, _>(|conn| {
            let now = Utc::now().naive_utc();

            let consumed: Option<(Uuid, Uuid)> = diesel::update(
                world_invites::table
                    .filter(world_invites::invite_code.eq(&code_for_txn))
                    .filter(world_invites::revoked.eq(false))
                    .filter(
                        world_invites::expires_at
                            .is_null()
                            .or(world_invites::expires_at.gt(now)),
                    )
                    .filter(
                        world_invites::max_uses
                            .eq(0)
                            .or(world_invites::used_count.lt(world_invites::max_uses)),
                    ),
            )
            .set((
                world_invites::used_count.eq(world_invites::used_count + 1),
                world_invites::updated_at.eq(now),
            ))
            .returning((world_invites::id, world_invites::world_id))
            .get_result::<(Uuid, Uuid)>(conn)
            .optional()?;

            // No row matched the predicate: the link is unusable. Rolling back
            // with NotFound keeps the caller from learning which condition
            // applied.
            let (_invite_id, world_id) = consumed.ok_or(diesel::result::Error::NotFound)?;

            let new_member = NewWorldMember {
                id: Uuid::now_v7(),
                world_id,
                user_id,
                role: "Player".to_string(),
                joined_at: now,
                created_at: now,
                updated_at: now,
            };

            // Inside the transaction: if this fails, the use is returned.
            diesel::insert_into(world_members::table)
                .values(&new_member)
                .execute(conn)?;

            Ok(new_member)
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new(LINK_UNAVAILABLE_MESSAGE))?;

    let world_id = new_member.world_id;

    // Record event for audit trail and real-time sync. Deliberately outside
    // the transaction: a failure to announce the join must not undo it.
    //
    // The invite code is NOT included in the payload. It used to be, but that
    // put a live credential into an audit row readable by anyone who can read
    // world events — and now that a code can be rotated to contain a leak,
    // copying it into a second place defeats the point.
    let event_payload = serde_json::json!({
        "user_id": new_member.user_id,
        "role": new_member.role,
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

/// Spec 027 (FR-011 / SC-005): the single message every unusable link gets.
///
/// Unknown, revoked, expired, and exhausted codes are indistinguishable —
/// identical text, identical shape. Possessing a dead code must reveal nothing
/// about whether it was ever real or what world it belonged to. Wording
/// deliberately matches `load_active_share`'s, so invites and content shares
/// fail the same way.
pub const LINK_UNAVAILABLE_MESSAGE: &str = "This invite link is no longer available.";

/// Distinct from the uniform failure on purpose: reaching this requires a
/// *valid* code, so it leaks nothing an attacker could not already establish,
/// and a player who clicks their own link twice deserves a message that tells
/// them what actually happened.
pub const ALREADY_A_MEMBER_MESSAGE: &str = "You are already a member of this world.";

/// Spec 027 (T021, FR-002): retire a link permanently, with no replacement.
///
/// Idempotent — revoking an already-revoked link succeeds and returns it
/// unchanged, so a double-click is not an error. Has no effect on anyone who
/// already joined (FR-005).
pub async fn revoke_invite_code_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    invite_id: Uuid,
) -> GraphQLResult<WorldInvitePayload> {
    let world_id = world_id_of_invite(state, invite_id).await?;
    require_dm_of_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let updated = tokio::task::spawn_blocking(move || {
        diesel::update(world_invites::table.find(invite_id))
            .set((
                world_invites::revoked.eq(true),
                world_invites::updated_at.eq(Utc::now().naive_utc()),
            ))
            .returning(WorldInvite::as_select())
            .get_result::<WorldInvite>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| Error::new(format!("Failed to revoke invite: {}", e)))?;

    Ok(WorldInvitePayload::from_row(&updated))
}

/// Spec 027 (T022, FR-003/FR-004/FR-014): retire a link and issue its
/// replacement in one atomic action. Returns the **new** link.
///
/// The replacement inherits the retired link's cap and expiry with its count
/// reset to zero — a refresh yields "this link, but new". Note the consequence
/// recorded in ADR-050: because the count resets, a DM can rotate a 1-use link
/// indefinitely. That is accepted (a DM can already create unlimited links),
/// which is why the cap is a convenience control and must never be described
/// to GMs as a security boundary.
///
/// Rotation is allowed on an expired or exhausted link — a GM can always
/// revive a dead link. It is refused on an already-revoked one, which would
/// otherwise produce two replacements for a single original.
pub async fn rotate_invite_code_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    invite_id: Uuid,
) -> GraphQLResult<WorldInvitePayload> {
    let world_id = world_id_of_invite(state, invite_id).await?;
    require_dm_of_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let new_code = generate_link_code();

    let replacement = tokio::task::spawn_blocking(move || {
        conn.transaction::<WorldInvite, diesel::result::Error, _>(|conn| {
            let now = Utc::now().naive_utc();

            // Retire the source, guarded on it not already being retired. Zero
            // rows means it was — abort rather than mint a second replacement.
            let retired: Option<WorldInvite> = diesel::update(
                world_invites::table
                    .find(invite_id)
                    .filter(world_invites::revoked.eq(false)),
            )
            .set((
                world_invites::revoked.eq(true),
                world_invites::updated_at.eq(now),
            ))
            .returning(WorldInvite::as_select())
            .get_result::<WorldInvite>(conn)
            .optional()?;

            let retired = retired.ok_or(diesel::result::Error::NotFound)?;

            // Issue the replacement in the same transaction. FR-004: a failure
            // here rolls the retirement back, leaving exactly one usable link.
            let replacement = NewWorldInvite {
                id: Uuid::now_v7(),
                world_id: retired.world_id,
                invite_code: new_code,
                max_uses: retired.max_uses,
                used_count: 0,
                expires_at: rotated_expiry(&retired, now),
                // The rotating GM, who need not be the original creator.
                created_by: user_id,
                created_at: now,
                updated_at: now,
                revoked: false,
                rotated_from: Some(retired.id),
            };

            diesel::insert_into(world_invites::table)
                .values(&replacement)
                .returning(WorldInvite::as_select())
                .get_result::<WorldInvite>(conn)
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| match e {
        diesel::result::Error::NotFound => {
            Error::new("This link has already been revoked and cannot be rotated.")
        }
        other => Error::new(format!("Failed to rotate invite: {}", other)),
    })?;

    Ok(WorldInvitePayload::from_row(&replacement))
}

/// Spec 027: the expiry a rotation's replacement should carry.
///
/// # Why this is not a plain copy
///
/// FR-014 says the replacement inherits the retired link's expiry, and US1
/// scenario 4 says rotating an **expired** link yields a **usable** one.
/// Copying the expiry verbatim satisfies the first and breaks the second: the
/// replacement is born already dead. Implementation surfaced the conflict;
/// this resolves it.
///
/// The resolution follows the same principle already settled for the use cap.
/// Rotation resets `used_count` because uses-spent is *consumed state*, while
/// the cap is the GM's *setting*. Elapsed time is consumed state by exactly the
/// same logic, and the chosen lifetime is the setting. So a rotated link keeps
/// the lifetime the GM picked — "this link lasts a week" — measured again from
/// now. Cap resets, clock resets, and both settings survive.
///
/// A link with no expiry still has none. A degenerate row whose expiry does not
/// follow its creation keeps its original absolute expiry rather than having a
/// nonsensical duration projected forward.
fn rotated_expiry(
    retired: &WorldInvite,
    now: chrono::NaiveDateTime,
) -> Option<chrono::NaiveDateTime> {
    let expires_at = retired.expires_at?;
    let lifetime = expires_at - retired.created_at;
    if lifetime > chrono::Duration::zero() {
        Some(now + lifetime)
    } else {
        Some(expires_at)
    }
}

/// Resolves the world a link belongs to, so authorization can be checked
/// against it. A missing link is reported as unavailable rather than "not
/// found", keeping this consistent with FR-011.
async fn world_id_of_invite(state: &AppState, invite_id: Uuid) -> GraphQLResult<Uuid> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_invites::table
            .find(invite_id)
            .select(world_invites::world_id)
            .first::<Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| Error::new(format!("Database error: {}", e)))?
    .ok_or_else(|| Error::new(LINK_UNAVAILABLE_MESSAGE))
}

/// Spec 027 (FR-008): only a world's DM may create, revoke, or rotate its
/// links.
async fn require_dm_of_world(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<()> {
    if crate::auth::world_membership::is_dm_of_world(state, user_id, is_admin, world_id).await? {
        Ok(())
    } else {
        Err(
            Error::new("Only Owners and GMs can manage this world's invite links")
                .extend_with(|_, ext| ext.set("code", "FORBIDDEN")),
        )
    }
}

#[path = "mutations_invites_membership.rs"]
pub mod membership;
pub use membership::*;

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

    /// Spec 027 (FR-002): permanently retire an invite link, with no
    /// replacement. Owner/GM only. Idempotent.
    pub async fn revoke_invite_code(
        &self,
        ctx: &Context<'_>,
        invite_id: Uuid,
    ) -> GraphQLResult<WorldInvitePayload> {
        let state = get_app_state(ctx)?;
        let auth_user = get_authenticated_user(ctx)?;
        revoke_invite_code_impl(&state, auth_user.user_id, auth_user.is_admin, invite_id).await
    }

    /// Spec 027 (FR-003): retire an invite link and issue its replacement in
    /// one atomic action — the old code stops working immediately. Owner/GM
    /// only. Returns the **new** link.
    pub async fn rotate_invite_code(
        &self,
        ctx: &Context<'_>,
        invite_id: Uuid,
    ) -> GraphQLResult<WorldInvitePayload> {
        let state = get_app_state(ctx)?;
        let auth_user = get_authenticated_user(ctx)?;
        rotate_invite_code_impl(&state, auth_user.user_id, auth_user.is_admin, invite_id).await
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
#[path = "mutations_invites_tests.rs"]
mod tests;
