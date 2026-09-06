use super::*;

#[test]
fn distance_point_to_segment_on_the_line() {
    let d = distance_point_to_segment(
        Vec2::new(5.0, 0.0),
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
    );
    assert!(d.abs() < 1e-5);
}

#[test]
fn distance_point_to_segment_perpendicular() {
    let d = distance_point_to_segment(
        Vec2::new(5.0, 3.0),
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
    );
    assert!((d - 3.0).abs() < 1e-5);
}

#[test]
fn distance_point_to_segment_beyond_endpoint() {
    let d = distance_point_to_segment(
        Vec2::new(15.0, 0.0),
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
    );
    assert!((d - 5.0).abs() < 1e-5);
}

#[test]
fn distance_point_to_segment_degenerate_segment() {
    // Zero-length segment: distance is just distance to the point.
    let d = distance_point_to_segment(
        Vec2::new(3.0, 4.0),
        Vec2::new(0.0, 0.0),
        Vec2::new(0.0, 0.0),
    );
    assert!((d - 5.0).abs() < 1e-5);
}

#[test]
fn wall_color_prioritizes_selection_over_door() {
    let wall = Wall {
        id: "w1".to_string(),
        x1: 0.0,
        y1: 0.0,
        x2: 1.0,
        y2: 1.0,
        blocks_vision: true,
        blocks_movement: false,
        door_state: DoorState::Closed,
        locked: false,
        secret: false,
    };
    assert_eq!(wall_color(&wall, true), SELECTED_COLOR);
    assert_eq!(wall_color(&wall, false), DOOR_COLOR);
}
