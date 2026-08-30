//! What a move costs, and whether a creature can afford it.
//!
//! `movement.rs` answers "where would the token go"; this answers "may it,
//! and what is left". They are separate because a path is worth drawing
//! before it is known to be affordable — showing the route and its cost in
//! red is more useful than refusing to draw it.
//!
//! # Terrain multiplies a step; a dash multiplies the budget
//!
//! These look interchangeable and are not, which is the mistake this module
//! exists to avoid. "Difficult terrain halves your movement" is how the rule
//! is *said*, and halving the budget is wrong the moment only part of a route
//! is rough — the common case. What actually happens is that each rough cell
//! costs double, so a creature crossing three rough cells out of six pays
//! nine, not six, while one crossing none pays six.
//!
//! Dashing is genuinely a budget change: it is a whole action that buys
//! another full move, and it applies to the turn rather than to the ground.
//! So terrain scales cost per step and dashing scales the budget, and neither
//! is expressible as the other.
//!
//! # Why floating point is safe here
//!
//! Cost accumulates one step at a time, and drift over a long path would be a
//! real hazard — except that every multiplier any ruleset actually uses is a
//! binary fraction: 1, 2 (difficult), 1/2, 3/2. All are exact in `f32`, and
//! sums of exact binary fractions are exact until they exhaust the mantissa,
//! which a path of a few hundred cells does not approach. A ruleset wanting
//! thirds would need a rational type; none of the eight here does.
//!
//! # Speeds are declared, not enumerated
//!
//! Walk, fly, swim, climb, burrow — and whatever a system this crate has
//! never heard of calls its own. Keyed by string for the same reason
//! attributes are: an engine that can name a movement type privileges the
//! ruleset that has it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::measure::GridUnits;

/// What a cell costs to enter, and to whom.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
pub struct TerrainCost {
    /// Multiplier applied to a step entering this cell. 2.0 is the usual
    /// "difficult terrain".
    pub multiplier: f32,
    /// Movement types this does **not** apply to.
    ///
    /// The reason terrain is not simply a number: a creature flying over a
    /// bog is not slowed by it, and a rule engine that cannot express that
    /// forces every table to adjudicate flight by hand. Named rather than
    /// inferred, because which types ignore which hazards is a ruleset's
    /// decision and not this crate's.
    #[serde(default)]
    pub ignored_by: Vec<String>,
}

impl Default for TerrainCost {
    /// Open ground: one cell costs one cell, for everybody.
    fn default() -> Self {
        Self {
            multiplier: 1.0,
            ignored_by: Vec::new(),
        }
    }
}

impl TerrainCost {
    /// Difficult terrain, as almost every d20 ruleset defines it.
    pub fn difficult() -> Self {
        Self {
            multiplier: 2.0,
            ignored_by: Vec::new(),
        }
    }

    /// What entering this cell costs a creature moving in `kind`.
    ///
    /// A non-finite or non-positive multiplier falls back to open ground
    /// rather than propagating: a scene carrying rubbish should make a move
    /// cost the normal amount, not zero (free infinite movement) or NaN
    /// (a budget that can never be satisfied).
    pub fn cost_for(&self, kind: &str) -> f32 {
        if self.ignored_by.iter().any(|k| k == kind) {
            return 1.0;
        }
        if self.multiplier.is_finite() && self.multiplier > 0.0 {
            self.multiplier
        } else {
            1.0
        }
    }
}

/// The speeds a creature has, in the scene's spoken units.
///
/// Stated the way a stat block states them — "30 feet", "60 feet flying" —
/// rather than in cells, because that is what a book says and what somebody
/// types in. Converting to cells is the grid's business, not the sheet's.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(transparent)]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
pub struct MovementSpeeds(pub BTreeMap<String, f32>);

/// The movement type used when a creature has exactly one, and the
/// conventional name for moving on the ground.
pub const DEFAULT_MOVEMENT: &str = "walk";

impl MovementSpeeds {
    pub fn new(speeds: impl IntoIterator<Item = (String, f32)>) -> Self {
        Self(speeds.into_iter().collect())
    }

