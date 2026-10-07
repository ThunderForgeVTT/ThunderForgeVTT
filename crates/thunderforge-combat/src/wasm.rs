//! The browser façade (spec 079 FR-002, ADR-113).
//!
//! JSON in, JSON out, as `thunderforge-pdf`'s façade does: the demo's backend
//! holds its creatures and combat as plain objects, so a rule takes those and
//! gives back what it decided. Every function here is a thin wrapper over the
//! module that holds the rule; none decides anything itself.
//!
//! A malformed argument is refused with one sentence, never a panic.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use thunderforge_canvas_core::Vec2;
use thunderforge_canvas_core::grid::Footprint;
use thunderforge_canvas_core::measure::GridUnits;
use thunderforge_canvas_core::wall::{DoorState, Wall, WallSet};
use thunderforge_dice::PlaceholderBindings;
use wasm_bindgen::prelude::*;

use crate::attack;
use crate::budget::{self, Spend, Spent};
use crate::dice::{ScriptedDice, SeededDice};
use crate::hit_points::{self, HitPointChangeKind, HitPoints};
use crate::manifest;
use crate::order::{self, Seat};
use crate::reach::{self, Reach};
use crate::size;
use crate::turn::{self, Party};
use crate::turn_structure;

fn parse<T: DeserializeOwned>(what: &str, json: &str) -> Result<T, String> {
    serde_json::from_str(json).map_err(|e| format!("Unreadable {what}: {e}"))
}

fn to_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
}

/// The dice a fight is rolled with: seeded, or named faces for a test.
#[wasm_bindgen]
pub struct Dice {
    inner: DiceKind,
}

enum DiceKind {
    Seeded(SeededDice),
    Scripted(ScriptedDice),
}

#[wasm_bindgen]
impl Dice {
    /// A stream from a seed. The demo seeds it from `crypto.getRandomValues`.
    #[wasm_bindgen(js_name = seeded)]
    pub fn seeded(seed: f64) -> Dice {
        Dice {
            inner: DiceKind::Seeded(SeededDice::new(seed as u64)),
        }
    }

    /// Faces that come up as named, in order, then round again.
    #[wasm_bindgen(js_name = scripted)]
    pub fn scripted(faces: Vec<u32>) -> Dice {
        Dice {
            inner: DiceKind::Scripted(ScriptedDice::faces(faces)),
        }
    }
}

impl Dice {
    fn roll(
        &mut self,
        source: &str,
        bindings: &PlaceholderBindings,
    ) -> Result<(String, f64), String> {
        let (resolution, value) = match &mut self.inner {
            DiceKind::Seeded(d) => attack::roll(source, bindings, d)?,
            DiceKind::Scripted(d) => attack::roll(source, bindings, d)?,
        };
        Ok((to_json(&resolution), value))
    }
}

