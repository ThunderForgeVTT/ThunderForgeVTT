//! The rows an attack and an offer are kept in (spec 046 data-model.md).
//!
//! Beside the rules rather than in `models.rs`: nothing outside `combat` and
//! its GraphQL resolvers reads them, and none of them may be sent to a client
//! as it stands — every read goes through `combat::redaction` first.

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::{world_attacks, world_offers};

/// `world_attacks.outcome`.
pub const OUTCOME_HIT: &str = "hit";
pub const OUTCOME_MISS: &str = "miss";
pub const OUTCOME_NO_DEFENCE: &str = "no_defence";
pub const OUTCOME_NO_TARGET: &str = "no_target";

/// `world_attacks.flags` (contract §1 `AttackFlag`). Flags describe an attack;
/// none of them refuses one (C3).
pub const FLAG_OUT_OF_REACH: &str = "out_of_reach";
pub const FLAG_LONG_RANGE: &str = "long_range";
pub const FLAG_BEYOND_RANGE: &str = "beyond_range";
pub const FLAG_NO_LINE_OF_SIGHT: &str = "no_line_of_sight";
pub const FLAG_NO_REACH_DECLARED: &str = "no_reach_declared";
/// Spent past what the turn affords (C9): shown, never refused.
pub const FLAG_OVERSPENT: &str = "overspent";
/// A legendary action taken on its owner's own turn (FR-051): shown, never
/// refused.
pub const FLAG_LEGENDARY_ON_OWN_TURN: &str = "legendary_on_own_turn";

/// `world_combatants.kind` and `world_attacks.attacker_kind`.
pub const KIND_CREATURE: &str = "creature";
pub const KIND_LAIR: &str = "lair";

/// `world_offers.status`.
pub const OFFER_PENDING: &str = "pending";
pub const OFFER_TAKEN: &str = "taken";
pub const OFFER_DECLINED: &str = "declined";
pub const OFFER_APPLIED: &str = "applied";

/// `world_offers.kind`.
pub const OFFER_DAMAGE: &str = "damage";
pub const OFFER_HEALING: &str = "healing";

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = world_attacks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AttackRecord {
    pub id: Uuid,
    pub world_id: Uuid,
    pub scene_id: Uuid,
    pub combat_id: Option<Uuid>,
    pub attacker_token_id: Option<Uuid>,
    pub target_token_id: Option<Uuid>,
    pub attacker_label: String,
    pub target_label: Option<String>,
    pub ability_id: Option<Uuid>,
    pub item_id: Option<Uuid>,
    pub ability_name: String,
    pub multiattack_of: Option<Uuid>,
    pub to_hit_roll_id: Option<Uuid>,
    pub damage_roll_id: Option<Uuid>,
    pub defence: Option<i32>,
    pub outcome: String,
    pub distance: Option<f64>,
    pub flags: Vec<Option<String>>,
    pub action_cost: String,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    /// `creature`, or `lair` for a lair's action (no attacker token).
    pub attacker_kind: String,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = world_offers)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OfferRecord {
    pub id: Uuid,
    pub world_id: Uuid,
    pub scene_id: Uuid,
    pub attack_id: Option<Uuid>,
    pub target_token_id: Uuid,
    pub target_linked: bool,
    pub kind: String,
    pub amount: i32,
    pub status: String,
    pub resolved_by: Option<Uuid>,
    pub resolved_on_behalf: bool,
    pub resolved_at: Option<chrono::NaiveDateTime>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}
