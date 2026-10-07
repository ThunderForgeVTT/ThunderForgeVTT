use super::*;

/// A door 200 units long on the x axis, centred on the origin.
fn door(state: DoorState, locked: bool, secret: bool) -> Wall {
    Wall {
        id: "door".to_string(),
        x1: -100.0,
        y1: 0.0,
        x2: 100.0,
        y2: 0.0,
        blocks_vision: true,
        blocks_movement: true,
        door_state: state,
        locked,
        secret,
    }
}

const SQUARE: f32 = 100.0;

fn nobody_near(pointer: Option<Vec2>) -> Proximity<'static> {
    Proximity {
        pointer,
        pointer_reach: ICON_POINTER_REACH_PX,
        tokens: &[],
        square: SQUARE,
    }
}

fn shown(wall: &Wall, near: &Proximity) -> bool {
    icon_shown(wall, &IconPlace::for_wall(wall, SQUARE, 0.0), near)
}

// --- Which icon (FR-007) ---------------------------------------------------

#[test]
fn a_shut_door_shows_the_open_icon() {
    assert_eq!(
        door_icon(&door(DoorState::Closed, false, false)),
        Some(DoorIcon::Open)
    );
}

#[test]
fn an_open_door_shows_the_close_icon() {
    assert_eq!(
        door_icon(&door(DoorState::Open, false, false)),
        Some(DoorIcon::Close)
    );
}

/// Players see the padlock as well as the Game Master: the icon takes no role,
/// so there is no way for the two to be shown different things.
#[test]
fn a_locked_door_shows_the_padlock_shut_or_open() {
    assert_eq!(
        door_icon(&door(DoorState::Closed, true, false)),
        Some(DoorIcon::Padlock)
    );
    assert_eq!(
        door_icon(&door(DoorState::Open, true, false)),
        Some(DoorIcon::Padlock)
    );
}

/// To the table a secret door is wall; an icon on it would say where it is.
#[test]
fn a_secret_door_shows_nothing_in_any_state() {
    for state in [DoorState::Closed, DoorState::Open] {
        for locked in [false, true] {
            assert_eq!(door_icon(&door(state, locked, true)), None);
        }
    }
}

#[test]
fn a_plain_wall_shows_nothing() {
    assert_eq!(door_icon(&door(DoorState::None, false, false)), None);
    assert_eq!(door_icon(&door(DoorState::None, true, false)), None);
}

#[test]
fn the_icons_have_the_names_the_web_app_reads() {
    assert_eq!(DoorIcon::Open.as_id(), "open");
    assert_eq!(DoorIcon::Close.as_id(), "close");
    assert_eq!(DoorIcon::Padlock.as_id(), "padlock");
}

// --- Where it is drawn -----------------------------------------------------

#[test]
fn the_icon_sits_at_the_door_s_midpoint() {
    let mut wall = door(DoorState::Closed, false, false);
    wall.x1 = 0.0;
    wall.x2 = 200.0;
    wall.y1 = 50.0;
    wall.y2 = 50.0;
    let place = IconPlace::for_wall(&wall, SQUARE, 0.0);
    assert_eq!(place.center, Vec2::new(100.0, 50.0));
    assert!((place.side - SQUARE * ICON_GRID_SHARE).abs() < 1e-4);
}

/// A scene without a map keeps the 5-unit grid it was created with; a share
/// of that is a speck no one could press, so the icon keeps a size on screen.
#[test]
fn on_a_tiny_grid_the_icon_keeps_its_size_on_screen() {
    let wall = door(DoorState::Closed, false, false);
    let least = ICON_MIN_SCREEN_PX * 0.5;
    let place = IconPlace::for_wall(&wall, 5.0, least);
    assert!((place.side - least).abs() < 1e-4);
    // A grid large enough to need no floor is not shrunk to it.
    let place = IconPlace::for_wall(&wall, SQUARE, least);
    assert!((place.side - SQUARE * ICON_GRID_SHARE).abs() < 1e-4);
}

// --- When it shows (FR-008) ------------------------------------------------

#[test]
fn nothing_near_shows_nothing() {
    let wall = door(DoorState::Closed, false, false);
    assert!(!shown(&wall, &nobody_near(None)));
    assert!(!shown(&wall, &nobody_near(Some(Vec2::new(0.0, 300.0)))));
}

#[test]
fn the_pointer_within_reach_of_the_door_shows_it() {
    let wall = door(DoorState::Closed, false, false);
    // Over the door far from its middle, 9 units off the line.
    assert!(shown(&wall, &nobody_near(Some(Vec2::new(-90.0, 9.0)))));
    // And not 11 units off it, away from the icon.
    assert!(!shown(&wall, &nobody_near(Some(Vec2::new(-90.0, 11.0)))));
}

/// Moving onto the icon's corner, outside the door's reach, must not make the
/// icon it is about to press disappear.
#[test]
fn the_pointer_on_the_icon_shows_it() {
    let wall = door(DoorState::Closed, false, false);
    let half = SQUARE * ICON_GRID_SHARE / 2.0;
    assert!(half > ICON_POINTER_REACH_PX, "the case this guards");
    let corner = Vec2::new(half - 1.0, half - 1.0);
    assert!(shown(&wall, &nobody_near(Some(corner))));
}

#[test]
fn the_reach_is_on_the_screen_so_it_grows_when_zoomed_out() {
    let wall = door(DoorState::Closed, false, false);
    let far = Some(Vec2::new(-90.0, 25.0));
    assert!(!shown(&wall, &nobody_near(far)));
    let zoomed_out = Proximity {
        pointer_reach: ICON_POINTER_REACH_PX * 3.0,
        ..nobody_near(far)
    };
    assert!(shown(&wall, &zoomed_out));
}

#[test]
fn a_controlled_token_in_the_next_square_shows_it() {
    let wall = door(DoorState::Closed, false, false);
    // A one-square token whose edge is one square from the door.
    let tokens = [NearbyToken {
        center: Vec2::new(0.0, 150.0),
        side: SQUARE,
    }];
    let near = Proximity {
        tokens: &tokens,
        ..nobody_near(None)
    };
    assert!(shown(&wall, &near));
}

#[test]
fn a_controlled_token_two_squares_off_does_not() {
    let wall = door(DoorState::Closed, false, false);
    let tokens = [NearbyToken {
        center: Vec2::new(0.0, 251.0),
        side: SQUARE,
    }];
    let near = Proximity {
        tokens: &tokens,
        ..nobody_near(None)
    };
    assert!(!shown(&wall, &near));
}

/// Past the end of the door counts by distance to its end, not to its line.
#[test]
fn a_token_beyond_the_door_s_end_is_measured_to_the_end() {
    let wall = door(DoorState::Closed, false, false);
    let tokens = [NearbyToken {
        center: Vec2::new(400.0, 0.0),
        side: SQUARE,
    }];
    let near = Proximity {
        tokens: &tokens,
        ..nobody_near(None)
    };
    assert!(!shown(&wall, &near));
}

#[test]
fn distance_to_a_segment() {
    let (a, b) = (Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0));
    assert_eq!(distance_to_segment(Vec2::new(5.0, 3.0), a, b), 3.0);
    assert_eq!(distance_to_segment(Vec2::new(13.0, 4.0), a, b), 5.0);
    assert_eq!(distance_to_segment(Vec2::new(3.0, 4.0), a, a), 5.0);
}
