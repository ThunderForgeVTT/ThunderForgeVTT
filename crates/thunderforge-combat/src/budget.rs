//! What a turn affords and what it has spent (spec 046 C9).
//!
//! **Never refused, never capped**: a spend past an allowance is a debt, shown
//! and flagged. The server's `combat::budget` keeps one row per combatant and
//! calls these between its read and its write; the demo keeps a [`Spent`] in
//! memory and calls the same ones.

use thunderforge_canvas_core::Vec2;
use thunderforge_canvas_core::grid::{Footprint, GridKind, GridSpec};
use thunderforge_canvas_core::measure::GridUnits;
use thunderforge_canvas_core::movement_budget::{MovementBudget, TerrainCost, cost_path};

use crate::attack::ActionCost;
use crate::manifest::SystemTurnBudget;
use crate::records::{FLAG_LEGENDARY_ON_OWN_TURN, FLAG_OVERSPENT};

/// What one combatant has spent this turn: the server's budget row, without
/// its keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spent {
    pub action_spent: i32,
    pub bonus_action_spent: i32,
    pub reaction_spent: i32,
    pub movement_spent: f64,
    pub legendary_per_round: Option<i32>,
    pub legendary_remaining: Option<i32>,
}

impl Spent {
    /// Record a spend, as the server's `UPDATE` does: one more of a kind,
    /// movement added (never less than nothing), and a legendary cost taken
    /// from the pool — a creature with no pool keeps none.
    pub fn take(&mut self, what: Spend) {
        match what {
            Spend::Action => self.action_spent += 1,
            Spend::BonusAction => self.bonus_action_spent += 1,
            Spend::Reaction => self.reaction_spent += 1,
            Spend::Movement(distance) => self.movement_spent += distance.max(0.0),
            Spend::Legendary(cost) => {
                self.legendary_remaining = self.legendary_remaining.map(|r| r - cost)
            }
        }
    }

    /// The start of this combatant's turn: everything comes back, and the
    /// legendary pool refills to what it has each round.
    pub fn start_turn(&mut self) {
        self.action_spent = 0;
        self.bonus_action_spent = 0;
        self.reaction_spent = 0;
        self.movement_spent = 0.0;
        self.legendary_remaining = self.legendary_per_round;
    }
}

/// One line of a turn's budget. `remaining` may be negative: that is a debt,
/// and it is shown (C9).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "graphql", derive(async_graphql::SimpleObject))]
pub struct BudgetLine {
    pub allowed: f64,
    pub spent: f64,
    pub remaining: f64,
}

impl BudgetLine {
    pub fn new(allowed: f64, spent: f64) -> Self {
        // Two decimals: 29.999999 ft from float arithmetic is 30 ft.
        let tidy = |v: f64| (v * 100.0).round() / 100.0;
        BudgetLine {
            allowed: tidy(allowed),
            spent: tidy(spent),
            remaining: tidy(allowed - spent),
        }
    }

    pub fn is_overspent(&self) -> bool {
        self.spent > self.allowed
    }
}

/// What a combatant's turn affords and what it has spent (contract §1).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "graphql", derive(async_graphql::SimpleObject))]
pub struct TurnBudget {
    pub action: BudgetLine,
    pub bonus_action: BudgetLine,
    pub reaction: BudgetLine,
    /// In the system's units.
    pub movement: BudgetLine,
    /// Null when the creature has no legendary actions (Phase 9).
    pub legendary: Option<BudgetLine>,
    /// What the system calls its distances ("ft"), so movement can be said.
    pub unit: String,
}

/// What has been spent, resolved against the pack's allowances and the
/// creature's speed.
pub fn resolve(declared: &SystemTurnBudget, speed: f64, spent: &Spent, unit: &str) -> TurnBudget {
    let whole = |n: Option<u32>| n.unwrap_or(0) as f64;
    let movement_allowed = if declared.movement.is_some() {
        speed
    } else {
        0.0
    };
    TurnBudget {
        action: BudgetLine::new(whole(declared.action), spent.action_spent as f64),
        bonus_action: BudgetLine::new(
            whole(declared.bonus_action),
            spent.bonus_action_spent as f64,
        ),
        reaction: BudgetLine::new(whole(declared.reaction), spent.reaction_spent as f64),
        movement: BudgetLine::new(movement_allowed, spent.movement_spent),
        legendary: match (spent.legendary_per_round, spent.legendary_remaining) {
            (Some(per_round), Some(remaining)) => Some(BudgetLine::new(
                per_round as f64,
                (per_round - remaining) as f64,
            )),
            _ => None,
        },
        unit: unit.to_string(),
    }
}

/// What one spend takes.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Spend {
    Action,
    BonusAction,
    Reaction,
    /// In the system's units.
    Movement(f64),
    /// From the legendary pool: the ability's `legendary_cost`.
    Legendary(i32),
}

impl Spend {
    /// The line an attack of this cost spends; `None` for free. A legendary
    /// action spends `legendary_cost` from the pool.
    pub fn for_attack(cost: ActionCost, legendary_cost: i32) -> Option<Spend> {
        match cost {
            ActionCost::Action => Some(Spend::Action),
            ActionCost::BonusAction => Some(Spend::BonusAction),
            ActionCost::Reaction => Some(Spend::Reaction),
            ActionCost::Legendary => Some(Spend::Legendary(legendary_cost.max(0))),
            ActionCost::Free => None,
        }
    }

    pub fn is_legendary(self) -> bool {
        matches!(self, Spend::Legendary(_))
    }
}

