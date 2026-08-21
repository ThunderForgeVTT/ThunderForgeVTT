//! Token authoring input (click-drag move, resize-handle drag, rotate-
//! handle drag, legacy keyboard shortcuts) and handle-sprite rendering
//! sync (spec 006, closing spec 004 US2's keyboard-shortcut stand-in).
//!
//! Wiring: see `plugins/token.rs`'s `TokenPlugin`.
//!
//! `handle_token_drag` and `handle_token_resize_rotate_keyboard` were
//! relocated here from `systems/selection.rs` (research.md §1) —
//! behavior-preserving move, no logic change to either. The keyboard
//! shortcuts are kept as a secondary/power-user input path per spec.md's
//! Assumptions (Acceptance Scenario 5: either keeping or removing them is
//! acceptable) — canvas handles below are now the primary, discoverable
//! mechanism.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use serde_json::json;

use crate::resources::{CanvasLayer, DraggingToken, IsGameMaster, SelectedToken};
use crate::{ActiveWorld, TOKEN_SIZE, TokenIdentity, emit_event};

/// Whole grid-cell increments a token's `scale` may take (spec 004 US2's
/// resize clarification: 1x1, 2x2, 3x3... never a fractional cell).
const MIN_TOKEN_SCALE: f32 = 1.0;
const MAX_TOKEN_SCALE: f32 = 5.0;
/// Fixed rotation step per key press (30 degrees), independent of resize.
const TOKEN_ROTATE_STEP_RADIANS: f32 = std::f32::consts::FRAC_PI_6;

const HANDLE_GRAB_RADIUS: f32 = 10.0;
const RESIZE_HANDLE_SIZE: Vec2 = Vec2::new(10.0, 10.0);
const ROTATE_HANDLE_SIZE: Vec2 = Vec2::new(10.0, 10.0);
const RESIZE_HANDLE_COLOR: Color = Color::srgb(0.95, 0.95, 0.95);
const ROTATE_HANDLE_COLOR: Color = Color::srgb(0.35, 0.75, 0.95);
/// Distance (px, at scale 1) the rotate handle sits from the token center,
/// along the token's local "up" (+Y) direction.
const ROTATE_HANDLE_OFFSET: f32 = TOKEN_SIZE.y / 2.0 + 24.0;

/// GM-only resize-handle sprite marker, rendered at the selected token's
/// corner. Mirrors `systems::wall::WallHandle`'s marker + rebuild-each-pass
/// pattern (research.md §2).
#[derive(Component)]
pub(crate) struct TokenResizeHandle;

/// GM-only rotate-handle sprite marker, rendered offset from the selected
/// token's center along its current facing.
#[derive(Component)]
pub(crate) struct TokenRotateHandle;

#[derive(Default, PartialEq, Eq)]
enum TokenDragMode {
    #[default]
    Idle,
    Resizing,
    Rotating,
}

/// Session-local token-handle drag state (not persisted). While non-`Idle`,
/// `handle_token_drag` yields so a handle drag never also moves the token's
/// whole body in the same gesture.
#[derive(Resource, Default)]
pub(crate) struct TokenDragState {
    mode: TokenDragMode,
}

/// Convert the cursor's window-pixel position into Bevy world space,
/// duplicated from `systems/wall.rs` (itself duplicated from this module's
/// prior private copy) — each canvas-authoring system module keeps its own
/// rather than changing a shared helper's visibility for an unrelated
/// feature, matching the codebase's established convention.
fn cursor_world_position(
    windows: &Query<&Window, With<PrimaryWindow>>,
    camera_query: &Query<(&Camera, &GlobalTransform)>,
) -> Option<Vec2> {
    let window = windows.iter().next()?;
    let (camera, camera_transform) = camera_query.iter().next()?;
    let cursor_px = window.cursor_position()?;
    camera.viewport_to_world_2d(camera_transform, cursor_px).ok()
}

/// Hit-test: Check if point is within rectangle bounds
/// Used for token selection based on mouse click
fn is_point_in_rect(point: Vec2, rect_center: Vec2, rect_size: Vec2) -> bool {
    let half_size = rect_size / 2.0;
    let min = rect_center - half_size;
    let max = rect_center + half_size;

    point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
}

