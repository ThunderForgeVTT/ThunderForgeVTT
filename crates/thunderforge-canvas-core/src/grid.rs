//! Grid geometry: the single source of truth for where cells are.
//!
//! Pure math, no Bevy — `thunderforge_engine` wraps `GridSpec` in a Bevy
//! `Resource` newtype (`resources::SceneGrid`). Living here is what makes
//! these tests actually execute under a plain `cargo test`; the engine crate
//! only targets wasm and cannot even link `winit` natively.
//!
//! Every grid-dependent behaviour in the engine — snapping a dragged token,
//! measuring movement, drawing the grid, deciding which cell a click landed
//! in — resolves through this module. That matters because the alternative
//! was already in the codebase and had gone wrong: `plugins/grid.rs` drew
//! lines at one hardcoded size, `movement::apply_grid_snapping` snapped to a
//! *different* hardcoded size (32.0), and neither had any relationship to the
//! `pixels_per_grid` a dd2vtt import actually recorded. A grid you can see and
//! a grid you snap to must be the same grid.
//!
//! # Conventions
//!
//! - **Y is up.** These are Bevy world coordinates, not image coordinates.
//! - **`size` is centre-to-centre spacing between edge-adjacent cells**, in
//!   world units. For squares that is the side length. For hexes it is the
//!   distance across the flats, which is what Universal VTT's
//!   `pixels_per_grid` means — so an imported map's value can be used
//!   directly for either topology without conversion.
//! - **`origin`** is the world position of the corner of cell (0, 0), so a
//!   grid can be aligned to imported art rather than assuming the art was
//!   authored around the world origin.

use glam::Vec2;

/// Which tiling a scene uses.
///
/// Hex is split by orientation because they are not interchangeable: the same
/// cell coordinates land in different places, and a map authored for one reads
/// as visibly wrong under the other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GridKind {
    #[default]
    Square,
    /// Hexes with a vertex at the top; columns interlock horizontally.
    HexPointyTop,
    /// Hexes with a flat edge at the top; rows interlock vertically.
    HexFlatTop,
    /// No tiling. Tokens position freely and nothing snaps.
    Gridless,
}

impl GridKind {
    /// Parses the server's `scenes.grid_type` vocabulary.
    ///
    /// The server stores `"square" | "hex" | "gridless"` with no orientation,
    /// so a bare `"hex"` resolves to pointy-top — the orientation Universal
    /// VTT exporters and most published hex maps use. The explicit spellings
    /// are accepted so a scene can pin the other one.
    pub fn from_server_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "hex" | "hexagonal" | "hex_pointy" | "hexpointytop" => GridKind::HexPointyTop,
            "hex_flat" | "hexflattop" => GridKind::HexFlatTop,
            "gridless" | "none" => GridKind::Gridless,
            _ => GridKind::Square,
        }
    }

    pub fn is_hex(self) -> bool {
        matches!(self, GridKind::HexPointyTop | GridKind::HexFlatTop)
    }
}

/// A scene's grid, as the engine understands it.
#[derive(Clone, Copy, Debug)]
pub struct GridSpec {
    pub kind: GridKind,
    /// Centre-to-centre spacing of edge-adjacent cells, in world units.
    pub size: f32,
    /// World position of cell (0, 0)'s corner.
    pub origin: Vec2,
}

impl Default for GridSpec {
    fn default() -> Self {
        Self {
            kind: GridKind::Square,
            size: 128.0,
            origin: Vec2::ZERO,
        }
    }
}

/// How many cells across a token occupies.
///
/// Mirrors the size categories every tabletop ruleset uses — Tiny at a half
/// cell, Medium at one, Large at two, Huge at three, Gargantuan at four and up
/// — but is stored as a plain multiplier so a system can use whatever ladder it
/// likes without this crate knowing the names.
///
/// Clamped at a half cell on the low end. Below that a token is smaller than
/// the thing it stands on, unclickable at play zoom, and there is no
/// sub-half-cell position for it to snap to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint(f32);

