//! The one place that decides where a placed thing ends up.
//!
//! Everything that lands on the canvas — a token dragged across the board, a
//! token being placed for the first time, a wall endpoint, a light — asks this
//! module, and each of them asks the *same* function for its kind of content.
//! That is the whole point of the module existing rather than each caller
//! reaching for [`GridSpec`] directly: FR-006 requires a placed token to obey
//! the same grid rules as a dragged one, and the only way to guarantee that
//! over time is for there to be a single call, not two implementations that
//! currently agree.
//!
//! The geometry itself lives in [`crate::grid`] and is not duplicated here —
//! this module is the *policy* layer on top of it:
//!
//! - **Whether to snap at all.** FR-024 makes snapping a GM-controlled setting,
//!   on by default, and the engine additionally lets a single token opt out
//!   (`resources::TokenGridBehaviour::snap`, ANDed with the scene-wide
//!   `resources::GridSnapEnabled`). Both inputs land in [`SnapRule`] so the AND
//!   is written down once and tested, instead of every call site rediscovering
//!   it. When the answer is "no", the position passes through *untouched* —
//!   free placement means exactly where the user let go, not a gentler snap.
//! - **Which lattice feature a given kind of content belongs on.** A token sits
//!   in a cell; a light sits in a cell; a wall endpoint sits on a cell
//!   *corner*. Snapping a wall to cell centres would draw every wall through
//!   the middle of a row of cells instead of along its edge, which is why
//!   [`SnapRule::vertex`] exists alongside [`SnapRule::cell`].
//!
//! Grid *type* is honoured throughout (FR-025). Hex is not an afterthought
//! here: [`GridSpec`] already models both orientations, and this module never
//! branches on "square or not" — it delegates, so a hex scene gets hex answers
//! for tokens, lights and wall corners alike.
//!
//! # Hex orientation
//!
//! This module does not choose one. [`crate::grid::GridKind`] already carries
//! `HexPointyTop` and `HexFlatTop` as distinct kinds, and
//! `GridKind::from_server_str` already fixes the mapping from the server's
//! vocabulary — a bare `"hex"` from `scenes.grid_type` means pointy-top,
//! because that is what Universal VTT exporters and most published hex maps
//! use. Snapping inherits that decision rather than re-making it, which is
//! deliberate: a second place that decided orientation is a second place that
//! could disagree with the grid being drawn.

use glam::Vec2;

use crate::grid::{Cell, Footprint, GridKind, GridSpec};

/// A snapping decision, ready to apply.
///
/// Cheap to construct and `Copy`, so the intended usage is to build one per
/// system run (or per gesture) from the scene's grid and the current setting,
/// then call it for each position. Holding the grid alongside the flag is what
/// makes "snapping is off" and "this scene has no grid" answerable in one
/// place — both mean *pass the position through* and callers should not have to
/// remember the second case.
#[derive(Clone, Copy, Debug)]
pub struct SnapRule {
    pub grid: GridSpec,
    /// The GM's setting, already combined with any per-target opt-out.
    enabled: bool,
}

impl SnapRule {
    /// A rule from the scene's grid and the GM's scene-wide snapping setting.
    pub fn new(grid: GridSpec, enabled: bool) -> Self {
        Self { grid, enabled }
    }

    /// A rule for one target that can opt out of an otherwise-on setting.
    ///
    /// The two flags are ANDed, never merged: turning the scene switch back on
    /// restores each target's own preference rather than flattening them, which
    /// is what makes the scene switch usable as a held "free placement"
    /// modifier without destroying per-token intent.
    pub fn for_target(grid: GridSpec, scene_enabled: bool, target_snaps: bool) -> Self {
        Self::new(grid, scene_enabled && target_snaps)
    }

    /// Whether this rule will actually move anything.
    ///
    /// False when the GM turned snapping off *or* the scene is gridless. UI
    /// that shows a snap indicator should gate on this rather than on the
    /// setting alone, so a gridless scene does not advertise a lattice that
    /// isn't there.
    pub fn is_active(&self) -> bool {
        self.enabled && self.grid.kind != GridKind::Gridless
    }

    /// Where a token of `footprint` comes to rest.
    ///
    /// This is the call both the drag path and the placement path make — see
    /// the module docs. It delegates to [`GridSpec::snap_footprint`], which is
    /// already what `systems::token_grid::snap_tokens_to_grid` uses for drags,
    /// so routing placement through here cannot change dragging behaviour: it
    /// is the identical function, not a reimplementation of it.
    pub fn token(&self, world: Vec2, footprint: Footprint) -> Vec2 {
        if !self.is_active() {
            return world;
        }
        self.grid.snap_footprint(world, footprint)
    }