/// Notifies the frontend of a token selection change, mirroring
/// `wall.rs`'s `emit_wall_selection` exactly (same `bevy`-sourced-event
/// convention, so `bindWorldStore` never re-forwards it back into the
/// engine and no loop results).
fn emit_token_selection(token_id: Option<&str>) {
    emit_event(json!({
        "type": "select_token",
        "tokenId": token_id,
    }));
}

/// World-space position of the resize handle: the token's bottom-right
/// corner, accounting for the token's current scale and rotation.
fn resize_handle_world_pos(transform: &Transform) -> Vec2 {
    let half = (TOKEN_SIZE * transform.scale.truncate()) / 2.0;
    let local_corner = Vec2::new(half.x, -half.y);
    let rotation_radians = transform.rotation.to_euler(EulerRot::ZYX).0;
    transform.translation.truncate() + Vec2::from_angle(rotation_radians).rotate(local_corner)
}

/// World-space position of the rotate handle: offset from the token center
/// along its current facing, scaled with the token's current size so it
/// tracks the resize handle instead of drifting inside/outside the token's
/// footprint as scale changes.
fn rotate_handle_world_pos(transform: &Transform) -> Vec2 {
    let rotation_radians = transform.rotation.to_euler(EulerRot::ZYX).0;
    let local_offset = Vec2::new(0.0, ROTATE_HANDLE_OFFSET * transform.scale.y);
    transform.translation.truncate()
        + Vec2::from_angle(rotation_radians).rotate(local_offset)
}

/// Half-diagonal (px) of an unrotated, unscaled token — the resize handle's
/// distance from center at `scale == 1.0`. Scales linearly with `scale`,
/// so `cursor_distance / this` recovers the intended whole-cell scale.
fn token_half_diagonal() -> f32 {
    TOKEN_SIZE.length() / 2.0
}

/// Click-to-select and click-drag-to-move for tokens on the live
/// `TokenIdentity` pipeline (the one that's actually wired to the server via
/// `emit_event`/`apply_external_commands` — see lib.rs).
///
/// - Press on a token: select it and begin dragging.
/// - Press on empty space: deselect.
/// - Hold + move: token follows the cursor, preserving the original
///   grab-point offset.
/// - Release while dragging: emit an `upsert_token` event with the final
///   position so the move persists to the server, mirroring how
///   `emit_player_state` pushes state out for the WASD demo token.
///
/// Yields entirely while a resize/rotate handle drag is in progress
/// (`TokenDragState`) so a handle grab never also triggers a body move.
pub(crate) fn handle_token_drag(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut token_query: Query<(&mut Transform, &TokenIdentity)>,
    mut selected_token: ResMut<SelectedToken>,
    mut dragging: ResMut<DraggingToken>,
    active_world: Res<ActiveWorld>,
    drag_state: Res<TokenDragState>,
) {
    if drag_state.mode != TokenDragMode::Idle {
        return;
    }

    let Some(cursor_world) = cursor_world_position(&windows, &camera_query) else {
        return;
    };

    if mouse_button.just_pressed(MouseButton::Left) {
        let mut hit: Option<(String, Vec2)> = None;

        for (transform, identity) in token_query.iter() {
            let token_pos = transform.translation.truncate();
            if is_point_in_rect(cursor_world, token_pos, TOKEN_SIZE) {
                hit = Some((identity.0.clone(), token_pos - cursor_world));
                break; // First hit wins (topmost token)
            }
        }

        match hit {
            Some((token_id, offset)) => {
                selected_token.select(token_id.clone());
                emit_token_selection(Some(&token_id));
                dragging.0 = Some((token_id, offset));
            }
            None => {
                selected_token.deselect();
                emit_token_selection(None);
                dragging.0 = None;
            }
        }
        return;
    }

    if mouse_button.pressed(MouseButton::Left) {
        let Some((token_id, offset)) = dragging.0.clone() else {
            return;
        };
        let new_pos = cursor_world + offset;
        for (mut transform, identity) in token_query.iter_mut() {
            if identity.0 == token_id {
                transform.translation.x = new_pos.x;
                transform.translation.y = new_pos.y;
                break;
            }
        }
        return;
    }

    if mouse_button.just_released(MouseButton::Left) {
        let Some((token_id, _offset)) = dragging.0.take() else {
            return;
        };
        for (transform, identity) in token_query.iter() {
            if identity.0 == token_id {
                // Include scale/rotation, not just position: this event
                // fires on *every* select-click release, not only a real
                // drag (dragging.0 is set on press, released here even
                // for a plain click-to-select with no movement). Omitting
                // them made the world-store reducer's full-replace
                // `upsert_token` case silently wipe a token's
                // already-persisted scale/rotation from the *client-side*
                // store back to `undefined` on every reselect — the
                // server value was untouched (this event's mutation
                // bridge input only forwards fields that are present),
                // but `TokenTool.tsx`'s displayed size/facing reverted to
                // default the moment a GM clicked the token again after a
                // reload, discovered live while building T020's e2e
                // coverage (spec 004 US2).
                let rotation_radians = transform.rotation.to_euler(EulerRot::ZYX).0;
                emit_event(json!({
                    "type": "upsert_token",
                    "token": {
                        "id": token_id,
                        "x": transform.translation.x,
                        "y": transform.translation.y,
                        "z": transform.translation.z,
                        "scale": transform.scale.x,
                        "rotation": rotation_radians,
                    },
                    "worldId": active_world.0,
                }));
                break;
            }
        }
    }
}

