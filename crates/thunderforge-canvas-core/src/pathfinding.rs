//! Spec 097: the way round.
//!
//! The engine could always say whether a move crosses a wall
//! ([`movement_blocked_by`]); this finds a move that does not. It asks that
//! same question of every step it takes, so a path planned here is never one
//! the engine or the server then refuses.
//!
//! A* over a square grid's cells, eight ways, then pulled taut: every
//! waypoint whose neighbours see each other is dropped, so a token walking
//! the answer crosses open floor in a straight line instead of a staircase
//! of cell centres. Square grids only; hex is a follow-up.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use glam::Vec2;

use crate::grid::{Cell, GridKind, GridSpec};
use crate::wall::{WallSet, movement_blocked_by};

/// What bounds a search.
#[derive(Clone, Debug)]
pub struct PathOptions {
    /// The rectangle of cells a path may use, both corners inclusive. `None`
    /// leaves only the budget to stop a search for an unreachable goal.
    pub bounds: Option<(Cell, Cell)>,
    /// Cells no step may enter: other tokens, a map to walk round. The goal's
    /// cell never counts, so a host can ask to stand beside a token.
    pub blocked: HashSet<Cell>,
    /// The most cells the search expands before it gives up.
    pub max_nodes: usize,
}

impl Default for PathOptions {
    fn default() -> Self {
        Self {
            bounds: None,
            blocked: HashSet::new(),
            max_nodes: 20_000,
        }
    }
}

/// How far, in cells, a pulled-taut leg keeps from a wall it passes. Without
/// it the hero's art brushes every corner it rounds.
const CLEARANCE: f32 = 0.3;

/// The eight steps, orthogonal first: the order breaks ties, so the same
/// question always gets the same path.
const STEPS: [(i32, i32); 8] = [
    (0, -1),
    (1, 0),
    (0, 1),
    (-1, 0),
    (1, -1),
    (1, 1),
    (-1, 1),
    (-1, -1),
];

/// A cell waiting to be expanded, smallest estimate first.
#[derive(PartialEq)]
struct Open {
    f: f32,
    h: f32,
    cell: Cell,
}

impl Eq for Open {}

impl Ord for Open {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reversed: `BinaryHeap` pops its largest.
        other
            .f
            .total_cmp(&self.f)
            .then_with(|| other.h.total_cmp(&self.h))
            .then_with(|| (other.cell.r, other.cell.q).cmp(&(self.cell.r, self.cell.q)))
    }
}

impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Octile distance between two cells, in cells.
fn octile(a: Cell, b: Cell) -> f32 {
    let dx = (a.q - b.q).unsigned_abs() as f32;
    let dy = (a.r - b.r).unsigned_abs() as f32;
    dx.max(dy) + (std::f32::consts::SQRT_2 - 1.0) * dx.min(dy)
}

