//! Spec 088 FR-090: the fixtures of data-model.md, mirrored in the demo's
//! `mapImport.test.ts`.

use super::*;

/// The ambush map as stored: 48 x 27 cells, 85 px a cell, so the art is
/// 4080 x 2295 and runs from (-2040, 1147.5) to (2040, -1147.5).
const AMBUSH: ScenePlacement = ScenePlacement {
    grid_size: 85.0,
    width: 4080.0,
    height: 2295.0,
};
const W: f64 = 2040.0;
const H: f64 = 1147.5;

fn file_wall(x1: f64, y1: f64, x2: f64, y2: f64) -> WallInsert {
    WallInsert {
        x1,
        y1,
        x2,
        y2,
        blocks_vision: true,
        blocks_movement: true,
        door_state: "none",
        perimeter: false,
    }
}

/// A wall's ends, rounded to a tenth of a pixel and put in one order, so
/// two walls along the same stretch compare equal whichever way they run.
fn ends(wall: &WallInsert) -> [i64; 4] {
    let r = |v: f64| (v * 10.0).round() as i64;
    let (a, b) = ((r(wall.x1), r(wall.y1)), (r(wall.x2), r(wall.y2)));
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    [a.0, a.1, b.0, b.1]
}

fn sorted(walls: &[WallInsert]) -> Vec<[i64; 4]> {
    let mut all: Vec<_> = walls.iter().map(ends).collect();
    all.sort();
    all
}

fn full_edges() -> Vec<[i64; 4]> {
    sorted(&[
        file_wall(-W, H, W, H),
        file_wall(W, H, W, -H),
        file_wall(-W, -H, W, -H),
        file_wall(-W, -H, -W, H),
    ])
}

#[test]
fn a_map_with_no_walls_gets_its_four_full_edges() {
    let walls = perimeter_walls(&AMBUSH, &[]);
    assert_eq!(sorted(&walls), full_edges());
}

#[test]
fn every_edge_wall_blocks_both_and_is_marked() {
    for wall in perimeter_walls(&AMBUSH, &[]) {
        assert!(wall.blocks_vision && wall.blocks_movement);
        assert_eq!(wall.door_state, "none");
        assert!(wall.perimeter, "{wall:?} is not marked");
    }
}

#[test]
fn a_file_wall_along_the_whole_top_leaves_three() {
    let walls = perimeter_walls(&AMBUSH, &[file_wall(W, H, -W, H)]);
    assert_eq!(walls.len(), 3);
    assert!(walls.iter().all(|wall| !(wall.y1 == H && wall.y2 == H)));
}

#[test]
fn a_file_wall_along_the_middle_third_of_the_left_splits_it_in_two() {
    let third = 2.0 * H / 3.0;
    let walls = perimeter_walls(&AMBUSH, &[file_wall(-W, -H + third, -W, H - third)]);
    assert_eq!(walls.len(), 5);
    let left: Vec<_> = walls
        .iter()
        .filter(|wall| wall.x1 == -W && wall.x2 == -W)
        .map(ends)
        .collect();
    let r = |v: f64| (v * 10.0).round() as i64;
    let mut left = left;
    left.sort();
    assert_eq!(
        left,
        vec![
            [r(-W), r(-H), r(-W), r(-H + third)],
            [r(-W), r(H - third), r(-W), r(H)],
        ]
    );
}

#[test]
fn a_wall_crossing_an_edge_at_an_angle_covers_nothing() {
    let walls = perimeter_walls(&AMBUSH, &[file_wall(-100.0, H - 50.0, 100.0, H + 50.0)]);
    assert_eq!(sorted(&walls), full_edges());
}

#[test]
fn two_overlapping_file_walls_on_one_edge_are_merged() {
    // Bottom edge: [-W, 0] and [-500, 500] overlap; the gap left is [500, W].
    let walls = perimeter_walls(
        &AMBUSH,
        &[file_wall(-W, -H, 0.0, -H), file_wall(-500.0, -H, 500.0, -H)],
    );
    assert_eq!(walls.len(), 4);
    let bottom: Vec<_> = walls
        .iter()
        .filter(|wall| wall.y1 == -H && wall.y2 == -H)
        .collect();
    assert_eq!(bottom.len(), 1);
    assert_eq!(ends(bottom[0]), ends(&file_wall(500.0, -H, W, -H)));
}

#[test]
fn a_wall_a_third_of_a_pixel_inside_the_edge_covers_it() {
    let walls = perimeter_walls(&AMBUSH, &[file_wall(-W, H - 0.3, W, H - 0.3)]);
    assert_eq!(walls.len(), 3);
}

#[test]
fn a_wall_two_pixels_inside_the_edge_does_not() {
    let walls = perimeter_walls(&AMBUSH, &[file_wall(-W, H - 2.0, W, H - 2.0)]);
    assert_eq!(sorted(&walls), full_edges());
}

#[test]
fn a_gap_narrower_than_the_tolerance_is_not_walled() {
    let walls = perimeter_walls(
        &AMBUSH,
        &[file_wall(-W, H, -0.2, H), file_wall(0.1, H, W, H)],
    );
    assert_eq!(walls.len(), 3);
}

#[test]
fn a_wall_on_the_bounds_is_found_again_and_a_moved_one_is_not() {
    assert!(lies_on_bounds(&file_wall(-W, H, W, H), 4080.0, 2295.0));
    assert!(lies_on_bounds(
        &file_wall(W, 0.0, W + 0.4, -H),
        4080.0,
        2295.0
    ));
    // Moved in from the edge.
    assert!(!lies_on_bounds(
        &file_wall(-W, H - 40.0, W, H - 40.0),
        4080.0,
        2295.0
    ));
    // On the line of the edge, but past its end.
    assert!(!lies_on_bounds(
        &file_wall(W, H, W + 300.0, H),
        4080.0,
        2295.0
    ));
    // One end on one edge and the other on another: a diagonal across a corner.
    assert!(!lies_on_bounds(&file_wall(-W, 0.0, 0.0, H), 4080.0, 2295.0));
}

#[test]
fn the_mark_is_read_back_from_a_walls_metadata() {
    assert!(is_marked(Some(&perimeter_metadata())));
    assert!(!is_marked(None));
    assert!(!is_marked(Some(&serde_json::json!({ "perimeter": false }))));
    assert!(!is_marked(Some(&serde_json::json!({ "other": true }))));
}
