//! An attack is aimed at something, and resolved on the server (spec 046 US1,
//! research R1, ADR-101).
//!
//! Before this, an attack was `rollDice` with a formula: a number returned to
//! the person who rolled it, aimed at nobody, compared with nothing, shown to
//! nobody else. `make_attack` is the whole of what an attack does, in the one
//! order the contract gives (contracts/fight.md §2):
//!
//! 1. **C10** — play in the world is not paused.
//! 2. **C2** — the caller controls the attacker (`combat::controllers`), or
//!    runs the world.
//! 3. **C1** — it is the attacker's turn, unless the attack is a reaction or
//!    the caller runs the world (`combat::turn`).
//! 4. The to-hit roll: the ability's or item's `ATTACK_ROLL` formula, rolled
//!    here with the dice crate and recorded as an ordinary roll record.
//! 5. The target's defence, as its pack declares it (`combat.defence`). A copy
//!    holds only hit points, so it reads its defence from the NPC it copies;
//!    a copy whose NPC is gone has none.
//! 6. The outcome: `no_target`, `no_defence`, `hit` (total ≥ defence) or
//!    `miss`. **C3**: reach, range and line of sight never refuse; they are
//!    measured from footprint to footprint into `distance` and `flags` by
//!    `combat::reach`, the same measurement `preview_attack` warns with.
//! 7. On a hit, the damage roll, in the same action.
//! 8. **C4** — a miss, or no target, offers nothing. **C5** — a hit offers its
//!    damage to whoever controls the target, pending; or, when auto-apply
//!    holds (research R15), applies it through `apply_hit_point_change` in the
//!    same transaction and records the offer as `applied`. Damage to a
//!    creature any player controls is always pending.
//! 9. Events 29 (each attack) and 30 (each offer), ids only, committed with
//!    everything else.
//!
//! A multiattack (FR-044) makes one row per named attack, in order, each
//! rolled on its own and each against its own target; the first part is the
//! parent the rest point at.

use std::collections::HashMap;

use diesel::PgConnection;
use diesel::prelude::*;
use rand::Rng;
use thunderforge_dice::{DiceFormula, PlaceholderBindings, ResolutionKind, RollResolution};
use uuid::Uuid;

use crate::combat::controllers::{may_act_for, player_controllers, token_control};
use crate::combat::hit_points::{HitPointChangeKind, apply_hit_point_change, slot_column};
use crate::combat::manifest::{combat_for_system, slot_key};
use crate::combat::reach::{Measured, Reach, SceneMeasure};
use crate::combat::records::*;
use crate::combat::turn::{TurnCheck, turn_check};
use crate::models::{NewRollRecord, WorldAbility, WorldItem};
use crate::play_pause::gate::{GateError, refuse_if_paused};
use crate::schema::{
    scenes, tokens, world_abilities, world_ability_effects, world_actor_abilities,
    world_actor_inventory, world_actor_system_data, world_actors, world_attacks, world_combats,
    world_item_abilities, world_item_effects, world_items, world_offers, world_roll_records,
    worlds,
};
use crate::world_events::{EVENT_CODE_ATTACK_MADE, EVENT_CODE_OFFER_CHANGED, record_world_event};

/// What an attack costs (`world_attacks.action_cost`, research R13).
#[derive(async_graphql::Enum, Copy, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "ActionCost")]
pub enum ActionCost {
    Action,
    BonusAction,
    Reaction,
    Legendary,
    Free,
}

impl ActionCost {
    pub fn as_db_str(self) -> &'static str {
        match self {
            ActionCost::Action => "action",
            ActionCost::BonusAction => "bonus_action",
            ActionCost::Reaction => "reaction",
            ActionCost::Legendary => "legendary",
            ActionCost::Free => "free",
        }
    }

    /// An unrecognised stored value reads as an action, the default a row
    /// written before the column existed has.
    pub fn from_db_str(value: &str) -> Self {
        match value {
            "bonus_action" => ActionCost::BonusAction,
            "reaction" => ActionCost::Reaction,
            "legendary" => ActionCost::Legendary,
            "free" => ActionCost::Free,
            _ => ActionCost::Action,
        }
    }
}

