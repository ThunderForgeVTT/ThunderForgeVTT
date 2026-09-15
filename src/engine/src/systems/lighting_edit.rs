//! Editing a placed light once it is on the board: resizing its two
//! reaches, toggling and deleting it, undoing those, and showing the Game
//! Master the reaches being edited.
//!
//! Split from `systems/lighting.rs`, which places and draws lights and lights
//! the board, to keep that file within the length limit. Wiring: see
//! `plugins/lighting.rs`'s `LightingPlugin`.

use std::collections::HashMap;

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use serde_json::json;

use crate::plugins::camera::read_wheel_notches;
use crate::resources::{IsGameMaster, LightEdit, LightSet, SelectedLight};
use crate::systems::lighting::{
    MIN_LIGHT_RADIUS, RESIZE_STEP, SELECTED_LIGHT_COLOR, effective_light_position,
    report_selected_light,
};
use crate::{ActiveWorld, TokenIdentity, emit_event};

/// T037: scroll-wheel resize control for the selected light, GM-only.
///
/// Spec 045 FR-061: a light has two reaches, and the wheel edits both. A plain
/// scroll moves the **dim** reach, the light's outer edge, and a bright reach
/// it would fall inside of comes in with it; **Shift**+scroll moves the
/// **bright** reach, never past the dim one. The Lights panel sets the same
/// two numbers in the system's units.
///
/// Clamped at `MIN_LIGHT_RADIUS` so scrolling can never produce a
/// zero-or-negative radius (T041 applies to resize as well as creation).
pub(crate) fn handle_light_resize(
    mut wheel_events: MessageReader<MouseWheel>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    mut light_set: ResMut<LightSet>,
    selected_light: Res<SelectedLight>,
    is_gm: Res<IsGameMaster>,
    active_world: Res<ActiveWorld>,
) {
    if !is_gm.0 {
        wheel_events.clear();
        return;
    }

    // Notches, not raw deltas — see `read_wheel_notches`. Unnormalised, a
    // single browser wheel notch was 100 units of radius change here for the
    // same reason it was a hundred zoom steps in the camera.
    let scroll = read_wheel_notches(&mut wheel_events);
    if scroll == 0.0 {
        return;
    }

    let Some(light_id) = selected_light.get_selected().cloned() else {
        return;
    };

    let Some(light) = light_set.get(&light_id).cloned() else {
        return;
    };

    // A carried light is its character's sheet's, not the Game Master's to
    // resize (spec 045 T065).
    if light.is_carried() {
        return;
    }

    let shift = keyboard
        .as_ref()
        .is_some_and(|keys| keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight));
    let prior_radius = light.radius;
    let prior_bright_radius = light.bright_radius;
    let prior_intensity = light.intensity;
    let prior_bright = light.bright();
    let mut updated = light;
    if shift {
        updated.bright_radius =
            Some((prior_bright + scroll * RESIZE_STEP).clamp(0.0, updated.radius));
    } else {
        updated.radius = (updated.radius + scroll * RESIZE_STEP).max(MIN_LIGHT_RADIUS);
        updated.bright_radius = Some(prior_bright.min(updated.radius));
    }
    let bright = updated.bright();
    light_set.upsert(updated.clone());
    light_set.push_undo(LightEdit::Resize {
        light_id: light_id.clone(),
        prior_radius,
        prior_bright_radius,
        prior_intensity,
    });
    // Both reaches, every time: the server keeps a bright reach within the dim
    // one, and naming both leaves it nothing to decide.
    emit_event(json!({
        "type": "update_light",
        "lightId": light_id,
        "changes": { "radius": updated.radius, "brightRadius": bright },
        "worldId": active_world.0,
    }));
}

/// T037: `L` toggles `casts_shadows` on the selected light; Delete/Backspace
/// removes it. GM-only, same gating as `handle_light_input`. Keybinds
/// chosen to avoid `systems/wall.rs`'s `V`/`B`/`O`/Ctrl+Z and
/// `move_player`'s WASD (`S` is taken by player-token movement, so `L` is
/// used instead of the spec's suggested `S`).
pub(crate) fn handle_light_keyboard_toggles(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut light_set: ResMut<LightSet>,
    mut selected_light: ResMut<SelectedLight>,
    is_gm: Res<IsGameMaster>,
    active_world: Res<ActiveWorld>,
) {
    if !is_gm.0 {
        return;
    }

    let Some(light_id) = selected_light.get_selected().cloned() else {
        return;
    };

    if keyboard.just_pressed(KeyCode::KeyL) {
        if let Some(light) = light_set.get(&light_id).cloned() {
            let prior_casts_shadows = light.casts_shadows;
            let mut updated = light;
            updated.casts_shadows = !updated.casts_shadows;
            light_set.upsert(updated.clone());
            light_set.push_undo(LightEdit::FlagsToggle {
                light_id: light_id.clone(),
                prior_casts_shadows,
            });
            emit_event(json!({
                "type": "update_light",
                "lightId": light_id,
                "changes": { "castsShadows": updated.casts_shadows },
                "worldId": active_world.0,
            }));
        }
        return;
    }

    if (keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace))
        && let Some(deleted) = light_set.remove(&light_id)
    {
        light_set.push_undo(LightEdit::Delete { deleted });
        selected_light.deselect();
        report_selected_light(None);
        emit_event(json!({
            "type": "delete_light",
            "lightId": light_id,
            "worldId": active_world.0,
        }));
    }
}