/// The smallest footprint a token may have, in cells.
pub const MIN_FOOTPRINT: f32 = 0.5;

impl Default for Footprint {
    fn default() -> Self {
        Self(1.0)
    }
}

impl Footprint {
    /// Clamps to at least [`MIN_FOOTPRINT`]. A non-finite value falls back to
    /// one cell rather than poisoning every later position calculation.
    pub fn new(cells: f32) -> Self {
        if !cells.is_finite() {
            return Self(1.0);
        }
        Self(cells.max(MIN_FOOTPRINT))
    }

    pub fn cells(self) -> f32 {
        self.0
    }

    /// Side length in world units on a grid of `cell_size`.
    pub fn world_size(self, cell_size: f32) -> f32 {
        self.0 * cell_size
    }

    /// The lattice a token of this size snaps its *corner* to, in cells.
    ///
    /// Whole cells for anything a cell or larger; half cells for a Tiny token,
    /// which is the only size that legitimately sits inside one cell.
    fn corner_step(self) -> f32 {
        if self.0 < 1.0 { 0.5 } else { 1.0 }
    }
}

/// A cell address.
///
/// Square grids read these as column/row. Hex grids read them as axial
/// coordinates (`q`, `r`) — the same two numbers, interpreted by `kind`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cell {
    pub q: i32,
    pub r: i32,
}

impl Cell {
    pub const fn new(q: i32, r: i32) -> Self {
        Self { q, r }
    }
}

impl GridSpec {
    /// A grid aligned to a map's own painted cells.
    ///
    /// Imported maps already have a grid drawn on them, and ours has to land
    /// on it. The engine centres a scene's background sprite on the world
    /// origin, so the map spans `-size/2 .. +size/2` — which means anchoring
    /// the lattice at the world origin only coincides with the painted grid
    /// when half the map is a whole number of cells.
    ///
    /// That is a coin flip in practice. Of this project's eight example maps,
    /// four (`demo` at 35x20 cells, and three at 48x27) have an odd cell count
    /// on an axis, and on those the centred lattice is off by exactly half a
    /// cell. Anchoring cell (0,0)'s corner at the map's corner instead is
    /// correct for any dimensions, odd or even.
    pub fn anchored_to_map(kind: GridKind, size: f32, map_size: Vec2) -> Self {
        Self {
            kind,
            size,
            origin: -map_size / 2.0,
        }
    }

    /// A usable spacing, guarding against a zero or negative size arriving
    /// from outside. Every conversion below divides by this.
    fn safe_size(&self) -> f32 {
        if self.size.is_finite() && self.size > f32::EPSILON {
            self.size
        } else {
            Self::default().size
        }
    }

    /// Circumradius (centre to vertex) of a hex whose across-flats distance is
    /// `size`.
    fn hex_radius(&self) -> f32 {
        self.safe_size() / 3.0_f32.sqrt()
    }

    /// The cell containing `world`.
    pub fn world_to_cell(&self, world: Vec2) -> Cell {
        let local = world - self.origin;
        match self.kind {
            // Gridless has no cells; report (0,0) rather than inventing a
            // tiling. Callers gate on `kind` before using this.
            GridKind::Gridless => Cell::new(0, 0),
            GridKind::Square => {
                let size = self.safe_size();
                Cell::new(
                    (local.x / size).floor() as i32,
                    (local.y / size).floor() as i32,
                )
            }
            GridKind::HexPointyTop => {
                let radius = self.hex_radius();
                let q = (3.0_f32.sqrt() / 3.0 * local.x - local.y / 3.0) / radius;
                let r = (2.0 / 3.0 * local.y) / radius;
                axial_round(q, r)
            }
            GridKind::HexFlatTop => {
                let radius = self.hex_radius();
                let q = (2.0 / 3.0 * local.x) / radius;
                let r = (-local.x / 3.0 + 3.0_f32.sqrt() / 3.0 * local.y) / radius;
                axial_round(q, r)
            }
        }
    }