/// Roll one formula: `{ resolution, value }`.
#[wasm_bindgen(js_name = roll)]
pub fn roll(dice: &mut Dice, source: &str, bindings_json: &str) -> Result<String, String> {
    let bindings: PlaceholderBindings = parse("bindings", bindings_json)?;
    let (resolution, value) = dice.roll(source, &bindings)?;
    Ok(format!(
        "{{\"resolution\":{resolution},\"value\":{}}}",
        to_json(&value)
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PartIn {
    to_hit: String,
    #[serde(default)]
    damage: Vec<String>,
    #[serde(default)]
    bindings: PlaceholderBindings,
    has_target: bool,
    defence: Option<i32>,
}

/// One part of an attack, in the server's order: to-hit, judged against the
/// defence, then damage only on a hit. `{ toHit, toHitTotal, outcome,
/// damage, amount }`, `damage` and `amount` null on anything but a hit.
#[wasm_bindgen(js_name = attackPart)]
pub fn attack_part(dice: &mut Dice, part_json: &str) -> Result<String, String> {
    let part: PartIn = parse("attack part", part_json)?;
    let (to_hit, total) = dice.roll(&part.to_hit, &part.bindings)?;
    let outcome = attack::judge(part.has_target, part.defence, total);
    let damage = match attack::damage_source(&part.damage) {
        Some(source) if outcome == crate::records::OUTCOME_HIT => {
            Some(dice.roll(&source, &part.bindings)?)
        }
        _ => None,
    };
    let (damage, amount) = match damage {
        Some((resolution, value)) => (resolution, to_json(&attack::offered_amount(value))),
        None => ("null".to_string(), "null".to_string()),
    };
    Ok(format!(
        "{{\"toHit\":{to_hit},\"toHitTotal\":{},\"outcome\":{},\"damage\":{damage},\"amount\":{amount}}}",
        to_json(&total),
        to_json(&outcome)
    ))
}

/// An ability's to-hit and damage out of its `[kind, formula]` effects:
/// `{ toHit, damage }`, or the sentence that it cannot attack.
#[wasm_bindgen(js_name = attackFormulas)]
pub fn attack_formulas(name: &str, effects_json: &str) -> Result<String, String> {
    let effects: Vec<(String, String)> = parse("effects", effects_json)?;
    let (to_hit, damage) = attack::attack_formulas(name, &effects)?;
    Ok(serde_json::json!({ "toHit": to_hit, "damage": damage }).to_string())
}

/// Whether an offer applies itself (research R15).
#[wasm_bindgen(js_name = autoApplyHolds)]
pub fn auto_apply_holds(
    effective_setting: bool,
    any_player_controls_target: bool,
    flags_json: &str,
    needs_line_of_sight: bool,
) -> Result<bool, String> {
    let flags: Vec<String> = parse("flags", flags_json)?;
    Ok(attack::auto_apply_holds(
        effective_setting,
        any_player_controls_target,
        &flags,
        needs_line_of_sight,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WallIn {
    id: String,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    #[serde(default = "yes")]
    blocks_vision: bool,
    #[serde(default = "yes")]
    blocks_movement: bool,
    #[serde(default)]
    door_state: String,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Placed {
    x: f32,
    y: f32,
    #[serde(default = "one")]
    footprint: f32,
}

fn one() -> f32 {
    1.0
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MeasureIn {
    grid_type: String,
    grid_size: i32,
    #[serde(default)]
    width: i32,
    #[serde(default)]
    height: i32,
    #[serde(default)]
    units: Option<UnitsIn>,
    #[serde(default)]
    walls: Vec<WallIn>,
    from: Placed,
    to: Placed,
    #[serde(default)]
    reach: Reach,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnitsIn {
    per_cell: f32,
    label: String,
}

/// Distance and flags between two placed creatures on a scene, as the
/// server's `SceneMeasure` measures them: `{ distance, flags }`.
#[wasm_bindgen(js_name = measure)]
pub fn measure(measure_json: &str) -> Result<String, String> {
    let m: MeasureIn = parse("measurement", measure_json)?;
    let grid = reach::scene_grid(&m.grid_type, m.grid_size, m.width, m.height);
    let units = m
        .units
        .map(|u| GridUnits {
            per_cell: u.per_cell,
            label: u.label,
        })
        .unwrap_or_default();
    let mut walls = WallSet::default();
    for w in m.walls {
        walls.upsert(Wall {
            id: w.id,
            x1: w.x1,
            y1: w.y1,
            x2: w.x2,
            y2: w.y2,
            blocks_vision: w.blocks_vision,
            blocks_movement: w.blocks_movement,
            door_state: DoorState::from_str_loose(&w.door_state),
            locked: false,
            secret: false,
        });
    }
    let at = |p: &Placed| (Vec2::new(p.x, p.y), Footprint::new(p.footprint));
    Ok(to_json(&reach::measure(
        &grid,
        &units,
        &walls,
        at(&m.from),
        at(&m.to),
        &m.reach,
    )))
}

/// A creature's hit points out of its slot, by the system's `combat` block:
/// `{ current, max, temporary }` or null.
#[wasm_bindgen(js_name = readHitPoints)]
pub fn read_hit_points(manifest_json: &str, slot_json: &str) -> Result<String, String> {
    let combat = manifest::combat_from_manifest(&parse("manifest", manifest_json)?);
    let Some(declared) = combat.hit_points else {
        return Ok("null".to_string());
    };
    let slot: serde_json::Value = parse("slot", slot_json)?;
    Ok(to_json(&hit_points::read_hit_points(&slot, &declared)?))
}

/// The slot with these hit points written into it.
#[wasm_bindgen(js_name = writeHitPoints)]
pub fn write_hit_points(
    manifest_json: &str,
    slot_json: &str,
    hit_points_json: &str,
) -> Result<String, String> {
    let combat = manifest::combat_from_manifest(&parse("manifest", manifest_json)?);
    let declared = combat
        .hit_points
        .ok_or_else(|| "This system declares no hit points".to_string())?;
    let slot: serde_json::Value = parse("slot", slot_json)?;
    let value: HitPoints = parse("hit points", hit_points_json)?;
    Ok(hit_points::write_hit_points(&slot, &declared, value).to_string())
}

/// Damage or healing, the arithmetic (C7). `kind` is `Damage` or `Healing`.
#[wasm_bindgen(js_name = applyHitPoints)]
pub fn apply_hit_points(hit_points_json: &str, kind: &str, amount: i32) -> Result<String, String> {
    let before: HitPoints = parse("hit points", hit_points_json)?;
    let kind: HitPointChangeKind = parse("change", &to_json(&kind))?;
    Ok(to_json(&hit_points::apply_to(before, kind, amount)))
}

/// What a change does to a combatant's place in the fight (C8):
/// `{ active, downedBy }`, or null when nothing changes.
#[wasm_bindgen(js_name = standingAfter)]
pub fn standing_after(current: i32, active: bool, downed_by: Option<String>) -> String {
    match hit_points::standing_after(current, active, downed_by.as_deref()) {
        Some((active, downed_by)) => {
            serde_json::json!({ "active": active, "downedBy": downed_by }).to_string()
        }
        None => "null".to_string(),
    }
}

/// The footprint a creature's slot names, by the system's sizes.
#[wasm_bindgen(js_name = footprintFrom)]
pub fn footprint_from(manifest_json: &str, slot_json: &str) -> Result<f32, String> {
    let combat = manifest::combat_from_manifest(&parse("manifest", manifest_json)?);
    let Some(sizes) = combat.sizes else {
        return Ok(size::DEFAULT_FOOTPRINT);
    };
    let slot: serde_json::Value = parse("slot", slot_json)?;
    Ok(size::footprint_from(&sizes, Some(&slot)))
}

/// The system's `combat` block, read as the server reads it.
#[wasm_bindgen(js_name = combatFromManifest)]
pub fn combat_from_manifest(manifest_json: &str) -> Result<String, String> {
    Ok(to_json(&manifest::combat_from_manifest(&parse(
        "manifest",
        manifest_json,
    )?)))
}

/// What the system calls a round, or null when it counts none.
#[wasm_bindgen(js_name = roundLabel)]
pub fn round_label(manifest_json: &str) -> Result<Option<String>, String> {
    Ok(turn_structure::from_manifest(&parse("manifest", manifest_json)?).round_label)
}

#[derive(Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SeatIn {
    id: String,
    #[serde(default)]
    initiative: i32,
    #[serde(default)]
    tiebreak: i32,
    #[serde(default = "yes")]
    active: bool,
    #[serde(default)]
    token_id: Option<String>,
    #[serde(default)]
    actor_id: Option<String>,
    #[serde(flatten)]
    rest: serde_json::Map<String, serde_json::Value>,
}

/// Seats are matched by their index in the list, so a string id need not be
/// `Copy`.
struct Indexed<'a>(usize, &'a SeatIn);

impl Seat for Indexed<'_> {
    type Id = usize;
    fn seat_id(&self) -> usize {
        self.0
    }
    fn initiative(&self) -> i32 {
        self.1.initiative
    }
    fn tiebreak(&self) -> i32 {
        self.1.tiebreak
    }
    fn is_active(&self) -> bool {
        self.1.active
    }
}

/// Combatants in turn order (`{ id, initiative, tiebreak, active, ... }`;
/// everything else on each is kept).
#[wasm_bindgen(js_name = sortSeats)]
pub fn sort_seats(seats_json: &str) -> Result<String, String> {
    let seats: Vec<SeatIn> = parse("combatants", seats_json)?;
    let mut indexed: Vec<Indexed> = seats
        .iter()
        .enumerate()
        .map(|(i, s)| Indexed(i, s))
        .collect();
    order::sort_seats(&mut indexed);
    let sorted: Vec<&SeatIn> = indexed.iter().map(|i| i.1).collect();
    Ok(to_json(&sorted))
}

/// Who is next after `activeId`, among combatants already in turn order:
/// `{ index, newRound }`, or null when nobody can act.
#[wasm_bindgen(js_name = nextTurn)]
pub fn next_turn(ordered_json: &str, active_id: Option<String>) -> Result<String, String> {
    let seats: Vec<SeatIn> = parse("combatants", ordered_json)?;
    let indexed: Vec<Indexed> = seats
        .iter()
        .enumerate()
        .map(|(i, s)| Indexed(i, s))
        .collect();
    let active = active_id.and_then(|id| seats.iter().position(|s| s.id == id));
    Ok(match order::next_turn(&indexed, active) {
        Some((index, new_round)) => {
            serde_json::json!({ "index": index, "newRound": new_round }).to_string()
        }
        None => "null".to_string(),
    })
}

/// The label of the combatant whose turn holds `tokenId` back, or null when
/// it may act. A Game Master is never asked; the caller does not call this.
#[wasm_bindgen(js_name = heldBy)]
pub fn held_by(
    combatants_json: &str,
    active_id: &str,
    token_id: &str,
    actor_id: Option<String>,
) -> Result<Option<String>, String> {
    let seats: Vec<SeatIn> = parse("combatants", combatants_json)?;
    // Ids are strings in the browser; the rule compares them by position in
    // a shared table so it keeps its `Copy` ids.
    let mut ids: Vec<String> = Vec::new();
    let mut intern = |s: &str| -> usize {
        match ids.iter().position(|i| i == s) {
            Some(i) => i,
            None => {
                ids.push(s.to_string());
                ids.len() - 1
            }
        }
    };
    struct P {
        id: usize,
        token: Option<usize>,
        actor: Option<usize>,
        seat: usize,
    }
    impl Party for P {
        type Id = usize;
        fn party_id(&self) -> usize {
            self.id
        }
        fn token(&self) -> Option<usize> {
            self.token
        }
        fn actor(&self) -> Option<usize> {
            self.actor
        }
    }
    let parties: Vec<P> = seats
        .iter()
        .enumerate()
        .map(|(seat, s)| P {
            id: intern(&s.id),
            token: s.token_id.as_deref().map(&mut intern),
            actor: s.actor_id.as_deref().map(&mut intern),
            seat,
        })
        .collect();
    let active = intern(active_id);
    let token = intern(token_id);
    let actor = actor_id.as_deref().map(&mut intern);
    Ok(turn::held_by(&parties, active, token, actor).map(|p| {
        let seat = &seats[p.seat];
        seat.rest
            .get("label")
            .and_then(|l| l.as_str())
            .unwrap_or(&seat.id)
            .to_string()
    }))
}

/// "It is <label>'s turn".
#[wasm_bindgen(js_name = turnRefusal)]
pub fn turn_refusal(label: &str) -> String {
    turn::turn_refusal(label)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BudgetIn {
    manifest: serde_json::Value,
    speed: f64,
    #[serde(default)]
    spent: Spent,
    #[serde(default)]
    unit: Option<String>,
}

fn declared_budget(
    manifest: &serde_json::Value,
) -> Result<crate::manifest::SystemTurnBudget, String> {
    manifest::turn_budget_from_manifest(manifest)
        .ok_or_else(|| "This system declares no turn budget".to_string())
}

/// What a turn affords and has spent: a `TurnBudget`.
#[wasm_bindgen(js_name = resolveBudget)]
pub fn resolve_budget(budget_json: &str) -> Result<String, String> {
    let b: BudgetIn = parse("budget", budget_json)?;
    let declared = declared_budget(&b.manifest)?;
    let unit = b.unit.unwrap_or_else(|| "ft".to_string());
    Ok(to_json(&budget::resolve(
        &declared, b.speed, &b.spent, &unit,
    )))
}

/// The spend an attack of this cost makes, or null for a free one.
#[wasm_bindgen(js_name = spendForAttack)]
pub fn spend_for_attack(cost: &str, legendary_cost: i32) -> Result<String, String> {
    let cost: attack::ActionCost = parse("action cost", &to_json(&cost))?;
    Ok(to_json(&Spend::for_attack(cost, legendary_cost)))
}

/// `spent` after `spend` (`"Action"`, `{"Movement": 15}`, ...).
#[wasm_bindgen(js_name = takeSpend)]
pub fn take_spend(spent_json: &str, spend_json: &str) -> Result<String, String> {
    let mut spent: Spent = parse("spent", spent_json)?;
    spent.take(parse("spend", spend_json)?);
    Ok(to_json(&spent))
}

/// `spent` at the start of its combatant's turn.
#[wasm_bindgen(js_name = startTurn)]
pub fn start_turn(spent_json: &str) -> Result<String, String> {
    let mut spent: Spent = parse("spent", spent_json)?;
    spent.start_turn();
    Ok(to_json(&spent))
}

/// The flags a spend puts on an attack: judged after the spend (`after`
/// true, for the record) or before it (for the warning).
#[wasm_bindgen(js_name = spendFlags)]
pub fn spend_flags(
    budget_json: &str,
    spend_json: &str,
    own_turn: bool,
    after: bool,
) -> Result<String, String> {
    let b: budget::TurnBudget = parse("turn budget", budget_json)?;
    let what: Spend = parse("spend", spend_json)?;
    let flags = if after {
        budget::flags_after_spend(&b, what, own_turn)
    } else {
        budget::flags_before_spend(&b, what, own_turn)
    };
    Ok(to_json(&flags))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveIn {
    grid_type: String,
    grid_size: i32,
    #[serde(default)]
    width: i32,
    #[serde(default)]
    height: i32,
    #[serde(default)]
    units: Option<UnitsIn>,
    #[serde(default = "one")]
    footprint: f32,
    #[serde(default = "walk")]
    speed_kind: String,
    from: [f32; 2],
    #[serde(default)]
    route: Vec<[f32; 2]>,
    to: [f32; 2],
}

fn walk() -> String {
    "walk".to_string()
}

/// What a move costs, in the system's units.
#[wasm_bindgen(js_name = moveCost)]
pub fn move_cost(move_json: &str) -> Result<f64, String> {
    let m: MoveIn = parse("move", move_json)?;
    let grid = reach::scene_grid(&m.grid_type, m.grid_size, m.width, m.height);
    let units = m
        .units
        .map(|u| GridUnits {
            per_cell: u.per_cell,
            label: u.label,
        })
        .unwrap_or_default();
    let point = |p: [f32; 2]| Vec2::new(p[0], p[1]);
    let route: Vec<Vec2> = m.route.into_iter().map(point).collect();
    Ok(budget::move_cost(
        &grid,
        &units,
        Footprint::new(m.footprint),
        &m.speed_kind,
        point(m.from),
        &route,
        point(m.to),
    ))
}
