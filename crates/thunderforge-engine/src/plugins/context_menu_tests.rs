use super::*;

fn wall(id: &str, y: f32, door_state: DoorState) -> Wall {
    Wall {
        id: id.to_string(),
        x1: 0.0,
        y1: y,
        x2: 100.0,
        y2: y,
        blocks_vision: true,
        blocks_movement: true,
        door_state,
        locked: false,
        secret: false,
    }
}

fn id_of(found: Option<&Wall>) -> Option<&str> {
    found.map(|wall| wall.id.as_str())
}

#[test]
fn a_game_master_is_asking_about_the_wall_within_reach() {
    let walls = [
        wall("near", 0.0, DoorState::None),
        wall("far", 50.0, DoorState::None),
    ];
    assert_eq!(
        id_of(wall_under(&walls, Vec2::new(40.0, 4.0), 10.0, true)),
        Some("near")
    );
    assert_eq!(
        id_of(wall_under(&walls, Vec2::new(40.0, 25.0), 10.0, true)),
        None
    );
}

#[test]
fn a_player_is_only_ever_asking_about_a_door_they_are_shown() {
    let mut secret = wall("secret", 20.0, DoorState::Closed);
    secret.secret = true;
    let walls = [
        wall("plain", 0.0, DoorState::None),
        secret,
        wall("door", 40.0, DoorState::Closed),
    ];

    assert_eq!(
        id_of(wall_under(&walls, Vec2::new(40.0, 0.0), 10.0, false)),
        None
    );
    assert_eq!(
        id_of(wall_under(&walls, Vec2::new(40.0, 20.0), 10.0, false)),
        None
    );
    assert_eq!(
        id_of(wall_under(&walls, Vec2::new(40.0, 40.0), 10.0, false)),
        Some("door")
    );
    // The Game Master is shown it, so the Game Master may ask.
    assert_eq!(
        id_of(wall_under(&walls, Vec2::new(40.0, 20.0), 10.0, true)),
        Some("secret")
    );
}

#[test]
fn a_door_beats_the_wall_it_is_set_in() {
    let walls = [
        wall("plain", 0.0, DoorState::None),
        wall("door", 6.0, DoorState::Open),
    ];
    // Nearer the plain wall, and still about the door.
    assert_eq!(
        id_of(wall_under(&walls, Vec2::new(40.0, 1.0), 10.0, true)),
        Some("door")
    );
}