    /// The world position of a cell's centre.
    pub fn cell_center(&self, cell: Cell) -> Vec2 {
        let (q, r) = (cell.q as f32, cell.r as f32);
        let local = match self.kind {
            GridKind::Gridless => Vec2::ZERO,
            GridKind::Square => {
                let size = self.safe_size();
                Vec2::new((q + 0.5) * size, (r + 0.5) * size)
            }
            GridKind::HexPointyTop => {
                let radius = self.hex_radius();
                Vec2::new(radius * 3.0_f32.sqrt() * (q + r / 2.0), radius * 1.5 * r)
            }
            GridKind::HexFlatTop => {
                let radius = self.hex_radius();
                Vec2::new(radius * 1.5 * q, radius * 3.0_f32.sqrt() * (r + q / 2.0))
            }
        };
        local + self.origin
    }

    /// Snaps a token of a given size, returning its new centre.
    ///
    /// The naive implementation — snap the centre to the nearest cell centre —
    /// is wrong for even footprints and is the classic VTT grid bug. A 2x2
    /// token covers four cells, so its centre belongs on the *vertex* where
    /// those four meet, not on any cell's middle; snapping it to a centre
    /// leaves it straddling half of each neighbouring cell.
    ///
    /// Snapping the token's lower-left **corner** to the lattice gets every
    /// size right for free: the centre then falls at `corner + size/2`, which
    /// lands on a cell centre for odd footprints, on a vertex for even ones,
    /// and on a quarter position for a half-cell token.
    ///
    /// Gridless returns the position untouched, as does [`Self::snap`].
    pub fn snap_footprint(&self, world: Vec2, footprint: Footprint) -> Vec2 {
        if self.kind == GridKind::Gridless {
            return world;
        }

        let size = self.safe_size();
        let half_extent = footprint.world_size(size) / 2.0;

        // Hex grids get centre snapping regardless of size. A multi-hex
        // footprint is not a square block — a Large creature on hexes covers a
        // seven-hex flower — so scaling the lattice the way a square grid does
        // would be wrong in a way that looks plausible. Until that shape is
        // modelled, a large hex token sits centred on one hex.
        if self.kind.is_hex() {
            return self.cell_center(self.world_to_cell(world));
        }

        let step = footprint.corner_step() * size;
        let local_corner = world - self.origin - Vec2::splat(half_extent);

        // Half-up, not `round()`.
        //
        // `f32::round` breaks halves *away from zero*, which makes a point
        // exactly on a cell boundary snap outward on the negative side and
        // inward on the positive — so this disagreed with `world_to_cell`,
        // which is floor-based and puts a boundary in the cell it opens.
        //
        // The two therefore agreed on every positive boundary and differed on
        // every negative one, which is exactly how it survived: dragging a
        // token to (-60, 60) on a 5-unit grid snapped y correctly to 62.5 and
        // x to -62.5 where the rest of the system said -57.5. One cell out,
        // in one axis, only when negative.
        //
        // `(v + 0.5).floor()` rounds halves toward positive infinity, which is
        // the same tie-break floor division already makes.
        let half_up = |v: f32| (v / step + 0.5).floor() * step;
        let snapped_corner = Vec2::new(half_up(local_corner.x), half_up(local_corner.y));

        snapped_corner + self.origin + Vec2::splat(half_extent)
    }

    /// Snaps a world position to its cell's centre.
    ///
    /// Gridless returns the input untouched — a scene with no tiling has
    /// nothing to snap to, and silently pulling tokens to an invented lattice
    /// would be worse than leaving them where the user put them.
    pub fn snap(&self, world: Vec2) -> Vec2 {
        if self.kind == GridKind::Gridless {
            return world;
        }
        self.cell_center(self.world_to_cell(world))
    }

