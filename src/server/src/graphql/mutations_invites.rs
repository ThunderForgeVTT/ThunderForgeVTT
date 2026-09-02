//! GraphQL mutations for campaign invites and world membership (Phase 4.10)

use async_graphql::{
    Context, Error, ErrorExtensions, InputObject, Result as GraphQLResult, SimpleObject,
};
use chrono::Utc;
use diesel::prelude::*;
use std::str::FromStr;
use uuid::Uuid;

use crate::auth::world_membership::{WorldMembershipError, require_world_member};
use crate::auth_middleware::AuthenticatedUser;
use crate::graphql::share_codes::generate_link_code;
use crate::models::{NewWorldInvite, NewWorldMember, WorldInvite, WorldMember};
use crate::schema::world_events;
use crate::schema::world_invites;
use crate::schema::world_members;
use crate::state::AppState;
use thunderforge_core::models::invites::WorldMemberRole;

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
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

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

    // ===== Spec 027 US1: revoke and rotate =====

    fn load_invite(conn: &mut PgConnection, id: Uuid) -> WorldInvite {
        world_invites::table
            .find(id)
            .select(WorldInvite::as_select())
            .first(conn)
            .expect("invite row must exist")
    }

    /// FR-003 / SC-001: the retired code fails on its very next use, and the
    /// replacement works. This is the whole point of the feature.
    #[tokio::test]
    async fn rotating_kills_the_old_code_immediately_and_issues_a_working_one() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let (invite_id, old_code) = insert_test_invite(&mut conn, world_id, owner_id, 10, 0, None);
        drop(conn);

        // Control: the code works before rotation.
        let first_joiner = {
            let mut conn = state.db_pool.get().unwrap();
            insert_test_user(&mut conn)
        };
        join_world_impl(
            &state,
            first_joiner,
            JoinWorldInput {
                invite_code: old_code.clone(),
            },
        )
        .await
        .expect("the code must work before rotation — otherwise this proves nothing");

        let replacement = rotate_invite_code_impl(&state, owner_id, false, invite_id)
            .await
            .expect("a DM must be able to rotate their world's link");

        assert_ne!(
            replacement.invite_code, old_code,
            "a new code must be issued"
        );
        assert_eq!(replacement.state, WorldAccessLinkState::Active);

        // The retired code fails on its next use, with no grace window.
        let second_joiner = {
            let mut conn = state.db_pool.get().unwrap();
            insert_test_user(&mut conn)
        };
        let refused = join_world_impl(
            &state,
            second_joiner,
            JoinWorldInput {
                invite_code: old_code,
            },
        )
        .await;
        assert!(
            refused.is_err(),
            "the retired code must fail on its very next use (SC-001)"
        );

        // The replacement works.
        let third_joiner = {
            let mut conn = state.db_pool.get().unwrap();
            insert_test_user(&mut conn)
        };
        join_world_impl(
            &state,
            third_joiner,
            JoinWorldInput {
                invite_code: replacement.invite_code,
            },
        )
        .await
        .expect("the replacement code must work");
    }

    /// FR-014: the replacement is a clean instance of the same link — same
    /// cap, same expiry, count back at zero.
    #[tokio::test]
    async fn rotation_inherits_cap_and_expiry_but_resets_the_count() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let expiry = Utc::now().naive_utc() + chrono::Duration::days(3);
        let (invite_id, _) = insert_test_invite(&mut conn, world_id, owner_id, 10, 3, Some(expiry));
        drop(conn);

        let replacement = rotate_invite_code_impl(&state, owner_id, false, invite_id)
            .await
            .expect("rotation should succeed");

        assert_eq!(replacement.max_uses, 10, "cap must be inherited");
        assert_eq!(replacement.used_count, 0, "count must reset (FR-014)");
        assert_eq!(replacement.remaining_uses, Some(10));

        // The GM chose a ~3-day lifetime; the replacement carries that same
        // lifetime measured from now (see `rotated_expiry`). The source was
        // created moments ago in this test, so the new expiry lands within a
        // few seconds of the original — asserted as a window rather than an
        // equality, since Postgres stores microseconds while chrono carries
        // nanoseconds.
        let new_expiry = chrono::NaiveDateTime::parse_from_str(
            replacement
                .expires_at
                .as_ref()
                .expect("an expiring link must rotate into an expiring link"),
            "%Y-%m-%d %H:%M:%S%.f",
        )
        .expect("expiry must round-trip as a timestamp");
        let drift = (new_expiry - expiry).num_seconds().abs();
        assert!(
            drift <= 5,
            "the chosen lifetime must be preserved; drifted {drift}s"
        );
        assert!(
            new_expiry > Utc::now().naive_utc(),
            "a rotated link must not be born expired"
        );

        assert_eq!(
            replacement.rotated_from,
            Some(invite_id),
            "the replacement must record what it replaced"
        );

        let mut conn = state.db_pool.get().unwrap();
        assert!(
            load_invite(&mut conn, invite_id).revoked,
            "the source link must be retired by the same action"
        );
    }

    /// FR-005: rotation governs future joins only. Anyone already admitted
    /// stays — it is not a retroactive removal.
    #[tokio::test]
    async fn rotation_leaves_existing_members_untouched() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let (invite_id, code) = insert_test_invite(&mut conn, world_id, owner_id, 10, 0, None);
        let joiner = insert_test_user(&mut conn);
        drop(conn);

        join_world_impl(&state, joiner, JoinWorldInput { invite_code: code })
            .await
            .expect("join should succeed");

        rotate_invite_code_impl(&state, owner_id, false, invite_id)
            .await
            .expect("rotation should succeed");

        let mut conn = state.db_pool.get().unwrap();
        let still_a_member = world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(joiner))
            .select(WorldMember::as_select())
            .first::<WorldMember>(&mut conn)
            .optional()
            .unwrap();
        assert!(
            still_a_member.is_some(),
            "rotation must never retroactively remove someone who already joined"
        );
    }

    /// US1-4: a dead link can always be revived by rotation. But rotating an
    /// already-revoked link is refused — it would yield two replacements for
    /// one original.
    #[tokio::test]
    async fn expired_and_exhausted_links_rotate_but_revoked_ones_do_not() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);

        // A realistically expired link: created two days ago with a one-day
        // lifetime, so it lapsed a day ago. `insert_test_invite` stamps
        // `created_at` as now, which would describe a link that expired before
        // it existed — impossible through the API, and it would exercise
        // `rotated_expiry`'s defensive branch instead of the real path.
        let past = Utc::now().naive_utc() - chrono::Duration::days(1);
        let (expired_id, _) = insert_test_invite(&mut conn, world_id, owner_id, 5, 0, Some(past));
        diesel::update(world_invites::table.find(expired_id))
            .set(world_invites::created_at.eq(Utc::now().naive_utc() - chrono::Duration::days(2)))
            .execute(&mut conn)
            .expect("backdate the expired link's creation");
        let (exhausted_id, _) = insert_test_invite(&mut conn, world_id, owner_id, 3, 3, None);
        let (revoked_id, _) =
            insert_test_invite_with_revocation(&mut conn, world_id, owner_id, 5, 0, None, true);
        drop(conn);

        let from_expired = rotate_invite_code_impl(&state, owner_id, false, expired_id)
            .await
            .expect("rotating an expired link must yield a usable one");
        assert_eq!(from_expired.state, WorldAccessLinkState::Active);

        let from_exhausted = rotate_invite_code_impl(&state, owner_id, false, exhausted_id)
            .await
            .expect("rotating an exhausted link must yield a usable one");
        assert_eq!(from_exhausted.state, WorldAccessLinkState::Active);
        assert_eq!(from_exhausted.used_count, 0);

        assert!(
            rotate_invite_code_impl(&state, owner_id, false, revoked_id)
                .await
                .is_err(),
            "an already-revoked link must not rotate again"
        );
    }

    /// FR-002 / FR-008: revoke is idempotent, and neither operation is open to
    /// a non-DM.
    #[tokio::test]
    async fn revoke_is_idempotent_and_both_operations_are_dm_only() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let (invite_id, _) = insert_test_invite(&mut conn, world_id, owner_id, 10, 0, None);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let outsider_id = insert_test_user(&mut conn);
        drop(conn);

        // A plain member and a non-member are both refused, for both verbs.
        for actor in [player_id, outsider_id] {
            assert!(
                revoke_invite_code_impl(&state, actor, false, invite_id)
                    .await
                    .is_err(),
                "only a DM may revoke"
            );
            assert!(
                rotate_invite_code_impl(&state, actor, false, invite_id)
                    .await
                    .is_err(),
                "only a DM may rotate"
            );
        }

        let first = revoke_invite_code_impl(&state, owner_id, false, invite_id)
            .await
            .expect("the DM must be able to revoke");
        assert_eq!(first.state, WorldAccessLinkState::Revoked);

        let second = revoke_invite_code_impl(&state, owner_id, false, invite_id)
            .await
            .expect("revoking twice must succeed rather than error");
        assert_eq!(second.state, WorldAccessLinkState::Revoked);
    }

    /// FR-012 — **fails before spec 027's atomic consume**.
    ///
    /// The previous implementation read the invite, validated it in memory,
    /// then wrote back a computed count. Two joins racing for the last use
    /// both read `used_count = N`, both computed `N + 1`, and both wrote it —
    /// admitting two members against one remaining use.
    #[tokio::test]
    async fn concurrent_joins_on_the_last_use_admit_exactly_one() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        // Cap 5 with 4 spent: exactly one use remains.
        let (invite_id, code) = insert_test_invite(&mut conn, world_id, owner_id, 5, 4, None);

        // Enough contenders that a lost update is near-certain if the race is
        // still present, rather than relying on catching a two-way tie.
        let racers: Vec<Uuid> = (0..8).map(|_| insert_test_user(&mut conn)).collect();
        drop(conn);

        let attempts = racers.into_iter().map(|user_id| {
            let state = state.clone();
            let code = code.clone();
            tokio::spawn(async move {
                join_world_impl(&state, user_id, JoinWorldInput { invite_code: code })
                    .await
                    .is_ok()
            })
        });

        let mut succeeded = 0;
        for attempt in attempts {
            if attempt.await.expect("join task must not panic") {
                succeeded += 1;
            }
        }

        assert_eq!(
            succeeded, 1,
            "exactly one racer may claim the last use (FR-012)"
        );

        let mut conn = state.db_pool.get().unwrap();
        let invite = load_invite(&mut conn, invite_id);
        assert_eq!(
            invite.used_count, 5,
            "used_count must land exactly on the cap, never past it"
        );

        let members: i64 = world_members::table
            .filter(world_members::world_id.eq(world_id))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(members, 1, "only one membership may be created");
    }

    // ===== Spec 027 US4: unusable links fail identically =====

    /// FR-011 / SC-005: unknown, expired, exhausted and revoked are
    /// indistinguishable. Possessing a dead code must reveal nothing about
    /// whether it was ever real, or which world it belonged to.
    #[tokio::test]
    async fn every_unusable_code_fails_with_the_same_message() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);

        let past = Utc::now().naive_utc() - chrono::Duration::days(1);
        let (_, expired) = insert_test_invite(&mut conn, world_id, owner_id, 5, 0, Some(past));
        let (_, exhausted) = insert_test_invite(&mut conn, world_id, owner_id, 3, 3, None);
        let (_, revoked) =
            insert_test_invite_with_revocation(&mut conn, world_id, owner_id, 5, 0, None, true);
        let never_issued = "ZZZZZZZZZZZZZZZZZZZZ".to_string();
        drop(conn);

        let mut messages = Vec::new();
        for (label, code) in [
            ("expired", expired),
            ("exhausted", exhausted),
            ("revoked", revoked),
            ("never issued", never_issued),
        ] {
            let joiner = {
                let mut conn = state.db_pool.get().unwrap();
                insert_test_user(&mut conn)
            };
            let err = join_world_impl(&state, joiner, JoinWorldInput { invite_code: code })
                .await
                .expect_err(&format!("a {label} code must be refused"));
            messages.push((label, err.message));
        }

        for (label, message) in &messages {
            assert_eq!(
                message, LINK_UNAVAILABLE_MESSAGE,
                "a {label} code must return the uniform message, not its own"
            );
        }

        // Belt and braces: prove they are all literally equal to each other,
        // so a future change that gives one case its own wording fails here.
        let first = &messages[0].1;
        assert!(
            messages.iter().all(|(_, m)| m == first),
            "all unusable-code failures must be indistinguishable: {messages:?}"
        );
    }

    /// US4-2: an existing member gets their own message — and critically, this
    /// consumes **no use**, so a repeat click never burns the GM's cap.
    #[tokio::test]
    async fn an_existing_member_gets_a_distinct_message_and_consumes_no_use() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let (invite_id, code) = insert_test_invite(&mut conn, world_id, owner_id, 10, 0, None);
        let joiner = insert_test_user(&mut conn);
        drop(conn);

        join_world_impl(
            &state,
            joiner,
            JoinWorldInput {
                invite_code: code.clone(),
            },
        )
        .await
        .expect("first join should succeed");

        let count_after_first = {
            let mut conn = state.db_pool.get().unwrap();
            load_invite(&mut conn, invite_id).used_count
        };
        assert_eq!(count_after_first, 1);

        let err = join_world_impl(&state, joiner, JoinWorldInput { invite_code: code })
            .await
            .expect_err("a second join by the same user must be refused");
        assert_eq!(
            err.message, ALREADY_A_MEMBER_MESSAGE,
            "an existing member deserves a message that says what happened"
        );
        assert_ne!(
            err.message, LINK_UNAVAILABLE_MESSAGE,
            "the link is fine — do not report it as dead"
        );

        let count_after_second = {
            let mut conn = state.db_pool.get().unwrap();
            load_invite(&mut conn, invite_id).used_count
        };
        assert_eq!(
            count_after_second, 1,
            "a repeat click must not burn a use of the GM's cap"
        );
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
        let (world_id, owner_id, member_id) = world_with_a_fully_granted_member(&mut conn, "Owner");
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

        let (world_a, owner_a, member_id) = world_with_a_fully_granted_member(&mut conn, "Editor");

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
            .expect(
                "the world's own owner, with no world_members row, must be able to remove members",
            );
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