/// T039: light undo (FR-012). Ctrl+Z pops `LightSet`'s undo stack and
/// re-issues the inverse mutation through the same outbound-event path a
/// normal edit uses (research.md §4) — applied locally first (optimistic),
/// then emitted so other clients converge once the server confirms it.
/// Mirrors `systems/wall.rs`'s `handle_wall_undo` exactly.
pub(crate) fn handle_light_undo(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut light_set: ResMut<LightSet>,
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

    let Some(edit) = light_set.pop_undo() else {
        return;
    };

    match edit {
        LightEdit::Move {
            light_id,
            prior_x,
            prior_y,
        } => {
            if let Some(mut light) = light_set.get(&light_id).cloned() {
                light.x = prior_x;
                light.y = prior_y;
                light_set.upsert(light);
            }
            emit_event(json!({
                "type": "update_light",
                "lightId": light_id,
                "changes": { "x": prior_x, "y": prior_y },
                "worldId": active_world.0,
            }));
        }
        LightEdit::Resize {
            light_id,
            prior_radius,
            prior_bright_radius,
            prior_intensity,
        } => {
            let prior_bright = prior_bright_radius
                .unwrap_or(prior_radius * 0.5)
                .clamp(0.0, prior_radius);
            if let Some(mut light) = light_set.get(&light_id).cloned() {
                light.radius = prior_radius;
                light.bright_radius = prior_bright_radius;
                light.intensity = prior_intensity;
                light_set.upsert(light);
            }
            emit_event(json!({
                "type": "update_light",
                "lightId": light_id,
                "changes": {
                    "radius": prior_radius,
                    "brightRadius": prior_bright,
                    "intensity": prior_intensity,
                },
                "worldId": active_world.0,
            }));
        }
        LightEdit::FlagsToggle {
            light_id,
            prior_casts_shadows,
        } => {
            if let Some(mut light) = light_set.get(&light_id).cloned() {
                light.casts_shadows = prior_casts_shadows;
                light_set.upsert(light);
            }
            emit_event(json!({
                "type": "update_light",
                "lightId": light_id,
                "changes": { "castsShadows": prior_casts_shadows },
                "worldId": active_world.0,
            }));
        }
        LightEdit::Delete { deleted } => {
            // Re-creates the light; the server assigns a new id (the
            // original id cannot be resurrected, same caveat as
            // `WallEdit::Delete`'s undo).
            emit_event(json!({
                "type": "create_light",
                "light": {
                    "x": deleted.x,
                    "y": deleted.y,
                    "radius": deleted.radius,
                    "brightRadius": deleted.bright(),
                    "intensity": deleted.intensity,
                    "color": deleted.color,
                    "attachedTokenId": deleted.attached_token_id,
                    "castsShadows": deleted.casts_shadows,
                },
                "worldId": active_world.0,
            }));
        }
    }
}

/// The selected light's two reaches, drawn for the Game Master who is editing
/// them (spec 045 FR-061): a solid ring at the bright reach and a faint one at
/// the dim reach, around the handle `sync_light_visuals` draws.
///
/// Rings, not fills, for the reason the marker is a fixed size: a filled disc
/// would cover the map the light is meant to show. The debug overlay draws the
/// same two rings for every light; this is only the one being edited, and only
/// for a Game Master, since a player never selects a light.
pub(crate) fn draw_selected_light_reach(
    light_set: Res<LightSet>,
    selected_light: Res<SelectedLight>,
    is_gm: Res<IsGameMaster>,
    token_positions: Query<(&Transform, &TokenIdentity)>,
    mut gizmos: Gizmos,
) {
    if !is_gm.0 {
        return;
    }
    let Some(light) = selected_light
        .get_selected()
        .and_then(|id| light_set.get(id))
    else {
        return;
    };
    let mut positions: HashMap<String, Vec2> = HashMap::new();
    if let Some(token) = light.attached_token_id.as_deref() {
        for (transform, identity) in token_positions.iter() {
            if identity.0 == token {
                positions.insert(identity.0.clone(), transform.translation.truncate());
            }
        }
    }
    let at = effective_light_position(light, &positions);
    gizmos
        .circle_2d(at, light.bright(), SELECTED_LIGHT_COLOR.with_alpha(0.9))
        .resolution(64);
    gizmos
        .circle_2d(at, light.radius, SELECTED_LIGHT_COLOR.with_alpha(0.4))
        .resolution(64);
}
