//! Small sync DB helpers, run inside `spawn_blocking`, and the
//! authorization checks every mutation here starts with.

use super::*;

// ============================================================================
// Small sync DB helpers (run inside spawn_blocking, mirroring
// world_events.rs::world_id_for_scene's pattern for other FK lookups)
// ============================================================================

pub(crate) fn load_session_row(
    conn: &mut PgConnection,
    session_id: Uuid,
) -> Result<GenieSession, String> {
    world_genie_sessions::table
        .filter(world_genie_sessions::id.eq(session_id))
        .select(GenieSession::as_select())
        .first::<GenieSession>(conn)
        .map_err(|_| "Genie session not found".to_string())
}

pub(crate) fn load_puzzle_clock_row(
    conn: &mut PgConnection,
    clock_id: Uuid,
) -> Result<GeniePuzzleClock, String> {
    world_genie_puzzle_clocks::table
        .filter(world_genie_puzzle_clocks::id.eq(clock_id))
        .select(GeniePuzzleClock::as_select())
        .first::<GeniePuzzleClock>(conn)
        .map_err(|_| "Puzzle Clock not found".to_string())
}

pub(crate) fn load_puzzle_clocks_for_session(
    conn: &mut PgConnection,
    session_id: Uuid,
) -> Result<Vec<GeniePuzzleClock>, String> {
    world_genie_puzzle_clocks::table
        .filter(world_genie_puzzle_clocks::session_id.eq(session_id))
        .order(world_genie_puzzle_clocks::created_at.asc())
        .select(GeniePuzzleClock::as_select())
        .load::<GeniePuzzleClock>(conn)
        .map_err(|e| format!("Failed to load puzzle clocks: {e}"))
}

/// FR-016: the session's win condition — every Puzzle Clock row for the
/// session has a non-null `resolved_at`. A session with zero Puzzle
/// Clocks never "wins" by this check (vacuous truth is deliberately
/// excluded — `!clocks.is_empty()`).
pub(crate) fn all_puzzle_clocks_resolved(clocks: &[GeniePuzzleClock]) -> bool {
    !clocks.is_empty() && clocks.iter().all(|c| c.resolved_at.is_some())
}

pub(crate) fn load_holding_quantity(
    conn: &mut PgConnection,
    session_id: Uuid,
    actor_id: Uuid,
    resource_type: &str,
) -> Result<i32, String> {
    world_genie_resource_holdings::table
        .filter(world_genie_resource_holdings::session_id.eq(session_id))
        .filter(world_genie_resource_holdings::actor_id.eq(actor_id))
        .filter(world_genie_resource_holdings::resource_type.eq(resource_type))
        .select(world_genie_resource_holdings::quantity)
        .first::<i32>(conn)
        .optional()
        .map_err(|e| format!("Failed to load holding: {e}"))
        .map(|v| v.unwrap_or(0))
}

/// Sets a holding row to an absolute `new_quantity` (upsert on the
/// `(session_id, actor_id, resource_type)` unique constraint,
/// data-model.md's validation rule that holdings are incremented/
/// decremented in place rather than appended as new rows). Caller is
/// responsible for having already validated `new_quantity >= 0`.
pub(crate) fn set_holding_quantity(
    conn: &mut PgConnection,
    session_id: Uuid,
    actor_id: Uuid,
    resource_type: &str,
    new_quantity: i32,
) -> Result<GenieResourceHolding, String> {
    let now = Utc::now().naive_utc();
    diesel::insert_into(world_genie_resource_holdings::table)
        .values((
            world_genie_resource_holdings::session_id.eq(session_id),
            world_genie_resource_holdings::actor_id.eq(actor_id),
            world_genie_resource_holdings::resource_type.eq(resource_type),
            world_genie_resource_holdings::quantity.eq(new_quantity),
            world_genie_resource_holdings::created_at.eq(now),
            world_genie_resource_holdings::updated_at.eq(now),
        ))
        .on_conflict((
            world_genie_resource_holdings::session_id,
            world_genie_resource_holdings::actor_id,
            world_genie_resource_holdings::resource_type,
        ))
        .do_update()
        .set((
            world_genie_resource_holdings::quantity.eq(new_quantity),
            world_genie_resource_holdings::updated_at.eq(now),
        ))
        .returning(GenieResourceHolding::as_returning())
        .get_result::<GenieResourceHolding>(conn)
        .map_err(|e| format!("Failed to update holding: {e}"))
}

