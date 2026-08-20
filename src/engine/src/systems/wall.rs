//! Wall authoring input, rendering sync, undo, and vision-occlusion systems
//! (T012-T014, T016 of specs/001-bevy-canvas-authoring/tasks.md).
//!
//! Wiring: see `plugins/wall.rs`'s `WallPlugin`.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use serde_json::json;

use crate::resources::{
    CanvasLayer, DoorState, IsGameMaster, SelectedWall, Wall, WallEdit, WallSet, is_visible,
};
use crate::{ActiveWorld, PlayerToken, TokenIdentity, emit_event};

/// Rendered height (px) of a wall's thin sprite (T012: "a small fixed
/// height like 4.0 px").
const WALL_VISUAL_HEIGHT: f32 = 4.0;

/// Minimum drag distance (px) for a click-drag to count as a wall instead
/// of being rejected as a zero-length click (T016).
const MIN_WALL_LENGTH: f32 = 1.0;

/// How close (px) the cursor must be to an existing wall's endpoint to
/// grab it for a move-drag, rather than starting a new wall or selecting
/// the wall's body.
const ENDPOINT_GRAB_RADIUS: f32 = 10.0;

/// How close (px) the cursor must be to a wall's body (the segment
/// itself, not an endpoint) to select it with a plain click.
const WALL_SELECT_DISTANCE: f32 = 6.0;

const UNSELECTED_COLOR: Color = Color::srgb(0.75, 0.75, 0.78);
const SELECTED_COLOR: Color = Color::srgb(0.95, 0.85, 0.25);
const DOOR_COLOR: Color = Color::srgb(0.55, 0.35, 0.2);
const HANDLE_COLOR: Color = Color::srgb(0.95, 0.95, 0.95);
const HANDLE_SIZE: Vec2 = Vec2::new(8.0, 8.0);

/// Marker on the sprite entity rendered for a given `WallSet` wall id.
#[derive(Component)]
pub(crate) struct WallVisual;

/// Marker on a GM-only endpoint-handle sprite (T012's "wall edit handles",
/// gated GM-only per `CanvasLayer::Walls.editing_is_gm_only()`).
#[derive(Component)]
pub(crate) struct WallHandle;

/// Maps `WallSet` wall ids to their spawned sprite entity, mirroring the
/// `TokenEntities` pattern in lib.rs.
#[derive(Resource, Default)]
pub(crate) struct WallEntities(HashMap<String, Entity>);

#[derive(Default)]
enum WallDragMode {
    #[default]
    Idle,
    /// Click-dragging out a brand new wall from `start` to the live
    /// cursor position.
    Creating { start: Vec2 },
    /// Dragging an existing wall's endpoint (`is_start` selects which of
    /// the two endpoints). `prior_*` is the wall's full endpoint state at
    /// drag-start, captured for the undo stack.
    MovingEndpoint {
        wall_id: String,
        is_start: bool,
        prior_x1: f32,
        prior_y1: f32,
        prior_x2: f32,
        prior_y2: f32,
    },
}

/// Session-local wall-tool drag state (not persisted, not part of
/// `WallSet`).
#[derive(Resource, Default)]
pub(crate) struct WallDragState {
    mode: WallDragMode,
}

/// Convert the cursor's window-pixel position into Bevy world space,
/// mirroring `systems/selection.rs`'s private `cursor_world_position`
/// helper (not exported from that module, so duplicated here rather than
/// changing that module's visibility for an unrelated feature).
fn cursor_world_position(
    windows: &Query<&Window, With<PrimaryWindow>>,
    camera_query: &Query<(&Camera, &GlobalTransform)>,
) -> Option<Vec2> {
    let window = windows.iter().next()?;
    let (camera, camera_transform) = camera_query.iter().next()?;
    let cursor_px = window.cursor_position()?;
    camera.viewport_to_world_2d(camera_transform, cursor_px).ok()
}