/// GM-only drag on the selected token's resize handle: grows/shrinks the
/// footprint in whole grid-cell increments (never fractional), driven by
/// cursor distance from the token center instead of key presses
/// (research.md §2).
pub(crate) fn handle_token_resize_drag(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut token_query: Query<(&mut Transform, &TokenIdentity)>,
    selected_token: Res<SelectedToken>,
    is_gm: Res<IsGameMaster>,
    mut drag_state: ResMut<TokenDragState>,
    active_world: Res<ActiveWorld>,
) {
    if !is_gm.0 {
        return;
    }

    let Some(cursor) = cursor_world_position(&windows, &camera_query) else {
        return;
    };

    if mouse_button.just_pressed(MouseButton::Left) {
        if drag_state.mode != TokenDragMode::Idle {
            return;
        }
        let Some(selected_id) = selected_token.get_selected() else {
            return;
        };
        let Some((transform, _)) = token_query.iter().find(|(_, id)| id.0 == *selected_id) else {
            return;
        };
        if cursor.distance(resize_handle_world_pos(transform)) <= HANDLE_GRAB_RADIUS {
            drag_state.mode = TokenDragMode::Resizing;
        }
        return;
    }

    if mouse_button.pressed(MouseButton::Left) {
        if drag_state.mode != TokenDragMode::Resizing {
            return;
        }
        let Some(selected_id) = selected_token.get_selected().cloned() else {
            return;
        };
        for (mut transform, identity) in token_query.iter_mut() {
            if identity.0 != selected_id {
                continue;
            }
            let dist = cursor.distance(transform.translation.truncate());
            let raw_scale = (dist / token_half_diagonal()).round();
            let new_scale = raw_scale.clamp(MIN_TOKEN_SCALE, MAX_TOKEN_SCALE);
            transform.scale = Vec3::splat(new_scale);
            break;
        }
        return;
    }

    if mouse_button.just_released(MouseButton::Left) {
        if drag_state.mode != TokenDragMode::Resizing {
            return;
        }
        drag_state.mode = TokenDragMode::Idle;
        let Some(selected_id) = selected_token.get_selected().cloned() else {
            return;
        };
        for (transform, identity) in token_query.iter() {
            if identity.0 != selected_id {
                continue;
            }
            let rotation_radians = transform.rotation.to_euler(EulerRot::ZYX).0;
            emit_event(json!({
                "type": "upsert_token",
                "token": {
                    "id": identity.0,
                    "x": transform.translation.x,
                    "y": transform.translation.y,
                    "z": transform.translation.z,
                    "scale": transform.scale.x,
                    "rotation": rotation_radians,
                },
                "worldId": active_world.0,
            }));
            break;
        }
    }
}

