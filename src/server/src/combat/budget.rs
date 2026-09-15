//! What a creature has spent of its turn (spec 046 US5, FR-040–FR-045,
//! research R13, contract C9).
//!
//! A turn affords what the pack's `turnStructure.budget` says (5e: an action,
//! a bonus action, a reaction, and movement up to its `walk` speed). This is
//! where each is spent and where it comes back.
//!
//! # Shown, never refused (decision 2, C9)
//!
//! Nothing here returns a refusal. An attack made with the action already
//! gone is recorded, `spent` goes past `allowed`, `remaining` goes negative,
//! and the attack is flagged `overspent`. A table that lets somebody overspend
//! and a Game Master who calls it a debt are the game being played.
//!
//! # When a budget comes back
//!
//! At the start of its owner's turn, and only then: `advanceTurn` (and a
//! removal that hands the turn on) resets the **new** active combatant's
//! action, bonus action, reaction and movement. Nobody else's budget changes,
//! so a reaction spent on somebody else's turn stays spent until its owner's
//! own turn begins (FR-041).
//!
//! # What spends
//!
//! - **An attack** spends one of the line its `action_cost` names. A
//!   multiattack is one attack for this purpose: one action, however many
//!   parts (FR-044). `free` spends nothing; `legendary` is Phase 9's pool.
//! - **A move** spends its cost in the system's units, for a token that is a
//!   combatant in a running combat in its scene, whoever moved it — a Game
//!   Master dragging a creature moves that creature, and it is shown against
//!   the creature. Every path that commits a move spends: `moveOwnToken`,
//!   `updateToken` and an offline move replayed by `reconcileQueuedChanges`.
//!
//! # What a move costs
//!
//! Counted from the steps the move actually takes, in cells, times the
//! system's `unitsPerCell` (research R13):
//!
//! - **A route** (`moveOwnToken`'s `path`): the token's position, then each
//!   point of the route, then the destination, and each step between two of
//!   them costs the cells the token's **footprint** moves by — on squares the
//!   Chebyshev shift of the block it covers (5-5-5, as reach counts), on hexes
//!   the axial distance, on a gridless scene the straight-line length in
//!   cells. A Large token's route runs vertex to vertex, and its block moves
//!   one square per square stepped, exactly as a Medium one's does.
//! - **A drag, or anything else with no route** (`updateToken`, an offline
//!   move): one step, from where the token was to where it is put — the
//!   footprint's displacement, not the distance between the two footprints
//!   (which is zero when they overlap).
//!
//! The steps go through `movement_budget::cost_path` one open-ground cell at a
//! time, so difficult terrain has one place to arrive when a scene can say
//! where it is. On a gridless scene a step is its length in cells.
//!
//! # Every seat sees it
//!
//! A budget is numbers and nothing else: no label, no token, no actor. It
//! rides the combatant it belongs to, so a combatant a player reads as
//! "Unknown" shows its budget under "Unknown" and gives nothing else away.
//! Budgets travel with the combat refetch (event 18), never a per-token fan-out.

use std::collections::HashMap;

use diesel::PgConnection;
use diesel::prelude::*;
use thunderforge_canvas_core::Vec2;
use thunderforge_canvas_core::grid::{Footprint, GridKind, GridSpec};
use thunderforge_canvas_core::measure::GridUnits;
use thunderforge_canvas_core::movement_budget::{
    MovementBudget, TerrainCost, cost_path, speeds_from,
};
use uuid::Uuid;

use crate::combat::attack::ActionCost;
use crate::combat::manifest::{SystemTurnBudget, turn_budget_for_system};
use crate::models::{ActorSystemData, Combatant};
use crate::schema::{
    scenes, tokens, world_actor_system_data, world_combatant_budgets, world_combatants, worlds,
};

/// One combatant's row: what it has spent this turn.
#[derive(Queryable, Selectable, Insertable, Debug, Clone, PartialEq)]
#[diesel(table_name = world_combatant_budgets)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct BudgetRow {
    pub combatant_id: Uuid,
    pub action_spent: i32,
    pub bonus_action_spent: i32,
    pub reaction_spent: i32,
    pub movement_spent: f64,
    pub legendary_per_round: Option<i32>,
    pub legendary_remaining: Option<i32>,
}