    /// Distance between two cells, **in cells**.
    ///
    /// Square grids use Chebyshev distance — diagonal costs the same as
    /// orthogonal, the rule D&D 5e and most VTTs use by default. Hex grids use
    /// axial distance, where every neighbour is exactly 1 and the awkward
    /// diagonal question does not arise.
    pub fn cell_distance(&self, a: Cell, b: Cell) -> i32 {
        match self.kind {
            GridKind::Gridless => 0,
            GridKind::Square => (a.q - b.q).abs().max((a.r - b.r).abs()),
            GridKind::HexPointyTop | GridKind::HexFlatTop => {
                let (dq, dr) = (a.q - b.q, a.r - b.r);
                // Cube distance via the implicit third axis s = -q - r.
                ((dq.abs() + dr.abs() + (dq + dr).abs()) / 2) as i32
            }
        }
    }

    /// The polygon outlining one cell, for drawing.
    ///
    /// Returned closed (first point repeated last) so a caller can render it
    /// as a single line strip without special-casing the wrap.
    pub fn cell_outline(&self, cell: Cell) -> Vec<Vec2> {
        let center = self.cell_center(cell);
        match self.kind {
            GridKind::Gridless => Vec::new(),
            GridKind::Square => {
                let half = self.safe_size() / 2.0;
                vec![
                    center + Vec2::new(-half, -half),
                    center + Vec2::new(half, -half),
                    center + Vec2::new(half, half),
                    center + Vec2::new(-half, half),
                    center + Vec2::new(-half, -half),
                ]
            }
            GridKind::HexPointyTop | GridKind::HexFlatTop => {
                let radius = self.hex_radius();
                // Pointy-top has a vertex at 90°, flat-top at 0°.
                let start = if self.kind == GridKind::HexPointyTop {
                    std::f32::consts::FRAC_PI_2
                } else {
                    0.0
                };
                let mut points: Vec<Vec2> = (0..6)
                    .map(|i| {
                        let angle = start + i as f32 * std::f32::consts::FRAC_PI_3;
                        center + Vec2::new(radius * angle.cos(), radius * angle.sin())
                    })
                    .collect();
                points.push(points[0]);
                points
            }
        }
    }
}

/// Rounds fractional axial coordinates to the nearest hex.
///
/// Done in cube space: round all three axes, then correct whichever drifted
/// furthest. Rounding `q` and `r` independently does not work — it produces
/// coordinates that are not a valid hex and shows up as tokens snapping to the
/// wrong cell near shared edges.
fn axial_round(q: f32, r: f32) -> Cell {
    let s = -q - r;
    let (mut rq, mut rr, rs) = (q.round(), r.round(), s.round());
    let (dq, dr, ds) = ((rq - q).abs(), (rr - r).abs(), (rs - s).abs());

    if dq > dr && dq > ds {
        rq = -rr - rs;
    } else if dr > ds {
        rr = -rq - rs;
    }

    Cell::new(rq as i32, rr as i32)
}

#[cfg(test)]
mod boundary_tie_break_tests {
    use super::*;

    /// A point exactly on a cell boundary must land in the same cell however
    /// it is snapped.
    ///
    /// `world_to_cell` is floor-based, so a boundary belongs to the cell it
    /// *opens*. `snap_footprint` snapped the token's corner with `round`,
    /// which breaks halves **away from zero** — so the two agreed on positive
    /// boundaries and disagreed on negative ones.
    ///
    /// Found by `token-authoring.spec.ts`, dragging a token to (-60, 60) on a
    /// 5-unit grid: y snapped to 62.5 by both routes, x snapped to -57.5 by
    /// one and -62.5 by the other — adjacent cell centres, one cell apart.
    #[test]
    fn a_boundary_point_snaps_the_same_way_by_either_route() {
        let spec = GridSpec {
            kind: GridKind::Square,
            size: 5.0,
            origin: Vec2::ZERO,
        };

        for boundary in [-60.0_f32, -5.0, 0.0, 5.0, 60.0] {
            let point = Vec2::new(boundary, boundary);
            assert_eq!(
                spec.snap_footprint(point, Footprint::default()),
                spec.snap(point),
                "boundary {boundary} disagreed between snap_footprint and snap",
            );
        }
    }

