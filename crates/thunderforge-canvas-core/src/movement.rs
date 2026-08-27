//! Planned movement: the path a token would take, before it takes it.
//!
//! Turn-based play is a two-step gesture — plan, then commit — because a move
//! costs a resource the player is budgeting. Showing the route and its cost
//! *before* it happens is the whole point; a token that simply teleports on
//! keypress gives the player nothing to reason about.
//!
//! The path is a list of cells, not a list of world positions. Cost, legality
//! and rendering all derive from it, and a cell list survives a change of zoom
//! or grid origin that a pixel list would not.

use crate::grid::{Cell, GridKind, GridSpec};

/// A compass step, in the four directions a keyboard offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    North,
    South,
    East,
    West,
}

impl Step {
    /// The neighbouring cell in this direction.
    ///
    /// Square grids move along their axes. Hex grids have no true north on a
    /// pointy-top layout, so vertical input resolves to the nearest of the two
    /// diagonals — which is what every hex game does with a four-way input
    /// device.
    pub fn apply(self, from: Cell, kind: GridKind) -> Cell {
        match kind {
            GridKind::Square | GridKind::Gridless => match self {
                Step::North => Cell::new(from.q, from.r + 1),
                Step::South => Cell::new(from.q, from.r - 1),
                Step::East => Cell::new(from.q + 1, from.r),
                Step::West => Cell::new(from.q - 1, from.r),
            },
            // Axial neighbours. North/south pick the axis that actually moves
            // a pointy-top hex up or down the board.
            GridKind::HexPointyTop => match self {
                Step::North => Cell::new(from.q, from.r + 1),
                Step::South => Cell::new(from.q, from.r - 1),
                Step::East => Cell::new(from.q + 1, from.r),
                Step::West => Cell::new(from.q - 1, from.r),
            },
            GridKind::HexFlatTop => match self {
                Step::North => Cell::new(from.q, from.r + 1),
                Step::South => Cell::new(from.q, from.r - 1),
                Step::East => Cell::new(from.q + 1, from.r - 1),
                Step::West => Cell::new(from.q - 1, from.r + 1),
            },
        }
    }
}

/// A route under consideration, not yet taken.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PlannedPath {
    /// Where the token stands now.
    pub origin: Cell,
    /// Cells visited, in order. Empty means "planned, but not moved yet".
    pub steps: Vec<Cell>,
}

impl PlannedPath {
    pub fn new(origin: Cell) -> Self {
        Self {
            origin,
            steps: Vec::new(),
        }
    }