impl BudgetRow {
    /// Nothing spent: a combatant whose row is not there yet reads as this.
    pub fn fresh(combatant_id: Uuid) -> Self {
        BudgetRow {
            combatant_id,
            action_spent: 0,
            bonus_action_spent: 0,
            reaction_spent: 0,
            movement_spent: 0.0,
            legendary_per_round: None,
            legendary_remaining: None,
        }
    }
}

/// One line of a turn's budget. `remaining` may be negative: that is a debt,
/// and it is shown (C9).
#[derive(async_graphql::SimpleObject, Clone, Copy, Debug, PartialEq)]
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
#[derive(async_graphql::SimpleObject, Clone, Debug, PartialEq)]
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

/// A row resolved against the pack's allowances and the creature's speed
/// (pure).
pub fn resolve(declared: &SystemTurnBudget, speed: f64, row: &BudgetRow, unit: &str) -> TurnBudget {
    let whole = |n: Option<u32>| n.unwrap_or(0) as f64;
    let movement_allowed = if declared.movement.is_some() {
        speed
    } else {
        0.0
    };
    TurnBudget {
        action: BudgetLine::new(whole(declared.action), row.action_spent as f64),
        bonus_action: BudgetLine::new(whole(declared.bonus_action), row.bonus_action_spent as f64),
        reaction: BudgetLine::new(whole(declared.reaction), row.reaction_spent as f64),
        movement: BudgetLine::new(movement_allowed, row.movement_spent),
        legendary: match (row.legendary_per_round, row.legendary_remaining) {
            (Some(per_round), Some(remaining)) => Some(BudgetLine::new(
                per_round as f64,
                (per_round - remaining) as f64,
            )),
            _ => None,
        },
        unit: unit.to_string(),
    }
}

/// A budget for a new combatant, nothing spent. A second call is harmless.
pub fn create_for(conn: &mut PgConnection, combatant_id: Uuid, user_id: Uuid) -> QueryResult<()> {
    diesel::insert_into(world_combatant_budgets::table)
        .values((
            world_combatant_budgets::combatant_id.eq(combatant_id),
            world_combatant_budgets::created_by.eq(user_id),
            world_combatant_budgets::updated_by.eq(user_id),
        ))
        .on_conflict(world_combatant_budgets::combatant_id)
        .do_nothing()
        .execute(conn)?;
    Ok(())
}

/// What one spend takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spend {
    Action,
    BonusAction,
    Reaction,
    /// In the system's units.
    Movement(f64),
}

impl Spend {
    /// The line an attack of this cost spends; `None` for free, and for
    /// legendary (its own pool, Phase 9).
    pub fn for_attack(cost: ActionCost) -> Option<Spend> {
        match cost {
            ActionCost::Action => Some(Spend::Action),
            ActionCost::BonusAction => Some(Spend::BonusAction),
            ActionCost::Reaction => Some(Spend::Reaction),
            ActionCost::Legendary | ActionCost::Free => None,
        }
    }
}

/// Record a spend. Never refused and never capped (C9): the row is created if
/// it is missing, and the line goes as far past its allowance as it is taken.
pub fn spend(
    conn: &mut PgConnection,
    combatant_id: Uuid,
    what: Spend,
    user_id: Uuid,
) -> QueryResult<BudgetRow> {
    create_for(conn, combatant_id, user_id)?;
    let row = world_combatant_budgets::table
        .filter(world_combatant_budgets::combatant_id.eq(combatant_id));
    let now = chrono::Utc::now().naive_utc();
    let stamp = (
        world_combatant_budgets::updated_by.eq(user_id),
        world_combatant_budgets::updated_at.eq(now),
    );
    let query = diesel::update(row);
    match what {
        Spend::Action => query
            .set((
                world_combatant_budgets::action_spent.eq(world_combatant_budgets::action_spent + 1),
                stamp,
            ))
            .returning(BudgetRow::as_returning())
            .get_result(conn),
        Spend::BonusAction => query
            .set((
                world_combatant_budgets::bonus_action_spent
                    .eq(world_combatant_budgets::bonus_action_spent + 1),
                stamp,
            ))
            .returning(BudgetRow::as_returning())
            .get_result(conn),
        Spend::Reaction => query
            .set((
                world_combatant_budgets::reaction_spent
                    .eq(world_combatant_budgets::reaction_spent + 1),
                stamp,
            ))
            .returning(BudgetRow::as_returning())
            .get_result(conn),
        Spend::Movement(distance) => query
            .set((
                world_combatant_budgets::movement_spent
                    .eq(world_combatant_budgets::movement_spent + distance.max(0.0)),
                stamp,
            ))
            .returning(BudgetRow::as_returning())
            .get_result(conn),
    }
}