pub(crate) async fn require_member_of_session_world(
    state: &AppState,
    user_id: Uuid,
    session_id: Uuid,
) -> GraphQLResult<Uuid> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || -> Result<Uuid, String> {
        let session = load_session_row(&mut conn, session_id)?;
        require_world_member(&mut conn, user_id, session.world_id)
            .map_err(|_| "You must be a member of this world".to_string())?;
        Ok(session.world_id)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)
}

/// Error type for `conn.transaction(...)` closures that need to reject
/// with a specific human-readable message (e.g. "insufficient holding")
/// as well as propagate ordinary Diesel errors via `?`. Diesel's
/// `Error::RollbackTransaction` variant (unlike some other ORMs' rollback
/// helpers) is a unit variant with no message payload, so a custom error
/// type is needed to carry one out of the closure.
pub(crate) enum TxError {
    Diesel(diesel::result::Error),
    Msg(String),
}

impl From<diesel::result::Error> for TxError {
    fn from(e: diesel::result::Error) -> Self {
        TxError::Diesel(e)
    }
}

impl From<TxError> for String {
    fn from(e: TxError) -> Self {
        match e {
            TxError::Diesel(inner) => format!("Database error: {inner}"),
            TxError::Msg(msg) => msg,
        }
    }
}

/// "Do I control this actor" for Session Resource purposes: the world's
/// GM always controls every actor (mirrors `is_dm_of_world`'s role
/// everywhere else in this file), or the actor's `world_actors.owned_by`
/// column names this caller directly — the same column the live-play
/// token-ownership check already keys off of (`owner_user_id` on the
/// `tokens` table, `apps/web/src/engine/world/sync/tokens.ts`'s
/// `moveOwnToken` path), rather than the separate, more granular
/// `world_actor_permissions` "ownership block" (spec 010) that specs
/// 011/013's DM-only *content* mutations use — that block defaults every
/// non-DM member to `Viewer` with no automatic `Owner` grant for a
/// claimed/created player character, which would make a player unable
/// to trade or spend their own Session Resources without an explicit
/// GM-granted permission row.
///
/// Spec 019 fix: `owned_by` alone isn't the whole story — a real player's
/// character, per spec 017's onboarding flow, is one the *GM* created and
/// the *player* then claimed via `world_actor_claims` (join
/// `world_members`); `owned_by` stays the GM/creator forever, it never
/// transfers on claim. Without also checking the claim, any player who
/// joined the normal way (not one who created their own actor) got
/// "You do not control this actor" for every Session Resource action
/// tied to their own character — found live while wiring
/// `genieTradeProposals` and confirmed via a real two-account e2e run
/// (`genie-resource-trade.spec.ts`).
pub(crate) async fn caller_controls_actor(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> GraphQLResult<bool> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let (world_id, owned_by, claimed_by_user_id) =
        tokio::task::spawn_blocking(move || -> Result<(Uuid, Uuid, Option<Uuid>), String> {
            use thunderforge_server::schema::{world_actor_claims, world_actors, world_members};

            let (world_id, owned_by) = world_actors::table
                .filter(world_actors::id.eq(actor_id))
                .select((world_actors::world_id, world_actors::owned_by))
                .first::<(Uuid, Uuid)>(&mut conn)
                .map_err(|_| "Actor not found".to_string())?;

            let claimed_by_user_id = world_actor_claims::table
                .inner_join(
                    world_members::table
                        .on(world_members::id.eq(world_actor_claims::world_member_id)),
                )
                .filter(world_actor_claims::actor_id.eq(actor_id))
                .select(world_members::user_id)
                .first::<Uuid>(&mut conn)
                .optional()
                .map_err(|e| format!("Failed to load actor claim: {e}"))?;

            Ok((world_id, owned_by, claimed_by_user_id))
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    if owned_by == user_id || claimed_by_user_id == Some(user_id) {
        return Ok(true);
    }
    is_dm_of_world(state, user_id, is_admin, world_id).await
}

pub(crate) async fn require_caller_controls_actor(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> GraphQLResult<()> {
    if !caller_controls_actor(state, user_id, is_admin, actor_id).await? {
        return Err(Error::new("You do not control this actor"));
    }
    Ok(())
}