    /// The centre of the cell containing `world`.
    ///
    /// For content that occupies a cell without having a footprint — a light,
    /// a marker, an interaction hotspot.
    pub fn cell(&self, world: Vec2) -> Vec2 {
        if !self.is_active() {
            return world;
        }
        self.grid.snap(world)
    }

    /// The nearest lattice corner to `world`.
    ///
    /// For wall endpoints and anything else drawn *between* cells rather than
    /// inside one. Two walls drawn to the same corner from different directions
    /// meet exactly, which is what lets a room close and its interior read as
    /// enclosed by the vision pass.
    pub fn vertex(&self, world: Vec2) -> Vec2 {
        if !self.is_active() {
            return world;
        }
        match self.grid.kind {
            // Unreachable: `is_active` already returned false for it.
            GridKind::Gridless => world,
            GridKind::Square => {
                let size = self.effective_size();
                let local = world - self.grid.origin;
                Vec2::new(
                    (local.x / size).round() * size,
                    (local.y / size).round() * size,
                ) + self.grid.origin
            }
            GridKind::HexPointyTop | GridKind::HexFlatTop => {
                // A hex lattice's corners are not a regular grid, so there is
                // no closed form to round into. But every corner near a point
                // is a corner of the hex that point is in, so the six corners
                // of the containing cell are the complete candidate set — no
                // neighbour search needed.
                let cell = self.grid.world_to_cell(world);
                let outline = self.grid.cell_outline(cell);
                outline
                    .iter()
                    .take(6)
                    .copied()
                    .min_by(|a, b| {
                        a.distance_squared(world)
                            .total_cmp(&b.distance_squared(world))
                    })
                    .unwrap_or(world)
            }
        }
    }

    /// The cell a position belongs to, for callers that need the address
    /// rather than the world point.
    pub fn cell_address(&self, world: Vec2) -> Cell {
        self.grid.world_to_cell(world)
    }

