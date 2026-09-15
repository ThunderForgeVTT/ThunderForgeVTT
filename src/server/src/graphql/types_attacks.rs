//! Attacks and offers as a viewer reads them (spec 046 contracts/fight.md §1,
//! §3).
//!
//! Every value here is built **for one viewer** from `combat::records` rows,
//! through `combat::redaction::SceneSight`. There is no `From<AttackRecord>`:
//! a conversion that did not ask who is looking is the way a hidden ogre's
//! name would reach a player, so the only constructors take a sight.

use std::collections::HashMap;

use async_graphql::{Enum, InputObject, SimpleObject};
use diesel::PgConnection;
use diesel::prelude::*;
use thunderforge_dice::{ResolutionKind, RollResolution};
use uuid::Uuid;

use crate::combat::attack::ActionCost;
use crate::combat::hit_points::HitPointChangeKind;
use crate::combat::records::*;
use crate::combat::redaction::SceneSight;
use crate::graphql::mutations_roll::PlaceholderBindingInput;
use crate::graphql::types::GraphQLRollResolution;
use crate::models::RollRecord;
use crate::schema::{users, world_offers, world_roll_records};

#[derive(Enum, Copy, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "AttackOutcome")]
pub enum AttackOutcome {
    Hit,
    Miss,
    NoDefence,
    NoTarget,
}

impl AttackOutcome {
    fn from_db_str(value: &str) -> Self {
        match value {
            OUTCOME_HIT => AttackOutcome::Hit,
            OUTCOME_MISS => AttackOutcome::Miss,
            OUTCOME_NO_DEFENCE => AttackOutcome::NoDefence,
            _ => AttackOutcome::NoTarget,
        }
    }
}

/// What an attack is recorded as having done that the table should know
/// (C3: never a refusal). Phase 7 sets reach, range and sight; Phases 8 and 9
/// the budget flags.
#[derive(Enum, Copy, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "AttackFlag")]
pub enum AttackFlag {
    OutOfReach,
    LongRange,
    BeyondRange,
    NoLineOfSight,
    NoReachDeclared,
    Overspent,
    LegendaryOnOwnTurn,
}

impl AttackFlag {
    pub fn from_db_str(value: &str) -> Option<Self> {
        Some(match value {
            "out_of_reach" => AttackFlag::OutOfReach,
            "long_range" => AttackFlag::LongRange,
            "beyond_range" => AttackFlag::BeyondRange,
            "no_line_of_sight" => AttackFlag::NoLineOfSight,
            "no_reach_declared" => AttackFlag::NoReachDeclared,
            "overspent" => AttackFlag::Overspent,
            "legendary_on_own_turn" => AttackFlag::LegendaryOnOwnTurn,
            _ => return None,
        })
    }
}

#[derive(Enum, Copy, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "OfferStatus")]
pub enum OfferStatus {
    Pending,
    Taken,
    Declined,
    Applied,
}

impl OfferStatus {
    fn from_db_str(value: &str) -> Self {
        match value {
            OFFER_TAKEN => OfferStatus::Taken,
            OFFER_DECLINED => OfferStatus::Declined,
            OFFER_APPLIED => OfferStatus::Applied,
            _ => OfferStatus::Pending,
        }
    }
}

/// One side of an attack. `tokenId` null and `label` "Unknown" when this
/// viewer may not know it (contract §3).
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "AttackParty")]
pub struct GraphQLAttackParty {
    pub token_id: Option<Uuid>,
    pub label: String,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "Offer")]
pub struct GraphQLOffer {
    pub id: Uuid,
    pub scene_id: Uuid,
    /// The attack it came from; null for a direct offer.
    pub attack_id: Option<Uuid>,
    pub kind: HitPointChangeKind,
    pub amount: i32,
    pub target: GraphQLAttackParty,
    pub status: OfferStatus,
    /// Who resolved it: their name, or "Game Master" when a Game Master
    /// resolved a player's offer on their behalf (FR-009).
    pub resolved_by: Option<String>,
    pub resolved_on_behalf: bool,
    /// Whether this viewer may take or decline it.
    pub may_resolve: bool,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "Attack")]