/// Why a fight's rule refused. A sentence for a person, and a kind for the
/// callers that report a refusal as something other than an error (offline
/// replay).
#[derive(Clone, Debug, PartialEq)]
pub enum FightRefusal {
    /// C10.
    Paused(GateError),
    /// C1, with "It is <label>'s turn".
    NotYourTurn(String),
    /// C2.
    NotControlled,
    /// The attacker, target, ability or offer is not there.
    NotFound(String),
    /// Asked for something the rules cannot do, said why.
    Invalid(String),
    /// Something failed that is nobody's fault; retrying may help.
    Failed(String),
}

/// C2's sentence.
pub const NOT_CONTROLLED: &str = "You do not control that creature";

impl FightRefusal {
    pub fn message(&self) -> String {
        match self {
            FightRefusal::Paused(_) => "Play in this world has been paused by an operator.".into(),
            FightRefusal::NotYourTurn(sentence) => sentence.clone(),
            FightRefusal::NotControlled => NOT_CONTROLLED.into(),
            FightRefusal::NotFound(sentence)
            | FightRefusal::Invalid(sentence)
            | FightRefusal::Failed(sentence) => sentence.clone(),
        }
    }
}

impl From<diesel::result::Error> for FightRefusal {
    fn from(error: diesel::result::Error) -> Self {
        FightRefusal::Failed(format!("Failed to resolve the attack: {error}"))
    }
}

impl From<FightRefusal> for async_graphql::Error {
    fn from(refusal: FightRefusal) -> Self {
        match refusal {
            // The gate's own error, with its code, as every paused mutation.
            FightRefusal::Paused(gate) => gate.into(),
            other => async_graphql::Error::new(other.message()),
        }
    }
}

/// What an attack is asked to be.
#[derive(Clone, Debug, Default)]
pub struct AttackRequest {
    pub attacker_token_id: Uuid,
    pub ability_id: Option<Uuid>,
    pub item_id: Option<Uuid>,
    pub target_token_id: Option<Uuid>,
    /// Per part of a multiattack; overrides `target_token_id` for that part.
    pub targets: Option<Vec<Uuid>>,
    pub action_cost: Option<ActionCost>,
    pub bindings: Vec<(String, f64)>,
}

/// What `make_attack` wrote.
#[derive(Clone, Debug)]
pub struct MadeAttack {
    pub world_id: Uuid,
    pub scene_id: Uuid,
    /// In order: a multiattack's parent first.
    pub attack_ids: Vec<Uuid>,
    pub offer_ids: Vec<Uuid>,
}

/// One attack an ability or item makes: its name and its formulas.
#[derive(Clone, Debug)]
struct Part {
    ability_id: Option<Uuid>,
    item_id: Option<Uuid>,
    name: String,
    to_hit: String,
    damage: Vec<String>,
    reach: Reach,
}

/// What the attack is made with, once found.
enum Weapon {
    Ability(WorldAbility),
    Item(WorldItem),
}

impl Weapon {
    fn action_cost(&self) -> ActionCost {
        ActionCost::from_db_str(match self {
            Weapon::Ability(a) => &a.action_cost,
            Weapon::Item(i) => &i.action_cost,
        })
    }

    fn multiattack(&self) -> Vec<Uuid> {
        match self {
            Weapon::Ability(a) => &a.multiattack,
            Weapon::Item(i) => &i.multiattack,
        }
        .iter()
        .flatten()
        .copied()
        .collect()
    }
}

/// A token's name as the server knows it: its own label, else its actor's.
pub fn token_label(conn: &mut PgConnection, token_id: Uuid) -> QueryResult<String> {
    let (metadata, actor_id) = tokens::table
        .filter(tokens::token_id.eq(token_id))
        .select((tokens::metadata, tokens::actor_id))
        .first::<(Option<serde_json::Value>, Option<Uuid>)>(conn)?;
    if let Some(label) = crate::graphql::types_scene::written_label(metadata.as_ref()) {
        return Ok(label);
    }
    let actor_label = match actor_id {
        Some(actor_id) => world_actors::table
            .filter(world_actors::id.eq(actor_id))
            .select(world_actors::label)
            .first::<String>(conn)
            .optional()?,
        None => None,
    };
    Ok(actor_label
        .filter(|label| !label.trim().is_empty())
        .unwrap_or_else(|| "Unnamed creature".to_string()))
}

