use super::*;
use crate::wall::{DoorState, Wall, path_blocked_by};

const CELL: f32 = 10.0;

fn grid() -> GridSpec {
    GridSpec {
        kind: GridKind::Square,
        size: CELL,
        origin: Vec2::ZERO,
    }
}

fn wall(id: &str, x1: f32, y1: f32, x2: f32, y2: f32) -> Wall {
    Wall {
        id: id.to_string(),
        x1,
        y1,
        x2,
        y2,
        blocks_vision: true,
        blocks_movement: true,
        door_state: DoorState::None,
        locked: false,
        secret: false,
    }
}

fn walls(list: Vec<Wall>) -> WallSet {
    let mut set = WallSet::default();
    for w in list {
        set.upsert(w);
    }
    set
}

fn bounded() -> PathOptions {
    PathOptions {
        bounds: Some((Cell::new(-10, -10), Cell::new(20, 20))),
        ..PathOptions::default()
    }
}

/// The centre of cell (q, r).
fn at(q: i32, r: i32) -> Vec2 {
    grid().cell_center(Cell::new(q, r))
}

#[test]
fn an_open_plane_is_a_straight_line() {
    let path = find_path(&grid(), at(0, 0), at(7, 3), &WallSet::default(), &bounded()).unwrap();
    assert_eq!(path, vec![at(0, 0), at(7, 3)]);
}

#[test]
fn the_same_cell_is_the_two_points() {
    let from = Vec2::new(1.0, 2.0);
    let to = Vec2::new(8.0, 9.0);
    let path = find_path(&grid(), from, to, &WallSet::default(), &bounded()).unwrap();
    assert_eq!(path, vec![from, to]);
}

#[test]
fn a_wall_with_a_gap_is_walked_round_through_the_gap() {
    // A wall across x = 50 from y = -100 to y = 40: the gap is below y = 40.
    let set = walls(vec![wall("w", 50.0, -100.0, 50.0, 40.0)]);
    let path = find_path(&grid(), at(0, 0), at(9, 0), &set, &bounded()).unwrap();
    assert!(path_blocked_by(&path, &set).is_none(), "{path:?}");
    assert!(
        path.iter().any(|p| p.y > 40.0),
        "goes through the gap: {path:?}"
    );
    assert_eq!(*path.first().unwrap(), at(0, 0));
    assert_eq!(*path.last().unwrap(), at(9, 0));
}

#[test]
fn the_route_keeps_its_corner() {
    let set = walls(vec![wall("w", 50.0, -100.0, 50.0, 40.0)]);
    let path = find_path(&grid(), at(0, 0), at(9, 0), &set, &bounded()).unwrap();
    assert!(path.len() >= 3, "a corner at least: {path:?}");
    assert!(path.len() <= 4, "smoothed, not a staircase: {path:?}");
}

/// A room of four walls round cells (0,0)..(2,2), with a door on its south
/// side in cell (1,2)'s edge.
fn room(door: DoorState) -> WallSet {
    let mut door_wall = wall("door", 10.0, 30.0, 20.0, 30.0);
    door_wall.door_state = door;
    walls(vec![
        wall("n", 0.0, 0.0, 30.0, 0.0),
        wall("e", 30.0, 0.0, 30.0, 30.0),
        wall("w", 0.0, 0.0, 0.0, 30.0),
        wall("s1", 0.0, 30.0, 10.0, 30.0),
        wall("s2", 20.0, 30.0, 30.0, 30.0),
        door_wall,
    ])
}

#[test]
fn a_shut_door_is_no_way_out() {
    let set = room(DoorState::Closed);
    assert!(find_path(&grid(), at(1, 1), at(1, 6), &set, &bounded()).is_none());
}

#[test]
fn an_open_door_is_the_way_out() {
    let set = room(DoorState::Open);
    let path = find_path(&grid(), at(0, 0), at(5, -3), &set, &bounded()).unwrap();
    assert!(path_blocked_by(&path, &set).is_none(), "{path:?}");
    // Out through the door's cell, below the room, then round it.
    assert!(path.iter().any(|p| p.y > 30.0), "{path:?}");
}

#[test]
fn no_squeezing_between_walls_that_meet_at_a_corner() {
    // Two walls meet at (10,10): one runs left from it, one runs down. A
    // diagonal from cell (0,0) to (1,1) would slip through their corner.
    let set = walls(vec![
        wall("a", -100.0, 10.0, 10.0, 10.0),
        wall("b", 10.0, 10.0, 10.0, 100.0),
    ]);
    let path = find_path(&grid(), at(0, 1), at(1, 0), &set, &bounded());
    // (0,1) is below wall a and left of wall b: boxed in on two sides, and
    // the way to (1,0) is round the far ends, never through the corner.
    if let Some(path) = path {
        assert!(path_blocked_by(&path, &set).is_none(), "{path:?}");
        assert!(path.len() > 2, "{path:?}");
    }
}

#[test]
fn bounds_are_a_fence() {
    // A long wall whose way round lies outside the bounds.
    let set = walls(vec![wall("w", 50.0, -1000.0, 50.0, 40.0)]);
    let tight = PathOptions {
        bounds: Some((Cell::new(0, 0), Cell::new(9, 3))),
        ..PathOptions::default()
    };
    assert!(find_path(&grid(), at(0, 0), at(9, 0), &set, &tight).is_none());
    let loose = PathOptions {
        bounds: Some((Cell::new(0, 0), Cell::new(9, 5))),
        ..PathOptions::default()
    };
    assert!(find_path(&grid(), at(0, 0), at(9, 0), &set, &loose).is_some());
}

#[test]
fn blocked_cells_are_walked_round() {
    let options = PathOptions {
        blocked: [Cell::new(2, 0), Cell::new(2, 1), Cell::new(2, -1)]
            .into_iter()
            .collect(),
        ..bounded()
    };
    let path = find_path(&grid(), at(0, 0), at(4, 0), &WallSet::default(), &options).unwrap();
    for leg in path.windows(2) {
        for i in 0..=20 {
            let p = leg[0].lerp(leg[1], i as f32 / 20.0);
            assert!(
                !options.blocked.contains(&grid().world_to_cell(p)),
                "{path:?}"
            );
        }
    }
}

#[test]
fn the_goal_cell_is_never_blocked() {
    let options = PathOptions {
        blocked: [Cell::new(4, 0)].into_iter().collect(),
        ..bounded()
    };
    assert!(find_path(&grid(), at(0, 0), at(4, 0), &WallSet::default(), &options).is_some());
}

#[test]
fn the_budget_is_honoured() {
    let set = room(DoorState::Closed);
    let options = PathOptions {
        bounds: None,
        max_nodes: 500,
        ..PathOptions::default()
    };
    // Unbounded and unreachable: only the budget stops it.
    assert!(find_path(&grid(), at(1, 1), at(40, 40), &set, &options).is_none());
}

#[test]
fn the_same_question_gets_the_same_answer() {
    let set = walls(vec![
        wall("a", 50.0, -100.0, 50.0, 40.0),
        wall("b", 90.0, 0.0, 90.0, 200.0),
    ]);
    let first = find_path(&grid(), at(0, 0), at(12, 3), &set, &bounded());
    for _ in 0..5 {
        assert_eq!(
            find_path(&grid(), at(0, 0), at(12, 3), &set, &bounded()),
            first
        );
    }
}

#[test]
fn a_hex_grid_has_no_path_yet() {
    let hex = GridSpec {
        kind: GridKind::HexPointyTop,
        ..grid()
    };
    assert!(find_path(&hex, at(0, 0), at(3, 0), &WallSet::default(), &bounded()).is_none());
}