/// Shortest distance from `point` to the segment `a`-`b`. Pure/testable —
/// used for wall body hit-testing (select-by-click).
fn distance_point_to_segment(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq <= f32::EPSILON {
        return point.distance(a);
    }
    let t = ((point - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let projection = a + ab * t;
    point.distance(projection)
}

fn wall_color(wall: &Wall, selected: bool) -> Color {
    if selected {
        SELECTED_COLOR
    } else if wall.door_state != DoorState::None {
        DOOR_COLOR
    } else {
        UNSELECTED_COLOR
    }
}

/// T012: click-drag to create a wall, click to select, drag an endpoint to
/// move it. GM-only per `CanvasLayer::Walls.editing_is_gm_only()` — this
/// crate has no broader role system, so `IsGameMaster` gates it directly.
/// T016: a press-and-release without meaningfully dragging is rejected
/// (no wall created) rather than producing a zero-length segment.
pub(crate) fn handle_wall_input(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut wall_set: ResMut<WallSet>,
    mut selected_wall: ResMut<SelectedWall>,
    mut drag: ResMut<WallDragState>,
    is_gm: Res<IsGameMaster>,
    active_world: Res<ActiveWorld>,
) {
    if !is_gm.0 {
        return;
    }

    let Some(cursor) = cursor_world_position(&windows, &camera_query) else {
        return;
    };

    if mouse_button.just_pressed(MouseButton::Left) {
        // Endpoint grab takes priority over body-select/create.
        for wall in wall_set.walls() {
            if cursor.distance(wall.start()) <= ENDPOINT_GRAB_RADIUS {
                selected_wall.select(wall.id.clone());
                drag.mode = WallDragMode::MovingEndpoint {
                    wall_id: wall.id.clone(),
                    is_start: true,
                    prior_x1: wall.x1,
                    prior_y1: wall.y1,
                    prior_x2: wall.x2,
                    prior_y2: wall.y2,
                };
                return;
            }
            if cursor.distance(wall.end()) <= ENDPOINT_GRAB_RADIUS {
                selected_wall.select(wall.id.clone());
                drag.mode = WallDragMode::MovingEndpoint {
                    wall_id: wall.id.clone(),
                    is_start: false,
                    prior_x1: wall.x1,
                    prior_y1: wall.y1,
                    prior_x2: wall.x2,
                    prior_y2: wall.y2,
                };
                return;
            }
        }

        // Body select (no drag intent).
        for wall in wall_set.walls() {
            if distance_point_to_segment(cursor, wall.start(), wall.end()) <= WALL_SELECT_DISTANCE
            {
                selected_wall.select(wall.id.clone());
                drag.mode = WallDragMode::Idle;
                return;
            }
        }

        // Neither an endpoint nor a body: start creating a new wall.
        selected_wall.deselect();
        drag.mode = WallDragMode::Creating { start: cursor };
        return;
    }

    if mouse_button.pressed(MouseButton::Left) {
        if let WallDragMode::MovingEndpoint {
            wall_id, is_start, ..
        } = &drag.mode
        {
            if let Some(wall) = wall_set.get(wall_id).cloned() {
                let mut updated = wall;
                if *is_start {
                    updated.x1 = cursor.x;
                    updated.y1 = cursor.y;
                } else {
                    updated.x2 = cursor.x;
                    updated.y2 = cursor.y;
                }
                // Optimistic local move so the sprite tracks the cursor;
                // reconciled by the next `upsert_wall` confirmation from
                // the server (see lib.rs's `apply_external_commands`).
                wall_set.upsert(updated);
            }
        }
        return;
    }

    if mouse_button.just_released(MouseButton::Left) {
        match std::mem::take(&mut drag.mode) {
            WallDragMode::Creating { start } => {
                let end = cursor;
                if start.distance(end) < MIN_WALL_LENGTH {
                    // T016: reject zero-length (click without drag).
                    return;
                }

                emit_event(json!({
                    "type": "create_wall",
                    "wall": {
                        "x1": start.x,
                        "y1": start.y,
                        "x2": end.x,
                        "y2": end.y,
                        "blocksVision": true,
                        "blocksMovement": false,
                        "doorState": "none",
                    },
                    "worldId": active_world.0,
                }));
                // Deliberately no local WallSet entry yet: the server
                // assigns the wall's real id, so this stays untracked
                // until the matching `upsert_wall` command arrives (see
                // module doc / WallPlugin for the rationale).
            }
            WallDragMode::MovingEndpoint {
                wall_id,
                is_start,
                prior_x1,
                prior_y1,
                prior_x2,
                prior_y2,
            } => {
                if let Some(wall) = wall_set.get(&wall_id) {
                    let changes = if is_start {
                        json!({ "x1": wall.x1, "y1": wall.y1 })
                    } else {
                        json!({ "x2": wall.x2, "y2": wall.y2 })
                    };
                    emit_event(json!({
                        "type": "update_wall",
                        "wallId": wall_id,
                        "changes": changes,
                        "worldId": active_world.0,
                    }));
                }
                wall_set.push_undo(WallEdit::Move {
                    wall_id,
                    prior_x1,
                    prior_y1,
                    prior_x2,
                    prior_y2,
                });
            }
            WallDragMode::Idle => {}
        }
    }
}

/// T012: keybound toggles for the selected wall's `blocks_vision` /
/// `blocks_movement` / door-state, plus Delete to remove it. GM-only,
/// same gating as `handle_wall_input`.
///
/// Keybinds (chosen to avoid the existing WASD/arrow-key/+-/Home bindings
/// in `lib.rs`/`plugins/camera.rs`):
/// - `V`: toggle blocks_vision
/// - `B`: toggle blocks_movement
/// - `O`: cycle door state (none -> closed -> open -> closed -> ...)
/// - `Delete`/`Backspace`: delete the selected wall
pub(crate) fn handle_wall_keyboard_toggles(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut wall_set: ResMut<WallSet>,
    mut selected_wall: ResMut<SelectedWall>,
    is_gm: Res<IsGameMaster>,
    active_world: Res<ActiveWorld>,
) {
    if !is_gm.0 {
        return;
    }

    let Some(wall_id) = selected_wall.get_selected().cloned() else {
        return;
    };

    if keyboard.just_pressed(KeyCode::KeyV) {
        if let Some(wall) = wall_set.get(&wall_id).cloned() {
            let prior_blocks_vision = wall.blocks_vision;
            let prior_blocks_movement = wall.blocks_movement;
            let mut updated = wall;
            updated.blocks_vision = !updated.blocks_vision;
            wall_set.upsert(updated.clone());
            wall_set.push_undo(WallEdit::FlagsToggle {
                wall_id: wall_id.clone(),
                prior_blocks_vision,
                prior_blocks_movement,
            });
            emit_event(json!({
                "type": "update_wall",
                "wallId": wall_id,
                "changes": { "blocksVision": updated.blocks_vision },
                "worldId": active_world.0,
            }));
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyB) {
        if let Some(wall) = wall_set.get(&wall_id).cloned() {
            let prior_blocks_vision = wall.blocks_vision;
            let prior_blocks_movement = wall.blocks_movement;
            let mut updated = wall;
            updated.blocks_movement = !updated.blocks_movement;
            wall_set.upsert(updated.clone());
            wall_set.push_undo(WallEdit::FlagsToggle {
                wall_id: wall_id.clone(),
                prior_blocks_vision,
                prior_blocks_movement,
            });
            emit_event(json!({
                "type": "update_wall",
                "wallId": wall_id,
                "changes": { "blocksMovement": updated.blocks_movement },
                "worldId": active_world.0,
            }));
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyO) {
        if let Some(wall) = wall_set.get(&wall_id).cloned() {
            let prior_door_state = wall.door_state;
            let next = match wall.door_state {
                DoorState::None => DoorState::Closed,
                DoorState::Closed => DoorState::Open,
                DoorState::Open => DoorState::Closed,
            };
            let mut updated = wall;
            updated.door_state = next;
            wall_set.upsert(updated);
            wall_set.push_undo(WallEdit::DoorToggle {
                wall_id: wall_id.clone(),
                prior_door_state,
            });
            emit_event(json!({
                "type": "update_wall",
                "wallId": wall_id,
                "changes": { "doorState": next.as_str() },
                "worldId": active_world.0,
            }));
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace) {
        if let Some(deleted) = wall_set.remove(&wall_id) {
            wall_set.push_undo(WallEdit::Delete { deleted });
            selected_wall.deselect();
            emit_event(json!({
                "type": "delete_wall",
                "wallId": wall_id,
                "worldId": active_world.0,
            }));
        }
    }
}

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

/// T012/T015: keeps one thin rotated sprite per `WallSet` wall in sync
/// (spawn on new id, update transform/color on change, despawn on
/// removal) — the same "spawn/update/despawn by stable id" shape as
/// `TokenEntities` in lib.rs, just against `WallSet` instead of the
/// external-command queue. Also renders GM-only endpoint handles for the
/// selected wall (data-model.md's Canvas Layer: wall editing handles are
/// GM-only, unlike the effect they produce).
pub(crate) fn sync_wall_visuals(
    mut commands: Commands,
    wall_set: Res<WallSet>,
    selected_wall: Res<SelectedWall>,
    is_gm: Res<IsGameMaster>,
    mut wall_entities: ResMut<WallEntities>,
    mut sprite_query: Query<(&mut Transform, &mut Sprite), (With<WallVisual>, Without<WallHandle>)>,
    handle_query: Query<Entity, With<WallHandle>>,
) {
    let z = CanvasLayer::Walls.z();

    // Despawn sprites for walls that no longer exist in `WallSet`.
    let stale_ids: Vec<String> = wall_entities
        .0
        .keys()
        .filter(|id| wall_set.get(id).is_none())
        .cloned()
        .collect();
    for id in stale_ids {
        if let Some(entity) = wall_entities.0.remove(&id) {
            commands.entity(entity).despawn();
        }
    }

    for wall in wall_set.walls() {
        let selected = selected_wall.is_selected(&wall.id);
        let color = wall_color(wall, selected);
        let length = wall.length().max(0.5);
        let size = Vec2::new(length, WALL_VISUAL_HEIGHT);
        let translation_z = if selected { z + 1.0 } else { z };
        let transform = Transform {
            translation: wall.midpoint().extend(translation_z),
            rotation: Quat::from_rotation_z(wall.angle()),
            ..default()
        };

        if let Some(&entity) = wall_entities.0.get(&wall.id) {
            if let Ok((mut t, mut sprite)) = sprite_query.get_mut(entity) {
                *t = transform;
                sprite.color = color;
                sprite.custom_size = Some(size);
            }
        } else {
            let entity = commands
                .spawn((Sprite::from_color(color, size), transform, WallVisual))
                .id();
            wall_entities.0.insert(wall.id.clone(), entity);
        }
    }

    // GM-only endpoint handles for the selected wall (rebuilt each pass;
    // wall counts here are small enough that this isn't a hot path).
    for entity in handle_query.iter() {
        commands.entity(entity).despawn();
    }

    if is_gm.0
        && let Some(selected_id) = selected_wall.get_selected()
        && let Some(wall) = wall_set.get(selected_id)
    {
        for point in [wall.start(), wall.end()] {
            commands.spawn((
                Sprite::from_color(HANDLE_COLOR, HANDLE_SIZE),
                Transform::from_translation(point.extend(z + 2.0)),
                WallHandle,
            ));
        }
    }
}

/// T013: 2D shadow-casting vision occlusion, applied as a first-pass
/// visual proof (per-token hide/show relative to the locally-controlled
/// `PlayerToken`) rather than a full fog-of-war shadow mesh — there is no
/// fog-rendering plugin in this codebase yet (ADR-032 only sketched one).
/// The real, unit-tested geometry lives in `resources::wall::is_visible`;
/// this system just calls it and toggles `Visibility`. A follow-up task
/// should replace the toggle with an actual fog-of-war mesh/stencil once
/// that rendering plugin exists.
///
/// Recomputes when `WallSet` changed or any token's `Transform` changed
/// (research.md §3: "on change", not every frame).
pub(crate) fn apply_vision_occlusion(
    wall_set: Res<WallSet>,
    player_query: Query<&Transform, With<PlayerToken>>,
    mut other_tokens: Query<(&Transform, &mut Visibility), (With<TokenIdentity>, Without<PlayerToken>)>,
    transform_changed: Query<(), (Changed<Transform>, With<TokenIdentity>)>,
) {
    if !wall_set.is_changed() && transform_changed.is_empty() {
        return;
    }

    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let observer = player_transform.translation.truncate();

    for (transform, mut visibility) in other_tokens.iter_mut() {
        let target = transform.translation.truncate();
        let visible = is_visible(observer, target, &wall_set);
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

pub(crate) fn init_wall_systems_resources(app: &mut App) {
    app.init_resource::<WallDragState>()
        .init_resource::<WallEntities>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_point_to_segment_on_the_line() {
        let d = distance_point_to_segment(Vec2::new(5.0, 0.0), Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0));
        assert!(d.abs() < 1e-5);
    }

    #[test]
    fn distance_point_to_segment_perpendicular() {
        let d = distance_point_to_segment(Vec2::new(5.0, 3.0), Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0));
        assert!((d - 3.0).abs() < 1e-5);
    }

    #[test]
    fn distance_point_to_segment_beyond_endpoint() {
        let d = distance_point_to_segment(Vec2::new(15.0, 0.0), Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0));
        assert!((d - 5.0).abs() < 1e-5);
    }

    #[test]
    fn distance_point_to_segment_degenerate_segment() {
        // Zero-length segment: distance is just distance to the point.
        let d = distance_point_to_segment(Vec2::new(3.0, 4.0), Vec2::new(0.0, 0.0), Vec2::new(0.0, 0.0));
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
        };
        assert_eq!(wall_color(&wall, true), SELECTED_COLOR);
        assert_eq!(wall_color(&wall, false), DOOR_COLOR);
    }
}