pub struct GraphQLAttack {
    pub id: Uuid,
    pub scene_id: Uuid,
    pub attacker: GraphQLAttackParty,
    /// Null when the attack was made at nothing.
    pub target: Option<GraphQLAttackParty>,
    /// Null when the attacker is redacted.
    pub ability_name: Option<String>,
    pub to_hit: GraphQLRollResolution,
    /// A hit's damage.
    pub damage: Option<GraphQLRollResolution>,
    /// Null when the target has none, or is redacted.
    pub defence: Option<i32>,
    pub outcome: AttackOutcome,
    /// System units, footprint to footprint (Phase 7).
    pub distance: Option<f64>,
    pub flags: Vec<AttackFlag>,
    pub action_cost: ActionCost,
    /// Null on a miss, with no target, or when the system has no hit points.
    pub offer: Option<GraphQLOffer>,
    pub multiattack_of: Option<Uuid>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "TurnCheck")]
pub struct GraphQLTurnCheck {
    pub allowed: bool,
    pub active_label: Option<String>,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "AttackPreview")]
pub struct GraphQLAttackPreview {
    pub distance: Option<f64>,
    pub flags: Vec<AttackFlag>,
    pub turn: GraphQLTurnCheck,
    /// The attack's reach, in `unit`; null when it declares none.
    pub reach: Option<f64>,
    pub range_normal: Option<f64>,
    pub range_long: Option<f64>,
    /// What the system's distances are in ("ft"). Empty when nothing was
    /// measured.
    pub unit: String,
}

#[derive(InputObject, Clone, Debug)]
pub struct AttackInput {
    pub attacker_token_id: Uuid,
    /// Exactly one of `abilityId` and `itemId`.
    pub ability_id: Option<Uuid>,
    pub item_id: Option<Uuid>,
    /// Null: a roll into the air, which offers and applies nothing.
    pub target_token_id: Option<Uuid>,
    /// Per part of a multiattack; overrides `targetTokenId` for that part.
    pub targets: Option<Vec<Uuid>>,
    /// Defaults to the ability's. `REACTION` is not held to the turn.
    pub action_cost: Option<ActionCost>,
    pub bindings: Option<Vec<PlaceholderBindingInput>>,
}

impl AttackInput {
    pub fn into_request(self) -> crate::combat::attack::AttackRequest {
        crate::combat::attack::AttackRequest {
            attacker_token_id: self.attacker_token_id,
            ability_id: self.ability_id,
            item_id: self.item_id,
            target_token_id: self.target_token_id,
            targets: self.targets,
            action_cost: self.action_cost,
            bindings: self
                .bindings
                .unwrap_or_default()
                .into_iter()
                .map(|b| (b.name, b.value))
                .collect(),
        }
    }
}

/// What an ability or an item is as an attack (research R2), flattened onto
/// `Ability` and `Item`.
#[derive(SimpleObject, Clone, Debug)]
pub struct GraphQLAttackFields {
    pub reach: Option<f64>,
    pub range_normal: Option<f64>,
    pub range_long: Option<f64>,
    pub needs_line_of_sight: bool,
    pub action_cost: ActionCost,
    pub legendary_cost: i32,
    pub multiattack: Vec<Uuid>,
}

impl Default for GraphQLAttackFields {
    /// What a row written before these columns reads as.
    fn default() -> Self {
        GraphQLAttackFields {
            reach: None,
            range_normal: None,
            range_long: None,
            needs_line_of_sight: true,
            action_cost: ActionCost::Action,
            legendary_cost: 1,
            multiattack: Vec::new(),
        }
    }
}

impl GraphQLAttackFields {
    #[allow(clippy::too_many_arguments)]
    pub fn from_columns(
        reach: Option<f64>,
        range_normal: Option<f64>,
        range_long: Option<f64>,
        needs_line_of_sight: bool,
        action_cost: &str,
        legendary_cost: i32,
        multiattack: &[Option<Uuid>],
    ) -> Self {
        GraphQLAttackFields {
            reach,
            range_normal,
            range_long,
            needs_line_of_sight,
            action_cost: ActionCost::from_db_str(action_cost),
            legendary_cost,
            multiattack: multiattack.iter().flatten().copied().collect(),
        }
    }
}

