//! `covered_cells`, `footprint_points` and `footprint_distance` (spec 046
//! FR-035, research R11).
//!
//! Every square-grid case uses an origin off the world origin, so a test that
//! passes because everything happens to be centred on zero is caught.

use super::*;

const SIZE: f32 = 100.0;

fn square() -> GridSpec {
    GridSpec {
        kind: GridKind::Square,
        size: SIZE,
        origin: Vec2::new(-250.0, -450.0),
    }
}

/// Where a token of `cells` a side, snapped with its lower-left cell at
/// `(q, r)`, has its centre.
fn placed(grid: &GridSpec, q: i32, r: i32, cells: f32) -> Vec2 {
    let corner = grid.origin + Vec2::new(q as f32 * grid.size, r as f32 * grid.size);
    let centre = corner + Vec2::splat(cells * grid.size / 2.0);
    // The same centre the engine's snapping gives it.
    assert_eq!(grid.snap_footprint(centre, Footprint::new(cells)), centre);
    centre
}

fn fp(cells: f32) -> Footprint {
    Footprint::new(cells)
}

// --- covered cells ----------------------------------------------------------

#[test]
fn a_one_cell_token_covers_its_own_cell() {
    let grid = square();
    let at = placed(&grid, 3, -2, 1.0);
    assert_eq!(
        grid.covered_cells(at, fp(1.0)),
        CoveredCells {
            min: Cell::new(3, -2),
            max: Cell::new(3, -2)
        }
    );
    assert_eq!(grid.covered_cells(at, fp(1.0)).min, grid.world_to_cell(at));
}

#[test]
fn a_large_token_covers_the_four_cells_it_is_drawn_over() {
    let grid = square();
    let at = placed(&grid, 3, -2, 2.0);
    assert_eq!(
        grid.covered_cells(at, fp(2.0)),
        CoveredCells {
            min: Cell::new(3, -2),
            max: Cell::new(4, -1)
        }
    );
    assert_eq!(grid.footprint_points(at, fp(2.0)).len(), 4);
}

#[test]
fn a_gargantuan_token_covers_sixteen() {
    let grid = square();
    let at = placed(&grid, 0, 0, 4.0);
    let block = grid.covered_cells(at, fp(4.0));
    assert_eq!(block.min, Cell::new(0, 0));
    assert_eq!(block.max, Cell::new(3, 3));
    let points = grid.footprint_points(at, fp(4.0));
    assert_eq!(points.len(), 16);
    for point in points {
        let cell = grid.world_to_cell(point);
        assert!((0..=3).contains(&cell.q) && (0..=3).contains(&cell.r));
        assert_eq!(grid.cell_center(cell), point, "a point is a cell's centre");
    }
}

#[test]
fn an_unsnapped_token_covers_the_block_it_mostly_overlaps() {
    let grid = square();
    // A 2×2 dragged 0.4 of a cell right of the (3,-2) block.
    let at = placed(&grid, 3, -2, 2.0) + Vec2::new(0.4 * SIZE, 0.0);
    assert_eq!(grid.covered_cells(at, fp(2.0)).min, Cell::new(3, -2));
    // 0.6 of a cell: it now mostly overlaps the block one to the right.
    let at = placed(&grid, 3, -2, 2.0) + Vec2::new(0.6 * SIZE, 0.0);
    assert_eq!(grid.covered_cells(at, fp(2.0)).min, Cell::new(4, -2));
}

#[test]
fn a_tiny_token_covers_the_cell_it_stands_in_and_is_its_own_point() {
    let grid = square();
    let at = grid.cell_center(Cell::new(1, 1)) + Vec2::new(20.0, -20.0);
    let block = grid.covered_cells(at, fp(0.5));
    assert_eq!(block.min, Cell::new(1, 1));
    assert_eq!(block.max, Cell::new(1, 1));
    assert_eq!(grid.footprint_points(at, fp(0.5)), vec![at]);
}

// --- distance on a square grid ---------------------------------------------