/// The start of `combatant_id`'s turn: its action, bonus action, reaction and
/// movement come back. Nobody else's budget is touched.
pub fn start_turn(conn: &mut PgConnection, combatant_id: Uuid, user_id: Uuid) -> QueryResult<()> {
    create_for(conn, combatant_id, user_id)?;
    diesel::update(
        world_combatant_budgets::table
            .filter(world_combatant_budgets::combatant_id.eq(combatant_id)),
    )
    .set((
        world_combatant_budgets::action_spent.eq(0),
        world_combatant_budgets::bonus_action_spent.eq(0),
        world_combatant_budgets::reaction_spent.eq(0),
        world_combatant_budgets::movement_spent.eq(0.0),
        world_combatant_budgets::updated_by.eq(user_id),
        world_combatant_budgets::updated_at.eq(chrono::Utc::now().naive_utc()),
    ))
    .execute(conn)?;
    Ok(())
}

/// The world's system, if it has one.
fn world_system(conn: &mut PgConnection, world_id: Uuid) -> QueryResult<Option<String>> {
    Ok(worlds::table
        .filter(worlds::id.eq(world_id))
        .select(worlds::game_system_id)
        .first::<Option<String>>(conn)
        .optional()?
        .flatten())
}

/// Each combatant's speed for the budget's movement type, in system units:
/// read from its creature's sheet — the token's actor (a copy reads its NPC's),
/// else the combatant's own actor — through the pack's `movement` block, whose
/// declared default stands in for a sheet that says nothing.
fn speeds_for(
    conn: &mut PgConnection,
    systems_dir: &str,
    system_id: &str,
    speed_kind: &str,
    combatants: &[Combatant],
) -> QueryResult<HashMap<Uuid, f64>> {
    let declarations = crate::attributes::movement_declarations_for_system(systems_dir, system_id);
    let token_ids: Vec<Uuid> = combatants.iter().filter_map(|c| c.token_id).collect();
    let token_actors: HashMap<Uuid, Option<Uuid>> = if token_ids.is_empty() {
        HashMap::new()
    } else {
        tokens::table
            .filter(tokens::token_id.eq_any(&token_ids))
            .select((tokens::token_id, tokens::actor_id))
            .load::<(Uuid, Option<Uuid>)>(conn)?
            .into_iter()
            .collect()
    };
    let actor_of = |c: &Combatant| match c.token_id {
        Some(token) => token_actors.get(&token).copied().flatten().or(c.actor_id),
        None => c.actor_id,
    };
    let actor_ids: Vec<Uuid> = combatants.iter().filter_map(actor_of).collect();
    let sheets: Vec<ActorSystemData> = if actor_ids.is_empty() {
        Vec::new()
    } else {
        world_actor_system_data::table
            .filter(world_actor_system_data::actor_id.eq_any(&actor_ids))
            .select(ActorSystemData::as_select())
            .load(conn)?
    };
    let empty = serde_json::json!({});
    Ok(combatants
        .iter()
        .map(|c| {
            let sheet = actor_of(c).and_then(|actor| {
                sheets
                    .iter()
                    .filter(|row| row.actor_id == actor)
                    .max_by_key(|row| row.game_system_id == system_id)
            });
            let ability_data = sheet
                .and_then(|row| row.ability_data.as_ref())
                .unwrap_or(&empty);
            let speed = speeds_from(ability_data, &declarations)
                .get(speed_kind)
                .unwrap_or(0.0);
            (c.id, speed as f64)
        })
        .collect())
}