/// Setting what an ability or item is as an attack. Every field is written:
/// a null reach clears it.
#[derive(InputObject, Clone, Debug)]
pub struct AttackFieldsInput {
    pub reach: Option<f64>,
    pub range_normal: Option<f64>,
    pub range_long: Option<f64>,
    pub needs_line_of_sight: bool,
    pub action_cost: ActionCost,
    pub legendary_cost: i32,
    pub multiattack: Vec<Uuid>,
}

impl AttackFieldsInput {
    /// The rules the columns' CHECKs hold, said as sentences.
    pub fn check(&self) -> Result<(), String> {
        for (name, value) in [
            ("Reach", self.reach),
            ("Normal range", self.range_normal),
            ("Long range", self.range_long),
        ] {
            if value.is_some_and(|v| !v.is_finite() || v < 0.0) {
                return Err(format!("{name} cannot be negative"));
            }
        }
        if let (Some(normal), Some(long)) = (self.range_normal, self.range_long)
            && long < normal
        {
            return Err("Long range cannot be shorter than normal range".to_string());
        }
        if self.legendary_cost < 0 {
            return Err("A legendary cost cannot be negative".to_string());
        }
        Ok(())
    }
}

fn resolution_of(row: &RollRecord) -> GraphQLRollResolution {
    let resolution: RollResolution =
        serde_json::from_value(row.detail.clone()).unwrap_or(RollResolution {
            formula: row.formula.clone(),
            dice: Vec::new(),
            kind: ResolutionKind::Total(row.result_value),
        });
    GraphQLRollResolution::from(&resolution)
}

fn missing_roll() -> GraphQLRollResolution {
    GraphQLRollResolution::from(&RollResolution {
        formula: String::new(),
        dice: Vec::new(),
        kind: ResolutionKind::Total(0.0),
    })
}

/// Sights per scene, loaded once for a batch of answers.
pub struct Sights<'a> {
    systems_dir: &'a str,
    viewer: Uuid,
    is_admin: bool,
    by_scene: HashMap<Uuid, SceneSight>,
}

impl<'a> Sights<'a> {
    pub fn new(systems_dir: &'a str, viewer: Uuid, is_admin: bool) -> Self {
        Sights {
            systems_dir,
            viewer,
            is_admin,
            by_scene: HashMap::new(),
        }
    }

    fn for_scene(&mut self, conn: &mut PgConnection, scene_id: Uuid) -> QueryResult<&SceneSight> {
        if !self.by_scene.contains_key(&scene_id) {
            let sight = SceneSight::for_viewer(
                conn,
                self.systems_dir,
                self.viewer,
                self.is_admin,
                scene_id,
            )?;
            self.by_scene.insert(scene_id, sight);
        }
        Ok(&self.by_scene[&scene_id])
    }
}

fn party(sight: &SceneSight, token_id: Option<Uuid>, label: &str) -> GraphQLAttackParty {
    let party = sight.party(token_id, label);
    GraphQLAttackParty {
        token_id: party.token_id,
        label: party.label,
    }
}

