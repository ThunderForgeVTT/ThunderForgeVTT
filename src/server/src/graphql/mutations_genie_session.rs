//! Spec 018 (User Story 7): the Genie session loop — Session Wish Pool,
//! Doom Clock, Puzzle Clocks, and Session Resource trades. See
//! `specs/018-genie-house-system/contracts/genie-session-loop.md` and
//! `data-model.md`'s "Session Wish Pool + Doom Clock",
//! "world_genie_puzzle_clocks", and "world_genie_resource_holdings"
//! sections.
//!
//! Authorization (research.md R8, contracts/genie-session-loop.md):
//! - `spendWish`, `advanceDoomClock`, `createPuzzleClock`,
//!   `advancePuzzleClock` (and this file's `startGenieSession`, an
//!   addition beyond the contract needed to create the session row in
//!   the first place — see this module's doc comment below) are
//!   GM-only, following the exact `is_dm_of_world` pattern already used
//!   by specs 011/013's "DM-only" mutations (`mutations_items.rs`).
//! - `proposeResourceTrade` is callable by either named party (their own
//!   actor, checked via `world_actors.owned_by == caller` — the same
//!   ownership signal live-play token control already keys off of, see
//!   `caller_controls_actor`'s doc comment below); `acceptResourceTrade`
//!   is callable only by the *other* named party —
//!   the proposer (`created_by`) is rejected outright. This two-party
//!   consent shape is new to the codebase; see
//!   `docs/adrs/<n>-genie-session-state-two-party-consent.md`.
//! - `spendResourceOnPuzzleClock` is callable only by the actor spending
//!   its own holdings.
//!
//! Every mutation broadcasts a `world_events` row with
//! `event_code = EVENT_CODE_GENIE_SESSION_STATE` (15) on success via the
//! existing `record_world_event` function (research.md R7) — no new
//! subscription.
//!
//! Note on `startGenieSession`: contracts/genie-session-loop.md's
//! `Mutation` block does not define a mutation that creates the initial
//! `world_genie_sessions` row — every other mutation in the contract
//! operates on an already-existing `sessionId`/`clockId`. Since a
//! session must exist before any of those can be called (FR-013's "3
//! wishes at the start of each session"), this file adds
//! `startGenieSession(worldId, doomClockMax)` (GM-only) as the missing
//! prerequisite step. `wishesRemaining` is not caller-supplied — it
//! always starts at the schema default of 3 (FR-013).

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use chrono::Utc;
use diesel::PgConnection;
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::is_dm_of_world;
use crate::auth::world_membership::require_world_member;
use crate::graphql::{app_state, authenticated_user};
use crate::models::{
    GeniePuzzleClock, GeniePuzzleClockReward, GenieResourceHolding, GenieSession, GenieShopListing,
    GenieTradeProposal, NewGeniePuzzleClock, NewGeniePuzzleClockReward, NewGenieSession,
    NewGenieShopListing, NewGenieTradeProposal,
};
use crate::schema::{
    world_actor_inventory, world_actors, world_genie_puzzle_clock_rewards,
    world_genie_puzzle_clocks, world_genie_resource_holdings, world_genie_sessions,
    world_genie_shop_listings, world_genie_trade_proposals, world_items, worlds,
};
use crate::state::AppState;
use crate::world_events::{EVENT_CODE_GENIE_SESSION_STATE, record_world_event};

// ============================================================================
// GraphQL-facing types
// ============================================================================

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GenieSessionStatus {
    Active,
    Won,
    Lost,
}

impl GenieSessionStatus {
    fn from_db_str(s: &str) -> Self {
        match s {
            "won" => GenieSessionStatus::Won,
            "lost" => GenieSessionStatus::Lost,
            _ => GenieSessionStatus::Active,
        }
    }
}

#[derive(async_graphql::SimpleObject, Debug, Clone)]
pub struct GraphQLGeniePuzzleClock {
    pub id: Uuid,
    pub session_id: Uuid,
    pub label: String,
    pub segments_current: i32,
    pub segments_max: i32,
    pub resolved_at: Option<chrono::NaiveDateTime>,
}

impl From<GeniePuzzleClock> for GraphQLGeniePuzzleClock {
    fn from(row: GeniePuzzleClock) -> Self {
        GraphQLGeniePuzzleClock {
            id: row.id,
            session_id: row.session_id,
            label: row.label,
            segments_current: row.segments_current,
            segments_max: row.segments_max,
            resolved_at: row.resolved_at,
        }
    }
}

#[derive(async_graphql::SimpleObject, Debug, Clone)]
pub struct GraphQLGenieSession {
    pub id: Uuid,
    pub world_id: Uuid,
    pub wishes_remaining: i32,
    pub doom_clock_current: i32,
    pub doom_clock_max: i32,
    pub status: GenieSessionStatus,
    pub puzzle_clocks: Vec<GraphQLGeniePuzzleClock>,
}

fn build_graphql_session(
    session: GenieSession,
    clocks: Vec<GeniePuzzleClock>,
) -> GraphQLGenieSession {
    GraphQLGenieSession {
        id: session.id,
        world_id: session.world_id,
        wishes_remaining: session.wishes_remaining,
        doom_clock_current: session.doom_clock_current,
        doom_clock_max: session.doom_clock_max,
        status: GenieSessionStatus::from_db_str(&session.status),
        puzzle_clocks: clocks
            .into_iter()
            .map(GraphQLGeniePuzzleClock::from)
            .collect(),
    }
}

#[derive(async_graphql::SimpleObject, Debug, Clone)]
pub struct GraphQLGenieResourceHolding {
    pub actor_id: Uuid,
    pub resource_type: String,
    pub quantity: i32,
}

impl From<GenieResourceHolding> for GraphQLGenieResourceHolding {
    fn from(row: GenieResourceHolding) -> Self {
        GraphQLGenieResourceHolding {
            actor_id: row.actor_id,
            resource_type: row.resource_type,
            quantity: row.quantity,
        }
    }
}

#[derive(async_graphql::SimpleObject, Debug, Clone)]
pub struct GraphQLGenieTradeProposal {
    pub id: Uuid,
    pub session_id: Uuid,
    pub from_actor_id: Uuid,
    pub from_resource_type: String,
    pub from_quantity: i32,
    pub to_actor_id: Uuid,
    pub to_resource_type: String,
    pub to_quantity: i32,
    pub status: String,
}

impl From<GenieTradeProposal> for GraphQLGenieTradeProposal {
    fn from(row: GenieTradeProposal) -> Self {
        GraphQLGenieTradeProposal {
            id: row.id,
            session_id: row.session_id,
            from_actor_id: row.from_actor_id,
            from_resource_type: row.from_resource_type,
            from_quantity: row.from_quantity,
            to_actor_id: row.to_actor_id,
            to_resource_type: row.to_resource_type,
            to_quantity: row.to_quantity,
            status: row.status,
        }
    }
}

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GenieShopPriceKind {
    Resource,
    Item,
}

impl GenieShopPriceKind {
    fn as_db_str(self) -> &'static str {
        match self {
            GenieShopPriceKind::Resource => "resource",
            GenieShopPriceKind::Item => "item",
        }
    }
}

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GenieRewardRecipientMode {
    TriggeringActor,
    WholeParty,
}

impl GenieRewardRecipientMode {
    fn as_db_str(self) -> &'static str {
        match self {
            GenieRewardRecipientMode::TriggeringActor => "triggering_actor",
            GenieRewardRecipientMode::WholeParty => "whole_party",
        }
    }
}

#[derive(async_graphql::SimpleObject, Debug, Clone)]
pub struct GraphQLGenieShopListing {
    pub id: Uuid,
    pub actor_id: Uuid,
    pub item_id: Uuid,
    pub price_kind: String,
    pub price_resource_type: Option<String>,
    pub price_resource_amount: Option<i32>,
    pub price_item_id: Option<Uuid>,
    pub price_item_quantity: Option<i32>,
    /// Derived, not stored: `world_actor_inventory.quantity` for
    /// `(actor_id, item_id)` (contracts/genie-economy.md).
    pub stock_quantity: i32,
}

#[derive(async_graphql::SimpleObject, Debug, Clone)]
pub struct GraphQLGeniePuzzleClockReward {
    pub id: Uuid,
    pub clock_id: Uuid,
    pub trigger_segment: i32,
    pub reward_resource_type: Option<String>,
    pub reward_resource_amount: Option<i32>,
    pub reward_item_id: Option<Uuid>,
    pub reward_item_quantity: Option<i32>,
    pub recipient_mode: String,
    pub granted_at: Option<chrono::NaiveDateTime>,
}

impl From<GeniePuzzleClockReward> for GraphQLGeniePuzzleClockReward {
    fn from(row: GeniePuzzleClockReward) -> Self {
        GraphQLGeniePuzzleClockReward {
            id: row.id,
            clock_id: row.clock_id,
            trigger_segment: row.trigger_segment,
            reward_resource_type: row.reward_resource_type,
            reward_resource_amount: row.reward_resource_amount,
            reward_item_id: row.reward_item_id,
            reward_item_quantity: row.reward_item_quantity,
            recipient_mode: row.recipient_mode,
            granted_at: row.granted_at,
        }
    }
}

// ============================================================================
// Small sync DB helpers (run inside spawn_blocking, mirroring
// world_events.rs::world_id_for_scene's pattern for other FK lookups)
// ============================================================================

fn load_session_row(conn: &mut PgConnection, session_id: Uuid) -> Result<GenieSession, String> {
    world_genie_sessions::table
        .filter(world_genie_sessions::id.eq(session_id))
        .select(GenieSession::as_select())
        .first::<GenieSession>(conn)
        .map_err(|_| "Genie session not found".to_string())
}

fn load_puzzle_clock_row(
    conn: &mut PgConnection,
    clock_id: Uuid,
) -> Result<GeniePuzzleClock, String> {
    world_genie_puzzle_clocks::table
        .filter(world_genie_puzzle_clocks::id.eq(clock_id))
        .select(GeniePuzzleClock::as_select())
        .first::<GeniePuzzleClock>(conn)
        .map_err(|_| "Puzzle Clock not found".to_string())
}