/// GM-only drag on the selected token's rotate handle: changes facing
/// continuously (not in fixed steps), computed from cursor angle relative
/// to the token center, independent of any concurrent resize.
pub(crate) fn handle_token_rotate_drag(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut token_query: Query<(&mut Transform, &TokenIdentity)>,
    selected_token: Res<SelectedToken>,
    is_gm: Res<IsGameMaster>,
    mut drag_state: ResMut<TokenDragState>,
    active_world: Res<ActiveWorld>,
) {
    if !is_gm.0 {
        return;
    }

    let Some(cursor) = cursor_world_position(&windows, &camera_query) else {
        return;
    };

    if mouse_button.just_pressed(MouseButton::Left) {
        if drag_state.mode != TokenDragMode::Idle {
            return;
        }
        let Some(selected_id) = selected_token.get_selected() else {
            return;
        };
        let Some((transform, _)) = token_query.iter().find(|(_, id)| id.0 == *selected_id) else {
            return;
        };
        if cursor.distance(rotate_handle_world_pos(transform)) <= HANDLE_GRAB_RADIUS {
            drag_state.mode = TokenDragMode::Rotating;
        }
        return;
    }

    if mouse_button.pressed(MouseButton::Left) {
        if drag_state.mode != TokenDragMode::Rotating {
            return;
        }
        let Some(selected_id) = selected_token.get_selected().cloned() else {
            return;
        };
        for (mut transform, identity) in token_query.iter_mut() {
            if identity.0 != selected_id {
                continue;
            }
            let center = transform.translation.truncate();
            let delta = cursor - center;
            if delta.length_squared() > f32::EPSILON {
                let angle = delta.y.atan2(delta.x) - std::f32::consts::FRAC_PI_2;
                transform.rotation = Quat::from_rotation_z(angle);
            }
            break;
        }
        return;
    }

    if mouse_button.just_released(MouseButton::Left) {
        if drag_state.mode != TokenDragMode::Rotating {
            return;
        }
        drag_state.mode = TokenDragMode::Idle;
        let Some(selected_id) = selected_token.get_selected().cloned() else {
            return;
        };
        for (transform, identity) in token_query.iter() {
            if identity.0 != selected_id {
                continue;
            }
            let rotation_radians = transform.rotation.to_euler(EulerRot::ZYX).0;
            emit_event(json!({
                "type": "upsert_token",
                "token": {
                    "id": identity.0,
                    "x": transform.translation.x,
                    "y": transform.translation.y,
                    "z": transform.translation.z,
                    "scale": transform.scale.x,
                    "rotation": rotation_radians,
                },
                "worldId": active_world.0,
            }));
            break;
        }
    }
}

/// Spec 004 (US2): GM-only resize (`]`/`[`, whole grid-cell increments,
/// per the resize clarification — never a fractional cell) and rotate
/// (`.`/`,`, fixed-degree steps, independent of size) for the currently
/// selected token.
///
/// Kept as a secondary/power-user input path per spec.md's Assumptions
/// (Acceptance Scenario 5) now that `handle_token_resize_drag`/
/// `handle_token_rotate_drag` above provide the primary, discoverable
/// canvas-handle mechanism (T008).
pub(crate) fn handle_token_resize_rotate_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut token_query: Query<(&mut Transform, &TokenIdentity)>,
    selected_token: Res<SelectedToken>,
    is_gm: Res<IsGameMaster>,
    active_world: Res<ActiveWorld>,
) {
    if !is_gm.0 {
        return;
    }

    let Some(selected_id) = selected_token.get_selected() else {
        return;
    };

    let resize_delta = if keyboard.just_pressed(KeyCode::BracketRight) {
        1.0
    } else if keyboard.just_pressed(KeyCode::BracketLeft) {
        -1.0
    } else {
        0.0
    };

    let rotate_delta = if keyboard.just_pressed(KeyCode::Period) {
        -TOKEN_ROTATE_STEP_RADIANS
    } else if keyboard.just_pressed(KeyCode::Comma) {
        TOKEN_ROTATE_STEP_RADIANS
    } else {
        0.0
    };

    if resize_delta == 0.0 && rotate_delta == 0.0 {
        return;
    }

    for (mut transform, identity) in token_query.iter_mut() {
        if identity.0 != *selected_id {
            continue;
        }

        if resize_delta != 0.0 {
            let new_scale = (transform.scale.x + resize_delta).clamp(MIN_TOKEN_SCALE, MAX_TOKEN_SCALE);
            transform.scale = Vec3::splat(new_scale);
        }

        let mut rotation_radians = transform.rotation.to_euler(EulerRot::ZYX).0;
        if rotate_delta != 0.0 {
            rotation_radians += rotate_delta;
            transform.rotation = Quat::from_rotation_z(rotation_radians);
        }

        emit_event(json!({
            "type": "upsert_token",
            "token": {
                "id": identity.0,
                "x": transform.translation.x,
                "y": transform.translation.y,
                "z": transform.translation.z,
                "scale": transform.scale.x,
                "rotation": rotation_radians,
            },
            "worldId": active_world.0,
        }));
        break;
    }
}

