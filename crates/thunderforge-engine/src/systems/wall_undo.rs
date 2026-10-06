//! Wall undo (T014 of specs/001-bevy-canvas-authoring/tasks.md): Ctrl+Z
//! pops `WallSet`'s undo stack and re-issues the inverse mutation. Moved
//! out of `wall.rs` when spec 078's `Split` arm pushed that file past the
//! length gate; the system is wired by `plugins/wall.rs` as before.

use bevy::prelude::*;
use serde_json::json;

use crate::resources::{IsGameMaster, WallEdit, WallSet};
use crate::systems::wall_draw::{undo_created, undo_split};
use crate::{ActiveWorld, emit_event};

/// T014: wall undo. Ctrl+Z pops `WallSet`'s undo stack and re-issues the
/// inverse mutation through the same outbound-event path a normal edit
/// uses (research.md §4) — applied locally first (optimistic), then
/// emitted so other clients converge once the server confirms it.
pub(crate) fn handle_wall_undo(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut wall_set: ResMut<WallSet>,
    is_gm: Res<IsGameMaster>,
    active_world: Res<ActiveWorld>,
) {
    if !is_gm.0 {
        return;
    }

    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    if !ctrl || !keyboard.just_pressed(KeyCode::KeyZ) {
        return;
    }

    let Some(edit) = wall_set.pop_undo() else {
        return;
    };

    match edit {
        WallEdit::Move {
            wall_id,
            prior_x1,
            prior_y1,
            prior_x2,
            prior_y2,
        } => {
            if let Some(mut wall) = wall_set.get(&wall_id).cloned() {
                wall.x1 = prior_x1;
                wall.y1 = prior_y1;
                wall.x2 = prior_x2;
                wall.y2 = prior_y2;
                wall_set.upsert(wall);
            }
            emit_event(json!({
                "type": "update_wall",
                "wallId": wall_id,
                "changes": { "x1": prior_x1, "y1": prior_y1, "x2": prior_x2, "y2": prior_y2 },
                "worldId": active_world.0,
            }));
        }
        WallEdit::DoorToggle {
            wall_id,
            prior_door_state,
        } => {
            if let Some(mut wall) = wall_set.get(&wall_id).cloned() {
                wall.door_state = prior_door_state;
                wall_set.upsert(wall);
            }
            emit_event(json!({
                "type": "update_wall",
                "wallId": wall_id,
                "changes": { "doorState": prior_door_state.as_str() },
                "worldId": active_world.0,
            }));
        }
        WallEdit::FlagsToggle {
            wall_id,
            prior_blocks_vision,
            prior_blocks_movement,
        } => {
            if let Some(mut wall) = wall_set.get(&wall_id).cloned() {
                wall.blocks_vision = prior_blocks_vision;
                wall.blocks_movement = prior_blocks_movement;
                wall_set.upsert(wall);
            }
            emit_event(json!({
                "type": "update_wall",
                "wallId": wall_id,
                "changes": {
                    "blocksVision": prior_blocks_vision,
                    "blocksMovement": prior_blocks_movement,
                },
                "worldId": active_world.0,
            }));
        }
        WallEdit::Created { endpoints } => {
            undo_created(&mut wall_set, &active_world.0, &endpoints);
        }
        WallEdit::Split {
            wall_id,
            prior,
            remainder,
        } => {
            undo_split(&mut wall_set, &active_world.0, &wall_id, prior, remainder);
        }
        WallEdit::Delete { deleted } => {
            // Re-creates the wall; the server assigns a new id (the
            // original id cannot be resurrected — see the module's
            // scope note on optimistic reconciliation).
            emit_event(json!({
                "type": "create_wall",
                "wall": {
                    "x1": deleted.x1,
                    "y1": deleted.y1,
                    "x2": deleted.x2,
                    "y2": deleted.y2,
                    "blocksVision": deleted.blocks_vision,
                    "blocksMovement": deleted.blocks_movement,
                    "doorState": deleted.door_state.as_str(),
                },
                "worldId": active_world.0,
            }));
        }
    }
}