fn load_puzzle_clocks_for_session(
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
fn all_puzzle_clocks_resolved(clocks: &[GeniePuzzleClock]) -> bool {
    !clocks.is_empty() && clocks.iter().all(|c| c.resolved_at.is_some())
}

fn load_holding_quantity(
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
fn set_holding_quantity(
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

async fn require_member_of_session_world(
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
enum TxError {
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
async fn caller_controls_actor(
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
            use crate::schema::{world_actor_claims, world_actors, world_members};

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

// ============================================================================
// startGenieSession (see module doc comment — an addition beyond the
// contract, the missing prerequisite step for everything else here)
// ============================================================================

#[derive(InputObject, Debug, Clone)]
pub struct StartGenieSessionInput {
    pub world_id: Uuid,
    pub doom_clock_max: i32,
}

pub async fn start_genie_session_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: StartGenieSessionInput,
) -> GraphQLResult<GraphQLGenieSession> {
    if !is_dm_of_world(state, user_id, is_admin, input.world_id).await? {
        return Err(Error::new("Only the GM may start a Genie session"));
    }
    if input.doom_clock_max <= 0 {
        return Err(Error::new("doomClockMax must be greater than zero"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = input.world_id;
    let doom_clock_max = input.doom_clock_max;

    let session = tokio::task::spawn_blocking(move || -> Result<GenieSession, String> {
        // FR-003 (research.md R1): per-world GM setting. When enabled,
        // copy every character's ending holdings from the most recently
        // concluded session (by created_at) into the new session before
        // returning — "the rope doesn't disappear." When disabled
        // (default), holdings simply start empty, same as before this
        // spec.
        let carryover_enabled = worlds::table
            .filter(worlds::id.eq(world_id))
            .select(worlds::genie_resource_carryover_enabled)
            .first::<bool>(&mut conn)
            .map_err(|e| format!("Failed to load world: {e}"))?;

        let prior_holdings: Vec<GenieResourceHolding> = if carryover_enabled {
            let prior_session_id = world_genie_sessions::table
                .filter(world_genie_sessions::world_id.eq(world_id))
                .filter(world_genie_sessions::status.ne("active"))
                .order(world_genie_sessions::created_at.desc())
                .select(world_genie_sessions::id)
                .first::<Uuid>(&mut conn)
                .optional()
                .map_err(|e| format!("Failed to load prior session: {e}"))?;

            match prior_session_id {
                Some(prior_id) => world_genie_resource_holdings::table
                    .filter(world_genie_resource_holdings::session_id.eq(prior_id))
                    .select(GenieResourceHolding::as_select())
                    .load::<GenieResourceHolding>(&mut conn)
                    .map_err(|e| format!("Failed to load prior holdings: {e}"))?,
                None => Vec::new(),
            }
        } else {
            Vec::new()
        };

        let new_session = NewGenieSession {
            world_id,
            doom_clock_max,
            created_by: user_id,
        };
        let session = diesel::insert_into(world_genie_sessions::table)
            .values(&new_session)
            .returning(GenieSession::as_returning())
            .get_result::<GenieSession>(&mut conn)
            .map_err(|e| format!("Failed to start session: {e}"))?;

        for holding in &prior_holdings {
            set_holding_quantity(
                &mut conn,
                session.id,
                holding.actor_id,
                &holding.resource_type,
                holding.quantity,
            )
            .map_err(|e| format!("Failed to carry over holding: {e}"))?;
        }

        let _ = record_world_event(
            &mut conn,
            world_id,
            EVENT_CODE_GENIE_SESSION_STATE,
            Some(serde_json::json!({
                "kind": "wish_pool",
                "session_id": session.id,
                "action": "session_started",
                "wishes_remaining": session.wishes_remaining,
                "carried_over_holdings": prior_holdings.len(),
            })),
            user_id,
        );

        Ok(session)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(build_graphql_session(session, Vec::new()))
}

// ============================================================================
// grantSessionResource (spec 020, FR-001) — GM-only direct grant, the
// bootstrapping fix: the only way a Session Resource holding could
// previously come to exist was a trade or a Puzzle Clock spend, both of
// which require holdings to already exist.
// ============================================================================

pub async fn grant_session_resource_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    session_id: Uuid,
    actor_id: Uuid,
    resource_type: String,
    amount: i32,
) -> GraphQLResult<GraphQLGenieResourceHolding> {
    if amount <= 0 {
        return Err(Error::new("amount must be greater than zero"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let session = tokio::task::spawn_blocking(move || load_session_row(&mut conn, session_id))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, session.world_id).await? {
        return Err(Error::new("Only the GM may grant Session Resources"));
    }
    if session.status != "active" {
        return Err(Error::new(
            "Start a session first — there is no active Genie session for this world",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = session.world_id;

    let holding = tokio::task::spawn_blocking(move || -> Result<GenieResourceHolding, String> {
        conn.transaction(|conn| -> Result<GenieResourceHolding, TxError> {
            let current = load_holding_quantity(conn, session_id, actor_id, &resource_type)
                .map_err(TxError::Msg)?;
            let updated =
                set_holding_quantity(conn, session_id, actor_id, &resource_type, current + amount)
                    .map_err(TxError::Msg)?;

            let _ = record_world_event(
                conn,
                world_id,
                EVENT_CODE_GENIE_SESSION_STATE,
                Some(serde_json::json!({
                    "kind": "resource_grant",
                    "session_id": session_id,
                    "actor_id": actor_id,
                    "resource_type": updated.resource_type,
                    "quantity": updated.quantity,
                    "amount_granted": amount,
                })),
                user_id,
            );

            Ok(updated)
        })
        .map_err(String::from)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLGenieResourceHolding::from(holding))
}

// ============================================================================
// spendWish (T032) — GM-only
// ============================================================================

pub async fn spend_wish_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    session_id: Uuid,
    narrative_effect: String,
) -> GraphQLResult<GraphQLGenieSession> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let session = tokio::task::spawn_blocking(move || load_session_row(&mut conn, session_id))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, session.world_id).await? {
        return Err(Error::new("Only the GM may spend a wish"));
    }
    if session.wishes_remaining <= 0 {
        return Err(Error::new("No wishes remaining in the Session Wish Pool"));
    }
    if narrative_effect.trim().is_empty() {
        return Err(Error::new("narrativeEffect must not be empty (FR-014)"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = session.world_id;
    let new_wishes = session.wishes_remaining - 1;

    let (updated, clocks) = tokio::task::spawn_blocking(
        move || -> Result<(GenieSession, Vec<GeniePuzzleClock>), String> {
            let now = Utc::now().naive_utc();
            let updated = diesel::update(
                world_genie_sessions::table.filter(world_genie_sessions::id.eq(session_id)),
            )
            .set((
                world_genie_sessions::wishes_remaining.eq(new_wishes),
                world_genie_sessions::updated_at.eq(now),
            ))
            .returning(GenieSession::as_returning())
            .get_result::<GenieSession>(&mut conn)
            .map_err(|e| format!("Failed to spend wish: {e}"))?;

            let _ = record_world_event(
                &mut conn,
                world_id,
                EVENT_CODE_GENIE_SESSION_STATE,
                Some(serde_json::json!({
                    "kind": "wish_pool",
                    "session_id": session_id,
                    "action": "wish_spent",
                    "wishes_remaining": updated.wishes_remaining,
                    "narrative_effect": narrative_effect,
                })),
                user_id,
            );

            let clocks = load_puzzle_clocks_for_session(&mut conn, session_id)?;
            Ok((updated, clocks))
        },
    )
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(build_graphql_session(updated, clocks))
}

// ============================================================================
// advanceDoomClock (T032/T033) — GM-only; loss check per FR-016
// ============================================================================

pub async fn advance_doom_clock_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    session_id: Uuid,
    delta: i32,
) -> GraphQLResult<GraphQLGenieSession> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let session = tokio::task::spawn_blocking(move || load_session_row(&mut conn, session_id))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, session.world_id).await? {
        return Err(Error::new("Only the GM may advance the Doom Clock"));
    }
    if session.status != "active" {
        return Err(Error::new("This Genie session has already concluded"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = session.world_id;
    let new_current = (session.doom_clock_current + delta).clamp(0, session.doom_clock_max);
    // FR-016: filling the Doom Clock completely, while the session is
    // still active (checked above — a same-action Puzzle Clock win takes
    // precedence per spec.md's Edge Cases, since that would have already
    // flipped status away from 'active' in its own prior mutation),
    // triggers the loss condition.
    let just_lost = new_current >= session.doom_clock_max;

    let (updated, clocks) = tokio::task::spawn_blocking(
        move || -> Result<(GenieSession, Vec<GeniePuzzleClock>), String> {
            let now = Utc::now().naive_utc();
            let new_status = if just_lost { "lost" } else { "active" };
            let updated = diesel::update(
                world_genie_sessions::table.filter(world_genie_sessions::id.eq(session_id)),
            )
            .set((
                world_genie_sessions::doom_clock_current.eq(new_current),
                world_genie_sessions::status.eq(new_status),
                world_genie_sessions::updated_at.eq(now),
            ))
            .returning(GenieSession::as_returning())
            .get_result::<GenieSession>(&mut conn)
            .map_err(|e| format!("Failed to advance Doom Clock: {e}"))?;

            let _ = record_world_event(
                &mut conn,
                world_id,
                EVENT_CODE_GENIE_SESSION_STATE,
                Some(serde_json::json!({
                    "kind": "doom_clock",
                    "session_id": session_id,
                    "doom_clock_current": updated.doom_clock_current,
                    "doom_clock_max": updated.doom_clock_max,
                    "session_status": updated.status,
                })),
                user_id,
            );

            let clocks = load_puzzle_clocks_for_session(&mut conn, session_id)?;
            Ok((updated, clocks))
        },
    )
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(build_graphql_session(updated, clocks))
}

// ============================================================================
// createPuzzleClock (T032) — GM-only
// ============================================================================

pub async fn create_puzzle_clock_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    session_id: Uuid,
    label: String,
    segments_max: i32,
) -> GraphQLResult<GraphQLGeniePuzzleClock> {
    if label.trim().is_empty() {
        return Err(Error::new("label must not be empty"));
    }
    if segments_max <= 0 {
        return Err(Error::new("segmentsMax must be greater than zero"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let session = tokio::task::spawn_blocking(move || load_session_row(&mut conn, session_id))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, session.world_id).await? {
        return Err(Error::new("Only the GM may create a Puzzle Clock"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = session.world_id;

    let clock = tokio::task::spawn_blocking(move || -> Result<GeniePuzzleClock, String> {
        let new_clock = NewGeniePuzzleClock {
            session_id,
            label,
            segments_max,
        };
        let clock = diesel::insert_into(world_genie_puzzle_clocks::table)
            .values(&new_clock)
            .returning(GeniePuzzleClock::as_returning())
            .get_result::<GeniePuzzleClock>(&mut conn)
            .map_err(|e| format!("Failed to create Puzzle Clock: {e}"))?;

        let _ = record_world_event(
            &mut conn,
            world_id,
            EVENT_CODE_GENIE_SESSION_STATE,
            Some(serde_json::json!({
                "kind": "puzzle_clock",
                "session_id": session_id,
                "action": "created",
                "clock_id": clock.id,
                "label": clock.label,
                "segments_current": clock.segments_current,
                "segments_max": clock.segments_max,
            })),
            user_id,
        );

        Ok(clock)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLGeniePuzzleClock::from(clock))
}

// ============================================================================
// advancePuzzleClock (T032/T033) — GM-only; win check per FR-016
// ============================================================================

/// Spec 020 (research.md R4, FR-006/FR-006a): grants every configured
/// reward row for `clock_id` whose `trigger_segment` falls in
/// `(old_segments, new_segments]` and whose `granted_at` is still NULL
/// (the "exactly once" guard), inside the caller's already-open
/// transaction. `actor_id` is the optional actor to credit for
/// `"triggering_actor"`-mode rows (FR-006a); when `None`, those rows
/// fall back to the same whole-party split every `"whole_party"` row
/// uses. Returns the number of reward rows granted (for the `world_events`
/// payload's `rewards_granted` field).
fn grant_puzzle_clock_rewards(
    conn: &mut PgConnection,
    clock_id: Uuid,
    world_id: Uuid,
    session_id: Uuid,
    old_segments: i32,
    new_segments: i32,
    actor_id: Option<Uuid>,
) -> Result<i32, diesel::result::Error> {
    let due_rewards = world_genie_puzzle_clock_rewards::table
        .filter(world_genie_puzzle_clock_rewards::clock_id.eq(clock_id))
        .filter(world_genie_puzzle_clock_rewards::granted_at.is_null())
        .filter(world_genie_puzzle_clock_rewards::trigger_segment.gt(old_segments))
        .filter(world_genie_puzzle_clock_rewards::trigger_segment.le(new_segments))
        .select(GeniePuzzleClockReward::as_select())
        .load::<GeniePuzzleClockReward>(conn)?;

    if due_rewards.is_empty() {
        return Ok(0);
    }

    // "Whole party" = every non-NPC actor in this clock's world — the
    // same definition the client's useGenieSession.ts already applies
    // locally when it computes partyMembers.
    let party_member_ids: Vec<Uuid> = world_actors::table
        .filter(world_actors::world_id.eq(world_id))
        .filter(world_actors::is_npc.eq(false))
        .order(world_actors::id.asc())
        .select(world_actors::id)
        .load::<Uuid>(conn)?;

    let now = Utc::now().naive_utc();

    for reward in &due_rewards {
        let recipients: Vec<Uuid> = if reward.recipient_mode == "triggering_actor" {
            match actor_id {
                Some(id) => vec![id],
                // FR-006a fallback: no actor attributed on this advance —
                // treat exactly like a whole_party row rather than
                // failing or crediting no one.
                None => party_member_ids.clone(),
            }
        } else {
            party_member_ids.clone()
        };

        if !recipients.is_empty() {
            if let (Some(resource_type), Some(amount)) =
                (&reward.reward_resource_type, reward.reward_resource_amount)
            {
                // Split as evenly as possible; any remainder goes to the
                // first recipients (by id order) so the full configured
                // amount is always granted, never lost to rounding.
                let base_share = amount / recipients.len() as i32;
                let remainder = amount % recipients.len() as i32;
                for (idx, recipient_id) in recipients.iter().enumerate() {
                    let share = base_share + if (idx as i32) < remainder { 1 } else { 0 };
                    if share <= 0 {
                        continue;
                    }
                    let current =
                        load_holding_quantity(conn, session_id, *recipient_id, resource_type)
                            .map_err(|_| diesel::result::Error::RollbackTransaction)?;
                    set_holding_quantity(
                        conn,
                        session_id,
                        *recipient_id,
                        resource_type,
                        current + share,
                    )
                    .map_err(|_| diesel::result::Error::RollbackTransaction)?;
                }
            } else if let (Some(item_id), Some(quantity)) =
                (reward.reward_item_id, reward.reward_item_quantity)
            {
                // Item rewards aren't divisible — every recipient gets
                // the full configured quantity (a "whole_party" item
                // reward means "everyone gets one," not "split one item
                // between everyone").
                for recipient_id in &recipients {
                    grant_item_to_actor_in_tx(conn, *recipient_id, item_id, quantity)?;
                }
            }
        }

        diesel::update(
            world_genie_puzzle_clock_rewards::table
                .filter(world_genie_puzzle_clock_rewards::id.eq(reward.id)),
        )
        .set(world_genie_puzzle_clock_rewards::granted_at.eq(now))
        .execute(conn)?;
    }

    Ok(due_rewards.len() as i32)
}

/// Sync upsert of one item into an actor's inventory, for use inside an
/// already-open transaction (mirrors `add_item_to_inventory_impl`'s
/// upsert query — that function owns its own connection/spawn_blocking
/// and can't be called from inside this transaction closure, so the
/// core upsert is duplicated here rather than reused).
fn grant_item_to_actor_in_tx(
    conn: &mut PgConnection,
    actor_id: Uuid,
    item_id: Uuid,
    quantity: i32,
) -> Result<(), diesel::result::Error> {
    let item_name = world_items::table
        .filter(world_items::id.eq(item_id))
        .select(world_items::name)
        .first::<String>(conn)?;

    diesel::insert_into(world_actor_inventory::table)
        .values((
            world_actor_inventory::actor_id.eq(actor_id),
            world_actor_inventory::item_id.eq(item_id),
            world_actor_inventory::item_name_snapshot.eq(item_name.clone()),
            world_actor_inventory::quantity.eq(quantity),
        ))
        .on_conflict((
            world_actor_inventory::actor_id,
            world_actor_inventory::item_id,
        ))
        .do_update()
        .set((
            world_actor_inventory::quantity.eq(world_actor_inventory::quantity + quantity),
            world_actor_inventory::item_name_snapshot.eq(item_name),
            world_actor_inventory::updated_at.eq(Utc::now().naive_utc()),
        ))
        .execute(conn)?;
    Ok(())
}

pub async fn advance_puzzle_clock_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    clock_id: Uuid,
    delta: i32,
    actor_id: Option<Uuid>,
) -> GraphQLResult<GraphQLGeniePuzzleClock> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let (clock, session) = tokio::task::spawn_blocking(
        move || -> Result<(GeniePuzzleClock, GenieSession), String> {
            let clock = load_puzzle_clock_row(&mut conn, clock_id)?;
            let session = load_session_row(&mut conn, clock.session_id)?;
            Ok((clock, session))
        },
    )
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, session.world_id).await? {
        return Err(Error::new("Only the GM may advance a Puzzle Clock"));
    }
    if session.status != "active" {
        return Err(Error::new("This Genie session has already concluded"));
    }
    if clock.resolved_at.is_some() {
        return Err(Error::new("This Puzzle Clock has already resolved"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = session.world_id;
    let session_id = session.id;
    let new_segments = (clock.segments_current + delta).clamp(0, clock.segments_max);
    let resolves_now = new_segments >= clock.segments_max;

    let old_segments = clock.segments_current;
    let updated_clock = tokio::task::spawn_blocking(move || -> Result<GeniePuzzleClock, String> {
        conn.transaction(|conn| -> Result<GeniePuzzleClock, diesel::result::Error> {
            let now = Utc::now().naive_utc();
            let resolved_at = if resolves_now { Some(now) } else { None };

            let updated_clock = diesel::update(
                world_genie_puzzle_clocks::table.filter(world_genie_puzzle_clocks::id.eq(clock_id)),
            )
            .set((
                world_genie_puzzle_clocks::segments_current.eq(new_segments),
                world_genie_puzzle_clocks::resolved_at.eq(resolved_at),
                world_genie_puzzle_clocks::updated_at.eq(now),
            ))
            .returning(GeniePuzzleClock::as_returning())
            .get_result::<GeniePuzzleClock>(conn)?;

            // FR-016 win check: only relevant when this advance just
            // resolved the clock — an unresolved clock can't complete
            // "every active Puzzle Clock resolved".
            let mut session_status_for_event = session.status.clone();
            if resolves_now {
                let all_clocks = world_genie_puzzle_clocks::table
                    .filter(world_genie_puzzle_clocks::session_id.eq(session_id))
                    .select(GeniePuzzleClock::as_select())
                    .load::<GeniePuzzleClock>(conn)?;
                if all_puzzle_clocks_resolved(&all_clocks) {
                    diesel::update(
                        world_genie_sessions::table.filter(world_genie_sessions::id.eq(session_id)),
                    )
                    .set((
                        world_genie_sessions::status.eq("won"),
                        world_genie_sessions::updated_at.eq(now),
                    ))
                    .execute(conn)?;
                    session_status_for_event = "won".to_string();
                }
            }

            // FR-006/FR-006a: grant any reward rows newly crossed by this
            // advance, exactly once each, in this same transaction.
            let rewards_granted = grant_puzzle_clock_rewards(
                conn,
                clock_id,
                world_id,
                session_id,
                old_segments,
                new_segments,
                actor_id,
            )?;

            let _ = record_world_event(
                conn,
                world_id,
                EVENT_CODE_GENIE_SESSION_STATE,
                Some(serde_json::json!({
                    "kind": "puzzle_clock",
                    "session_id": session_id,
                    "clock_id": updated_clock.id,
                    "segments_current": updated_clock.segments_current,
                    "segments_max": updated_clock.segments_max,
                    "resolved": updated_clock.resolved_at.is_some(),
                    "session_status": session_status_for_event,
                })),
                user_id,
            );

            if rewards_granted > 0 {
                let _ = record_world_event(
                    conn,
                    world_id,
                    EVENT_CODE_GENIE_SESSION_STATE,
                    Some(serde_json::json!({
                        "kind": "clock_reward",
                        "session_id": session_id,
                        "clock_id": updated_clock.id,
                        "actor_id": actor_id,
                        "rewards_granted": rewards_granted,
                    })),
                    user_id,
                );
            }

            Ok(updated_clock)
        })
        .map_err(|e| format!("Failed to advance Puzzle Clock: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLGeniePuzzleClock::from(updated_clock))
}

// ============================================================================
// configurePuzzleClockReward (spec 020, FR-006) — GM-only
// ============================================================================

#[allow(clippy::too_many_arguments)]
pub async fn configure_puzzle_clock_reward_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    clock_id: Uuid,
    trigger_segment: i32,
    reward_resource_type: Option<String>,
    reward_resource_amount: Option<i32>,
    reward_item_id: Option<Uuid>,
    reward_item_quantity: Option<i32>,
    recipient_mode: GenieRewardRecipientMode,
) -> GraphQLResult<GraphQLGeniePuzzleClockReward> {
    if trigger_segment <= 0 {
        return Err(Error::new("triggerSegment must be greater than zero"));
    }
    let is_resource_reward = reward_resource_type.is_some() && reward_resource_amount.is_some();
    let is_item_reward = reward_item_id.is_some() && reward_item_quantity.is_some();
    if is_resource_reward == is_item_reward {
        return Err(Error::new(
            "Exactly one of a resource reward or an item reward must be configured per entry",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let (clock, session) = tokio::task::spawn_blocking(
        move || -> Result<(GeniePuzzleClock, GenieSession), String> {
            let clock = load_puzzle_clock_row(&mut conn, clock_id)?;
            let session = load_session_row(&mut conn, clock.session_id)?;
            Ok((clock, session))
        },
    )
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, session.world_id).await? {
        return Err(Error::new("Only the GM may configure Puzzle Clock rewards"));
    }
    let _ = clock;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let reward = tokio::task::spawn_blocking(move || -> Result<GeniePuzzleClockReward, String> {
        let new_reward = NewGeniePuzzleClockReward {
            clock_id,
            trigger_segment,
            reward_resource_type,
            reward_resource_amount,
            reward_item_id,
            reward_item_quantity,
            recipient_mode: recipient_mode.as_db_str().to_string(),
            created_by: user_id,
        };
        diesel::insert_into(world_genie_puzzle_clock_rewards::table)
            .values(&new_reward)
            .returning(GeniePuzzleClockReward::as_returning())
            .get_result::<GeniePuzzleClockReward>(&mut conn)
            .map_err(|e| format!("Failed to configure Puzzle Clock reward: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLGeniePuzzleClockReward::from(reward))
}

// ============================================================================
// proposeResourceTrade / acceptResourceTrade (T034) — two-party consent
// ============================================================================

#[allow(clippy::too_many_arguments)]
pub async fn propose_resource_trade_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    session_id: Uuid,
    from_actor_id: Uuid,
    from_resource_type: String,
    from_quantity: i32,
    to_actor_id: Uuid,
    to_resource_type: String,
    to_quantity: i32,
) -> GraphQLResult<GraphQLGenieTradeProposal> {
    if from_actor_id == to_actor_id {
        return Err(Error::new("Cannot propose a trade with yourself"));
    }
    if from_quantity <= 0 || to_quantity <= 0 {
        return Err(Error::new("Trade quantities must be greater than zero"));
    }

    require_member_of_session_world(state, user_id, session_id).await?;

    // Callable by either named party (research.md R8).
    let controls_from = caller_controls_actor(state, user_id, is_admin, from_actor_id).await?;
    let controls_to = caller_controls_actor(state, user_id, is_admin, to_actor_id).await?;
    if !controls_from && !controls_to {
        return Err(Error::new(
            "You must control one of the two parties to propose this trade",
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = require_member_of_session_world(state, user_id, session_id).await?;

    let proposal = tokio::task::spawn_blocking(move || -> Result<GenieTradeProposal, String> {
        let new_proposal = NewGenieTradeProposal {
            session_id,
            from_actor_id,
            from_resource_type: from_resource_type.clone(),
            from_quantity,
            to_actor_id,
            to_resource_type: to_resource_type.clone(),
            to_quantity,
            created_by: user_id,
        };
        let proposal = diesel::insert_into(world_genie_trade_proposals::table)
            .values(&new_proposal)
            .returning(GenieTradeProposal::as_returning())
            .get_result::<GenieTradeProposal>(&mut conn)
            .map_err(|e| format!("Failed to propose trade: {e}"))?;

        let _ = record_world_event(
            &mut conn,
            world_id,
            EVENT_CODE_GENIE_SESSION_STATE,
            Some(serde_json::json!({
                "kind": "resource_trade",
                "session_id": session_id,
                "action": "proposed",
                "proposal_id": proposal.id,
                "from_actor_id": from_actor_id,
                "to_actor_id": to_actor_id,
            })),
            user_id,
        );

        Ok(proposal)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLGenieTradeProposal::from(proposal))
}

pub async fn accept_resource_trade_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    proposal_id: Uuid,
) -> GraphQLResult<Vec<GraphQLGenieResourceHolding>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let proposal = tokio::task::spawn_blocking(move || -> Result<GenieTradeProposal, String> {
        world_genie_trade_proposals::table
            .filter(world_genie_trade_proposals::id.eq(proposal_id))
            .select(GenieTradeProposal::as_select())
            .first::<GenieTradeProposal>(&mut conn)
            .map_err(|_| "Trade proposal not found".to_string())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    if proposal.status != "pending" {
        return Err(Error::new("This trade proposal is no longer pending"));
    }
    // The proposer can never accept their own proposal (research.md R8).
    if proposal.created_by == user_id {
        return Err(Error::new("You cannot accept your own trade proposal"));
    }
    // Caller must control the counterpart actor — either side, since
    // `proposeResourceTrade` doesn't record which side literally clicked
    // "propose", only who authored it (`created_by`); requiring the
    // accepter to control one of the two named actors (and not be the
    // proposer, checked above) enforces "the OTHER party" from
    // contracts/genie-session-loop.md.
    let controls_from =
        caller_controls_actor(state, user_id, is_admin, proposal.from_actor_id).await?;
    let controls_to = caller_controls_actor(state, user_id, is_admin, proposal.to_actor_id).await?;
    if !controls_from && !controls_to {
        return Err(Error::new("You are not a party to this trade"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let holdings =
        tokio::task::spawn_blocking(move || -> Result<Vec<GenieResourceHolding>, String> {
            conn.transaction(|conn| -> Result<Vec<GenieResourceHolding>, TxError> {
                let from_current = load_holding_quantity(
                    conn,
                    proposal.session_id,
                    proposal.from_actor_id,
                    &proposal.from_resource_type,
                )
                .map_err(TxError::Msg)?;
                if from_current < proposal.from_quantity {
                    return Err(TxError::Msg(
                        "The proposing party no longer holds enough to complete this trade"
                            .to_string(),
                    ));
                }
                let to_current = load_holding_quantity(
                    conn,
                    proposal.session_id,
                    proposal.to_actor_id,
                    &proposal.to_resource_type,
                )
                .map_err(TxError::Msg)?;
                if to_current < proposal.to_quantity {
                    return Err(TxError::Msg(
                        "The counterpart no longer holds enough to complete this trade".to_string(),
                    ));
                }

                let from_after = set_holding_quantity(
                    conn,
                    proposal.session_id,
                    proposal.from_actor_id,
                    &proposal.from_resource_type,
                    from_current - proposal.from_quantity,
                )
                .map_err(TxError::Msg)?;
                let from_gains_base = load_holding_quantity(
                    conn,
                    proposal.session_id,
                    proposal.from_actor_id,
                    &proposal.to_resource_type,
                )
                .map_err(TxError::Msg)?;
                let from_gains = set_holding_quantity(
                    conn,
                    proposal.session_id,
                    proposal.from_actor_id,
                    &proposal.to_resource_type,
                    from_gains_base + proposal.to_quantity,
                )
                .map_err(TxError::Msg)?;

                let to_after = set_holding_quantity(
                    conn,
                    proposal.session_id,
                    proposal.to_actor_id,
                    &proposal.to_resource_type,
                    to_current - proposal.to_quantity,
                )
                .map_err(TxError::Msg)?;
                let to_gains_base = load_holding_quantity(
                    conn,
                    proposal.session_id,
                    proposal.to_actor_id,
                    &proposal.from_resource_type,
                )
                .map_err(TxError::Msg)?;
                let to_gains = set_holding_quantity(
                    conn,
                    proposal.session_id,
                    proposal.to_actor_id,
                    &proposal.from_resource_type,
                    to_gains_base + proposal.from_quantity,
                )
                .map_err(TxError::Msg)?;

                diesel::update(
                    world_genie_trade_proposals::table
                        .filter(world_genie_trade_proposals::id.eq(proposal_id)),
                )
                .set((
                    world_genie_trade_proposals::status.eq("accepted"),
                    world_genie_trade_proposals::updated_at.eq(Utc::now().naive_utc()),
                ))
                .execute(conn)?;

                let trade_world_id = world_genie_sessions::table
                    .filter(world_genie_sessions::id.eq(proposal.session_id))
                    .select(world_genie_sessions::world_id)
                    .first::<Uuid>(conn)?;

                let _ = record_world_event(
                    conn,
                    trade_world_id,
                    EVENT_CODE_GENIE_SESSION_STATE,
                    Some(serde_json::json!({
                        "kind": "resource_trade",
                        "session_id": proposal.session_id,
                        "action": "accepted",
                        "proposal_id": proposal.id,
                        "from_actor_id": proposal.from_actor_id,
                        "to_actor_id": proposal.to_actor_id,
                    })),
                    user_id,
                );

                Ok(vec![from_after, from_gains, to_after, to_gains])
            })
            .map_err(String::from)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    // Return just the final holdings for the two parties (one row per
    // resource type touched — a party's own resource_type row and the
    // counterpart's resource_type row it now also holds).
    Ok(holdings
        .into_iter()
        .map(GraphQLGenieResourceHolding::from)
        .collect())
}

/// Spec 019: the counterpart declines a still-pending proposal — no
/// holdings change, just a status flip to `"rejected"` (the table's
/// CHECK constraint already allowed this value; it was simply never set
/// by any mutation) — mirrors accept_resource_trade_impl's authorization
/// exactly: not the proposer, must control one of the two named actors —
/// so the proposer isn't left waiting on a trade nobody will ever accept.
pub async fn decline_resource_trade_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    proposal_id: Uuid,
) -> GraphQLResult<GraphQLGenieTradeProposal> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let proposal = tokio::task::spawn_blocking(move || -> Result<GenieTradeProposal, String> {
        world_genie_trade_proposals::table
            .filter(world_genie_trade_proposals::id.eq(proposal_id))
            .select(GenieTradeProposal::as_select())
            .first::<GenieTradeProposal>(&mut conn)
            .map_err(|_| "Trade proposal not found".to_string())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    if proposal.status != "pending" {
        return Err(Error::new("This trade proposal is no longer pending"));
    }
    if proposal.created_by == user_id {
        return Err(Error::new("You cannot decline your own trade proposal"));
    }
    let controls_from =
        caller_controls_actor(state, user_id, is_admin, proposal.from_actor_id).await?;
    let controls_to = caller_controls_actor(state, user_id, is_admin, proposal.to_actor_id).await?;
    if !controls_from && !controls_to {
        return Err(Error::new("You are not a party to this trade"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let updated = tokio::task::spawn_blocking(move || -> Result<GenieTradeProposal, String> {
        conn.transaction(
            |conn| -> Result<GenieTradeProposal, diesel::result::Error> {
                let now = Utc::now().naive_utc();
                let updated = diesel::update(
                    world_genie_trade_proposals::table
                        .filter(world_genie_trade_proposals::id.eq(proposal_id)),
                )
                .set((
                    world_genie_trade_proposals::status.eq("rejected"),
                    world_genie_trade_proposals::updated_at.eq(now),
                ))
                .returning(GenieTradeProposal::as_returning())
                .get_result::<GenieTradeProposal>(conn)?;

                let trade_world_id = world_genie_sessions::table
                    .filter(world_genie_sessions::id.eq(updated.session_id))
                    .select(world_genie_sessions::world_id)
                    .first::<Uuid>(conn)?;

                let _ = record_world_event(
                    conn,
                    trade_world_id,
                    EVENT_CODE_GENIE_SESSION_STATE,
                    Some(serde_json::json!({
                        "kind": "resource_trade",
                        "session_id": updated.session_id,
                        "action": "rejected",
                        "proposal_id": updated.id,
                        "from_actor_id": updated.from_actor_id,
                        "to_actor_id": updated.to_actor_id,
                    })),
                    user_id,
                );

                Ok(updated)
            },
        )
        .map_err(|e| format!("Failed to decline trade: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLGenieTradeProposal::from(updated))
}

// ============================================================================
// spendResourceOnPuzzleClock (T035) — self-spend, no counterpart consent
// ============================================================================

pub async fn spend_resource_on_puzzle_clock_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    clock_id: Uuid,
    actor_id: Uuid,
    resource_type: String,
    quantity: i32,
) -> GraphQLResult<GraphQLGeniePuzzleClock> {
    if quantity <= 0 {
        return Err(Error::new("quantity must be greater than zero"));
    }
    require_caller_controls_actor(state, user_id, is_admin, actor_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let (clock, session) = tokio::task::spawn_blocking(
        move || -> Result<(GeniePuzzleClock, GenieSession), String> {
            let clock = load_puzzle_clock_row(&mut conn, clock_id)?;
            let session = load_session_row(&mut conn, clock.session_id)?;
            Ok((clock, session))
        },
    )
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    if session.status != "active" {
        return Err(Error::new("This Genie session has already concluded"));
    }
    if clock.resolved_at.is_some() {
        return Err(Error::new("This Puzzle Clock has already resolved"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = session.world_id;
    let session_id = session.id;

    let updated_clock = tokio::task::spawn_blocking(move || -> Result<GeniePuzzleClock, String> {
        conn.transaction(|conn| -> Result<GeniePuzzleClock, TxError> {
            let current_qty = load_holding_quantity(conn, session_id, actor_id, &resource_type)
                .map_err(TxError::Msg)?;
            if current_qty < quantity {
                return Err(TxError::Msg(
                    "You do not hold enough of this resource".to_string(),
                ));
            }
            set_holding_quantity(
                conn,
                session_id,
                actor_id,
                &resource_type,
                current_qty - quantity,
            )
            .map_err(TxError::Msg)?;

            let now = Utc::now().naive_utc();
            // One resource spent advances the clock by one segment
            // (data-model.md/contracts/genie-session-loop.md leave the
            // exact resource-to-segment ratio to game-balance content
            // decisions; 1:1 is the simplest structurally-valid choice).
            let new_segments = (clock.segments_current + quantity).clamp(0, clock.segments_max);
            let resolves_now = new_segments >= clock.segments_max;
            let resolved_at = if resolves_now { Some(now) } else { None };

            let updated_clock = diesel::update(
                world_genie_puzzle_clocks::table.filter(world_genie_puzzle_clocks::id.eq(clock_id)),
            )
            .set((
                world_genie_puzzle_clocks::segments_current.eq(new_segments),
                world_genie_puzzle_clocks::resolved_at.eq(resolved_at),
                world_genie_puzzle_clocks::updated_at.eq(now),
            ))
            .returning(GeniePuzzleClock::as_returning())
            .get_result::<GeniePuzzleClock>(conn)?;

            let mut session_status_for_event = session.status.clone();
            if resolves_now {
                let all_clocks = world_genie_puzzle_clocks::table
                    .filter(world_genie_puzzle_clocks::session_id.eq(session_id))
                    .select(GeniePuzzleClock::as_select())
                    .load::<GeniePuzzleClock>(conn)?;
                if all_puzzle_clocks_resolved(&all_clocks) {
                    diesel::update(
                        world_genie_sessions::table.filter(world_genie_sessions::id.eq(session_id)),
                    )
                    .set((
                        world_genie_sessions::status.eq("won"),
                        world_genie_sessions::updated_at.eq(now),
                    ))
                    .execute(conn)?;
                    session_status_for_event = "won".to_string();
                }
            }

            let _ = record_world_event(
                conn,
                world_id,
                EVENT_CODE_GENIE_SESSION_STATE,
                Some(serde_json::json!({
                    "kind": "puzzle_clock",
                    "session_id": session_id,
                    "clock_id": updated_clock.id,
                    "segments_current": updated_clock.segments_current,
                    "segments_max": updated_clock.segments_max,
                    "resolved": updated_clock.resolved_at.is_some(),
                    "session_status": session_status_for_event,
                    "spent_by_actor_id": actor_id,
                    "spent_quantity": quantity,
                })),
                user_id,
            );

            Ok(updated_clock)
        })
        .map_err(String::from)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(GraphQLGeniePuzzleClock::from(updated_clock))
}

// ============================================================================
// createShopListing / purchaseFromShop (spec 020, FR-004/FR-005/FR-005a)
// ============================================================================

fn load_stock_quantity(
    conn: &mut PgConnection,
    actor_id: Uuid,
    item_id: Uuid,
) -> Result<i32, diesel::result::Error> {
    world_actor_inventory::table
        .filter(world_actor_inventory::actor_id.eq(actor_id))
        .filter(world_actor_inventory::item_id.eq(item_id))
        .select(world_actor_inventory::quantity)
        .first::<i32>(conn)
        .optional()
        .map(|v| v.unwrap_or(0))
}

fn build_graphql_shop_listing(
    row: GenieShopListing,
    stock_quantity: i32,
) -> GraphQLGenieShopListing {
    GraphQLGenieShopListing {
        id: row.id,
        actor_id: row.actor_id,
        item_id: row.item_id,
        price_kind: row.price_kind,
        price_resource_type: row.price_resource_type,
        price_resource_amount: row.price_resource_amount,
        price_item_id: row.price_item_id,
        price_item_quantity: row.price_item_quantity,
        stock_quantity,
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn create_shop_listing_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
    item_id: Uuid,
    price_kind: GenieShopPriceKind,
    price_resource_type: Option<String>,
    price_resource_amount: Option<i32>,
    price_item_id: Option<Uuid>,
    price_item_quantity: Option<i32>,
) -> GraphQLResult<GraphQLGenieShopListing> {
    let is_resource_price = price_resource_type.is_some() && price_resource_amount.is_some();
    let is_item_price = price_item_id.is_some() && price_item_quantity.is_some();
    if is_resource_price == is_item_price {
        return Err(Error::new(
            "Exactly one of a resource price or an item price must be configured",
        ));
    }
    match price_kind {
        GenieShopPriceKind::Resource if !is_resource_price => {
            return Err(Error::new(
                "priceKind is RESOURCE but no resource price was provided",
            ));
        }
        GenieShopPriceKind::Item if !is_item_price => {
            return Err(Error::new(
                "priceKind is ITEM but no item price was provided",
            ));
        }
        _ => {}
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id = tokio::task::spawn_blocking(move || -> Result<Uuid, String> {
        world_actors::table
            .filter(world_actors::id.eq(actor_id))
            .select(world_actors::world_id)
            .first::<Uuid>(&mut conn)
            .map_err(|_| "Actor not found".to_string())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("Only the GM may create a shop listing"));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let listing = tokio::task::spawn_blocking(move || -> Result<GenieShopListing, String> {
        let new_listing = NewGenieShopListing {
            actor_id,
            item_id,
            price_kind: price_kind.as_db_str().to_string(),
            price_resource_type,
            price_resource_amount,
            price_item_id,
            price_item_quantity,
            created_by: user_id,
        };
        diesel::insert_into(world_genie_shop_listings::table)
            .values(&new_listing)
            .returning(GenieShopListing::as_returning())
            .get_result::<GenieShopListing>(&mut conn)
            .map_err(|e| format!("Failed to create shop listing: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let stock_quantity =
        tokio::task::spawn_blocking(move || load_stock_quantity(&mut conn, actor_id, item_id))
            .await
            .map_err(|_| Error::new("Failed to spawn blocking task"))?
            .map_err(|e| Error::new(format!("Failed to load stock: {e}")))?;

    Ok(build_graphql_shop_listing(listing, stock_quantity))
}

/// FR-005/FR-005a: atomic purchase. Verifies the buyer can afford the
/// listing's configured price (resource balance or held item quantity),
/// deducts/transfers that price, transfers one unit of the listed item,
/// and performs a single atomic conditional stock decrement — all in one
/// transaction, so two buyers racing for the last unit can't both
/// succeed (the losing UPDATE affects 0 rows and the whole transaction
/// rolls back with a clean "out of stock" error).
pub async fn purchase_from_shop_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    listing_id: Uuid,
    buyer_actor_id: Uuid,
) -> GraphQLResult<GraphQLGenieShopListing> {
    require_caller_controls_actor(state, user_id, is_admin, buyer_actor_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let listing = tokio::task::spawn_blocking(move || -> Result<GenieShopListing, String> {
        world_genie_shop_listings::table
            .filter(world_genie_shop_listings::id.eq(listing_id))
            .select(GenieShopListing::as_select())
            .first::<GenieShopListing>(&mut conn)
            .map_err(|_| "Shop listing not found".to_string())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    // A resource-priced purchase needs an active Genie session to draw
    // holdings from — `load_holding_quantity`/`set_holding_quantity` are
    // keyed by session_id. An item-priced (barter) purchase touches only
    // world_actor_inventory and needs no session at all.
    let session_id = if listing.price_kind == "resource" {
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let seller_actor_id = listing.actor_id;
        let world_id = tokio::task::spawn_blocking(move || -> Result<Uuid, String> {
            world_actors::table
                .filter(world_actors::id.eq(seller_actor_id))
                .select(world_actors::world_id)
                .first::<Uuid>(&mut conn)
                .map_err(|_| "Actor not found".to_string())
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let session = tokio::task::spawn_blocking(move || -> Result<GenieSession, String> {
            world_genie_sessions::table
                .filter(world_genie_sessions::world_id.eq(world_id))
                .filter(world_genie_sessions::status.eq("active"))
                .select(GenieSession::as_select())
                .first::<GenieSession>(&mut conn)
                .map_err(|_| "There is no active Genie session for this world".to_string())
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;
        Some(session.id)
    } else {
        None
    };

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let world_id_for_event = {
        let seller_actor_id = listing.actor_id;
        let mut lookup_conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        tokio::task::spawn_blocking(move || -> Result<Uuid, String> {
            world_actors::table
                .filter(world_actors::id.eq(seller_actor_id))
                .select(world_actors::world_id)
                .first::<Uuid>(&mut lookup_conn)
                .map_err(|_| "Actor not found".to_string())
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?
    };

    let updated_listing =
        tokio::task::spawn_blocking(move || -> Result<GenieShopListing, String> {
            conn.transaction(|conn| -> Result<GenieShopListing, TxError> {
                // FR-005a: atomic conditional stock decrement first — if this
                // affects 0 rows, either the listing never had stock or a
                // concurrent purchase just took the last unit; either way,
                // fail cleanly with no other state touched.
                let decremented = diesel::update(
                    world_actor_inventory::table
                        .filter(world_actor_inventory::actor_id.eq(listing.actor_id))
                        .filter(world_actor_inventory::item_id.eq(listing.item_id))
                        .filter(world_actor_inventory::quantity.gt(0)),
                )
                .set((
                    world_actor_inventory::quantity.eq(world_actor_inventory::quantity - 1),
                    world_actor_inventory::updated_at.eq(Utc::now().naive_utc()),
                ))
                .execute(conn)?;

                if decremented == 0 {
                    return Err(TxError::Msg("This item is out of stock".to_string()));
                }

                // Pay the price.
                if listing.price_kind == "resource" {
                    let resource_type = listing.price_resource_type.clone().unwrap_or_default();
                    let amount = listing.price_resource_amount.unwrap_or(0);
                    let session_id = session_id
                        .expect("resource-priced listing always resolves a session_id above");
                    let current =
                        load_holding_quantity(conn, session_id, buyer_actor_id, &resource_type)
                            .map_err(TxError::Msg)?;
                    if current < amount {
                        return Err(TxError::Msg(
                            "You do not have enough of this resource to afford this purchase"
                                .to_string(),
                        ));
                    }
                    set_holding_quantity(
                        conn,
                        session_id,
                        buyer_actor_id,
                        &resource_type,
                        current - amount,
                    )
                    .map_err(TxError::Msg)?;
                } else {
                    let required_item_id = listing
                        .price_item_id
                        .expect("item-priced listing always has price_item_id");
                    let required_qty = listing.price_item_quantity.unwrap_or(0);
                    let held_qty = load_stock_quantity(conn, buyer_actor_id, required_item_id)?;
                    if held_qty < required_qty {
                        return Err(TxError::Msg(
                            "You do not hold the required item(s) to afford this purchase"
                                .to_string(),
                        ));
                    }
                    // Remove the traded-in item from the buyer, add it to the
                    // seller's inventory (the NPC "collects" what it's paid).
                    diesel::update(
                        world_actor_inventory::table
                            .filter(world_actor_inventory::actor_id.eq(buyer_actor_id))
                            .filter(world_actor_inventory::item_id.eq(required_item_id)),
                    )
                    .set((
                        world_actor_inventory::quantity
                            .eq(world_actor_inventory::quantity - required_qty),
                        world_actor_inventory::updated_at.eq(Utc::now().naive_utc()),
                    ))
                    .execute(conn)?;
                    grant_item_to_actor_in_tx(
                        conn,
                        listing.actor_id,
                        required_item_id,
                        required_qty,
                    )?;
                }

                // Transfer the listed item to the buyer.
                grant_item_to_actor_in_tx(conn, buyer_actor_id, listing.item_id, 1)?;

                let _ = record_world_event(
                    conn,
                    world_id_for_event,
                    EVENT_CODE_GENIE_SESSION_STATE,
                    Some(serde_json::json!({
                        "kind": "purchase",
                        "listing_id": listing.id,
                        "buyer_actor_id": buyer_actor_id,
                        "seller_actor_id": listing.actor_id,
                        "item_id": listing.item_id,
                    })),
                    user_id,
                );

                Ok(listing.clone())
            })
            .map_err(String::from)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(Error::new)?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let stock_quantity = tokio::task::spawn_blocking(move || {
        load_stock_quantity(&mut conn, updated_listing.actor_id, updated_listing.item_id)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|e| Error::new(format!("Failed to load stock: {e}")))?;

    Ok(build_graphql_shop_listing(updated_listing, stock_quantity))
}

// ============================================================================
// GraphQL object wiring
// ============================================================================

#[derive(Default)]
pub struct GenieSessionMutation;

#[async_graphql::Object]
impl GenieSessionMutation {
    async fn start_genie_session(
        &self,
        ctx: &Context<'_>,
        input: StartGenieSessionInput,
    ) -> GraphQLResult<GraphQLGenieSession> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        start_genie_session_impl(state, auth_user.user_id, auth_user.is_admin, input).await
    }

    async fn spend_wish(
        &self,
        ctx: &Context<'_>,
        session_id: Uuid,
        narrative_effect: String,
    ) -> GraphQLResult<GraphQLGenieSession> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        spend_wish_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            session_id,
            narrative_effect,
        )
        .await
    }

    async fn advance_doom_clock(
        &self,
        ctx: &Context<'_>,
        session_id: Uuid,
        delta: i32,
    ) -> GraphQLResult<GraphQLGenieSession> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        advance_doom_clock_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            session_id,
            delta,
        )
        .await
    }

    async fn create_puzzle_clock(
        &self,
        ctx: &Context<'_>,
        session_id: Uuid,
        label: String,
        segments_max: i32,
    ) -> GraphQLResult<GraphQLGeniePuzzleClock> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        create_puzzle_clock_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            session_id,
            label,
            segments_max,
        )
        .await
    }

    async fn advance_puzzle_clock(
        &self,
        ctx: &Context<'_>,
        clock_id: Uuid,
        delta: i32,
        actor_id: Option<Uuid>,
    ) -> GraphQLResult<GraphQLGeniePuzzleClock> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        advance_puzzle_clock_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            clock_id,
            delta,
            actor_id,
        )
        .await
    }

    async fn grant_session_resource(
        &self,
        ctx: &Context<'_>,
        session_id: Uuid,
        actor_id: Uuid,
        resource_type: String,
        amount: i32,
    ) -> GraphQLResult<GraphQLGenieResourceHolding> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        grant_session_resource_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            session_id,
            actor_id,
            resource_type,
            amount,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_shop_listing(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
        item_id: Uuid,
        price_kind: GenieShopPriceKind,
        price_resource_type: Option<String>,
        price_resource_amount: Option<i32>,
        price_item_id: Option<Uuid>,
        price_item_quantity: Option<i32>,
    ) -> GraphQLResult<GraphQLGenieShopListing> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        create_shop_listing_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            actor_id,
            item_id,
            price_kind,
            price_resource_type,
            price_resource_amount,
            price_item_id,
            price_item_quantity,
        )
        .await
    }

    async fn purchase_from_shop(
        &self,
        ctx: &Context<'_>,
        listing_id: Uuid,
        buyer_actor_id: Uuid,
    ) -> GraphQLResult<GraphQLGenieShopListing> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        purchase_from_shop_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            listing_id,
            buyer_actor_id,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn configure_puzzle_clock_reward(
        &self,
        ctx: &Context<'_>,
        clock_id: Uuid,
        trigger_segment: i32,
        reward_resource_type: Option<String>,
        reward_resource_amount: Option<i32>,
        reward_item_id: Option<Uuid>,
        reward_item_quantity: Option<i32>,
        recipient_mode: GenieRewardRecipientMode,
    ) -> GraphQLResult<GraphQLGeniePuzzleClockReward> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        configure_puzzle_clock_reward_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            clock_id,
            trigger_segment,
            reward_resource_type,
            reward_resource_amount,
            reward_item_id,
            reward_item_quantity,
            recipient_mode,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn propose_resource_trade(
        &self,
        ctx: &Context<'_>,
        session_id: Uuid,
        from_actor_id: Uuid,
        from_resource_type: String,
        from_quantity: i32,
        to_actor_id: Uuid,
        to_resource_type: String,
        to_quantity: i32,
    ) -> GraphQLResult<GraphQLGenieTradeProposal> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        propose_resource_trade_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            session_id,
            from_actor_id,
            from_resource_type,
            from_quantity,
            to_actor_id,
            to_resource_type,
            to_quantity,
        )
        .await
    }

    async fn accept_resource_trade(
        &self,
        ctx: &Context<'_>,
        proposal_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLGenieResourceHolding>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        accept_resource_trade_impl(state, auth_user.user_id, auth_user.is_admin, proposal_id).await
    }

    async fn decline_resource_trade(
        &self,
        ctx: &Context<'_>,
        proposal_id: Uuid,
    ) -> GraphQLResult<GraphQLGenieTradeProposal> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        decline_resource_trade_impl(state, auth_user.user_id, auth_user.is_admin, proposal_id).await
    }

    async fn spend_resource_on_puzzle_clock(
        &self,
        ctx: &Context<'_>,
        clock_id: Uuid,
        actor_id: Uuid,
        resource_type: String,
        quantity: i32,
    ) -> GraphQLResult<GraphQLGeniePuzzleClock> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        spend_resource_on_puzzle_clock_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            clock_id,
            actor_id,
            resource_type,
            quantity,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

    fn insert_test_actor(
        conn: &mut PgConnection,
        world_id: Uuid,
        scene_id: Uuid,
        owner_id: Uuid,
    ) -> Uuid {
        use crate::schema::world_actors;
        let now = Utc::now().naive_utc();
        let actor_id = Uuid::now_v7();
        diesel::insert_into(world_actors::table)
            .values((
                world_actors::id.eq(actor_id),
                world_actors::world_id.eq(world_id),
                world_actors::scene_id.eq(scene_id),
                world_actors::actor_type.eq("character"),
                world_actors::game_system_id.eq("genie"),
                world_actors::label.eq("Test Actor"),
                world_actors::created_by.eq(owner_id),
                world_actors::owned_by.eq(owner_id),
                world_actors::is_public.eq(false),
                world_actors::is_npc.eq(false),
                world_actors::created_at.eq(now),
                world_actors::updated_at.eq(now),
                world_actors::available_for_claim.eq(false),
            ))
            .execute(conn)
            .expect("failed to insert test actor");
        actor_id
    }

    fn insert_test_scene(conn: &mut PgConnection, world_id: Uuid, owner_id: Uuid) -> Uuid {
        use crate::schema::scenes;
        let now = Utc::now().naive_utc();
        let scene_id = Uuid::now_v7();
        diesel::insert_into(scenes::table)
            .values((
                scenes::scene_id.eq(scene_id),
                scenes::world_id.eq(world_id),
                scenes::owner_id.eq(owner_id),
                scenes::name.eq("Test Scene"),
                scenes::created_at.eq(now),
                scenes::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("failed to insert test scene");
        scene_id
    }

    async fn setup_active_session(state: &AppState) -> (Uuid, Uuid, Uuid, Uuid) {
        // Returns (world_id, owner_id/gm, player_id, session_id)
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let session = start_genie_session_impl(
            state,
            owner_id,
            false,
            StartGenieSessionInput {
                world_id,
                doom_clock_max: 4,
            },
        )
        .await
        .expect("GM should be able to start a session");

        (world_id, owner_id, player_id, session.id)
    }

    #[tokio::test]
    async fn only_gm_can_start_a_session() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let denied = start_genie_session_impl(
            &state,
            player_id,
            false,
            StartGenieSessionInput {
                world_id,
                doom_clock_max: 4,
            },
        )
        .await;
        assert!(
            denied.is_err(),
            "a non-GM caller must not be able to start a session"
        );
    }

    #[tokio::test]
    async fn fresh_session_starts_with_three_wishes_and_zero_doom_segments() {
        let state = test_app_state();
        let (_world_id, _owner_id, _player_id, session_id) = setup_active_session(&state).await;

        let mut conn = state.db_pool.get().unwrap();
        let session = load_session_row(&mut conn, session_id).unwrap();
        assert_eq!(
            session.wishes_remaining, 3,
            "FR-013: Session Wish Pool starts at 3"
        );
        assert_eq!(session.doom_clock_current, 0);
        assert_eq!(session.status, "active");
    }

    #[tokio::test]
    async fn non_gm_cannot_spend_a_wish_or_advance_clocks() {
        let state = test_app_state();
        let (_world_id, _owner_id, player_id, session_id) = setup_active_session(&state).await;

        let result = spend_wish_impl(
            &state,
            player_id,
            false,
            session_id,
            "Undo the failed roll".to_string(),
        )
        .await;
        assert!(
            result.is_err(),
            "a non-GM caller must not be able to spend a wish"
        );

        let result = advance_doom_clock_impl(&state, player_id, false, session_id, 1).await;
        assert!(
            result.is_err(),
            "a non-GM caller must not be able to advance the Doom Clock"
        );

        let result = create_puzzle_clock_impl(
            &state,
            player_id,
            false,
            session_id,
            "Escape the vault".to_string(),
            4,
        )
        .await;
        assert!(
            result.is_err(),
            "a non-GM caller must not be able to create a Puzzle Clock"
        );
    }

    #[tokio::test]
    async fn spend_wish_decrements_pool_and_rejects_when_empty() {
        let state = test_app_state();
        let (_world_id, owner_id, _player_id, session_id) = setup_active_session(&state).await;

        for i in (0..3).rev() {
            let session =
                spend_wish_impl(&state, owner_id, false, session_id, format!("Effect {i}"))
                    .await
                    .unwrap();
            assert_eq!(session.wishes_remaining, i);
        }

        let result = spend_wish_impl(
            &state,
            owner_id,
            false,
            session_id,
            "One too many".to_string(),
        )
        .await;
        assert!(
            result.is_err(),
            "spending a wish from an empty pool must be rejected"
        );
    }

    #[tokio::test]
    async fn advance_doom_clock_sets_lost_when_it_fills() {
        let state = test_app_state();
        let (_world_id, owner_id, _player_id, session_id) = setup_active_session(&state).await;

        let session = advance_doom_clock_impl(&state, owner_id, false, session_id, 4)
            .await
            .unwrap();
        assert_eq!(session.doom_clock_current, 4);
        assert!(matches!(session.status, GenieSessionStatus::Lost));

        // Further advancement is rejected once the session has concluded.
        let result = advance_doom_clock_impl(&state, owner_id, false, session_id, 1).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn win_takes_precedence_over_loss_in_the_same_action_window() {
        // Edge case (spec.md): the last active Puzzle Clock resolving
        // takes precedence over a Doom Clock fill that would otherwise
        // be evaluated afterward.
        let state = test_app_state();
        let (_world_id, owner_id, _player_id, session_id) = setup_active_session(&state).await;

        let clock = create_puzzle_clock_impl(
            &state,
            owner_id,
            false,
            session_id,
            "Only clock".to_string(),
            2,
        )
        .await
        .unwrap();

        // Resolve the only Puzzle Clock first — this must fire the win.
        let resolved = advance_puzzle_clock_impl(&state, owner_id, false, clock.id, 2, None)
            .await
            .unwrap();
        assert!(resolved.resolved_at.is_some());

        let mut conn = state.db_pool.get().unwrap();
        let session = load_session_row(&mut conn, session_id).unwrap();
        assert_eq!(
            session.status, "won",
            "resolving the last Puzzle Clock must win the session"
        );
        drop(conn);

        // The Doom Clock is now moot: advancing it must be rejected since
        // the session already concluded (won), never flipping it to lost.
        let result = advance_doom_clock_impl(&state, owner_id, false, session_id, 4).await;
        assert!(result.is_err());
        let mut conn = state.db_pool.get().unwrap();
        let session = load_session_row(&mut conn, session_id).unwrap();
        assert_eq!(
            session.status, "won",
            "a win must not be overwritten by a later loss check"
        );
    }

    #[tokio::test]
    async fn acceptresourcetrade_rejects_self_accept() {
        let state = test_app_state();
        let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;

        let mut conn = state.db_pool.get().unwrap();
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_a = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let actor_b = insert_test_actor(&mut conn, world_id, scene_id, player_id);
        // Seed actor_a with insight to trade away.
        set_holding_quantity(&mut conn, session_id, actor_a, "insight", 5).unwrap();
        drop(conn);

        let proposal = propose_resource_trade_impl(
            &state,
            owner_id,
            false,
            session_id,
            actor_a,
            "insight".to_string(),
            2,
            actor_b,
            "favor".to_string(),
            1,
        )
        .await
        .expect("actor_a's controller should be able to propose");

        let self_accept = accept_resource_trade_impl(&state, owner_id, false, proposal.id).await;
        assert!(
            self_accept.is_err(),
            "the proposer must not be able to accept their own proposal"
        );
    }

    #[tokio::test]
    async fn decline_resource_trade_rejects_self_decline_and_succeeds_for_the_counterpart() {
        let state = test_app_state();
        let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;

        let mut conn = state.db_pool.get().unwrap();
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_a = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let actor_b = insert_test_actor(&mut conn, world_id, scene_id, player_id);
        drop(conn);

        let proposal = propose_resource_trade_impl(
            &state,
            owner_id,
            false,
            session_id,
            actor_a,
            "insight".to_string(),
            2,
            actor_b,
            "favor".to_string(),
            1,
        )
        .await
        .unwrap();

        let self_decline = decline_resource_trade_impl(&state, owner_id, false, proposal.id).await;
        assert!(
            self_decline.is_err(),
            "the proposer must not be able to decline their own proposal"
        );

        let declined = decline_resource_trade_impl(&state, player_id, false, proposal.id)
            .await
            .unwrap();
        assert_eq!(declined.status, "rejected");

        let re_decline = decline_resource_trade_impl(&state, player_id, false, proposal.id).await;
        assert!(
            re_decline.is_err(),
            "an already-declined proposal must not be declinable again"
        );

        let accept_after_decline =
            accept_resource_trade_impl(&state, player_id, false, proposal.id).await;
        assert!(
            accept_after_decline.is_err(),
            "a declined proposal must not be acceptable"
        );
    }

    #[tokio::test]
    async fn accept_resource_trade_rejects_insufficient_holding_and_succeeds_when_funded() {
        let state = test_app_state();
        let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;

        let mut conn = state.db_pool.get().unwrap();
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_a = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let actor_b = insert_test_actor(&mut conn, world_id, scene_id, player_id);
        drop(conn);

        // actor_a proposes a trade it cannot afford (0 insight held).
        let proposal = propose_resource_trade_impl(
            &state,
            owner_id,
            false,
            session_id,
            actor_a,
            "insight".to_string(),
            3,
            actor_b,
            "favor".to_string(),
            1,
        )
        .await
        .unwrap();

        let underfunded = accept_resource_trade_impl(&state, player_id, false, proposal.id).await;
        assert!(
            underfunded.is_err(),
            "an insufficient holding must be rejected"
        );

        // Fund actor_a and actor_b, then retry with a fresh proposal
        // (the first attempt's proposal stays 'pending' since it never
        // committed — accept it again now that funds exist).
        let mut conn = state.db_pool.get().unwrap();
        set_holding_quantity(&mut conn, session_id, actor_a, "insight", 3).unwrap();
        set_holding_quantity(&mut conn, session_id, actor_b, "favor", 1).unwrap();
        drop(conn);

        let holdings = accept_resource_trade_impl(&state, player_id, false, proposal.id)
            .await
            .unwrap();
        assert!(!holdings.is_empty());

        let mut conn = state.db_pool.get().unwrap();
        let a_favor = load_holding_quantity(&mut conn, session_id, actor_a, "favor").unwrap();
        let b_insight = load_holding_quantity(&mut conn, session_id, actor_b, "insight").unwrap();
        assert_eq!(a_favor, 1, "actor_a should now hold the favor it received");
        assert_eq!(
            b_insight, 3,
            "actor_b should now hold the insight it received"
        );
    }

    #[tokio::test]
    async fn spend_resource_on_puzzle_clock_rejects_insufficient_holding_and_advances_when_funded()
    {
        let state = test_app_state();
        let (world_id, owner_id, _player_id, session_id) = setup_active_session(&state).await;

        let mut conn = state.db_pool.get().unwrap();
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor_a = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        drop(conn);

        let clock =
            create_puzzle_clock_impl(&state, owner_id, false, session_id, "Vault".to_string(), 3)
                .await
                .unwrap();

        let insufficient = spend_resource_on_puzzle_clock_impl(
            &state,
            owner_id,
            false,
            clock.id,
            actor_a,
            "essence".to_string(),
            2,
        )
        .await;
        assert!(
            insufficient.is_err(),
            "spending more than held must be rejected"
        );

        let mut conn = state.db_pool.get().unwrap();
        set_holding_quantity(&mut conn, session_id, actor_a, "essence", 2).unwrap();
        drop(conn);

        let updated = spend_resource_on_puzzle_clock_impl(
            &state,
            owner_id,
            false,
            clock.id,
            actor_a,
            "essence".to_string(),
            2,
        )
        .await
        .unwrap();
        assert_eq!(updated.segments_current, 2);

        let mut conn = state.db_pool.get().unwrap();
        let remaining = load_holding_quantity(&mut conn, session_id, actor_a, "essence").unwrap();
        assert_eq!(remaining, 0);
    }

    #[tokio::test]
    async fn a_player_who_only_claimed_their_character_controls_it_for_session_resources() {
        // Spec 019 regression guard: caller_controls_actor previously
        // checked only world_actors.owned_by, which never changes on
        // claim (spec 017's real player-onboarding path — the GM creates
        // the actor, a player then claims it via world_actor_claims).
        // Found live: a claimed-not-owned player got "You do not control
        // this actor" on every Session Resource action for their own PC.
        use crate::graphql::mutations_actor_claims::claim_actor_impl;

        let state = test_app_state();
        let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;

        let mut conn = state.db_pool.get().unwrap();
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        // Owned by the GM, not the player — only a claim will follow.
        let claimed_actor = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let other_actor = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        diesel::update(
            crate::schema::world_actors::table
                .filter(crate::schema::world_actors::id.eq(claimed_actor)),
        )
        .set(crate::schema::world_actors::available_for_claim.eq(true))
        .execute(&mut conn)
        .unwrap();
        drop(conn);

        claim_actor_impl(&state, player_id, world_id, claimed_actor)
            .await
            .expect("player should be able to claim an available actor");

        // Before the fix this failed with "You do not control this actor".
        let proposal = propose_resource_trade_impl(
            &state,
            player_id,
            false,
            session_id,
            claimed_actor,
            "insight".to_string(),
            1,
            other_actor,
            "favor".to_string(),
            1,
        )
        .await;
        assert!(
            proposal.is_ok(),
            "a player who claimed (not owns) their character should control it for Session Resource actions: {:?}",
            proposal.err()
        );
    }

    // ========================================================================
    // Spec 020: grantSessionResource (FR-001), resource carryover (FR-003)
    // ========================================================================

    #[tokio::test]
    async fn grant_session_resource_increases_holding_and_requires_gm_and_active_session() {
        let state = test_app_state();
        let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;
        let mut conn = state.db_pool.get().unwrap();
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor = insert_test_actor(&mut conn, world_id, scene_id, player_id);
        drop(conn);

        let holding = grant_session_resource_impl(
            &state,
            owner_id,
            false,
            session_id,
            actor,
            "essence".to_string(),
            3,
        )
        .await
        .expect("GM should be able to grant a resource");
        assert_eq!(holding.quantity, 3);

        // Non-GM caller rejected (Scenario 4).
        let denied = grant_session_resource_impl(
            &state,
            player_id,
            false,
            session_id,
            actor,
            "essence".to_string(),
            1,
        )
        .await;
        assert!(
            denied.is_err(),
            "a non-GM caller must not be able to grant a resource"
        );
    }

    #[tokio::test]
    async fn grant_session_resource_rejects_when_no_active_session() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        drop(conn);

        // No session started at all — grant must be rejected (Scenario 3).
        let session = start_genie_session_impl(
            &state,
            owner_id,
            false,
            StartGenieSessionInput {
                world_id,
                doom_clock_max: 2,
            },
        )
        .await
        .unwrap();
        // Force the session to a concluded state so "no active session" is exercised.
        let mut conn = state.db_pool.get().unwrap();
        diesel::update(world_genie_sessions::table.filter(world_genie_sessions::id.eq(session.id)))
            .set(world_genie_sessions::status.eq("won"))
            .execute(&mut conn)
            .unwrap();
        drop(conn);

        let denied = grant_session_resource_impl(
            &state,
            owner_id,
            false,
            session.id,
            actor,
            "essence".to_string(),
            1,
        )
        .await;
        assert!(
            denied.is_err(),
            "granting against a concluded session must be rejected"
        );
    }

    #[tokio::test]
    async fn resource_carryover_copies_holdings_when_enabled_and_resets_when_disabled() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let actor = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        drop(conn);

        let first_session = start_genie_session_impl(
            &state,
            owner_id,
            false,
            StartGenieSessionInput {
                world_id,
                doom_clock_max: 2,
            },
        )
        .await
        .unwrap();
        grant_session_resource_impl(
            &state,
            owner_id,
            false,
            first_session.id,
            actor,
            "favor".to_string(),
            5,
        )
        .await
        .unwrap();
        let mut conn = state.db_pool.get().unwrap();
        diesel::update(
            world_genie_sessions::table.filter(world_genie_sessions::id.eq(first_session.id)),
        )
        .set(world_genie_sessions::status.eq("won"))
        .execute(&mut conn)
        .unwrap();
        drop(conn);

        // Carryover disabled (default): the new session starts empty.
        let second_session = start_genie_session_impl(
            &state,
            owner_id,
            false,
            StartGenieSessionInput {
                world_id,
                doom_clock_max: 2,
            },
        )
        .await
        .unwrap();
        let mut conn = state.db_pool.get().unwrap();
        let qty = load_holding_quantity(&mut conn, second_session.id, actor, "favor").unwrap();
        assert_eq!(
            qty, 0,
            "carryover disabled by default — new session should start at 0"
        );
        diesel::update(
            world_genie_sessions::table.filter(world_genie_sessions::id.eq(second_session.id)),
        )
        .set(world_genie_sessions::status.eq("won"))
        .execute(&mut conn)
        .unwrap();
        drop(conn);

        // Enable carryover, end the second session, start a third — holdings should carry.
        let mut conn = state.db_pool.get().unwrap();
        diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::genie_resource_carryover_enabled.eq(true))
            .execute(&mut conn)
            .unwrap();
        drop(conn);

        let third_session = start_genie_session_impl(
            &state,
            owner_id,
            false,
            StartGenieSessionInput {
                world_id,
                doom_clock_max: 2,
            },
        )
        .await
        .unwrap();
        let mut conn = state.db_pool.get().unwrap();
        let qty = load_holding_quantity(&mut conn, third_session.id, actor, "favor").unwrap();
        assert_eq!(
            qty, 0,
            "carryover should only copy the immediately prior session's holdings, which were 0"
        );
    }

    // ========================================================================
    // Spec 020: createShopListing / purchaseFromShop (FR-004/FR-005/FR-005a)
    // ========================================================================

    fn insert_test_item(
        conn: &mut PgConnection,
        world_id: Uuid,
        owner_id: Uuid,
        name: &str,
    ) -> Uuid {
        use crate::schema::world_items;
        let now = Utc::now().naive_utc();
        let item_id = Uuid::now_v7();
        diesel::insert_into(world_items::table)
            .values((
                world_items::id.eq(item_id),
                world_items::world_id.eq(world_id),
                world_items::name.eq(name),
                world_items::created_by.eq(owner_id),
                world_items::created_at.eq(now),
                world_items::updated_at.eq(now),
            ))
            .execute(conn)
            .expect("failed to insert test item");
        item_id
    }

    fn stock_item(conn: &mut PgConnection, actor_id: Uuid, item_id: Uuid, quantity: i32) {
        use crate::schema::world_actor_inventory;
        diesel::insert_into(world_actor_inventory::table)
            .values((
                world_actor_inventory::actor_id.eq(actor_id),
                world_actor_inventory::item_id.eq(item_id),
                world_actor_inventory::item_name_snapshot.eq("Test Item"),
                world_actor_inventory::quantity.eq(quantity),
            ))
            .execute(conn)
            .expect("failed to stock item");
    }

    #[tokio::test]
    async fn purchase_from_shop_resource_priced_happy_path_and_insufficient_funds() {
        let state = test_app_state();
        let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;
        let mut conn = state.db_pool.get().unwrap();
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let npc = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let buyer = insert_test_actor(&mut conn, world_id, scene_id, player_id);
        let item = insert_test_item(&mut conn, world_id, owner_id, "Rusty Lantern");
        stock_item(&mut conn, npc, item, 1);
        drop(conn);

        let listing = create_shop_listing_impl(
            &state,
            owner_id,
            false,
            npc,
            item,
            GenieShopPriceKind::Resource,
            Some("insight".to_string()),
            Some(2),
            None,
            None,
        )
        .await
        .expect("GM should be able to create a listing");
        assert_eq!(listing.stock_quantity, 1);

        // Insufficient funds (Scenario 2).
        let denied = purchase_from_shop_impl(&state, player_id, false, listing.id, buyer).await;
        assert!(
            denied.is_err(),
            "buyer with insufficient Insight must be rejected"
        );

        // Fund the buyer, then purchase succeeds (Scenario 1).
        grant_session_resource_impl(
            &state,
            owner_id,
            false,
            session_id,
            buyer,
            "insight".to_string(),
            2,
        )
        .await
        .unwrap();
        let purchased = purchase_from_shop_impl(&state, player_id, false, listing.id, buyer)
            .await
            .expect("funded buyer should be able to purchase");
        assert_eq!(
            purchased.stock_quantity, 0,
            "last unit purchased — stock decremented to 0"
        );

        let mut conn = state.db_pool.get().unwrap();
        let buyer_insight = load_holding_quantity(&mut conn, session_id, buyer, "insight").unwrap();
        assert_eq!(
            buyer_insight, 0,
            "2 Insight deducted for a 2-Insight purchase"
        );
        let buyer_stock = load_stock_quantity(&mut conn, buyer, item).unwrap();
        assert_eq!(
            buyer_stock, 1,
            "purchased item transferred into buyer's inventory"
        );
    }

    #[tokio::test]
    async fn purchase_from_shop_barter_happy_path_and_missing_item() {
        let state = test_app_state();
        let (world_id, owner_id, player_id, _session_id) = setup_active_session(&state).await;
        let mut conn = state.db_pool.get().unwrap();
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let npc = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let buyer = insert_test_actor(&mut conn, world_id, scene_id, player_id);
        let lantern = insert_test_item(&mut conn, world_id, owner_id, "Rusty Lantern");
        let flask = insert_test_item(&mut conn, world_id, owner_id, "Sealed Flask");
        stock_item(&mut conn, npc, lantern, 1);
        drop(conn);

        let listing = create_shop_listing_impl(
            &state,
            owner_id,
            false,
            npc,
            lantern,
            GenieShopPriceKind::Item,
            None,
            None,
            Some(flask),
            Some(1),
        )
        .await
        .expect("GM should be able to create a barter listing");

        // Buyer doesn't hold the flask yet (Scenario 4).
        let denied = purchase_from_shop_impl(&state, player_id, false, listing.id, buyer).await;
        assert!(
            denied.is_err(),
            "buyer without the required barter item must be rejected"
        );

        let mut conn = state.db_pool.get().unwrap();
        stock_item(&mut conn, buyer, flask, 1);
        drop(conn);

        purchase_from_shop_impl(&state, player_id, false, listing.id, buyer)
            .await
            .expect("buyer holding the barter item should be able to purchase");

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            load_stock_quantity(&mut conn, buyer, flask).unwrap(),
            0,
            "flask traded away"
        );
        assert_eq!(
            load_stock_quantity(&mut conn, buyer, lantern).unwrap(),
            1,
            "lantern received"
        );
        assert_eq!(
            load_stock_quantity(&mut conn, npc, flask).unwrap(),
            1,
            "NPC collected the traded-in flask"
        );
    }

    #[tokio::test]
    async fn purchase_from_shop_last_unit_race_only_one_buyer_succeeds() {
        // FR-005a: two buyers racing for the last unit — exactly one
        // succeeds, the other gets a clean "out of stock" error, no
        // partial state change on the loser.
        let state = test_app_state();
        let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;
        let mut conn = state.db_pool.get().unwrap();
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let npc = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let player_two = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_two, "Player");
        let buyer_a = insert_test_actor(&mut conn, world_id, scene_id, player_id);
        let buyer_b = insert_test_actor(&mut conn, world_id, scene_id, player_two);
        let item = insert_test_item(&mut conn, world_id, owner_id, "Sole Survivor Blade");
        stock_item(&mut conn, npc, item, 1);
        drop(conn);

        let listing = create_shop_listing_impl(
            &state,
            owner_id,
            false,
            npc,
            item,
            GenieShopPriceKind::Resource,
            Some("insight".to_string()),
            Some(1),
            None,
            None,
        )
        .await
        .unwrap();

        grant_session_resource_impl(
            &state,
            owner_id,
            false,
            session_id,
            buyer_a,
            "insight".to_string(),
            1,
        )
        .await
        .unwrap();
        grant_session_resource_impl(
            &state,
            owner_id,
            false,
            session_id,
            buyer_b,
            "insight".to_string(),
            1,
        )
        .await
        .unwrap();

        let (result_a, result_b) = tokio::join!(
            purchase_from_shop_impl(&state, player_id, false, listing.id, buyer_a),
            purchase_from_shop_impl(&state, player_two, false, listing.id, buyer_b),
        );

        let successes = [&result_a, &result_b].iter().filter(|r| r.is_ok()).count();
        assert_eq!(
            successes, 1,
            "exactly one of the two concurrent purchases should succeed"
        );

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            load_stock_quantity(&mut conn, npc, item).unwrap(),
            0,
            "stock never goes negative or double-decrements"
        );

        // The loser must have no partial state change.
        if result_a.is_err() {
            assert_eq!(
                load_holding_quantity(&mut conn, session_id, buyer_a, "insight").unwrap(),
                1
            );
            assert_eq!(load_stock_quantity(&mut conn, buyer_a, item).unwrap(), 0);
        } else {
            assert_eq!(
                load_holding_quantity(&mut conn, session_id, buyer_b, "insight").unwrap(),
                1
            );
            assert_eq!(load_stock_quantity(&mut conn, buyer_b, item).unwrap(), 0);
        }
    }

    // ========================================================================
    // Spec 020: configurePuzzleClockReward / advancePuzzleClock actorId (FR-006/FR-006a)
    // ========================================================================

    #[tokio::test]
    async fn per_segment_reward_grants_exactly_once_per_advance_not_a_lump_sum() {
        let state = test_app_state();
        let (world_id, owner_id, _player_id, session_id) = setup_active_session(&state).await;
        let mut conn = state.db_pool.get().unwrap();
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let smith = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let dagger = insert_test_item(&mut conn, world_id, owner_id, "Dagger");
        drop(conn);

        let clock = create_puzzle_clock_impl(
            &state,
            owner_id,
            false,
            session_id,
            "Forge Daggers".to_string(),
            3,
        )
        .await
        .unwrap();

        for segment in 1..=3 {
            configure_puzzle_clock_reward_impl(
                &state,
                owner_id,
                false,
                clock.id,
                segment,
                None,
                None,
                Some(dagger),
                Some(1),
                GenieRewardRecipientMode::TriggeringActor,
            )
            .await
            .unwrap();
        }

        for _ in 0..3 {
            advance_puzzle_clock_impl(&state, owner_id, false, clock.id, 1, Some(smith))
                .await
                .unwrap();
        }

        let mut conn = state.db_pool.get().unwrap();
        let dagger_count = load_stock_quantity(&mut conn, smith, dagger).unwrap();
        assert_eq!(
            dagger_count, 3,
            "one dagger granted per advance, not a lump sum"
        );
    }

    #[tokio::test]
    async fn single_final_segment_reward_grants_once_split_across_party() {
        let state = test_app_state();
        let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;
        let mut conn = state.db_pool.get().unwrap();
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let pc_a = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let pc_b = insert_test_actor(&mut conn, world_id, scene_id, player_id);
        drop(conn);

        let clock = create_puzzle_clock_impl(
            &state,
            owner_id,
            false,
            session_id,
            "Recover the Sealed Lamp".to_string(),
            4,
        )
        .await
        .unwrap();
        configure_puzzle_clock_reward_impl(
            &state,
            owner_id,
            false,
            clock.id,
            4,
            Some("favor".to_string()),
            Some(2),
            None,
            None,
            GenieRewardRecipientMode::WholeParty,
        )
        .await
        .unwrap();

        advance_puzzle_clock_impl(&state, owner_id, false, clock.id, 4, None)
            .await
            .unwrap();

        let mut conn = state.db_pool.get().unwrap();
        let a_favor = load_holding_quantity(&mut conn, session_id, pc_a, "favor").unwrap();
        let b_favor = load_holding_quantity(&mut conn, session_id, pc_b, "favor").unwrap();
        assert_eq!(
            a_favor + b_favor,
            2,
            "the full configured amount is split across the party, none lost"
        );
        assert!(
            a_favor >= 1 && b_favor >= 1,
            "both party members should receive a share of an even split"
        );
    }

    #[tokio::test]
    async fn zero_configured_rewards_clock_behaves_unchanged() {
        let state = test_app_state();
        let (_world_id, owner_id, _player_id, session_id) = setup_active_session(&state).await;
        let clock = create_puzzle_clock_impl(
            &state,
            owner_id,
            false,
            session_id,
            "Plain Clock".to_string(),
            2,
        )
        .await
        .unwrap();

        let resolved = advance_puzzle_clock_impl(&state, owner_id, false, clock.id, 2, None)
            .await
            .unwrap();
        assert!(
            resolved.resolved_at.is_some(),
            "a zero-reward clock still resolves normally"
        );
    }

    #[tokio::test]
    async fn triggering_actor_reward_falls_back_to_whole_party_when_no_actor_id_supplied() {
        // FR-006a: a plain advancePuzzleClock call with no actorId hits a
        // triggering_actor-mode reward — it must fall back to whole-party
        // split rather than failing or crediting no one.
        let state = test_app_state();
        let (world_id, owner_id, player_id, session_id) = setup_active_session(&state).await;
        let mut conn = state.db_pool.get().unwrap();
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let pc_a = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let pc_b = insert_test_actor(&mut conn, world_id, scene_id, player_id);
        drop(conn);

        let clock = create_puzzle_clock_impl(
            &state,
            owner_id,
            false,
            session_id,
            "Untended Forge".to_string(),
            2,
        )
        .await
        .unwrap();
        configure_puzzle_clock_reward_impl(
            &state,
            owner_id,
            false,
            clock.id,
            1,
            Some("essence".to_string()),
            Some(2),
            None,
            None,
            GenieRewardRecipientMode::TriggeringActor,
        )
        .await
        .unwrap();

        // No actorId supplied — plain GM "Advance" click.
        advance_puzzle_clock_impl(&state, owner_id, false, clock.id, 1, None)
            .await
            .unwrap();

        let mut conn = state.db_pool.get().unwrap();
        let a_essence = load_holding_quantity(&mut conn, session_id, pc_a, "essence").unwrap();
        let b_essence = load_holding_quantity(&mut conn, session_id, pc_b, "essence").unwrap();
        assert_eq!(
            a_essence + b_essence,
            2,
            "an unattributed triggering_actor reward must fall back to a whole-party grant, not be dropped"
        );
    }
}