fn effects_of_ability(
    conn: &mut PgConnection,
    ability_id: Uuid,
) -> QueryResult<Vec<(String, String)>> {
    world_ability_effects::table
        .filter(world_ability_effects::ability_id.eq(ability_id))
        .order((
            world_ability_effects::sort_order,
            world_ability_effects::created_at,
        ))
        .select((
            world_ability_effects::effect_type,
            world_ability_effects::formula,
        ))
        .load(conn)
}

fn effects_of_item(conn: &mut PgConnection, item_id: Uuid) -> QueryResult<Vec<(String, String)>> {
    world_item_effects::table
        .filter(world_item_effects::item_id.eq(item_id))
        .order((
            world_item_effects::sort_order,
            world_item_effects::created_at,
        ))
        .select((world_item_effects::effect_type, world_item_effects::formula))
        .load(conn)
}

/// What an ability says about its reach, range and line of sight.
fn reach_of_ability(a: &WorldAbility) -> Reach {
    Reach {
        reach: a.reach,
        range_normal: a.range_normal,
        range_long: a.range_long,
        needs_line_of_sight: a.needs_line_of_sight,
    }
}

fn reach_of_item(i: &WorldItem) -> Reach {
    Reach {
        reach: i.reach,
        range_normal: i.range_normal,
        range_long: i.range_long,
        needs_line_of_sight: i.needs_line_of_sight,
    }
}

fn part_from(
    ability_id: Option<Uuid>,
    item_id: Option<Uuid>,
    name: String,
    reach: Reach,
    effects: Vec<(String, String)>,
) -> Result<Part, FightRefusal> {
    let to_hit = effects
        .iter()
        .find(|(kind, formula)| kind == "attack_roll" && !formula.trim().is_empty())
        .map(|(_, formula)| formula.trim().to_string())
        .ok_or_else(|| FightRefusal::Invalid(format!("{name} has no attack roll to make")))?;
    let damage = effects
        .iter()
        .filter(|(kind, formula)| kind == "damage" && !formula.trim().is_empty())
        .map(|(_, formula)| formula.trim().to_string())
        .collect();
    Ok(Part {
        ability_id,
        item_id,
        name,
        to_hit,
        damage,
        reach,
    })
}

/// Find what the attack is made with, and whether this caller may use it.
///
/// A Game Master may use anything in the world. A player uses what the
/// attacker has: an ability its actor knows, an item in its actor's
/// inventory, or an ability one of those items carries. A Game-Master-only
/// ability is not there for a player at all (spec 025 FR-024b).
fn find_weapon(
    conn: &mut PgConnection,
    world_id: Uuid,
    actor_id: Option<Uuid>,
    runs_the_world: bool,
    ability_id: Option<Uuid>,
    item_id: Option<Uuid>,
) -> Result<Weapon, FightRefusal> {
    let not_known = || FightRefusal::NotFound("That creature has no such attack".to_string());
    match (ability_id, item_id) {
        (Some(ability_id), None) => {
            let ability = world_abilities::table
                .filter(world_abilities::id.eq(ability_id))
                .filter(world_abilities::world_id.eq(world_id))
                .select(WorldAbility::as_select())
                .first::<WorldAbility>(conn)
                .optional()?
                .ok_or_else(not_known)?;
            if !runs_the_world {
                if ability.gm_only {
                    return Err(not_known());
                }
                let Some(actor_id) = actor_id else {
                    return Err(not_known());
                };
                let known = diesel::select(diesel::dsl::exists(
                    world_actor_abilities::table
                        .filter(world_actor_abilities::actor_id.eq(actor_id))
                        .filter(world_actor_abilities::ability_id.eq(ability_id)),
                ))
                .get_result::<bool>(conn)?;
                let carried = known
                    || diesel::select(diesel::dsl::exists(
                        world_item_abilities::table
                            .inner_join(
                                world_actor_inventory::table.on(world_actor_inventory::item_id
                                    .eq(world_item_abilities::item_id.nullable())),
                            )
                            .filter(world_actor_inventory::actor_id.eq(actor_id))
                            .filter(world_item_abilities::ability_id.eq(ability_id)),
                    ))
                    .get_result::<bool>(conn)?;
                if !carried {
                    return Err(not_known());
                }
            }
            Ok(Weapon::Ability(ability))
        }
        (None, Some(item_id)) => {
            let item = world_items::table
                .filter(world_items::id.eq(item_id))
                .filter(world_items::world_id.eq(world_id))
                .select(WorldItem::as_select())
                .first::<WorldItem>(conn)
                .optional()?
                .ok_or_else(not_known)?;
            if !runs_the_world {
                let Some(actor_id) = actor_id else {
                    return Err(not_known());
                };
                let held = diesel::select(diesel::dsl::exists(
                    world_actor_inventory::table
                        .filter(world_actor_inventory::actor_id.eq(actor_id))
                        .filter(world_actor_inventory::item_id.eq(item_id)),
                ))
                .get_result::<bool>(conn)?;
                if !held {
                    return Err(not_known());
                }
            }
            Ok(Weapon::Item(item))
        }
        _ => Err(FightRefusal::Invalid(
            "Choose one ability or one item to attack with".to_string(),
        )),
    }
}