/// Offers, as `sights`' viewer may read them.
pub fn build_offers(
    conn: &mut PgConnection,
    sights: &mut Sights<'_>,
    offers: Vec<OfferRecord>,
) -> QueryResult<Vec<GraphQLOffer>> {
    let resolvers: Vec<Uuid> = offers.iter().filter_map(|o| o.resolved_by).collect();
    let names: HashMap<Uuid, String> = users::table
        .filter(users::id.eq_any(&resolvers))
        .select((users::id, users::username))
        .load::<(Uuid, String)>(conn)?
        .into_iter()
        .collect();
    let mut out = Vec::with_capacity(offers.len());
    for offer in offers {
        let label = crate::combat::attack::token_label(conn, offer.target_token_id)
            .unwrap_or_else(|_| crate::combat::redaction::UNKNOWN.to_string());
        let may_resolve =
            crate::combat::offers::may_resolve(conn, sights.viewer, sights.is_admin, &offer)?;
        let sight = sights.for_scene(conn, offer.scene_id)?;
        let target = party(sight, Some(offer.target_token_id), &label);
        let resolved_by = if offer.resolved_on_behalf {
            Some("Game Master".to_string())
        } else {
            offer.resolved_by.and_then(|id| names.get(&id).cloned())
        };
        out.push(GraphQLOffer {
            id: offer.id,
            scene_id: offer.scene_id,
            attack_id: offer.attack_id,
            kind: if offer.kind == OFFER_HEALING {
                HitPointChangeKind::Healing
            } else {
                HitPointChangeKind::Damage
            },
            amount: offer.amount,
            target,
            status: OfferStatus::from_db_str(&offer.status),
            resolved_by,
            resolved_on_behalf: offer.resolved_on_behalf,
            may_resolve: may_resolve && offer.status == OFFER_PENDING,
            created_at: offer.created_at,
        });
    }
    Ok(out)
}

/// Attacks, as `sights`' viewer may read them, in the order given.
pub fn build_attacks(
    conn: &mut PgConnection,
    sights: &mut Sights<'_>,
    records: Vec<AttackRecord>,
) -> QueryResult<Vec<GraphQLAttack>> {
    let roll_ids: Vec<Uuid> = records
        .iter()
        .flat_map(|r| [r.to_hit_roll_id, r.damage_roll_id])
        .flatten()
        .collect();
    let rolls: HashMap<Uuid, RollRecord> = world_roll_records::table
        .filter(world_roll_records::id.eq_any(&roll_ids))
        .select(RollRecord::as_select())
        .load::<RollRecord>(conn)?
        .into_iter()
        .map(|r| (r.id, r))
        .collect();
    let attack_ids: Vec<Uuid> = records.iter().map(|r| r.id).collect();
    let offer_rows = world_offers::table
        .filter(world_offers::attack_id.eq_any(&attack_ids))
        .select(OfferRecord::as_select())
        .load::<OfferRecord>(conn)?;
    let mut offers: HashMap<Uuid, GraphQLOffer> = HashMap::new();
    for offer in build_offers(conn, sights, offer_rows)? {
        if let Some(attack_id) = offer.attack_id {
            offers.insert(attack_id, offer);
        }
    }

    let mut out = Vec::with_capacity(records.len());
    for record in records {
        let sight = sights.for_scene(conn, record.scene_id)?;
        let attacker = party(sight, record.attacker_token_id, &record.attacker_label);
        let attacker_known = sight.may_know(record.attacker_token_id);
        let target = record
            .target_label
            .as_deref()
            .map(|label| party(sight, record.target_token_id, label));
        let target_known = record.target_label.is_some() && sight.may_know(record.target_token_id);
        let runs_the_world = sight.runs_the_world();
        out.push(GraphQLAttack {
            id: record.id,
            scene_id: record.scene_id,
            attacker,
            target,
            ability_name: attacker_known.then(|| record.ability_name.clone()),
            to_hit: record
                .to_hit_roll_id
                .and_then(|id| rolls.get(&id))
                .map(resolution_of)
                .unwrap_or_else(missing_roll),
            damage: record
                .damage_roll_id
                .and_then(|id| rolls.get(&id))
                .map(resolution_of),
            defence: if target_known { record.defence } else { None },
            outcome: AttackOutcome::from_db_str(&record.outcome),
            distance: record.distance,
            flags: record
                .flags
                .iter()
                .flatten()
                .filter_map(|f| AttackFlag::from_db_str(f))
                // "No reach declared" is a note to the Game Master about their
                // own content (research R2), not something the table is told.
                .filter(|f| runs_the_world || *f != AttackFlag::NoReachDeclared)
                .collect(),
            action_cost: ActionCost::from_db_str(&record.action_cost),
            offer: offers.remove(&record.id),
            multiattack_of: record.multiattack_of,
            created_at: record.created_at,
        });
    }
    Ok(out)
}