    /// The grid's spacing, with [`GridSpec`]'s own zero/NaN guard applied.
    ///
    /// Recovered from `cell_center` rather than read off `GridSpec::size`
    /// directly: `safe_size` is private to that module, and reading the raw
    /// field would reintroduce the division-by-zero it exists to prevent. Cell
    /// (0, 0)'s centre sits half a cell from the origin on both axes.
    fn effective_size(&self) -> f32 {
        let half = self.grid.cell_center(Cell::new(0, 0)) - self.grid.origin;
        half.x * 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(kind: GridKind, size: f32) -> GridSpec {
        GridSpec {
            kind,
            size,
            origin: Vec2::ZERO,
        }
    }

    /// The size the e2e suite's `snapWorld` helper assumes
    /// (`DEFAULT_SCENE_GRID_SIZE`), so these numbers are the ones the browser
    /// tests already assert against.
    const DEFAULT_SCENE_GRID_SIZE: f32 = 5.0;

    #[test]
    fn square_token_snaps_to_the_cell_centre() {
        let rule = SnapRule::new(grid(GridKind::Square, 128.0), true);
        assert_eq!(
            rule.token(Vec2::new(10.0, 10.0), Footprint::default()),
            Vec2::new(64.0, 64.0)
        );
        assert_eq!(
            rule.token(Vec2::new(130.0, 5.0), Footprint::default()),
            Vec2::new(192.0, 64.0)
        );
    }

    #[test]
    fn square_snapping_matches_the_e2e_suites_expectation() {
        // `snapWorld` in apps/web/e2e/token-authoring.spec.ts is
        // `floor(v / cell) * cell + cell / 2`, and its comment names the
        // function it is modelling: `GridSpec::snap`. A drag aimed at
        // (100, -100) persists as (102.5, -97.5) on the default 5-unit grid.
        let rule = SnapRule::new(grid(GridKind::Square, DEFAULT_SCENE_GRID_SIZE), true);
        assert_eq!(rule.cell(Vec2::new(100.0, -100.0)), Vec2::new(102.5, -97.5));

        // The positions the suite's own comments record as settled tokens on
        // that grid are already cell centres, so every rule must leave them
        // exactly alone — this is the assertion that would catch a snap change
        // breaking the browser tests.
        for x in [-192.5_f32, -62.5, 2.5] {
            let point = Vec2::new(x, x);
            assert_eq!(rule.token(point, Footprint::default()), point, "token {x}");
            assert_eq!(rule.cell(point), point, "cell {x}");
        }
    }

    #[test]
    fn square_snapping_agrees_with_the_reference_formula_including_negatives() {
        // Exhaustive against the e2e helper's formula over a band that
        // straddles the origin, because truncation-vs-floor bugs only show up
        // on the negative side.
        let cell = DEFAULT_SCENE_GRID_SIZE;
        let rule = SnapRule::new(grid(GridKind::Square, cell), true);
        let reference = |v: f32| (v / cell).floor() * cell + cell / 2.0;

        for step in -400..=400 {
            // A tenth of a unit, deliberately offset so it never lands exactly
            // on a cell boundary — boundaries are the tie case and get their
            // own test.
            let v = step as f32 * 0.1 + 0.03;
            let snapped = rule.token(Vec2::new(v, -v), Footprint::default());
            assert!(
                (snapped.x - reference(v)).abs() < 1e-3,
                "x mismatch at {v}: {} vs {}",
                snapped.x,
                reference(v),
            );
            assert!(
                (snapped.y - reference(-v)).abs() < 1e-3,
                "y mismatch at {v}"
            );
        }
    }

    #[test]
    fn square_cell_boundaries_resolve_deterministically() {
        // A point exactly on the line between two cells has two equally
        // correct answers. Which one it picks matters less than that it always
        // picks the same one: a token re-saved at a boundary must not
        // oscillate between neighbours.
        let rule = SnapRule::new(grid(GridKind::Square, 100.0), true);
        let boundary = Vec2::new(100.0, -100.0);

        let first = rule.token(boundary, Footprint::default());
        for _ in 0..8 {
            assert_eq!(rule.token(boundary, Footprint::default()), first);
        }
        let first_cell = rule.cell(boundary);
        for _ in 0..8 {
            assert_eq!(rule.cell(boundary), first_cell);
        }

        // The two rules now agree on boundaries, and this test was updated
        // deliberately when they were made to — as the version that recorded
        // their disagreement asked a future change to do.
        //
        // Both are `floor`-based in effect: a boundary belongs to the cell it
        // opens. `cell` always was; `token` snapped the footprint's corner with
        // `round`, which in Rust breaks halves *away from zero* and so pulled
        // negative boundaries into the cell below.
        //
        // The earlier note called this "a curiosity rather than a visible bug"
        // on the grounds that a drag rarely lands exactly on a boundary. That
        // was wrong, and worth recording: `token-authoring.spec.ts` drags to
        // (-60, 60) on a 5-unit grid — both coordinates exactly on boundaries —
        // and had been failing on it. Because `round` and `floor` differ only
        // on the negative side, y was right and x was one cell out, which is
        // what made it look like a drag-precision problem rather than a
        // tie-break one.
        assert_eq!(first_cell, Vec2::new(150.0, -50.0));
        assert_eq!(first, Vec2::new(150.0, -50.0));
    }

    #[test]
    fn hex_token_snaps_to_a_hex_centre() {
        for kind in [GridKind::HexPointyTop, GridKind::HexFlatTop] {
            let spec = grid(kind, 128.0);
            let rule = SnapRule::new(spec, true);
            for (q, r) in [(0, 0), (2, -1), (-3, 4)] {
                let centre = spec.cell_center(Cell::new(q, r));
                // Nudged off-centre by well under half a hex, so the answer is
                // unambiguous: it must come back to the same hex's centre.
                let nudged = centre + Vec2::new(12.0, -9.0);
                let snapped = rule.token(nudged, Footprint::default());
                assert!(
                    snapped.distance(centre) < 1e-2,
                    "{kind:?} snapped {nudged:?} to {snapped:?}, expected {centre:?}",
                );
            }
        }
    }

    #[test]
    fn hex_boundaries_resolve_deterministically() {
        // The midpoint between two adjacent hex centres lies exactly on their
        // shared edge. Same requirement as the square case: one stable answer,
        // and it must be one of the two neighbours rather than some third hex
        // (which is what independent per-axis axial rounding would produce).
        let spec = grid(GridKind::HexPointyTop, 128.0);
        let rule = SnapRule::new(spec, true);
        let a = spec.cell_center(Cell::new(0, 0));
        let b = spec.cell_center(Cell::new(1, 0));
        let edge = (a + b) / 2.0;

        let first = rule.token(edge, Footprint::default());
        for _ in 0..8 {
            assert_eq!(rule.token(edge, Footprint::default()), first);
        }
        assert!(
            first.distance(a) < 1e-2 || first.distance(b) < 1e-2,
            "edge point snapped to {first:?}, neither {a:?} nor {b:?}",
        );
    }

    #[test]
    fn hex_orientation_changes_where_a_point_lands() {
        // Guards the FR-025 requirement that snapping honours the grid *type*.
        // If someone ever collapses the two hex orientations into one, or
        // routes hex through the square path, this is what notices.
        let point = Vec2::new(70.0, 40.0);
        let pointy = SnapRule::new(grid(GridKind::HexPointyTop, 128.0), true)
            .token(point, Footprint::default());
        let flat = SnapRule::new(grid(GridKind::HexFlatTop, 128.0), true)
            .token(point, Footprint::default());
        let square =
            SnapRule::new(grid(GridKind::Square, 128.0), true).token(point, Footprint::default());
        assert!(pointy.distance(flat) > 1.0, "{pointy:?} vs {flat:?}");
        assert!(pointy.distance(square) > 1.0, "{pointy:?} vs {square:?}");
    }

    #[test]
    fn snapping_is_idempotent_on_every_grid_kind() {
        // The property that stops a token drifting across saves: the engine
        // re-snaps on every transform change, and the server round-trips the
        // position back in, so `snap(snap(p))` differing from `snap(p)` would
        // walk a token a little further every time it was touched.
        let point = Vec2::new(-62.5, 137.25);
        for kind in [
            GridKind::Square,
            GridKind::HexPointyTop,
            GridKind::HexFlatTop,
            GridKind::Gridless,
        ] {
            let rule = SnapRule::new(grid(kind, 128.0), true);
            for footprint in [
                Footprint::new(0.5),
                Footprint::new(1.0),
                Footprint::new(2.0),
            ] {
                let once = rule.token(point, footprint);
                let twice = rule.token(once, footprint);
                assert!(
                    once.distance(twice) < 1e-3,
                    "{kind:?} at {:?} cells drifted: {once:?} -> {twice:?}",
                    footprint.cells(),
                );
            }
            let once = rule.vertex(point);
            assert!(
                once.distance(rule.vertex(once)) < 1e-3,
                "{kind:?} vertex drifted",
            );
            let once = rule.cell(point);
            assert!(
                once.distance(rule.cell(once)) < 1e-3,
                "{kind:?} cell drifted",
            );
        }
    }

    #[test]
    fn disabled_snapping_passes_positions_through_untouched() {
        // FR-024: the GM can turn it off, and off must mean *exactly* where the
        // user put it — not a smaller snap, not a nudge.
        let point = Vec2::new(-62.5, 137.25);
        for kind in [
            GridKind::Square,
            GridKind::HexPointyTop,
            GridKind::HexFlatTop,
        ] {
            let rule = SnapRule::new(grid(kind, 128.0), false);
            assert!(!rule.is_active());
            assert_eq!(rule.token(point, Footprint::default()), point);
            assert_eq!(rule.cell(point), point);
            assert_eq!(rule.vertex(point), point);
        }
    }

    #[test]
    fn a_gridless_scene_never_snaps_even_with_the_setting_on() {
        // There is no lattice to snap to; inventing one would move content the
        // GM deliberately placed freely.
        let point = Vec2::new(-62.5, 137.25);
        let rule = SnapRule::new(grid(GridKind::Gridless, 128.0), true);
        assert!(!rule.is_active());
        assert_eq!(rule.token(point, Footprint::default()), point);
        assert_eq!(rule.cell(point), point);
        assert_eq!(rule.vertex(point), point);
    }

    #[test]
    fn the_scene_setting_and_the_per_target_flag_are_anded() {
        // The engine already models these separately (`GridSnapEnabled` and
        // `TokenGridBehaviour::snap`). Composing them here keeps the AND in one
        // tested place instead of each call site rediscovering it.
        let spec = grid(GridKind::Square, 128.0);
        assert!(SnapRule::for_target(spec, true, true).is_active());
        assert!(!SnapRule::for_target(spec, true, false).is_active());
        assert!(!SnapRule::for_target(spec, false, true).is_active());
        assert!(!SnapRule::for_target(spec, false, false).is_active());
    }

    #[test]
    fn a_placed_token_lands_where_a_dragged_one_would() {
        // FR-006, stated as an equality rather than as prose. The drag path is
        // `GridSpec::snap_footprint` (see `systems::token_grid`), so asserting
        // the placement rule *is* that call is what makes the two paths
        // impossible to drift apart.
        for kind in [
            GridKind::Square,
            GridKind::HexPointyTop,
            GridKind::HexFlatTop,
        ] {
            let rule = SnapRule::new(grid(kind, 5.0), true);
            for target in [Vec2::new(13.7, -4.2), Vec2::new(-62.5, -192.5), Vec2::ZERO] {
                assert_eq!(
                    rule.token(target, Footprint::default()),
                    rule.grid.snap_footprint(target, Footprint::default()),
                    "{kind:?} diverged at {target:?}",
                );
            }
        }
    }

    #[test]
    fn square_vertices_are_the_lattice_corners_walls_run_between() {
        // Wall endpoints belong on cell *corners*, not centres: a wall drawn
        // down the middle of a row of cells would cut every one of them in
        // half. FR-024 puts walls under the same snapping setting, so they need
        // their own rule, not the token one.
        let rule = SnapRule::new(grid(GridKind::Square, 100.0), true);
        assert_eq!(rule.vertex(Vec2::new(10.0, 90.0)), Vec2::new(0.0, 100.0));
        assert_eq!(rule.vertex(Vec2::new(-10.0, -90.0)), Vec2::new(0.0, -100.0));
        assert_eq!(
            rule.vertex(Vec2::new(-160.0, 240.0)),
            Vec2::new(-200.0, 200.0)
        );
    }

    #[test]
    fn vertex_snapping_respects_a_shifted_origin() {
        // Imported maps anchor the lattice at the map's corner, not the world
        // origin (`GridSpec::anchored_to_map`), so a vertex rule that assumed
        // an origin of zero would be off by half a cell on half the maps.
        let spec = GridSpec {
            kind: GridKind::Square,
            size: 100.0,
            origin: Vec2::new(25.0, -50.0),
        };
        let rule = SnapRule::new(spec, true);
        assert_eq!(rule.vertex(Vec2::new(30.0, -45.0)), Vec2::new(25.0, -50.0));
        assert_eq!(rule.vertex(Vec2::new(90.0, 20.0)), Vec2::new(125.0, 50.0));
    }

    #[test]
    fn hex_vertices_land_on_a_corner_of_the_hex_under_the_point() {
        for kind in [GridKind::HexPointyTop, GridKind::HexFlatTop] {
            let spec = grid(kind, 128.0);
            let rule = SnapRule::new(spec, true);
            let cell = Cell::new(1, -2);
            let centre = spec.cell_center(cell);
            let corners = spec.cell_outline(cell);
            // A point three quarters of the way from the centre toward a
            // corner must resolve to that corner.
            for corner in corners.iter().take(6) {
                let probe = centre + (*corner - centre) * 0.75;
                let snapped = rule.vertex(probe);
                assert!(
                    snapped.distance(*corner) < 1e-2,
                    "{kind:?}: {probe:?} snapped to {snapped:?}, expected {corner:?}",
                );
            }
        }
    }

    #[test]
    fn a_wall_endpoint_and_a_light_use_different_but_stable_rules() {
        // Lights sit in a cell; walls sit on its corners. Both honour the same
        // on/off switch, which is the part FR-024 cares about.
        let spec = grid(GridKind::Square, 100.0);
        let rule = SnapRule::new(spec, true);
        let point = Vec2::new(37.0, 61.0);
        assert_eq!(rule.cell(point), Vec2::new(50.0, 50.0));
        assert_eq!(rule.vertex(point), Vec2::new(0.0, 100.0));
    }

    #[test]
    fn cell_address_reports_the_cell_regardless_of_the_snap_setting() {
        // "Which cell is the cursor over" is a question about the grid, not
        // about snapping — a highlight should still follow the cursor when the
        // GM has snapping off.
        let spec = grid(GridKind::Square, 100.0);
        let point = Vec2::new(137.0, -61.0);
        assert_eq!(
            SnapRule::new(spec, false).cell_address(point),
            SnapRule::new(spec, true).cell_address(point),
        );
        assert_eq!(
            SnapRule::new(spec, true).cell_address(point),
            Cell::new(1, -1)
        );
    }

    #[test]
    fn a_degenerate_grid_size_does_not_produce_nonsense() {
        // `GridSpec::safe_size` already guards division, but the snap rule is
        // the outermost caller and a NaN escaping here would land in a
        // transform and blank the canvas.
        for size in [0.0_f32, -32.0, f32::NAN] {
            let rule = SnapRule::new(grid(GridKind::Square, size), true);
            let snapped = rule.token(Vec2::new(10.0, 10.0), Footprint::default());
            assert!(snapped.is_finite(), "size {size} token -> {snapped:?}");
            let snapped = rule.vertex(Vec2::new(10.0, 10.0));
            assert!(snapped.is_finite(), "size {size} vertex -> {snapped:?}");
        }
    }
}
