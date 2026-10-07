//! The words a fight is recorded in (spec 046 data-model.md): an attack's
//! outcome and flags, a combatant's kind, an offer's status and kind.
//!
//! The server stores these strings in its rows; the demo shows them. One list,
//! so the two cannot spell an outcome differently.

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