    /// The cell the token currently would end on.
    pub fn head(&self) -> Cell {
        *self.steps.last().unwrap_or(&self.origin)
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Extends the path one step.
    ///
    /// Stepping back onto the previous cell **retracts** rather than extends.
    /// Without that, correcting an overshoot would cost two steps of movement
    /// instead of undoing one — the player would be charged for their own
    /// typo.
    pub fn push(&mut self, step: Step, kind: GridKind) {
        let next = step.apply(self.head(), kind);

        let previous = if self.steps.len() >= 2 {
            self.steps[self.steps.len() - 2]
        } else {
            self.origin
        };

        if !self.steps.is_empty() && next == previous {
            self.steps.pop();
            return;
        }

        self.steps.push(next);
    }

    /// Removes the last step.
    pub fn pop(&mut self) {
        self.steps.pop();
    }

    /// Total cost in cells.
    ///
    /// Counts *steps taken*, not the straight-line distance from origin to
    /// head. A path that doubles back covers more ground than its endpoints
    /// suggest, and the player pays for the ground.
    pub fn cost_in_cells(&self) -> f32 {
        self.steps.len() as f32
    }

    /// Every point on the route in world space, starting at the origin —
    /// ready to draw as a polyline.
    pub fn world_points(&self, grid: &GridSpec) -> Vec<glam::Vec2> {
        std::iter::once(self.origin)
            .chain(self.steps.iter().copied())
            .map(|cell| grid.cell_center(cell))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn square() -> GridSpec {
        GridSpec {
            kind: GridKind::Square,
            size: 100.0,
            origin: Vec2::ZERO,
        }
    }

    #[test]
    fn a_fresh_path_starts_where_the_token_stands() {
        let path = PlannedPath::new(Cell::new(3, 4));
        assert!(path.is_empty());
        assert_eq!(path.head(), Cell::new(3, 4));
        assert_eq!(path.cost_in_cells(), 0.0);
    }

    #[test]
    fn steps_extend_the_path_and_its_cost() {
        let mut path = PlannedPath::new(Cell::new(0, 0));
        path.push(Step::East, GridKind::Square);
        path.push(Step::East, GridKind::Square);
        path.push(Step::North, GridKind::Square);

        assert_eq!(path.head(), Cell::new(2, 1));
        assert_eq!(path.cost_in_cells(), 3.0);
    }

    /// The bug this guards: stepping back the way you came should undo the
    /// step, not add another. Otherwise correcting an overshoot costs two
    /// squares of movement and the player is charged for a typo.
    #[test]
    fn stepping_back_retracts_instead_of_extending() {
        let mut path = PlannedPath::new(Cell::new(0, 0));
        path.push(Step::East, GridKind::Square);
        path.push(Step::East, GridKind::Square);
        assert_eq!(path.cost_in_cells(), 2.0);

        path.push(Step::West, GridKind::Square);
        assert_eq!(path.cost_in_cells(), 1.0, "backtracking should retract");
        assert_eq!(path.head(), Cell::new(1, 0));

        path.push(Step::West, GridKind::Square);
        assert_eq!(path.cost_in_cells(), 0.0, "should retract to the origin");
        assert_eq!(path.head(), Cell::new(0, 0));
    }

    #[test]
    fn stepping_past_the_origin_extends_again() {
        // Retraction must stop at the origin, not run negative.
        let mut path = PlannedPath::new(Cell::new(0, 0));
        path.push(Step::East, GridKind::Square);
        path.push(Step::West, GridKind::Square);
        path.push(Step::West, GridKind::Square);

        assert_eq!(path.cost_in_cells(), 1.0);
        assert_eq!(path.head(), Cell::new(-1, 0));
    }

    #[test]
    fn a_loop_costs_the_ground_it_covers_not_the_displacement() {
        // Around a square and back: ends where it started, cost is four.
        let mut path = PlannedPath::new(Cell::new(0, 0));
        for step in [Step::East, Step::North, Step::West, Step::South] {
            path.push(step, GridKind::Square);
        }
        assert_eq!(path.head(), Cell::new(0, 0));
        assert_eq!(path.cost_in_cells(), 4.0, "a loop is not free");
    }

    #[test]
    fn world_points_include_the_origin_so_the_line_starts_at_the_token() {
        let grid = square();
        let mut path = PlannedPath::new(Cell::new(0, 0));
        path.push(Step::East, GridKind::Square);

        let points = path.world_points(&grid);
        assert_eq!(points.len(), 2, "origin plus one step");
        assert_eq!(points[0], grid.cell_center(Cell::new(0, 0)));
        assert_eq!(points[1], grid.cell_center(Cell::new(1, 0)));
    }

    #[test]
    fn hex_steps_land_on_real_neighbours() {
        for kind in [GridKind::HexPointyTop, GridKind::HexFlatTop] {
            let grid = GridSpec {
                kind,
                size: 128.0,
                origin: Vec2::ZERO,
            };
            let origin = Cell::new(0, 0);
            for step in [Step::North, Step::South, Step::East, Step::West] {
                let next = step.apply(origin, kind);
                assert_eq!(
                    grid.cell_distance(origin, next),
                    1,
                    "{kind:?} {step:?} did not reach an adjacent hex",
                );
            }
        }
    }

    #[test]
    fn opposite_steps_cancel_on_hex_too() {
        for kind in [GridKind::HexPointyTop, GridKind::HexFlatTop] {
            for (there, back) in [(Step::East, Step::West), (Step::North, Step::South)] {
                let mut path = PlannedPath::new(Cell::new(0, 0));
                path.push(there, kind);
                path.push(back, kind);
                assert_eq!(
                    path.cost_in_cells(),
                    0.0,
                    "{kind:?}: {there:?} then {back:?} should retract",
                );
            }
        }
    }
}