/// Every combatant's budget, resolved, for the tracker. Empty when the
/// world's system declares no budget: a ruleset without a turn economy is not
/// given one (FR-031's rule for rounds, again).
pub fn budgets_for(
    conn: &mut PgConnection,
    systems_dir: &str,
    world_id: Uuid,
    combatants: &[Combatant],
) -> QueryResult<HashMap<Uuid, TurnBudget>> {
    let Some(system_id) = world_system(conn, world_id)? else {
        return Ok(HashMap::new());
    };
    let Some(declared) = turn_budget_for_system(systems_dir, &system_id) else {
        return Ok(HashMap::new());
    };
    if combatants.is_empty() {
        return Ok(HashMap::new());
    }
    let ids: Vec<Uuid> = combatants.iter().map(|c| c.id).collect();
    let rows: HashMap<Uuid, BudgetRow> = world_combatant_budgets::table
        .filter(world_combatant_budgets::combatant_id.eq_any(&ids))
        .select(BudgetRow::as_select())
        .load::<BudgetRow>(conn)?
        .into_iter()
        .map(|row| (row.combatant_id, row))
        .collect();
    let unit = crate::vision_profiles::vision_declaration_for_system(systems_dir, &system_id)
        .grid_units()
        .label;
    let speeds = match declared.movement.as_ref() {
        Some(movement) => speeds_for(conn, systems_dir, &system_id, &movement.speed, combatants)?,
        None => HashMap::new(),
    };
    Ok(combatants
        .iter()
        .map(|c| {
            let row = rows
                .get(&c.id)
                .cloned()
                .unwrap_or_else(|| BudgetRow::fresh(c.id));
            let speed = speeds.get(&c.id).copied().unwrap_or(0.0);
            (c.id, resolve(&declared, speed, &row, &unit))
        })
        .collect())
}

/// The combatant a token acts as in the running combat in its scene: by
/// token, else a token-less combatant for the token's actor (as the turn check
/// finds it). `None` when no combat is running there, or the token is not in
/// it.
pub fn combatant_of_token(
    conn: &mut PgConnection,
    scene_id: Uuid,
    token_id: Uuid,
) -> QueryResult<Option<(Uuid, Combatant)>> {
    let Some(world_id) = scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .select(scenes::world_id)
        .first::<Uuid>(conn)
        .optional()?
    else {
        return Ok(None);
    };
    let Some((combat_id, _)) = crate::combat::turn::running_combat(conn, world_id, scene_id)?
    else {
        return Ok(None);
    };
    let actor_id = tokens::table
        .filter(tokens::token_id.eq(token_id))
        .select(tokens::actor_id)
        .first::<Option<Uuid>>(conn)
        .optional()?
        .flatten();
    let combatants = world_combatants::table
        .filter(world_combatants::combat_id.eq(combat_id))
        .select(Combatant::as_select())
        .load::<Combatant>(conn)?;
    let by_token = combatants.iter().find(|c| c.token_id == Some(token_id));
    let by_actor = || {
        combatants
            .iter()
            .find(|c| c.token_id.is_none() && actor_id.is_some() && c.actor_id == actor_id)
    };
    Ok(by_token.or_else(by_actor).cloned().map(|c| (world_id, c)))
}

/// One combatant's budget, resolved; `None` when its system declares none.
fn budget_of(
    conn: &mut PgConnection,
    systems_dir: &str,
    world_id: Uuid,
    combatant: &Combatant,
) -> QueryResult<Option<TurnBudget>> {
    Ok(
        budgets_for(conn, systems_dir, world_id, std::slice::from_ref(combatant))?
            .remove(&combatant.id),
    )
}

/// What an attack spends, spent against the attacker's combatant in the
/// running combat in its scene. Returns whether the line it spent is now
/// overspent — the attack's `overspent` flag. Never refuses (C9).
///
/// Called once per `makeAttack`, however many parts a multiattack makes: one
/// action (FR-044).
pub fn spend_for_attack(
    conn: &mut PgConnection,
    systems_dir: &str,
    scene_id: Uuid,
    attacker_token_id: Uuid,
    cost: ActionCost,
    user_id: Uuid,
) -> QueryResult<bool> {
    let Some(what) = Spend::for_attack(cost) else {
        return Ok(false);
    };
    let Some((world_id, combatant)) = combatant_of_token(conn, scene_id, attacker_token_id)? else {
        return Ok(false);
    };
    spend(conn, combatant.id, what, user_id)?;
    touch(conn, world_id, combatant.combat_id, user_id)?;
    Ok(budget_of(conn, systems_dir, world_id, &combatant)?
        .is_some_and(|budget| line_of(&budget, what).is_overspent()))
}

/// Whether an attack of this cost would overspend, for the warning before it
/// is rolled. Writes nothing.
pub fn attack_would_overspend(
    conn: &mut PgConnection,
    systems_dir: &str,
    scene_id: Uuid,
    attacker_token_id: Uuid,
    cost: ActionCost,
) -> QueryResult<bool> {
    let Some(what) = Spend::for_attack(cost) else {
        return Ok(false);
    };
    let Some((world_id, combatant)) = combatant_of_token(conn, scene_id, attacker_token_id)? else {
        return Ok(false);
    };
    Ok(budget_of(conn, systems_dir, world_id, &combatant)?
        .is_some_and(|budget| line_of(&budget, what).remaining < 1.0))
}

