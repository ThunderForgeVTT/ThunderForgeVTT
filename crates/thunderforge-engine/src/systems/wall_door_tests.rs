use serde_json::json;

use super::*;
use crate::resources::DoorState;

fn hidden_plain_wall() -> Wall {
    Wall {
        id: "w1".to_string(),
        x1: 0.0,
        y1: 0.0,
        x2: 100.0,
        y2: 0.0,
        blocks_vision: true,
        blocks_movement: true,
        door_state: DoorState::None,
        locked: true,
        secret: true,
    }
}

#[test]
fn a_hidden_wall_made_a_door_stays_hidden() {
    let (updated, _) = door_click_update(&hidden_plain_wall());

    assert!(updated.secret);
    assert_eq!(updated.door_state, DoorState::Closed);
    assert!(!updated.locked);
}

#[test]
fn the_door_click_changes_only_the_door_state_and_the_lock() {
    let (_, changes) = door_click_update(&hidden_plain_wall());

    assert_eq!(changes, json!({ "doorState": "closed", "locked": false }));
}

#[test]
fn a_visible_wall_made_a_door_stays_visible() {
    let mut wall = hidden_plain_wall();
    wall.secret = false;
    let (updated, _) = door_click_update(&wall);

    assert!(!updated.secret);
}