/// The parts one use of `weapon` makes (FR-044).
fn parts_of(
    conn: &mut PgConnection,
    world_id: Uuid,
    weapon: &Weapon,
) -> Result<Vec<Part>, FightRefusal> {
    let named = weapon.multiattack();
    if named.is_empty() {
        return Ok(vec![match weapon {
            Weapon::Ability(a) => part_from(
                Some(a.id),
                None,
                a.name.clone(),
                reach_of_ability(a),
                effects_of_ability(conn, a.id)?,
            )?,
            Weapon::Item(i) => part_from(
                None,
                Some(i.id),
                i.name.clone(),
                reach_of_item(i),
                effects_of_item(conn, i.id)?,
            )?,
        }]);
    }
    let abilities: HashMap<Uuid, WorldAbility> = world_abilities::table
        .filter(world_abilities::id.eq_any(&named))
        .filter(world_abilities::world_id.eq(world_id))
        .select(WorldAbility::as_select())
        .load::<WorldAbility>(conn)?
        .into_iter()
        .map(|a| (a.id, a))
        .collect();
    named
        .iter()
        .map(|id| {
            let ability = abilities.get(id).ok_or_else(|| {
                FightRefusal::NotFound("One of that multiattack's attacks is gone".to_string())
            })?;
            part_from(
                Some(ability.id),
                None,
                ability.name.clone(),
                reach_of_ability(ability),
                effects_of_ability(conn, ability.id)?,
            )
        })
        .collect()
}

/// The target's defence, as its pack declares it (research R3).
///
/// Read from the actor's sheet for a linked token, and for a copy too: a copy
/// holds a `resource_data`-shaped record of its hit points and nothing else
/// (ADR-102), so its armour is its NPC's. A copy of an NPC that is gone has no
/// defence, and nor has a marker with no actor.
pub fn defence_of(
    conn: &mut PgConnection,
    systems_dir: &str,
    world_system: Option<&str>,
    target_token_id: Uuid,
) -> QueryResult<Option<i32>> {
    let (actor_id, linked, system_data) = tokens::table
        .filter(tokens::token_id.eq(target_token_id))
        .select((tokens::actor_id, tokens::linked, tokens::system_data))
        .first::<(Option<Uuid>, bool, Option<serde_json::Value>)>(conn)?;

    let actor_system = match actor_id {
        Some(actor_id) => world_actors::table
            .filter(world_actors::id.eq(actor_id))
            .select(world_actors::game_system_id)
            .first::<Option<String>>(conn)
            .optional()?
            .flatten(),
        None => None,
    };
    let Some(system_id) = actor_system.or_else(|| world_system.map(str::to_string)) else {
        return Ok(None);
    };
    let Some(declared) = combat_for_system(systems_dir, &system_id).defence.clone() else {
        return Ok(None);
    };
    let key = slot_key(&declared.slot);
    let read = |slot: &serde_json::Value| {
        slot.get(&declared.field)
            .and_then(|v| v.as_i64())
            .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
    };

    // A pack that keeps its defence among a creature's resources: a copy has
    // its own.
    if key == "resource_data" && !linked {
        return Ok(system_data.as_ref().and_then(read));
    }
    let Some(actor_id) = actor_id else {
        return Ok(None);
    };
    let row = world_actor_system_data::table
        .filter(world_actor_system_data::actor_id.eq(actor_id))
        .select(crate::models::ActorSystemData::as_select())
        .first::<crate::models::ActorSystemData>(conn)
        .optional()?;
    Ok(row
        .and_then(|row| slot_column(&row, &key))
        .as_ref()
        .and_then(read))
}

