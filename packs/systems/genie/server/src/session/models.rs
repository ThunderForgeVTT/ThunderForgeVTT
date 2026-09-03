//! The rows Genie's session loop reads and writes.
//!
//! Moved out of `src/server/src/models.rs` with the tables they describe
//! (spec 032, ADR-063). The server has no reason to hold a struct for a Doom
//! Clock, and holding one is what made `models.rs` a place every ruleset
//! eventually adds to.

use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use super::schema::{
    world_genie_puzzle_clock_rewards, world_genie_puzzle_clocks, world_genie_resource_holdings,
    world_genie_sessions, world_genie_shop_listings, world_genie_trade_proposals,
};

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GenieSession {
    pub id: uuid::Uuid,
    pub world_id: uuid::Uuid,
    pub wishes_remaining: i32,
    pub doom_clock_current: i32,
    pub doom_clock_max: i32,
    pub status: String,
    pub created_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_sessions)]
pub struct NewGenieSession {
    pub world_id: uuid::Uuid,
    pub doom_clock_max: i32,
    pub created_by: uuid::Uuid,
}

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_puzzle_clocks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GeniePuzzleClock {
    pub id: uuid::Uuid,
    pub session_id: uuid::Uuid,
    pub label: String,
    pub segments_current: i32,
    pub segments_max: i32,
    pub resolved_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_puzzle_clocks)]
pub struct NewGeniePuzzleClock {
    pub session_id: uuid::Uuid,
    pub label: String,
    pub segments_max: i32,
}

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_resource_holdings)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GenieResourceHolding {
    pub id: uuid::Uuid,
    pub session_id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub resource_type: String,
    pub quantity: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_trade_proposals)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GenieTradeProposal {
    pub id: uuid::Uuid,
    pub session_id: uuid::Uuid,
    pub from_actor_id: uuid::Uuid,
    pub from_resource_type: String,
    pub from_quantity: i32,
    pub to_actor_id: uuid::Uuid,
    pub to_resource_type: String,
    pub to_quantity: i32,
    pub status: String,
    pub created_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_trade_proposals)]
pub struct NewGenieTradeProposal {
    pub session_id: uuid::Uuid,
    pub from_actor_id: uuid::Uuid,
    pub from_resource_type: String,
    pub from_quantity: i32,
    pub to_actor_id: uuid::Uuid,
    pub to_resource_type: String,
    pub to_quantity: i32,
    pub created_by: uuid::Uuid,
}

// ============================================================================
// Spec 020: Genie Session Resource Economy — NPC shop listings and
// configurable Puzzle Clock rewards. data-model.md
// "world_genie_shop_listings", "world_genie_puzzle_clock_rewards".
// ============================================================================

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_shop_listings)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GenieShopListing {
    pub id: uuid::Uuid,
    pub actor_id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub price_kind: String,
    pub price_resource_type: Option<String>,
    pub price_resource_amount: Option<i32>,
    pub price_item_id: Option<uuid::Uuid>,
    pub price_item_quantity: Option<i32>,
    pub created_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_shop_listings)]
pub struct NewGenieShopListing {
    pub actor_id: uuid::Uuid,
    pub item_id: uuid::Uuid,
    pub price_kind: String,
    pub price_resource_type: Option<String>,
    pub price_resource_amount: Option<i32>,
    pub price_item_id: Option<uuid::Uuid>,
    pub price_item_quantity: Option<i32>,
    pub created_by: uuid::Uuid,
}

#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_puzzle_clock_rewards)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GeniePuzzleClockReward {
    pub id: uuid::Uuid,
    pub clock_id: uuid::Uuid,
    pub trigger_segment: i32,
    pub reward_resource_type: Option<String>,
    pub reward_resource_amount: Option<i32>,
    pub reward_item_id: Option<uuid::Uuid>,
    pub reward_item_quantity: Option<i32>,
    pub recipient_mode: String,
    pub granted_at: Option<chrono::NaiveDateTime>,
    pub created_by: uuid::Uuid,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = world_genie_puzzle_clock_rewards)]
pub struct NewGeniePuzzleClockReward {
    pub clock_id: uuid::Uuid,
    pub trigger_segment: i32,
    pub reward_resource_type: Option<String>,
    pub reward_resource_amount: Option<i32>,
    pub reward_item_id: Option<uuid::Uuid>,
    pub reward_item_quantity: Option<i32>,
    pub recipient_mode: String,
    pub created_by: uuid::Uuid,
}