    /// The specific values the end-to-end suite records, so a regression is
    /// recognisable as *that* failure rather than as arithmetic drift.
    #[test]
    fn the_case_token_authoring_caught() {
        let spec = GridSpec {
            kind: GridKind::Square,
            size: 5.0,
            origin: Vec2::ZERO,
        };
        assert_eq!(
            spec.snap_footprint(Vec2::new(-60.0, 60.0), Footprint::default()),
            Vec2::new(-57.5, 62.5),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(size: f32) -> GridSpec {
        GridSpec {
            kind: GridKind::Square,
            size,
            origin: Vec2::ZERO,
        }
    }

    #[test]
    fn server_grid_vocabulary_maps_to_kinds() {
        assert_eq!(GridKind::from_server_str("square"), GridKind::Square);
        assert_eq!(GridKind::from_server_str("gridless"), GridKind::Gridless);
        // A bare "hex" is pointy-top — see `from_server_str`'s doc comment.
        assert_eq!(GridKind::from_server_str("hex"), GridKind::HexPointyTop);
        assert_eq!(GridKind::from_server_str("hex_flat"), GridKind::HexFlatTop);
        // Unknown values fall back to square rather than failing a scene load.
        assert_eq!(GridKind::from_server_str("wat"), GridKind::Square);
        assert_eq!(GridKind::from_server_str("  SQUARE "), GridKind::Square);
    }

    #[test]
    fn square_cells_round_trip_through_world_space() {
        let grid = square(128.0);
        for (q, r) in [(0, 0), (3, 5), (-2, -7), (48, 27)] {
            let cell = Cell::new(q, r);
            assert_eq!(grid.world_to_cell(grid.cell_center(cell)), cell);
        }
    }

    #[test]
    fn square_snapping_uses_the_scenes_real_size() {
        // The bug this guards: snapping used a hardcoded 32.0 regardless of
        // the scene's grid size, so an imported 128px map snapped to quarters
        // of a cell.
        let grid = square(128.0);
        assert_eq!(grid.snap(Vec2::new(10.0, 10.0)), Vec2::new(64.0, 64.0));
        assert_eq!(grid.snap(Vec2::new(130.0, 5.0)), Vec2::new(192.0, 64.0));
    }

    #[test]
    fn square_origin_offsets_the_whole_lattice() {
        let grid = GridSpec {
            kind: GridKind::Square,
            size: 100.0,
            origin: Vec2::new(25.0, -50.0),
        };
        assert_eq!(grid.cell_center(Cell::new(0, 0)), Vec2::new(75.0, 0.0));
        assert_eq!(grid.world_to_cell(Vec2::new(75.0, 0.0)), Cell::new(0, 0));
    }

    #[test]
    fn negative_positions_do_not_collapse_onto_zero() {
        // `as i32` truncates toward zero, so -0.5 and +0.5 would both land in
        // cell 0 and the row/column either side of the origin would be twice
        // as wide as every other. `floor` is what keeps the lattice uniform.
        let grid = square(100.0);
        assert_eq!(grid.world_to_cell(Vec2::new(-1.0, -1.0)), Cell::new(-1, -1));
        assert_eq!(grid.world_to_cell(Vec2::new(1.0, 1.0)), Cell::new(0, 0));
    }

    #[test]
    fn hex_cells_round_trip_in_both_orientations() {
        for kind in [GridKind::HexPointyTop, GridKind::HexFlatTop] {
            let grid = GridSpec {
                kind,
                size: 128.0,
                origin: Vec2::ZERO,
            };
            for (q, r) in [(0, 0), (1, 0), (0, 1), (-3, 2), (5, -4), (7, 7)] {
                let cell = Cell::new(q, r);
                assert_eq!(
                    grid.world_to_cell(grid.cell_center(cell)),
                    cell,
                    "{kind:?} failed to round-trip {cell:?}",
                );
            }
        }
    }

    #[test]
    fn hex_neighbours_are_one_step_apart_and_correctly_spaced() {
        let grid = GridSpec {
            kind: GridKind::HexPointyTop,
            size: 128.0,
            origin: Vec2::ZERO,
        };
        let center = Cell::new(0, 0);
        // The six axial neighbours.
        for (q, r) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, -1), (-1, 1)] {
            let neighbour = Cell::new(q, r);
            assert_eq!(grid.cell_distance(center, neighbour), 1);
            // `size` is across-flats, so adjacent centres sit exactly that far
            // apart. This is what lets a UVTT `pixels_per_grid` be used as-is.
            let spacing = grid
                .cell_center(center)
                .distance(grid.cell_center(neighbour));
            assert!(
                (spacing - 128.0).abs() < 0.01,
                "neighbour spacing was {spacing}, expected the grid size",
            );
        }
    }