#[test]
fn two_medium_creatures_side_by_side_are_one_apart() {
    let grid = square();
    let hero = placed(&grid, 0, 0, 1.0);
    for (q, r) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1)] {
        let other = placed(&grid, q, r, 1.0);
        assert_eq!(
            grid.footprint_distance(hero, fp(1.0), other, fp(1.0)),
            1.0,
            "({q},{r})"
        );
    }
}

#[test]
fn distance_is_chebyshev_five_five_five() {
    let grid = square();
    let hero = placed(&grid, 0, 0, 1.0);
    assert_eq!(
        grid.footprint_distance(hero, fp(1.0), placed(&grid, 4, 0, 1.0), fp(1.0)),
        4.0
    );
    assert_eq!(
        grid.footprint_distance(hero, fp(1.0), placed(&grid, 4, 3, 1.0), fp(1.0)),
        4.0
    );
    assert_eq!(
        grid.footprint_distance(hero, fp(1.0), placed(&grid, -2, 5, 1.0), fp(1.0)),
        5.0
    );
    // The grid's own cell distance agrees for one-cell tokens.
    assert_eq!(grid.cell_distance(Cell::new(0, 0), Cell::new(-2, 5)), 5);
}

#[test]
fn a_large_ogre_is_adjacent_from_any_of_its_four_squares() {
    let grid = square();
    // The ogre fills (0,0)..(1,1).
    let ogre = placed(&grid, 0, 0, 2.0);
    // Every cell in the ring around the block, corners included.
    let mut ring = Vec::new();
    for q in -1..=2 {
        for r in -1..=2 {
            if !((0..=1).contains(&q) && (0..=1).contains(&r)) {
                ring.push((q, r));
            }
        }
    }
    assert_eq!(ring.len(), 12);
    for (q, r) in ring {
        let hero = placed(&grid, q, r, 1.0);
        assert_eq!(
            grid.footprint_distance(hero, fp(1.0), ogre, fp(2.0)),
            1.0,
            "a hero at ({q},{r}) is adjacent to the ogre"
        );
        assert_eq!(
            grid.footprint_distance(ogre, fp(2.0), hero, fp(1.0)),
            1.0,
            "and it is symmetric"
        );
    }
    // Centre to centre, a corner hero is more than two cells away: a
    // centre-based measure would not call it adjacent.
    let corner_hero = placed(&grid, 2, 2, 1.0);
    assert!(ogre.distance(corner_hero) > 2.0 * SIZE);
}

#[test]
fn four_squares_from_a_large_ogre_is_four() {
    let grid = square();
    let ogre = placed(&grid, 0, 0, 2.0);
    // Its nearest column is 1; a hero in column 5 is four columns on.
    let hero = placed(&grid, 5, 0, 1.0);
    assert_eq!(grid.footprint_distance(hero, fp(1.0), ogre, fp(2.0)), 4.0);
    let hero = placed(&grid, -4, 1, 1.0);
    assert_eq!(grid.footprint_distance(hero, fp(1.0), ogre, fp(2.0)), 4.0);
}

#[test]
fn two_gargantuan_creatures_measure_between_their_nearest_edges() {
    let grid = square();
    let a = placed(&grid, 0, 0, 4.0); // (0..3, 0..3)
    let b = placed(&grid, 4, 2, 4.0); // (4..7, 2..5)
    assert_eq!(grid.footprint_distance(a, fp(4.0), b, fp(4.0)), 1.0);
    let c = placed(&grid, 10, -8, 4.0); // (10..13, -8..-5)
    // Columns: 10 - 3 = 7; rows: 0 - (-5) = 5.
    assert_eq!(grid.footprint_distance(a, fp(4.0), c, fp(4.0)), 7.0);
}

