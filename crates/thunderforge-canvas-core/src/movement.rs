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

use crate::grid::{Cell, Footprint, GridKind, GridSpec};

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

/// Where a token lands when its whole footprint moves the way a cell moves
/// from `origin` to `to` (spec 046 FR-031).
///
/// The token is carried by the offset between the two cells, then snapped for
/// its footprint. Snapping the destination *cell's centre* instead — which is
/// what keyboard moves did — is right for one square and wrong for two: a 2×2
/// token's centre is a vertex, the cell it reports as "its" cell is the one up
/// and to the right of that vertex, and a half-up snap from that cell's
/// neighbour to the west or south rounds straight back to where it started. A
/// Large creature could walk east and north, and never west or south.
pub fn footprint_moved(
    grid: &GridSpec,
    from: glam::Vec2,
    footprint: Footprint,
    origin: Cell,
    to: Cell,
) -> glam::Vec2 {
    grid.snap_footprint(
        from + (grid.cell_center(to) - grid.cell_center(origin)),
        footprint,
    )
}

/// One keyboard step for a token of any footprint.
pub fn step_token(
    grid: &GridSpec,
    from: glam::Vec2,
    footprint: Footprint,
    step: Step,
) -> glam::Vec2 {
    let here = grid.world_to_cell(from);
    footprint_moved(grid, from, footprint, here, step.apply(here, grid.kind))
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

    /// Where a token of `footprint` standing at `from` ends when the route
    /// is committed: carried by the route, as [`footprint_moved`] carries it.
    pub fn destination(
        &self,
        grid: &GridSpec,
        from: glam::Vec2,
        footprint: Footprint,
    ) -> glam::Vec2 {
        footprint_moved(grid, from, footprint, self.origin, self.head())
    }

    /// The route as the token's own centre walks it, starting at `from`: the
    /// cell centres shifted by how far the token's centre sits from its
    /// origin cell's. The same points as [`Self::world_points`] for a token of
    /// one square; for a Large one, the vertex its four squares meet at.
    pub fn world_points_from(&self, grid: &GridSpec, from: glam::Vec2) -> Vec<glam::Vec2> {
        let offset = from - grid.cell_center(self.origin);
        self.world_points(grid)
            .into_iter()
            .map(|point| point + offset)
            .collect()
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

    // --- a footprint's step (spec 046 FR-031) -------------------------

    fn anchored() -> GridSpec {
        GridSpec {
            kind: GridKind::Square,
            size: 50.0,
            origin: Vec2::new(-525.0, -375.0),
        }
    }

    #[test]
    fn a_large_token_steps_one_square_in_every_direction() {
        let grid = anchored();
        let large = Footprint::new(2.0);
        // Centred on a vertex, as a snapped 2×2 is.
        let at = grid.snap_footprint(Vec2::new(10.0, 10.0), large);
        for (step, expected) in [
            (Step::East, Vec2::new(50.0, 0.0)),
            (Step::West, Vec2::new(-50.0, 0.0)),
            (Step::North, Vec2::new(0.0, 50.0)),
            (Step::South, Vec2::new(0.0, -50.0)),
        ] {
            let landed = step_token(&grid, at, large, step);
            assert_eq!(landed - at, expected, "{step:?} moves the ogre one square");
        }
    }

    #[test]
    fn snapping_the_next_cells_centre_is_what_stranded_a_large_token() {
        // The formula keyboard moves used before spec 046. The cell a vertex
        // reports is the one up and to the right of it, so from that cell's
        // centre a 2×2 snaps half a square up and right again: west went
        // north instead, and south went east.
        let grid = anchored();
        let large = Footprint::new(2.0);
        let at = grid.snap_footprint(Vec2::new(10.0, 10.0), large);
        let old = |step: Step| {
            let next = step.apply(grid.world_to_cell(at), grid.kind);
            grid.snap_footprint(grid.cell_center(next), large)
        };
        assert_eq!(old(Step::West) - at, Vec2::new(0.0, 50.0));
        assert_eq!(old(Step::South) - at, Vec2::new(50.0, 0.0));
    }

    #[test]
    fn one_square_tokens_step_exactly_as_before() {
        let grid = anchored();
        let one = Footprint::default();
        let at = grid.snap_footprint(Vec2::new(10.0, 10.0), one);
        for step in [Step::North, Step::South, Step::East, Step::West] {
            let next = step.apply(grid.world_to_cell(at), grid.kind);
            assert_eq!(
                step_token(&grid, at, one, step),
                grid.snap_footprint(grid.cell_center(next), one),
                "{step:?}"
            );
        }
        // And a hex token still lands on a neighbouring hex's centre.
        let hex = GridSpec {
            kind: GridKind::HexPointyTop,
            size: 90.0,
            origin: Vec2::ZERO,
        };
        let at = hex.cell_center(Cell::new(2, 1));
        let landed = step_token(&hex, at, Footprint::new(2.0), Step::West);
        assert!((landed - hex.cell_center(Cell::new(1, 1))).length() < 0.01);
    }

    #[test]
    fn a_committed_route_carries_a_large_token_and_starts_at_its_centre() {
        let grid = anchored();
        let large = Footprint::new(2.0);
        let at = grid.snap_footprint(Vec2::new(10.0, 10.0), large);
        let mut path = PlannedPath::new(grid.world_to_cell(at));
        path.push(Step::West, GridKind::Square);
        path.push(Step::South, GridKind::Square);
        assert_eq!(
            path.destination(&grid, at, large) - at,
            Vec2::new(-50.0, -50.0)
        );
        let points = path.world_points_from(&grid, at);
        assert_eq!(points.first(), Some(&at));
        assert_eq!(points.last(), Some(&path.destination(&grid, at, large)));
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