fn line_of(budget: &TurnBudget, what: Spend) -> BudgetLine {
    match what {
        Spend::Action => budget.action,
        Spend::BonusAction => budget.bonus_action,
        Spend::Reaction => budget.reaction,
        Spend::Movement(_) => budget.movement,
    }
}

/// A budget changed: the tracker is re-read on every seat (event 18).
fn touch(
    conn: &mut PgConnection,
    world_id: Uuid,
    combat_id: Uuid,
    user_id: Uuid,
) -> QueryResult<()> {
    crate::graphql::mutations_combat::touch_and_broadcast(conn, combat_id, world_id, user_id)
        .map_err(|message| diesel::result::Error::QueryBuilderError(message.into()))
}

/// How many cells a token of `footprint` moves by going from `from` to `to`
/// in one step (pure). See the module documentation.
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

/// What a move costs, in the system's units (pure): the token's position, the
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

/// A committed move spends its cost against the token's combatant, when the
/// token is a combatant in a running combat in its scene. Whoever moved it.
/// Returns what was spent. Never refuses (C9).
#[allow(clippy::too_many_arguments)]
pub fn spend_for_move(
    conn: &mut PgConnection,
    systems_dir: &str,
    scene_id: Uuid,
    token_id: Uuid,
    from: (f64, f64),
    route: Option<&[(f64, f64)]>,
    to: (f64, f64),
    user_id: Uuid,
) -> QueryResult<Option<f64>> {
    if from == to && route.is_none_or(|r| r.is_empty()) {
        return Ok(None);
    }
    let Some((world_id, combatant)) = combatant_of_token(conn, scene_id, token_id)? else {
        return Ok(None);
    };
    let Some(system_id) = world_system(conn, world_id)? else {
        return Ok(None);
    };
    let Some(declared) = turn_budget_for_system(systems_dir, &system_id) else {
        return Ok(None);
    };
    let Some(movement) = declared.movement else {
        return Ok(None);
    };

    let (grid_type, grid_size, width, height) = scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .select((
            scenes::grid_type,
            scenes::grid_size,
            scenes::width,
            scenes::height,
        ))
        .first::<(String, i32, i32, i32)>(conn)?;
    let grid = crate::combat::reach::scene_grid(&grid_type, grid_size, width, height);
    let units =
        crate::vision_profiles::vision_declaration_for_system(systems_dir, &system_id).grid_units();
    let footprint = crate::combat::size::footprints_of(conn, systems_dir, scene_id, &[token_id])?
        .get(&token_id)
        .copied()
        .unwrap_or(crate::combat::size::DEFAULT_FOOTPRINT);

    let point = |(x, y): (f64, f64)| Vec2::new(x as f32, y as f32);
    let route: Vec<Vec2> = route
        .unwrap_or_default()
        .iter()
        .copied()
        .map(point)
        .collect();
    let distance = move_cost(
        &grid,
        &units,
        Footprint::new(footprint),
        &movement.speed,
        point(from),
        &route,
        point(to),
    );
    if distance <= 0.0 {
        return Ok(None);
    }
    spend(conn, combatant.id, Spend::Movement(distance), user_id)?;
    touch(conn, world_id, combatant.combat_id, user_id)?;
    Ok(Some(distance))
}

/// [`spend_for_move`] for a caller whose move has already been written: a
/// budget that cannot be recorded is logged, and the move stands. The
/// tracker is a record of the fight, never a reason a creature did not move.
#[allow(clippy::too_many_arguments)]
pub fn spend_for_move_logged(
    conn: &mut PgConnection,
    systems_dir: &str,
    scene_id: Uuid,
    token_id: Uuid,
    from: (f64, f64),
    route: Option<&[(f64, f64)]>,
    to: (f64, f64),
    user_id: Uuid,
) {
    // Its own savepoint, so a failure here cannot abort a caller's transaction.
    let spent = conn.transaction(|conn| {
        spend_for_move(
            conn,
            systems_dir,
            scene_id,
            token_id,
            from,
            route,
            to,
            user_id,
        )
    });
    if let Err(error) = spent {
        tracing::warn!(%token_id, %error, "a move's movement was not recorded");
    }
}

#[cfg(test)]
#[path = "budget_tests.rs"]
mod tests;
