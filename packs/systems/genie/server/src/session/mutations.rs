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
use diesel::prelude::*;
use diesel::PgConnection;
use uuid::Uuid;

use super::models::{
    GeniePuzzleClock, GeniePuzzleClockReward, GenieResourceHolding, GenieSession, GenieShopListing,
    GenieTradeProposal, NewGeniePuzzleClock, NewGeniePuzzleClockReward, NewGenieSession,
    NewGenieShopListing, NewGenieTradeProposal,
};
use super::schema::{
    world_genie_puzzle_clock_rewards, world_genie_puzzle_clocks, world_genie_resource_holdings,
    world_genie_sessions, world_genie_shop_listings, world_genie_trade_proposals,
};
use thunderforge_server::auth::world_membership::is_dm_of_world;
use thunderforge_server::auth::world_membership::require_world_member;
use thunderforge_server::graphql::{app_state, authenticated_user};
use thunderforge_server::schema::{world_actor_inventory, world_actors, world_items, worlds};
use thunderforge_server::state::AppState;
use thunderforge_server::world_events::record_world_event;

use super::EVENT_CODE_GENIE_SESSION_STATE;

#[path = "mutations/types.rs"]
pub mod types;
pub use types::*;

#[path = "mutations/helpers.rs"]
pub mod helpers;
pub(crate) use helpers::*;

#[path = "mutations/clocks.rs"]
pub mod clocks;
pub use clocks::*;

#[path = "mutations/trades.rs"]
pub mod trades;
pub use trades::*;

#[path = "mutations/shop.rs"]
pub mod shop;
pub use shop::*;

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
#[path = "mutations_tests.rs"]
mod tests;