/// Roll a formula and keep it as a roll record, inside the caller's
/// transaction. The same record `rollDice` writes (ADR-044): an attack's rolls
/// are ordinary rolls, linked from the attack.
fn roll_and_record<R: Rng>(
    conn: &mut PgConnection,
    world_id: Uuid,
    user_id: Uuid,
    source: &str,
    bindings: &PlaceholderBindings,
    rng: &mut R,
) -> Result<(Uuid, RollResolution, f64), FightRefusal> {
    let formula = DiceFormula::parse(source)
        .map_err(|e| FightRefusal::Invalid(format!("Roll rejected: {e}")))?;
    let resolution = thunderforge_dice::resolve(&formula, bindings, rng)
        .map_err(|e| FightRefusal::Invalid(format!("Roll rejected: {e}")))?;
    let (kind, value) = match resolution.kind {
        ResolutionKind::Total(v) => ("total", v),
        ResolutionKind::SuccessCount(n) => ("success_count", n as f64),
    };
    let detail = serde_json::to_value(&resolution)
        .map_err(|_| FightRefusal::Failed("Failed to record the roll".to_string()))?;
    let id = diesel::insert_into(world_roll_records::table)
        .values(&NewRollRecord {
            world_id,
            triggered_by: user_id,
            formula: resolution.formula.clone(),
            bindings: if bindings.is_empty() {
                None
            } else {
                serde_json::to_value(bindings).ok()
            },
            detail,
            result_kind: kind.to_string(),
            result_value: value,
        })
        .returning(world_roll_records::id)
        .get_result::<Uuid>(conn)?;
    Ok((id, resolution, value))
}

/// The world's running combat in this scene, and its auto-apply override.
fn running_combat(
    conn: &mut PgConnection,
    world_id: Uuid,
    scene_id: Uuid,
) -> QueryResult<Option<(Uuid, Option<bool>)>> {
    world_combats::table
        .filter(world_combats::world_id.eq(world_id))
        .filter(world_combats::ended_at.is_null())
        .filter(
            world_combats::scene_id
                .is_null()
                .or(world_combats::scene_id.eq(scene_id)),
        )
        .select((world_combats::id, world_combats::auto_apply))
        .first::<(Uuid, Option<bool>)>(conn)
        .optional()
}

/// Research R15, less the parts `make_attack` has already settled (a named
/// target that was hit): the effective setting is on, no player controls the
/// target, and the attack could see it or does not need to.
pub fn auto_apply_holds(
    effective_setting: bool,
    target_player_controllers: &[Uuid],
    flags: &[String],
    needs_line_of_sight: bool,
) -> bool {
    effective_setting
        && target_player_controllers.is_empty()
        && (!needs_line_of_sight || !flags.iter().any(|f| f == "no_line_of_sight"))
}

/// What `previewAttack` answers: what `make_attack` would record, and whether
/// it would be refused for the turn. Writes nothing.
#[derive(Clone, Debug)]
pub struct AttackPreview {
    pub distance: Option<f64>,
    pub flags: Vec<String>,
    pub turn: TurnCheck,
    /// The first part's reach and ranges, and the unit they are in, so a
    /// warning can say "Out of reach: 20 ft, reach 5 ft".
    pub reach: Reach,
    pub unit_label: String,
}

/// Measure every part of an attack against its target, with the scene loaded
/// once. The one measurement `make_attack` records and `preview_attack` warns
/// with.
fn measure_parts(
    conn: &mut PgConnection,
    systems_dir: &str,
    scene_id: Uuid,
    attacker: Uuid,
    parts: &[Part],
    target_for: impl Fn(usize) -> Option<Uuid>,
) -> QueryResult<(Vec<Measured>, String)> {
    let mut tokens_needed = vec![attacker];
    tokens_needed.extend((0..parts.len()).filter_map(&target_for));
    let needs_walls = parts.iter().any(|part| part.reach.needs_line_of_sight);
    let scene = SceneMeasure::load(conn, systems_dir, scene_id, &tokens_needed, needs_walls)?;
    let measured = parts
        .iter()
        .enumerate()
        .map(|(index, part)| scene.measure(attacker, target_for(index), &part.reach))
        .collect();
    Ok((measured, scene.unit_label().to_string()))
}

