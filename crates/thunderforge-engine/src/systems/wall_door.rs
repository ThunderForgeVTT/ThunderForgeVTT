//! What a click with the Door primitive does to a wall (spec 077 FR-016).
//!
//! Split from `wall` so the rule can be tested without a Bevy world. Spec 085
//! changed it: a wall the GM has hidden from the table stays hidden when it
//! becomes a door. Hiding is its own choice, made with its own checkbox, and
//! converting a wall is not a reason to reveal it.

use serde_json::{Value, json};

use crate::resources::{DoorState, Wall};

/// The wall after the click, and the changes to send for it.
pub(crate) fn door_click_update(wall: &Wall) -> (Wall, Value) {
    let mut updated = wall.clone();
    updated.door_state = DoorState::Closed;
    updated.locked = false;
    (updated, json!({ "doorState": "closed", "locked": false }))
}

#[cfg(test)]
#[path = "wall_door_tests.rs"]
mod tests;