/// The line a spend is counted against. `None` only for a legendary action
/// by a creature with no legendary actions: past an allowance of nothing, so
/// flagged overspent, and there is no pool to take it from.
pub fn line_of(budget: &TurnBudget, what: Spend) -> Option<BudgetLine> {
    match what {
        Spend::Action => Some(budget.action),
        Spend::BonusAction => Some(budget.bonus_action),
        Spend::Reaction => Some(budget.reaction),
        Spend::Movement(_) => Some(budget.movement),
        Spend::Legendary(_) => budget.legendary,
    }
}

/// The flags an attack's spend puts on every part, judged on the budget
/// **after** the spend: `overspent` when its line is past its allowance, and
/// `legendary_on_own_turn` for a legendary action on the creature's own turn.
pub fn flags_after_spend(budget: &TurnBudget, what: Spend, own_turn: bool) -> Vec<&'static str> {
    let mut flags = Vec::new();
    if line_of(budget, what).is_none_or(|line| line.is_overspent()) {
        flags.push(FLAG_OVERSPENT);
    }
    if what.is_legendary() && own_turn {
        flags.push(FLAG_LEGENDARY_ON_OWN_TURN);
    }
    flags
}

/// The flags an attack would be given, judged on the budget **before** it:
/// `overspent` when one more would go past. For the warning before a roll.
pub fn flags_before_spend(budget: &TurnBudget, what: Spend, own_turn: bool) -> Vec<&'static str> {
    let needed = match what {
        Spend::Legendary(cost) => cost as f64,
        _ => 1.0,
    };
    let mut flags = Vec::new();
    if line_of(budget, what).is_none_or(|line| line.remaining < needed) {
        flags.push(FLAG_OVERSPENT);
    }
    if what.is_legendary() && own_turn {
        flags.push(FLAG_LEGENDARY_ON_OWN_TURN);
    }
    flags
}

/// How many cells a token of `footprint` moves by going from `from` to `to`
/// in one step: on a square grid, cells between the corners of the squares
/// it covers, a diagonal counting as one; on a hex grid, cell distance; on a
/// gridless scene, its length in cells. The server's `combat::budget` module
/// documentation gives the reasoning.
pub fn step_cells(grid: &GridSpec, footprint: Footprint, from: Vec2, to: Vec2) -> f32 {
    match grid.kind {
        GridKind::Square => {
            let (a, b) = (
                grid.covered_cells(from, footprint).min,
                grid.covered_cells(to, footprint).min,
            );
            (a.q - b.q).abs().max((a.r - b.r).abs()) as f32
        }
        GridKind::HexPointyTop | GridKind::HexFlatTop => {
            grid.cell_distance(grid.world_to_cell(from), grid.world_to_cell(to)) as f32
        }
        GridKind::Gridless => {
            let size = if grid.size.is_finite() && grid.size > f32::EPSILON {
                grid.size
            } else {
                GridSpec::default().size
            };
            from.distance(to) / size
        }
    }
}

/// What a move costs, in the system's units: the token's position, the
/// route's points, then the destination, one step between each pair.
pub fn move_cost(
    grid: &GridSpec,
    units: &GridUnits,
    footprint: Footprint,
    speed_kind: &str,
    from: Vec2,
    route: &[Vec2],
    to: Vec2,
) -> f64 {
    let mut points = Vec::with_capacity(route.len() + 2);
    points.push(from);
    points.extend_from_slice(route);
    points.push(to);
    points.dedup();

    let mut steps: Vec<TerrainCost> = Vec::new();
    for pair in points.windows(2) {
        let cells = step_cells(grid, footprint, pair[0], pair[1]);
        if cells <= 0.0 {
            continue;
        }
        if grid.kind == GridKind::Gridless {
            // No cells to enter: a step is its own length.
            steps.push(TerrainCost {
                multiplier: cells,
                ignored_by: Vec::new(),
            });
        } else {
            steps.extend(std::iter::repeat_n(
                TerrainCost::default(),
                cells.round() as usize,
            ));
        }
    }
    let cost = cost_path(&steps, speed_kind, units, &MovementBudget::new(0.0));
    (cost.distance as f64 * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declared() -> SystemTurnBudget {
        serde_json::from_value(serde_json::json!({
            "action": 1, "bonusAction": 1, "reaction": 1,
            "movement": { "speed": "walk" }
        }))
        .unwrap()
    }

    #[test]
    fn a_second_action_is_a_debt_and_flagged() {
        let mut spent = Spent::default();
        spent.take(Spend::Action);
        let budget = resolve(&declared(), 30.0, &spent, "ft");
        assert!(flags_after_spend(&budget, Spend::Action, true).is_empty());
        assert_eq!(
            flags_before_spend(&budget, Spend::Action, true),
            vec![FLAG_OVERSPENT]
        );
        spent.take(Spend::Action);
        let budget = resolve(&declared(), 30.0, &spent, "ft");
        assert_eq!(budget.action.remaining, -1.0);
        assert_eq!(
            flags_after_spend(&budget, Spend::Action, true),
            vec![FLAG_OVERSPENT]
        );
    }

    #[test]
    fn a_turn_brings_everything_back_and_refills_the_pool() {
        let mut spent = Spent {
            legendary_per_round: Some(3),
            legendary_remaining: Some(3),
            ..Spent::default()
        };
        spent.take(Spend::Legendary(2));
        spent.take(Spend::Movement(15.0));
        assert_eq!(spent.legendary_remaining, Some(1));
        spent.start_turn();
        assert_eq!(spent.legendary_remaining, Some(3));
        assert_eq!(spent.movement_spent, 0.0);
    }

    #[test]
    fn a_legendary_action_with_no_pool_is_overspent() {
        let budget = resolve(&declared(), 30.0, &Spent::default(), "ft");
        assert_eq!(
            flags_after_spend(&budget, Spend::Legendary(1), true),
            vec![FLAG_OVERSPENT, FLAG_LEGENDARY_ON_OWN_TURN]
        );
    }
}