/// Keeps the selected token's resize/rotate handle sprites in sync
/// (despawn-all-then-respawn-for-current-selection each pass, mirroring
/// `wall.rs::sync_wall_visuals`'s exact GM-only endpoint-handle pattern —
/// research.md §2). Token counts are small enough that this isn't a hot
/// path, same tradeoff `wall.rs` already makes.
pub(crate) fn sync_token_visuals(
    mut commands: Commands,
    token_query: Query<(&Transform, &TokenIdentity)>,
    selected_token: Res<SelectedToken>,
    is_gm: Res<IsGameMaster>,
    handle_query: Query<Entity, Or<(With<TokenResizeHandle>, With<TokenRotateHandle>)>>,
) {
    for entity in handle_query.iter() {
        commands.entity(entity).despawn();
    }

    if !is_gm.0 {
        return;
    }

    let Some(selected_id) = selected_token.get_selected() else {
        return;
    };
    let Some((transform, _)) = token_query.iter().find(|(_, id)| id.0 == *selected_id) else {
        return;
    };

    let z = CanvasLayer::Tokens.z() + 2.0;

    commands.spawn((
        Sprite::from_color(RESIZE_HANDLE_COLOR, RESIZE_HANDLE_SIZE),
        Transform::from_translation(resize_handle_world_pos(transform).extend(z)),
        TokenResizeHandle,
    ));
    commands.spawn((
        Sprite::from_color(ROTATE_HANDLE_COLOR, ROTATE_HANDLE_SIZE),
        Transform::from_translation(rotate_handle_world_pos(transform).extend(z)),
        TokenRotateHandle,
    ));
}

