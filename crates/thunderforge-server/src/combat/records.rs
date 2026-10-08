//! The rows an attack and an offer are kept in (spec 046 data-model.md).
//!
//! Beside the rules rather than in `models.rs`: nothing outside `combat` and
//! its GraphQL resolvers reads them, and none of them may be sent to a client
//! as it stands — every read goes through `combat::redaction` first.

use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::{world_attacks, world_offers};

/// The words the rows are written in — outcomes, flags, kinds, an offer's
/// status and kind — are the shared rules' (`thunderforge_combat::records`),
/// so the server and the browser demo cannot spell one differently.
pub use thunderforge_combat::records::*;

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
    /// Spec 084: the missed attack this row replaces, when it is a reroll.
    pub reroll_of: Option<Uuid>,
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