#[test]
fn overlapping_creatures_are_no_distance_apart() {
    let grid = square();
    let ogre = placed(&grid, 0, 0, 2.0);
    let under_it = placed(&grid, 1, 1, 1.0);
    assert_eq!(
        grid.footprint_distance(ogre, fp(2.0), under_it, fp(1.0)),
        0.0
    );
    assert_eq!(grid.footprint_distance(ogre, fp(2.0), ogre, fp(2.0)), 0.0);
}

// --- hex --------------------------------------------------------------------

#[test]
fn on_hexes_a_footprint_is_its_centre_hex() {
    for kind in [GridKind::HexPointyTop, GridKind::HexFlatTop] {
        let grid = GridSpec {
            kind,
            size: 90.0,
            origin: Vec2::new(13.0, -7.0),
        };
        let a = grid.cell_center(Cell::new(0, 0));
        let b = grid.cell_center(Cell::new(3, -1));
        let expected = grid.cell_distance(Cell::new(0, 0), Cell::new(3, -1)) as f32;
        assert_eq!(expected, 3.0);
        // Size makes no difference: a Large creature on hexes is still one hex.
        for (fa, fb) in [(1.0, 1.0), (2.0, 1.0), (4.0, 3.0)] {
            assert_eq!(
                grid.footprint_distance(a, fp(fa), b, fp(fb)),
                expected,
                "{kind:?} {fa}/{fb}"
            );
        }
        let neighbour = grid.cell_center(Cell::new(1, 0));
        assert_eq!(grid.footprint_distance(a, fp(2.0), neighbour, fp(1.0)), 1.0);
        assert_eq!(grid.footprint_points(a, fp(2.0)), vec![a]);
    }
}

// --- gridless ---------------------------------------------------------------

fn gridless() -> GridSpec {
    GridSpec {
        kind: GridKind::Gridless,
        size: 50.0,
        origin: Vec2::ZERO,
    }
}

#[test]
fn gridless_one_cell_creatures_read_their_euclidean_distance_in_cells() {
    let grid = gridless();
    let a = Vec2::new(0.0, 0.0);
    let b = Vec2::new(150.0, 200.0); // 250 units: five cells
    assert!((grid.footprint_distance(a, fp(1.0), b, fp(1.0)) - 5.0).abs() < 1e-4);
    // Not Chebyshev: a diagonal is longer than its longer side.
    let diagonal = Vec2::new(200.0, 200.0);
    let d = grid.footprint_distance(a, fp(1.0), diagonal, fp(1.0));
    assert!((d - 4.0 * 2.0_f32.sqrt()).abs() < 1e-3, "{d}");
}

#[test]
fn gridless_touching_creatures_are_one_apart_whatever_their_size() {
    let grid = gridless();
    let ogre = Vec2::new(0.0, 0.0);
    // A 2-cell ogre's edge is 50 from its centre; a 1-cell hero touching it
    // has its centre 75 away.
    let hero = Vec2::new(75.0, 0.0);
    assert!((grid.footprint_distance(ogre, fp(2.0), hero, fp(1.0)) - 1.0).abs() < 1e-4);
    // Two ogres touching: 100 apart.
    let other = Vec2::new(0.0, 100.0);
    assert!((grid.footprint_distance(ogre, fp(2.0), other, fp(2.0)) - 1.0).abs() < 1e-4);
    // Never negative, even piled on top of each other.
    assert_eq!(grid.footprint_distance(ogre, fp(4.0), ogre, fp(4.0)), 0.0);
}

#[test]
fn gridless_points_are_a_lattice_across_the_footprint() {
    let grid = gridless();
    let at = Vec2::new(10.0, 10.0);
    assert_eq!(grid.footprint_points(at, fp(1.0)), vec![at]);
    let points = grid.footprint_points(at, fp(2.0));
    assert_eq!(points.len(), 4);
    for expected in [
        Vec2::new(-15.0, -15.0),
        Vec2::new(35.0, -15.0),
        Vec2::new(-15.0, 35.0),
        Vec2::new(35.0, 35.0),
    ] {
        assert!(points.contains(&expected), "{expected:?} in {points:?}");
    }
}