    #[test]
    fn square_distance_treats_diagonals_as_one() {
        let grid = square(50.0);
        assert_eq!(grid.cell_distance(Cell::new(0, 0), Cell::new(3, 3)), 3);
        assert_eq!(grid.cell_distance(Cell::new(0, 0), Cell::new(0, 4)), 4);
    }

    #[test]
    fn hex_distance_grows_one_per_ring() {
        let grid = GridSpec {
            kind: GridKind::HexPointyTop,
            size: 64.0,
            origin: Vec2::ZERO,
        };
        assert_eq!(grid.cell_distance(Cell::new(0, 0), Cell::new(2, -1)), 2);
        assert_eq!(grid.cell_distance(Cell::new(0, 0), Cell::new(-3, 0)), 3);
    }

    #[test]
    fn gridless_never_moves_a_token() {
        let grid = GridSpec {
            kind: GridKind::Gridless,
            size: 128.0,
            origin: Vec2::ZERO,
        };
        let free = Vec2::new(13.7, -91.2);
        assert_eq!(grid.snap(free), free);
        assert!(grid.cell_outline(Cell::new(0, 0)).is_empty());
    }

    #[test]
    fn a_nonsense_size_falls_back_instead_of_dividing_by_zero() {
        for bad in [0.0, -50.0, f32::NAN] {
            let grid = square(bad);
            let snapped = grid.snap(Vec2::new(10.0, 10.0));
            assert!(
                snapped.is_finite(),
                "size {bad} produced a non-finite position",
            );
        }
    }

    /// The bug this guards: a lattice anchored at the world origin lands on a
    /// centred map's painted grid only when the map is an even number of cells
    /// across. Anchoring to the corner has to work for both parities.
    #[test]
    fn a_map_anchored_grid_lands_on_the_maps_own_cells() {
        for (cells_x, cells_y) in [(48, 27), (10, 10), (35, 20)] {
            let size = 128.0;
            let map = Vec2::new(cells_x as f32 * size, cells_y as f32 * size);
            let grid = GridSpec::anchored_to_map(GridKind::Square, size, map);

            // The map's corners must be exact cell corners.
            let bottom_left = -map / 2.0;
            assert_eq!(
                grid.world_to_cell(bottom_left + Vec2::splat(1.0)),
                Cell::new(0, 0),
                "{cells_x}x{cells_y}: the map's corner should start cell (0,0)",
            );

            // And the far corner should be exactly `cells` cells away, with no
            // half-cell hanging off the end.
            let top_right = map / 2.0;
            let last = grid.world_to_cell(top_right - Vec2::splat(1.0));
            assert_eq!(
                (last.q, last.r),
                (cells_x - 1, cells_y - 1),
                "{cells_x}x{cells_y}: the map should span a whole number of cells",
            );
        }
    }