/// The waypoints from `from` to `to`, round every wall and closed door in
/// `walls`, or `None` when there is no way within the bounds and budget.
pub fn find_path(
    grid: &GridSpec,
    from: Vec2,
    to: Vec2,
    walls: &WallSet,
    options: &PathOptions,
) -> Option<Vec<Vec2>> {
    if grid.kind != GridKind::Square {
        return None;
    }
    let start = grid.world_to_cell(from);
    let goal = grid.world_to_cell(to);
    if start == goal && movement_blocked_by(from, to, walls).is_none() {
        return Some(vec![from, to]);
    }

    let inside = |c: Cell| {
        options
            .bounds
            .is_none_or(|(lo, hi)| c.q >= lo.q && c.q <= hi.q && c.r >= lo.r && c.r <= hi.r)
    };
    let free = |c: Cell| c == goal || (inside(c) && !options.blocked.contains(&c));
    let clear = |a: Cell, b: Cell| {
        movement_blocked_by(grid.cell_center(a), grid.cell_center(b), walls).is_none()
    };

    if movement_blocked_by(from, grid.cell_center(start), walls).is_some()
        || movement_blocked_by(grid.cell_center(goal), to, walls).is_some()
    {
        return None;
    }

    let mut open = BinaryHeap::new();
    let mut cost: HashMap<Cell, f32> = HashMap::new();
    let mut came: HashMap<Cell, Cell> = HashMap::new();
    let mut done: HashSet<Cell> = HashSet::new();
    cost.insert(start, 0.0);
    open.push(Open {
        f: octile(start, goal),
        h: octile(start, goal),
        cell: start,
    });

    let mut found = false;
    while let Some(Open { cell, .. }) = open.pop() {
        if !done.insert(cell) {
            continue;
        }
        if cell == goal {
            found = true;
            break;
        }
        if done.len() > options.max_nodes {
            return None;
        }
        let here = cost[&cell];
        for (dq, dr) in STEPS {
            let next = Cell::new(cell.q + dq, cell.r + dr);
            if done.contains(&next) || !free(next) || !clear(cell, next) {
                continue;
            }
            let diagonal = dq != 0 && dr != 0;
            if diagonal {
                // FR-003: both cells beside a diagonal step must be open, and
                // the way through each of them clear, or the step squeezes
                // between two walls that meet at a corner.
                let side_a = Cell::new(cell.q + dq, cell.r);
                let side_b = Cell::new(cell.q, cell.r + dr);
                let around = |side: Cell| {
                    inside(side)
                        && !options.blocked.contains(&side)
                        && clear(cell, side)
                        && clear(side, next)
                };
                if !around(side_a) || !around(side_b) {
                    continue;
                }
            }
            let step = if diagonal {
                std::f32::consts::SQRT_2
            } else {
                1.0
            };
            let g = here + step;
            if cost.get(&next).is_some_and(|&known| known <= g) {
                continue;
            }
            cost.insert(next, g);
            came.insert(next, cell);
            let h = octile(next, goal);
            open.push(Open {
                f: g + h,
                h,
                cell: next,
            });
        }
    }
    if !found {
        return None;
    }

    let mut cells = vec![goal];
    while let Some(&prev) = came.get(cells.last().unwrap()) {
        cells.push(prev);
    }
    cells.reverse();
    let mut raw = Vec::with_capacity(cells.len() + 2);
    raw.push(from);
    raw.extend(cells.iter().map(|&c| grid.cell_center(c)));
    raw.push(to);
    raw.dedup();
    Some(pull_taut(grid, &raw, walls, &options.blocked, goal))
}

/// Drops every waypoint its neighbours can do without (FR-007): from each kept
/// point, on to the farthest later one it sees.
fn pull_taut(
    grid: &GridSpec,
    raw: &[Vec2],
    walls: &WallSet,
    blocked: &HashSet<Cell>,
    goal: Cell,
) -> Vec<Vec2> {
    let mut kept = vec![raw[0]];
    let mut i = 0;
    while i < raw.len() - 1 {
        let mut j = raw.len() - 1;
        while j > i + 1 && !sees(grid, raw[i], raw[j], walls, blocked, goal) {
            j -= 1;
        }
        kept.push(raw[j]);
        i = j;
    }
    kept
}

/// Whether a straight leg from `a` to `b` is walkable: no wall across it or
/// across either of its edges a little to each side, and no blocked cell
/// under it.
fn sees(
    grid: &GridSpec,
    a: Vec2,
    b: Vec2,
    walls: &WallSet,
    blocked: &HashSet<Cell>,
    goal: Cell,
) -> bool {
    let along = b - a;
    let length = along.length();
    if length == 0.0 {
        return true;
    }
    let side = along.perp() / length * CLEARANCE * grid.safe_size();
    let lanes = [Vec2::ZERO, side, -side];
    if lanes
        .iter()
        .any(|&off| movement_blocked_by(a + off, b + off, walls).is_some())
    {
        return false;
    }
    if blocked.is_empty() {
        return true;
    }
    let samples = (length / (grid.safe_size() * 0.25)).ceil() as usize;
    (0..=samples).all(|k| {
        let p = a.lerp(b, k as f32 / samples as f32);
        lanes.iter().all(|&off| {
            let cell = grid.world_to_cell(p + off);
            cell == goal || !blocked.contains(&cell)
        })
    })
}

#[cfg(test)]
#[path = "pathfinding_tests.rs"]
mod tests;
