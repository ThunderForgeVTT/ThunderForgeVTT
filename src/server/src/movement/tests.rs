use super::*;
use thunderforge_canvas_core::wall::Wall;

/// A wall across the board at y = 0 that stops a token.
fn barrier(id: &str) -> Wall {
    Wall {
        id: id.to_string(),
        x1: -50.0,
        y1: 0.0,
        x2: 50.0,
        y2: 0.0,
        blocks_vision: true,
        blocks_movement: true,
        door_state: DoorState::None,
        locked: false,
        secret: false,
    }
}

fn scene(walls: Vec<Wall>) -> WallSet {
    let mut set = WallSet::default();
    for w in walls {
        set.upsert(w);
    }
    set
}

fn south() -> Vec2 {
    Vec2::new(0.0, -10.0)
}

fn north() -> Vec2 {
    Vec2::new(0.0, 10.0)
}

#[test]
fn a_straight_move_across_a_wall_is_refused_and_names_it() {
    let walls = scene(vec![barrier("w-1")]);
    assert_eq!(
        judge(south(), north(), None, &walls),
        Verdict::Refused(Refusal::Wall {
            wall_id: String::from("w-1")
        })
    );
}

#[test]
fn a_move_that_crosses_nothing_is_allowed() {
    let walls = scene(vec![barrier("w-1")]);
    assert_eq!(
        judge(
            Vec2::new(-20.0, -10.0),
            Vec2::new(20.0, -10.0),
            None,
            &walls
        ),
        Verdict::Allowed
    );
}

#[test]
fn an_empty_scene_refuses_nothing() {
    assert_eq!(
        judge(south(), north(), None, &scene(vec![])),
        Verdict::Allowed
    );
}

#[test]
fn a_route_the_long_way_round_is_allowed_though_its_endpoints_are_not() {
    let mut short_wall = barrier("w-1");
    short_wall.x1 = -10.0;
    short_wall.x2 = 10.0;
    let walls = scene(vec![short_wall]);

    let around = [Vec2::new(20.0, -10.0), Vec2::new(20.0, 10.0)];
    assert_eq!(
        judge(south(), north(), Some(&around), &walls),
        Verdict::Allowed
    );
    assert!(
        !judge(south(), north(), None, &walls).is_allowed(),
        "the straight line between the same two points does cross — which is \
         the whole reason a path is sent"
    );
}

#[test]
fn a_route_that_crosses_somewhere_in_its_middle_is_refused() {
    let walls = scene(vec![barrier("w-1")]);
    // Both ends south of the wall: the endpoints alone read as a legal move.
    let there_and_back = [Vec2::new(-10.0, 10.0)];
    assert_eq!(
        judge(
            Vec2::new(-10.0, -10.0),
            Vec2::new(10.0, -10.0),
            Some(&there_and_back),
            &walls
        ),
        Verdict::Refused(Refusal::Wall {
            wall_id: String::from("w-1")
        })
    );
}

#[test]
fn a_route_that_does_not_start_where_the_token_stands_is_still_judged() {
    // The attack this anchoring exists for: an innocent little route drawn
    // somewhere harmless, sent as cover for a move that crosses a wall. The
    // route's own legs are clear; the join from the token's real position to
    // the first of them is not.
    let walls = scene(vec![barrier("w-1")]);
    let elsewhere = [Vec2::new(30.0, 10.0), Vec2::new(20.0, 10.0)];
    assert_eq!(
        judge(south(), north(), Some(&elsewhere), &walls),
        Verdict::Refused(Refusal::Wall {
            wall_id: String::from("w-1")
        })
    );
}

#[test]
fn a_route_longer_than_the_bound_is_refused_as_malformed() {
    let walls = scene(vec![]);
    // Clear of every wall, so the only possible objection is its length.
    let too_long: Vec<Vec2> = (0..=MAX_PATH_POINTS)
        .map(|i| Vec2::new(i as f32, -10.0))
        .collect();
    assert_eq!(too_long.len(), MAX_PATH_POINTS + 1);
    assert_eq!(
        judge(south(), south(), Some(&too_long), &walls),
        Verdict::Refused(Refusal::MalformedPath)
    );

    let just_within: Vec<Vec2> = too_long[..MAX_PATH_POINTS].to_vec();
    assert_eq!(
        judge(south(), south(), Some(&just_within), &walls),
        Verdict::Allowed
    );
}

#[test]
fn a_route_with_a_coordinate_that_is_not_a_number_is_refused() {
    // NaN compares false against everything, so this does not merely give a
    // strange answer — it slips past the intersection test entirely, and a
    // move carrying one would be allowed through any wall.
    let walls = scene(vec![barrier("w-1")]);
    for poison in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            judge(south(), north(), Some(&[Vec2::new(poison, 0.0)]), &walls),
            Verdict::Refused(Refusal::MalformedPath),
            "a path carrying {poison} must be refused as malformed"
        );
    }
}

#[test]
fn an_empty_route_is_judged_as_the_straight_line_it_describes() {
    // Not the same as sending no path at all in shape, but it must reach the
    // same verdict: an empty route claims nothing, so the endpoints are all
    // there is to go on.
    let walls = scene(vec![barrier("w-1")]);
    assert_eq!(
        judge(south(), north(), Some(&[]), &walls),
        Verdict::Refused(Refusal::Wall {
            wall_id: String::from("w-1")
        })
    );
}

#[test]
fn an_open_door_is_not_a_wall_and_a_closed_one_is() {
    let mut door = barrier("door-1");
    door.door_state = DoorState::Open;
    assert_eq!(
        judge(south(), north(), None, &scene(vec![door.clone()])),
        Verdict::Allowed
    );

    door.door_state = DoorState::Closed;
    assert_eq!(
        judge(south(), north(), None, &scene(vec![door])),
        Verdict::Refused(Refusal::Wall {
            wall_id: String::from("door-1")
        })
    );
}

#[test]
fn a_wall_that_blocks_only_sight_does_not_refuse_a_move() {
    let mut window = barrier("window-1");
    window.blocks_movement = false;
    assert_eq!(
        judge(south(), north(), None, &scene(vec![window])),
        Verdict::Allowed
    );
}

#[test]
fn what_a_player_is_told_says_nothing_about_doors() {
    // FR-019: a closed secret door stops a player like any wall, and the
    // refusal must not be how they find out there is a door there.
    let lowered = BLOCKED_MESSAGE.to_lowercase();
    assert!(!lowered.contains("door"), "{BLOCKED_MESSAGE}");
    assert!(!lowered.contains("secret"), "{BLOCKED_MESSAGE}");
    assert!(!lowered.contains("lock"), "{BLOCKED_MESSAGE}");
}