    /// Demonstrates the failure directly: with an odd cell count, the old
    /// origin-anchored lattice cuts the map's edge cells in half.
    #[test]
    fn an_origin_anchored_grid_is_half_a_cell_out_on_an_odd_map() {
        let size = 128.0;
        // 27 cells tall — the shape three of the example maps have.
        let map = Vec2::new(48.0 * size, 27.0 * size);
        let origin_anchored = GridSpec {
            kind: GridKind::Square,
            size,
            origin: Vec2::ZERO,
        };

        // The map's bottom edge should be a cell boundary. Under the centred
        // lattice it falls in the middle of a cell instead.
        let bottom_edge_y = -map.y / 2.0;
        let offset = bottom_edge_y.rem_euclid(size);
        assert!(
            (offset - size / 2.0).abs() < 0.01,
            "expected a half-cell offset, got {offset}",
        );

        // The corner-anchored grid has no such offset.
        let anchored = GridSpec::anchored_to_map(GridKind::Square, size, map);
        let corner = anchored.cell_center(Cell::new(0, 0)) - Vec2::splat(size / 2.0);
        assert!((corner.y - bottom_edge_y).abs() < 0.01);
        let _ = origin_anchored;
    }

    // --- footprints ------------------------------------------------------

    fn square_grid() -> GridSpec {
        // Origin off the world origin, so a test passing by accident because
        // everything is centred on zero would be caught.
        GridSpec {
            kind: GridKind::Square,
            size: 100.0,
            origin: Vec2::new(-250.0, -450.0),
        }
    }

    #[test]
    fn a_footprint_never_goes_below_half_a_cell() {
        assert_eq!(Footprint::new(0.1).cells(), MIN_FOOTPRINT);
        assert_eq!(Footprint::new(0.0).cells(), MIN_FOOTPRINT);
        assert_eq!(Footprint::new(-4.0).cells(), MIN_FOOTPRINT);
        assert_eq!(Footprint::new(f32::NAN).cells(), 1.0);
        // Anything at or above the floor is kept exactly.
        assert_eq!(Footprint::new(0.5).cells(), 0.5);
        assert_eq!(Footprint::new(3.0).cells(), 3.0);
        assert_eq!(Footprint::default().cells(), 1.0);
    }

    #[test]
    fn a_footprint_is_sized_in_whole_cells() {
        assert_eq!(Footprint::new(1.0).world_size(128.0), 128.0);
        assert_eq!(Footprint::new(2.0).world_size(128.0), 256.0);
        assert_eq!(Footprint::new(0.5).world_size(128.0), 64.0);
    }

    #[test]
    fn an_odd_token_snaps_to_a_cell_centre() {
        let grid = square_grid();
        for cells in [1.0, 3.0, 5.0] {
            let footprint = Footprint::new(cells);
            // Nudge off-centre, then snap.
            let target = grid.cell_center(Cell::new(2, 3)) + Vec2::new(19.0, -23.0);
            let snapped = grid.snap_footprint(target, footprint);
            assert!(
                (snapped - grid.cell_center(Cell::new(2, 3))).length() < 0.01,
                "{cells}-cell token should centre on a cell, got {snapped:?}",
            );
        }
    }

    /// The bug this guards: snapping an even-footprint token to a cell centre
    /// leaves it straddling half of each neighbouring cell. A 2x2 covers four
    /// cells, so its centre belongs on the vertex where they meet.
    #[test]
    fn an_even_token_snaps_to_a_cell_vertex() {
        let grid = square_grid();
        for cells in [2.0, 4.0] {
            let footprint = Footprint::new(cells);
            let snapped = grid.snap_footprint(Vec2::new(-31.0, -87.0), footprint);

            // Distance from the origin must be a whole number of cells on both
            // axes — that is what "on a vertex" means.
            let local = snapped - grid.origin;
            for axis in [local.x, local.y] {
                let cells_from_origin = axis / grid.size;
                assert!(
                    (cells_from_origin - cells_from_origin.round()).abs() < 0.001,
                    "{cells}-cell token centre sits {cells_from_origin} cells from origin, \
                     which is not a vertex",
                );
            }
        }
    }

