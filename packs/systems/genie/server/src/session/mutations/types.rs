//! The GraphQL-facing types the session loop returns.

use super::*;

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

pub(crate) fn build_graphql_session(
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
    pub(crate) fn as_db_str(self) -> &'static str {
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
    pub(crate) fn as_db_str(self) -> &'static str {
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