pub fn preview_attack(
    conn: &mut PgConnection,
    systems_dir: &str,
    user_id: Uuid,
    is_admin: bool,
    request: &AttackRequest,
) -> Result<AttackPreview, FightRefusal> {
    let control = token_control(conn, request.attacker_token_id)?
        .ok_or_else(|| FightRefusal::NotFound("That creature is not on the board".to_string()))?;
    if crate::auth::world_membership::actor_in_world(conn, user_id, is_admin, control.world_id)
        .role
        .is_none()
        && !is_admin
    {
        return Err(FightRefusal::NotControlled);
    }
    let scene_id = tokens::table
        .filter(tokens::token_id.eq(request.attacker_token_id))
        .select(tokens::scene_id)
        .first::<Uuid>(conn)?;
    let turn = if request.action_cost == Some(ActionCost::Reaction) {
        TurnCheck {
            allowed: true,
            active_label: None,
        }
    } else {
        turn_check(conn, scene_id, request.attacker_token_id, user_id, is_admin)?
    };
    // The turn is answered whatever else happens: it is the one thing that
    // refuses, and a warning about reach must never stand in front of it.
    let mut preview = AttackPreview {
        distance: None,
        flags: Vec::new(),
        turn,
        reach: Reach::default(),
        unit_label: String::new(),
    };
    let runs_the_world =
        crate::auth::world_membership::actor_in_world(conn, user_id, is_admin, control.world_id)
            .runs_the_world();
    // Something `make_attack` would refuse as not there, or not an attack,
    // has nothing to measure; the attempt says why when it is made.
    let Ok(weapon) = find_weapon(
        conn,
        control.world_id,
        control.actor_id,
        runs_the_world,
        request.ability_id,
        request.item_id,
    ) else {
        return Ok(preview);
    };
    let Ok(parts) = parts_of(conn, control.world_id, &weapon) else {
        return Ok(preview);
    };
    let (measured, unit_label) = measure_parts(
        conn,
        systems_dir,
        scene_id,
        request.attacker_token_id,
        &parts,
        |index| target_of(request, index),
    )?;
    preview.distance = measured.first().and_then(|m| m.distance);
    for flag in measured.into_iter().flat_map(|m| m.flags) {
        if !preview.flags.contains(&flag) {
            preview.flags.push(flag);
        }
    }
    preview.reach = parts.first().map(|p| p.reach).unwrap_or_default();
    preview.unit_label = unit_label;
    Ok(preview)
}

/// The target of one part of a multiattack: its own, else the attack's.
fn target_of(request: &AttackRequest, index: usize) -> Option<Uuid> {
    request
        .targets
        .as_ref()
        .and_then(|targets| targets.get(index).copied())
        .or(request.target_token_id)
}