    #[test]
    fn a_half_cell_token_snaps_within_a_single_cell() {
        let grid = square_grid();
        let footprint = Footprint::new(0.5);
        let snapped = grid.snap_footprint(Vec2::new(11.0, 7.0), footprint);

        // Its corner lands on a half-cell, so its centre lands on a quarter.
        let local = snapped - grid.origin;
        for axis in [local.x, local.y] {
            let quarters = axis / (grid.size / 4.0);
            assert!(
                (quarters - quarters.round()).abs() < 0.001,
                "half-cell token centre is not on a quarter position: {axis}",
            );
        }
    }

    /// Whatever the size, the token's own edges must land on grid lines —
    /// that is the property that makes a snapped token look correct.
    #[test]
    fn every_footprint_lands_its_edges_on_grid_lines() {
        let grid = square_grid();
        for cells in [0.5, 1.0, 2.0, 3.0, 4.0, 7.0] {
            let footprint = Footprint::new(cells);
            let centre = grid.snap_footprint(Vec2::new(137.0, -62.0), footprint);
            let half = footprint.world_size(grid.size) / 2.0;

            let lower_left = centre - Vec2::splat(half) - grid.origin;
            let step = if cells < 1.0 {
                grid.size / 2.0
            } else {
                grid.size
            };
            for axis in [lower_left.x, lower_left.y] {
                let steps = axis / step;
                assert!(
                    (steps - steps.round()).abs() < 0.001,
                    "{cells}-cell token corner is off the lattice by {} steps",
                    steps - steps.round(),
                );
            }
        }
    }

    #[test]
    fn snapping_is_idempotent() {
        // Snapping an already-snapped token must not drift it — a system that
        // snaps every frame would otherwise walk tokens across the board.
        let grid = square_grid();
        for cells in [0.5, 1.0, 2.0, 3.0] {
            let footprint = Footprint::new(cells);
            let once = grid.snap_footprint(Vec2::new(88.0, -13.0), footprint);
            let twice = grid.snap_footprint(once, footprint);
            assert!(
                (once - twice).length() < 0.001,
                "{cells}-cell snapping drifted on reapplication",
            );
        }
    }

    #[test]
    fn gridless_leaves_a_token_of_any_size_alone() {
        let grid = GridSpec {
            kind: GridKind::Gridless,
            size: 100.0,
            origin: Vec2::ZERO,
        };
        let free = Vec2::new(13.7, -91.2);
        for cells in [0.5, 1.0, 3.0] {
            assert_eq!(grid.snap_footprint(free, Footprint::new(cells)), free);
        }
    }

    #[test]
    fn a_hex_token_centres_on_a_hex_whatever_its_size() {
        // Multi-hex footprints are not square blocks, so large hex tokens
        // centre on one hex rather than snapping to a scaled lattice.
        let grid = GridSpec {
            kind: GridKind::HexPointyTop,
            size: 128.0,
            origin: Vec2::ZERO,
        };
        for cells in [1.0, 2.0, 3.0] {
            let snapped = grid.snap_footprint(Vec2::new(40.0, 25.0), Footprint::new(cells));
            let cell = grid.world_to_cell(snapped);
            assert!((snapped - grid.cell_center(cell)).length() < 0.01);
        }
    }

    #[test]
    fn cell_outlines_are_closed_rings() {
        for kind in [
            GridKind::Square,
            GridKind::HexPointyTop,
            GridKind::HexFlatTop,
        ] {
            let grid = GridSpec {
                kind,
                size: 90.0,
                origin: Vec2::ZERO,
            };
            let outline = grid.cell_outline(Cell::new(2, -1));
            let expected = if kind == GridKind::Square { 5 } else { 7 };
            assert_eq!(outline.len(), expected, "{kind:?}");
            assert_eq!(
                outline.first(),
                outline.last(),
                "{kind:?} ring is not closed"
            );
        }
    }
}