pub(crate) fn init_token_systems_resources(app: &mut App) {
    app.init_resource::<TokenDragState>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_in_rect_center() {
        assert!(is_point_in_rect(
            Vec2::new(10.0, 10.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(20.0, 20.0),
        ));
    }

    #[test]
    fn test_point_in_rect_left_edge() {
        assert!(is_point_in_rect(
            Vec2::new(0.0, 10.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(20.0, 20.0),
        ));
    }

    #[test]
    fn test_point_in_rect_right_edge() {
        assert!(is_point_in_rect(
            Vec2::new(20.0, 10.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(20.0, 20.0),
        ));
    }

    #[test]
    fn test_point_out_of_rect() {
        assert!(!is_point_in_rect(
            Vec2::new(50.0, 50.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(20.0, 20.0),
        ));
    }

    #[test]
    fn test_point_just_outside_rect() {
        assert!(!is_point_in_rect(
            Vec2::new(20.1, 10.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(20.0, 20.0),
        ));
    }

    #[test]
    fn test_point_in_rect_corners() {
        let rect_center = Vec2::new(50.0, 50.0);
        let rect_size = Vec2::new(40.0, 40.0);

        assert!(is_point_in_rect(Vec2::new(30.0, 70.0), rect_center, rect_size));
        assert!(is_point_in_rect(Vec2::new(70.0, 70.0), rect_center, rect_size));
        assert!(is_point_in_rect(Vec2::new(30.0, 30.0), rect_center, rect_size));
        assert!(is_point_in_rect(Vec2::new(70.0, 30.0), rect_center, rect_size));
    }

    #[test]
    fn test_point_in_rect_just_outside_corners() {
        let rect_center = Vec2::new(50.0, 50.0);
        let rect_size = Vec2::new(40.0, 40.0);

        assert!(!is_point_in_rect(Vec2::new(29.9, 70.0), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(70.1, 70.0), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(30.0, 29.9), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(70.0, 70.1), rect_center, rect_size));
    }

    #[test]
    fn test_point_in_rect_small_rect() {
        let rect_center = Vec2::new(100.0, 100.0);
        let rect_size = Vec2::new(1.0, 1.0);

        assert!(is_point_in_rect(Vec2::new(100.0, 100.0), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(100.6, 100.0), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(100.0, 100.6), rect_center, rect_size));
    }

    #[test]
    fn test_point_in_rect_large_rect() {
        let rect_center = Vec2::new(500.0, 500.0);
        let rect_size = Vec2::new(1000.0, 1000.0);

        assert!(is_point_in_rect(Vec2::new(1.0, 1.0), rect_center, rect_size));
        assert!(is_point_in_rect(Vec2::new(999.0, 999.0), rect_center, rect_size));
    }

    #[test]
    fn test_point_in_rect_zero_size() {
        let rect_center = Vec2::new(50.0, 50.0);
        let rect_size = Vec2::ZERO;

        assert!(is_point_in_rect(Vec2::new(50.0, 50.0), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(50.001, 50.0), rect_center, rect_size));
    }

    #[test]
    fn test_point_in_rect_negative_coordinates() {
        let rect_center = Vec2::new(-50.0, -50.0);
        let rect_size = Vec2::new(40.0, 40.0);

        assert!(is_point_in_rect(Vec2::new(-50.0, -50.0), rect_center, rect_size));
        assert!(is_point_in_rect(Vec2::new(-70.0, -70.0), rect_center, rect_size));
        assert!(is_point_in_rect(Vec2::new(-30.0, -30.0), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(0.0, 0.0), rect_center, rect_size));
    }

    #[test]
    fn test_point_in_rect_asymmetric_rect() {
        let rect_center = Vec2::new(100.0, 100.0);
        let rect_size = Vec2::new(40.0, 20.0);

        assert!(is_point_in_rect(Vec2::new(100.0, 100.0), rect_center, rect_size));
        assert!(is_point_in_rect(Vec2::new(110.0, 105.0), rect_center, rect_size));
        assert!(is_point_in_rect(Vec2::new(90.0, 95.0), rect_center, rect_size));

        assert!(!is_point_in_rect(Vec2::new(121.0, 100.0), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(79.0, 100.0), rect_center, rect_size));

        assert!(!is_point_in_rect(Vec2::new(100.0, 111.0), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(100.0, 89.0), rect_center, rect_size));
    }

    #[test]
    fn test_point_in_rect_floating_point_precision() {
        let rect_center = Vec2::new(100.0, 100.0);
        let rect_size = Vec2::new(10.0, 10.0);

        assert!(is_point_in_rect(Vec2::new(104.999, 100.0), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(105.001, 100.0), rect_center, rect_size));
        assert!(is_point_in_rect(Vec2::new(100.0, 104.999), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(100.0, 105.001), rect_center, rect_size));
    }

    #[test]
    fn resize_handle_world_pos_at_identity_transform() {
        let transform = Transform::IDENTITY;
        let pos = resize_handle_world_pos(&transform);
        let half = TOKEN_SIZE / 2.0;
        assert!((pos.x - half.x).abs() < 1e-4);
        assert!((pos.y - (-half.y)).abs() < 1e-4);
    }

    #[test]
    fn rotate_handle_world_pos_at_identity_transform() {
        let transform = Transform::IDENTITY;
        let pos = rotate_handle_world_pos(&transform);
        assert!((pos.x).abs() < 1e-4);
        assert!((pos.y - ROTATE_HANDLE_OFFSET).abs() < 1e-4);
    }

    #[test]
    fn token_half_diagonal_matches_scale_one() {
        let transform = Transform::IDENTITY;
        let dist = resize_handle_world_pos(&transform).length();
        assert!((dist - token_half_diagonal()).abs() < 1e-4);
    }
}