/// Make an attack. See the module documentation for the order of its rules.
pub fn make_attack<R: Rng>(
    conn: &mut PgConnection,
    systems_dir: &str,
    user_id: Uuid,
    is_admin: bool,
    request: &AttackRequest,
    rng: &mut R,
) -> Result<MadeAttack, FightRefusal> {
    let control = token_control(conn, request.attacker_token_id)?
        .ok_or_else(|| FightRefusal::NotFound("That creature is not on the board".to_string()))?;
    let world_id = control.world_id;

    // C10.
    refuse_if_paused(conn, world_id).map_err(FightRefusal::Paused)?;

    // C2.
    if !may_act_for(conn, user_id, is_admin, &control)? {
        return Err(FightRefusal::NotControlled);
    }
    let runs_the_world =
        crate::auth::world_membership::actor_in_world(conn, user_id, is_admin, world_id)
            .runs_the_world();

    let scene_id = tokens::table
        .filter(tokens::token_id.eq(request.attacker_token_id))
        .select(tokens::scene_id)
        .first::<Uuid>(conn)?;

    let weapon = find_weapon(
        conn,
        world_id,
        control.actor_id,
        runs_the_world,
        request.ability_id,
        request.item_id,
    )?;
    let action_cost = request.action_cost.unwrap_or_else(|| weapon.action_cost());

    // C1. A reaction is taken between turns; a Game Master is never held.
    if action_cost != ActionCost::Reaction {
        let check = turn_check(conn, scene_id, request.attacker_token_id, user_id, is_admin)?;
        if let Some(sentence) = check.refusal() {
            return Err(FightRefusal::NotYourTurn(sentence));
        }
    }

    let parts = parts_of(conn, world_id, &weapon)?;
    let target_for = |index: usize| target_of(request, index);
    // Every target is on this scene, checked before anything is rolled.
    for index in 0..parts.len() {
        if let Some(target) = target_for(index) {
            let on_scene = tokens::table
                .filter(tokens::token_id.eq(target))
                .select(tokens::scene_id)
                .first::<Uuid>(conn)
                .optional()?;
            if on_scene != Some(scene_id) {
                return Err(FightRefusal::NotFound(
                    "That target is not on this scene".to_string(),
                ));
            }
        }
    }

    let bindings: PlaceholderBindings = request.bindings.iter().cloned().collect();
    let attacker_label = token_label(conn, request.attacker_token_id)?;
    // Reach, range and line of sight: flags, never refusals (C3). Measured
    // before anything is written, where each creature stands now.
    let (measured, _) = measure_parts(
        conn,
        systems_dir,
        scene_id,
        request.attacker_token_id,
        &parts,
        target_for,
    )?;

    conn.transaction::<MadeAttack, FightRefusal, _>(|conn| {
        let world_system = worlds::table
            .filter(worlds::id.eq(world_id))
            .select((worlds::game_system_id, worlds::auto_apply_npc_damage))
            .first::<(Option<String>, bool)>(conn)?;
        let (world_system, world_auto_apply) = world_system;
        let combat = running_combat(conn, world_id, scene_id)?;
        let effective_auto_apply = combat
            .and_then(|(_, override_)| override_)
            .unwrap_or(world_auto_apply);

        let mut made = MadeAttack {
            world_id,
            scene_id,
            attack_ids: Vec::new(),
            offer_ids: Vec::new(),
        };
        let now = chrono::Utc::now().naive_utc();

        for (index, part) in parts.iter().enumerate() {
            let target = target_for(index);
            let (to_hit_roll_id, _, total) =
                roll_and_record(conn, world_id, user_id, &part.to_hit, &bindings, rng)?;

            let defence = match target {
                Some(target) => defence_of(conn, systems_dir, world_system.as_deref(), target)?,
                None => None,
            };
            let outcome = match (target, defence) {
                (None, _) => OUTCOME_NO_TARGET,
                (Some(_), None) => OUTCOME_NO_DEFENCE,
                (Some(_), Some(defence)) if total >= defence as f64 => OUTCOME_HIT,
                (Some(_), Some(_)) => OUTCOME_MISS,
            };
            let Measured { distance, flags } = measured[index].clone();

            let damage = if outcome == OUTCOME_HIT && !part.damage.is_empty() {
                let source = if part.damage.len() == 1 {
                    part.damage[0].clone()
                } else {
                    part.damage
                        .iter()
                        .map(|f| format!("({f})"))
                        .collect::<Vec<_>>()
                        .join("+")
                };
                Some(roll_and_record(conn, world_id, user_id, &source, &bindings, rng)?)
            } else {
                None
            };

            let attack_id = Uuid::now_v7();
            let target_label = match target {
                Some(target) => Some(token_label(conn, target)?),
                None => None,
            };
            diesel::insert_into(world_attacks::table)
                .values(&AttackRecord {
                    id: attack_id,
                    world_id,
                    scene_id,
                    combat_id: combat.map(|(id, _)| id),
                    attacker_token_id: Some(request.attacker_token_id),
                    target_token_id: target,
                    attacker_label: attacker_label.clone(),
                    target_label,
                    ability_id: part.ability_id,
                    item_id: part.item_id,
                    ability_name: part.name.clone(),
                    multiattack_of: made.attack_ids.first().copied(),
                    to_hit_roll_id: Some(to_hit_roll_id),
                    damage_roll_id: damage.as_ref().map(|(id, _, _)| *id),
                    defence,
                    outcome: outcome.to_string(),
                    distance,
                    flags: flags.iter().cloned().map(Some).collect(),
                    action_cost: action_cost.as_db_str().to_string(),
                    created_by: user_id,
                    updated_by: user_id,
                    created_at: now,
                    updated_at: now,
                })
                .execute(conn)?;
            made.attack_ids.push(attack_id);
            let _ = record_world_event(
                conn,
                world_id,
                EVENT_CODE_ATTACK_MADE,
                Some(serde_json::json!({ "attackId": attack_id })),
                user_id,
            );

            // C4: a miss, no target, or nothing to take offers nothing.
            let (Some(target), Some((_, _, amount))) = (target, damage) else {
                continue;
            };
            let target_system = target_system(conn, world_system.as_deref(), target)?;
            // M1: a pack with no hit points shows the number and offers nothing.
            let declares_hit_points = target_system
                .as_deref()
                .is_some_and(|id| combat_for_system(systems_dir, id).hit_points.is_some());
            if !declares_hit_points {
                continue;
            }

            let target_control = token_control(conn, target)?.ok_or_else(|| {
                FightRefusal::NotFound("That target is not on this scene".to_string())
            })?;
            let target_linked = tokens::table
                .filter(tokens::token_id.eq(target))
                .select(tokens::linked)
                .first::<bool>(conn)?;
            let amount = amount.round().max(0.0).min(i32::MAX as f64) as i32;
            let offer_id = Uuid::now_v7();
            diesel::insert_into(world_offers::table)
                .values(&OfferRecord {
                    id: offer_id,
                    world_id,
                    scene_id,
                    attack_id: Some(attack_id),
                    target_token_id: target,
                    target_linked,
                    kind: OFFER_DAMAGE.to_string(),
                    amount,
                    status: OFFER_PENDING.to_string(),
                    resolved_by: None,
                    resolved_on_behalf: false,
                    resolved_at: None,
                    created_by: user_id,
                    updated_by: user_id,
                    created_at: now,
                    updated_at: now,
                })
                .execute(conn)?;
            made.offer_ids.push(offer_id);

            // C5: auto-apply, or the offer waits for whoever controls the target.
            let players = player_controllers(conn, &target_control)?;
            if auto_apply_holds(
                effective_auto_apply,
                &players,
                &flags,
                part.reach.needs_line_of_sight,
            ) {
                // A savepoint: a creature with no hit points recorded leaves
                // the offer pending for a person to deal with, rather than
                // losing the attack.
                let applied = conn.transaction::<_, FightRefusal, _>(|conn| {
                    apply_hit_point_change(
                        conn,
                        systems_dir,
                        target,
                        HitPointChangeKind::Damage,
                        amount,
                        user_id,
                    )
                    .map_err(FightRefusal::Invalid)?;
                    diesel::update(world_offers::table.filter(world_offers::id.eq(offer_id)))
                        .set((
                            world_offers::status.eq(OFFER_APPLIED),
                            world_offers::resolved_at.eq(Some(now)),
                        ))
                        .execute(conn)?;
                    Ok(())
                });
                if let Err(FightRefusal::Invalid(reason)) = &applied {
                    tracing::info!(offer_id = %offer_id, %reason, "auto-apply left an offer pending");
                } else {
                    applied?;
                }
            }
            let _ = record_world_event(
                conn,
                world_id,
                EVENT_CODE_OFFER_CHANGED,
                Some(serde_json::json!({ "offerId": offer_id })),
                user_id,
            );
        }

        Ok(made)
    })
}

/// The system a target's hit points are declared by: its actor's, else the
/// world's — the same resolution `apply_hit_point_change` makes.
fn target_system(
    conn: &mut PgConnection,
    world_system: Option<&str>,
    target: Uuid,
) -> QueryResult<Option<String>> {
    let actor_system = tokens::table
        .inner_join(world_actors::table.on(world_actors::id.nullable().eq(tokens::actor_id)))
        .filter(tokens::token_id.eq(target))
        .select(world_actors::game_system_id)
        .first::<Option<String>>(conn)
        .optional()?
        .flatten();
    Ok(actor_system.or_else(|| world_system.map(str::to_string)))
}

/// The scene an attack belongs to, for a caller holding only its id.
pub fn scene_of_attack(
    conn: &mut PgConnection,
    attack_id: Uuid,
) -> QueryResult<Option<(Uuid, Uuid)>> {
    world_attacks::table
        .inner_join(scenes::table.on(scenes::scene_id.eq(world_attacks::scene_id)))
        .filter(world_attacks::id.eq(attack_id))
        .select((world_attacks::world_id, world_attacks::scene_id))
        .first::<(Uuid, Uuid)>(conn)
        .optional()
}

#[cfg(test)]
#[path = "attack_tests.rs"]
mod tests;