    /// The speed for one movement type, if the creature has it.
    ///
    /// `None` means the creature cannot move that way at all — which is a
    /// different statement from a speed of zero (it can, but is currently
    /// prevented), and the two must not be conflated: a grappled creature has
    /// a walk speed of 0, a fish has none.
    pub fn get(&self, kind: &str) -> Option<f32> {
        self.0.get(kind).copied()
    }

    /// Every movement type this creature has, in a stable order.
    pub fn kinds(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// One movement type a system declares.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../apps/web/src/engine/sdk/")]
pub struct MovementDeclaration {
    /// The system's own identifier — `walk`, `fly`, `stride`.
    pub id: String,
    /// What a person is shown. Systems disagree even where ids match:
    /// Pathfinder 2e calls its ground speed simply "Speed".
    pub label: String,
    /// Where to read the value in the actor's stored sheet.
    pub source: String,
    /// Speed to assume when the sheet says nothing.
    ///
    /// Only for the movement type every creature has — a ground speed. A
    /// default on flight would give wings to everything that failed to
    /// mention it, so most declarations leave this unset and are simply
    /// absent for a creature that cannot move that way.
    pub default: Option<f32>,
    pub order: usize,
}

/// Read declared speeds out of an actor's stored sheet.
///
/// A type the actor stores nothing for is omitted unless the declaration
/// carries a default. That is the distinction `MovementSpeeds::get` rests on:
/// absent means "cannot move this way", and it must not be manufactured into
/// a zero.
pub fn speeds_from(
    slot: &serde_json::Value,
    declarations: &[MovementDeclaration],
) -> MovementSpeeds {
    let mut ordered: Vec<&MovementDeclaration> = declarations.iter().collect();
    ordered.sort_by_key(|d| d.order);

    MovementSpeeds(
        ordered
            .into_iter()
            .filter_map(|declaration| {
                let stored = slot
                    .get(&declaration.source)
                    .and_then(read_speed)
                    .or(declaration.default)?;
                Some((declaration.id.clone(), stored))
            })
            .collect(),
    )
}

/// A speed from stored JSON, accepting the shapes a real sheet uses.
fn read_speed(raw: &serde_json::Value) -> Option<f32> {
    let raw = if raw.is_object() {
        raw.get("value")?
    } else {
        raw
    };
    let value = raw.as_f64()?;
    // A negative or non-finite speed is not a slower creature, it is broken
    // data, and treating it as zero would silently immobilise a token.
    if value.is_finite() && value >= 0.0 {
        Some(value as f32)
    } else {
        None
    }
}

/// What a creature may spend this turn.
#[derive(Clone, Debug, PartialEq)]
pub struct MovementBudget {
    /// Speed for the movement type in use, in the scene's units.
    pub speed: f32,
    /// Whole-turn budget multiplier — 2.0 for a dash, 3.0 where a ruleset
    /// allows two. Applies to the allowance, never to the ground.
    pub multiplier: f32,
    /// Units already spent this turn, so a move can be planned from a
    /// partially-used allowance rather than only from a fresh one.
    pub already_spent: f32,
}

impl MovementBudget {
    pub fn new(speed: f32) -> Self {
        Self {
            speed,
            multiplier: 1.0,
            already_spent: 0.0,
        }
    }

    /// Dashing, or any other whole-turn doubling.
    pub fn with_multiplier(mut self, multiplier: f32) -> Self {
        self.multiplier = multiplier;
        self
    }

    pub fn with_spent(mut self, already_spent: f32) -> Self {
        self.already_spent = already_spent;
        self
    }

    /// Total allowance for the turn, in the scene's units.
    pub fn total(&self) -> f32 {
        let multiplier = if self.multiplier.is_finite() && self.multiplier > 0.0 {
            self.multiplier
        } else {
            1.0
        };
        let speed = if self.speed.is_finite() && self.speed > 0.0 {
            self.speed
        } else {
            0.0
        };
        speed * multiplier
    }

    /// What is left before this move.
    pub fn remaining(&self) -> f32 {
        (self.total() - self.already_spent).max(0.0)
    }
}

/// The answer to "can it go there, and what does it cost".
#[derive(Clone, Debug, PartialEq)]
pub struct MovementCost {
    /// Cost in cells, with terrain applied.
    pub cells: f32,
    /// The same cost in the scene's spoken units.
    pub distance: f32,
    /// What would be left afterwards. Zero rather than negative when the
    /// move is unaffordable — `affordable` is what says whether it fits.
    pub remaining: f32,
    /// Whether the budget covers it.
    pub affordable: bool,
    /// How far over, in the scene's units. Zero when affordable, so a caller
    /// can say "12 ft over" without recomputing.
    pub overage: f32,
}

/// Cost a planned route, cell by cell.
///
/// `terrain` is the cost of entering each cell of the path in order — one
/// entry per step, not per cell of the map. A shorter list is treated as open
/// ground beyond its end rather than as an error: a partially-known map
/// should quote an optimistic cost, not refuse to quote one.
pub fn cost_path(
    terrain: &[TerrainCost],
    movement_kind: &str,
    units: &GridUnits,
    budget: &MovementBudget,
) -> MovementCost {
    // Steps past the end of the known terrain cost open ground, which is what
    // summing only the entries present amounts to.
    let cells: f32 = terrain
        .iter()
        .map(|cell| cell.cost_for(movement_kind))
        .sum();

    let distance = units.distance(cells);
    let remaining = budget.remaining();
    let affordable = distance <= remaining + f32::EPSILON;

    MovementCost {
        cells,
        distance,
        remaining: (remaining - distance).max(0.0),
        affordable,
        overage: if affordable {
            0.0
        } else {
            distance - remaining
        },
    }
}

/// How far a creature could still go, in cells of open ground.
///
/// For drawing a reachable area before a route exists. Deliberately assumes
/// open ground: a range indicator that pre-applied terrain would have to
/// solve a shortest-path problem per cell, and would still be wrong the
/// moment the player chose a different route.
pub fn reach_in_cells(units: &GridUnits, budget: &MovementBudget) -> f32 {
    let per_cell = units.distance(1.0);
    if per_cell <= 0.0 {
        return 0.0;
    }
    budget.remaining() / per_cell
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feet() -> GridUnits {
        GridUnits::new(5.0, "ft")
    }

    fn open(n: usize) -> Vec<TerrainCost> {
        vec![TerrainCost::default(); n]
    }

    /// Six cells of open ground at 5ft each is 30ft, which a speed-30
    /// creature affords exactly.
    #[test]
    fn a_full_move_across_open_ground_is_exactly_affordable() {
        let cost = cost_path(
            &open(6),
            DEFAULT_MOVEMENT,
            &feet(),
            &MovementBudget::new(30.0),
        );
        assert_eq!(cost.cells, 6.0);
        assert_eq!(cost.distance, 30.0);
        assert!(cost.affordable, "30ft of movement must cover 30ft");
        assert_eq!(cost.remaining, 0.0);
        assert_eq!(cost.overage, 0.0);
    }

    /// The property this module exists for.
    ///
    /// "Difficult terrain halves your movement" is how the rule is said, and
    /// halving the budget would let this creature cross six rough cells. It
    /// cannot: each costs double, so thirty feet buys three.
    #[test]
    fn difficult_terrain_costs_double_rather_than_halving_the_allowance() {
        let rough = vec![TerrainCost::difficult(); 6];
        let cost = cost_path(
            &rough,
            DEFAULT_MOVEMENT,
            &feet(),
            &MovementBudget::new(30.0),
        );

        assert_eq!(cost.cells, 12.0, "six rough cells cost twelve");
        assert!(!cost.affordable);
        assert_eq!(cost.overage, 30.0);
    }

    /// And the case that makes halving the budget indefensible: a route that
    /// is only partly rough. Halving would allow three cells; the truth is
    /// four and a half of six.
    #[test]
    fn a_partly_rough_route_costs_only_for_its_rough_cells() {
        let mixed = vec![
            TerrainCost::default(),
            TerrainCost::default(),
            TerrainCost::default(),
            TerrainCost::difficult(),
            TerrainCost::difficult(),
        ];
        let cost = cost_path(
            &mixed,
            DEFAULT_MOVEMENT,
            &feet(),
            &MovementBudget::new(35.0),
        );

        assert_eq!(cost.cells, 7.0, "three open plus two rough");
        assert_eq!(cost.distance, 35.0);
        assert!(cost.affordable);
    }

    /// Flight is why terrain is not just a number.
    #[test]
    fn a_flier_is_not_slowed_by_ground_it_never_touches() {
        let bog = vec![
            TerrainCost {
                multiplier: 2.0,
                ignored_by: vec!["fly".into()],
            };
            6
        ];

        let walking = cost_path(&bog, "walk", &feet(), &MovementBudget::new(30.0));
        let flying = cost_path(&bog, "fly", &feet(), &MovementBudget::new(30.0));

        assert_eq!(walking.cells, 12.0);
        assert_eq!(flying.cells, 6.0, "the bog does not slow a flier");
        assert!(flying.affordable);
        assert!(!walking.affordable);
    }

    /// Dashing scales the allowance, and stacks with terrain rather than
    /// cancelling it.
    #[test]
    fn a_dash_doubles_the_allowance_and_terrain_still_applies() {
        let rough = vec![TerrainCost::difficult(); 6];
        let dashing = MovementBudget::new(30.0).with_multiplier(2.0);

        assert_eq!(dashing.total(), 60.0);
        let cost = cost_path(&rough, DEFAULT_MOVEMENT, &feet(), &dashing);
        assert_eq!(cost.cells, 12.0, "dashing does not smooth the ground");
        assert_eq!(cost.distance, 60.0);
        assert!(cost.affordable, "but it does pay for it");
    }

    /// A move planned from a partly-spent turn.
    #[test]
    fn movement_already_spent_comes_off_the_allowance() {
        let budget = MovementBudget::new(30.0).with_spent(20.0);
        assert_eq!(budget.remaining(), 10.0);

        let cost = cost_path(&open(3), DEFAULT_MOVEMENT, &feet(), &budget);
        assert!(!cost.affordable, "15ft does not fit in the 10ft left");
        assert_eq!(cost.overage, 5.0);
    }

    /// Speeds differ per creature and per type, which was the starting
    /// complaint: a hard-coded 30 is one creature's speed, not a fact.
    #[test]
    fn different_creatures_and_types_get_different_allowances() {
        let speeds = MovementSpeeds::new([("walk".to_string(), 40.0), ("fly".to_string(), 80.0)]);

        assert_eq!(speeds.get("walk"), Some(40.0));
        assert_eq!(speeds.get("fly"), Some(80.0));
        assert_eq!(speeds.get("swim"), None, "it cannot swim at all");
        assert_eq!(speeds.kinds(), vec!["fly", "walk"]);
    }

    /// The distinction that decides whether a grappled creature may move.
    #[test]
    fn a_speed_of_zero_is_not_the_same_as_having_no_such_speed() {
        let grappled = MovementSpeeds::new([("walk".to_string(), 0.0)]);
        assert_eq!(grappled.get("walk"), Some(0.0), "it walks, but not far");
        assert_eq!(grappled.get("fly"), None, "it does not fly at all");

        let cost = cost_path(
            &open(1),
            DEFAULT_MOVEMENT,
            &feet(),
            &MovementBudget::new(0.0),
        );
        assert!(!cost.affordable);
    }

    /// The unit is whatever the scene says, and the arithmetic follows it.
    #[test]
    fn the_same_route_costs_differently_under_different_units() {
        let route = open(4);
        let budget = MovementBudget::new(6.0);

        let metric = cost_path(&route, DEFAULT_MOVEMENT, &GridUnits::new(1.5, "m"), &budget);
        assert_eq!(metric.distance, 6.0);
        assert!(metric.affordable, "four cells at 1.5m is 6m");

        // The same four cells on a hex map counted in hexes.
        let hexes = cost_path(
            &route,
            DEFAULT_MOVEMENT,
            &GridUnits::new(1.0, "hexes"),
            &budget,
        );
        assert_eq!(hexes.distance, 4.0);
        assert!(hexes.affordable);
    }

    /// A scene carrying nonsense must not grant free movement.
    #[test]
    fn rubbish_terrain_costs_open_ground_rather_than_nothing() {
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let cost = cost_path(
                &[TerrainCost {
                    multiplier: bad,
                    ignored_by: Vec::new(),
                }],
                DEFAULT_MOVEMENT,
                &feet(),
                &MovementBudget::new(30.0),
            );
            assert_eq!(
                cost.cells, 1.0,
                "multiplier {bad} must fall back to open ground"
            );
        }
    }

    /// Reach is quoted over open ground, and shrinks as the turn is spent.
    #[test]
    fn reach_reflects_what_is_left_of_the_turn() {
        let fresh = MovementBudget::new(30.0);
        assert_eq!(reach_in_cells(&feet(), &fresh), 6.0);

        let spent = MovementBudget::new(30.0).with_spent(25.0);
        assert_eq!(reach_in_cells(&feet(), &spent), 1.0);

        let dashing = MovementBudget::new(30.0).with_multiplier(2.0);
        assert_eq!(reach_in_cells(&feet(), &dashing), 12.0);
    }

    /// An empty route is free, and must not be reported as unaffordable.
    #[test]
    fn a_route_of_no_steps_costs_nothing() {
        let cost = cost_path(&[], DEFAULT_MOVEMENT, &feet(), &MovementBudget::new(0.0));
        assert_eq!(cost.cells, 0.0);
        assert!(cost.affordable, "standing still is always allowed");
    }

    fn declare(id: &str, source: &str, default: Option<f32>, order: usize) -> MovementDeclaration {
        MovementDeclaration {
            id: id.to_string(),
            label: id.to_string(),
            source: source.to_string(),
            default,
            order,
        }
    }

    /// A creature with wings has both; one without has only the ground speed.
    #[test]
    fn only_the_speeds_a_creature_actually_has_are_resolved() {
        let declarations = vec![
            declare("walk", "speed_walk", Some(30.0), 0),
            declare("fly", "speed_fly", None, 1),
        ];

        let dragon = speeds_from(
            &serde_json::json!({ "speed_walk": 40, "speed_fly": 80 }),
            &declarations,
        );
        assert_eq!(dragon.get("walk"), Some(40.0));
        assert_eq!(dragon.get("fly"), Some(80.0));

        let commoner = speeds_from(&serde_json::json!({}), &declarations);
        assert_eq!(
            commoner.get("walk"),
            Some(30.0),
            "a ground speed is defaulted, because everything walks"
        );
        assert_eq!(
            commoner.get("fly"),
            None,
            "nothing gains wings by failing to mention them"
        );
    }

    /// A sheet's own number beats the default.
    #[test]
    fn a_stored_speed_overrides_the_declared_default() {
        let declarations = vec![declare("walk", "speed_walk", Some(30.0), 0)];
        let dwarf = speeds_from(&serde_json::json!({ "speed_walk": 25 }), &declarations);
        assert_eq!(dwarf.get("walk"), Some(25.0));
    }

    /// Zero is storable and meaningful — a grappled or restrained creature.
    #[test]
    fn a_stored_zero_speed_survives_resolution() {
        let declarations = vec![declare("walk", "speed_walk", Some(30.0), 0)];
        let held = speeds_from(&serde_json::json!({ "speed_walk": 0 }), &declarations);
        assert_eq!(
            held.get("walk"),
            Some(0.0),
            "zero is a real speed and must not fall through to the default"
        );
    }

    /// Broken data must not immobilise a token silently.
    #[test]
    fn a_broken_stored_speed_falls_back_to_the_default_rather_than_zero() {
        let declarations = vec![declare("walk", "speed_walk", Some(30.0), 0)];
        for bad in [
            serde_json::json!({ "speed_walk": -10 }),
            serde_json::json!({ "speed_walk": "fast" }),
            serde_json::json!({ "speed_walk": null }),
        ] {
            assert_eq!(
                speeds_from(&bad, &declarations).get("walk"),
                Some(30.0),
                "{bad} should fall back, not immobilise"
            );
        }
    }

    /// A system may legitimately declare no movement at all.
    #[test]
    fn a_system_that_measures_no_movement_resolves_none() {
        let speeds = speeds_from(&serde_json::json!({ "speed_walk": 30 }), &[]);
        assert!(
            speeds.is_empty(),
            "Blades has no measured movement; inventing one would be this crate deciding its rules"
        );
    }

    /// Sheets nest.
    #[test]
    fn a_speed_stored_beside_other_facts_is_still_read() {
        let declarations = vec![declare("walk", "speed_walk", None, 0)];
        let stored = serde_json::json!({ "speed_walk": { "value": 35, "source": "boots" } });
        assert_eq!(speeds_from(&stored, &declarations).get("walk"), Some(35.0));
    }
}
