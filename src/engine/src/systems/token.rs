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

use crate::resources::{
    CanvasLayer, DraggingToken, IsGameMaster, SceneGrid, SelectedToken, TokenGridBehaviour,
};
use crate::{ActiveWorld, TOKEN_SIZE, TokenIdentity, emit_event};
use thunderforge_canvas_core::grid::Footprint;
use thunderforge_canvas_core::token_stack::{StackCandidate, tokens_at};

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
    camera
        .viewport_to_world_2d(camera_transform, cursor_px)
        .ok()
}

/// Notifies the frontend of the full selection, topmost first.
///
/// Emitted alongside the single-token `select_token` below rather than
/// replacing it: every existing consumer reads the primary, and breaking
/// them to add stacks would be a much larger change than this needs to be.
fn emit_stack_selection(token_ids: &[String]) {
    emit_event(json!({
        "type": "select_tokens",
        "tokenIds": token_ids,
    }));
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
    transform.translation.truncate() + Vec2::from_angle(rotation_radians).rotate(local_offset)
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
    mut token_query: Query<(&mut Transform, &TokenIdentity, Option<&TokenGridBehaviour>)>,
    mut selected_token: ResMut<SelectedToken>,
    mut dragging: ResMut<DraggingToken>,
    active_world: Res<ActiveWorld>,
    drag_state: Res<TokenDragState>,
    // Optional for the same reason every plugin-owned resource in the
    // command loop is: without `GridPlugin` there is no scene grid, and the
    // hit area falls back to the default token size rather than panicking.
    grid: Option<Res<SceneGrid>>,
) {
    if drag_state.mode != TokenDragMode::Idle {
        return;
    }

    let Some(cursor_world) = cursor_world_position(&windows, &camera_query) else {
        return;
    };

    if mouse_button.just_pressed(MouseButton::Left) {
        // The hit area is the token's grid footprint, matching how
        // `size_tokens_to_grid` sizes it. It used to be the fixed
        // `TOKEN_SIZE` constant, which is wrong in both directions on any
        // scene whose grid is not that size — on a 5px-per-cell scene it
        // made every token an oversized click target that overlapped its
        // neighbours.
        let candidates: Vec<StackCandidate> = token_query
            .iter()
            .map(|(transform, identity, behaviour)| {
                let footprint = behaviour.map_or_else(Footprint::default, |b| b.footprint);
                let side = grid
                    .as_ref()
                    .map(|grid| footprint.world_size(grid.size))
                    .unwrap_or(TOKEN_SIZE.y);
                StackCandidate {
                    id: identity.0.clone(),
                    center: transform.translation.truncate(),
                    footprint_side: side,
                    z: transform.translation.z,
                }
            })
            .collect();

        let stack = tokens_at(&candidates, cursor_world);

        if stack.is_empty() {
            selected_token.deselect();
            emit_token_selection(None);
            emit_stack_selection(&[]);
            dragging.0.clear();
            return;
        }

        // Double-click — "which one of these?" — is detected in the
        // frontend, not here. Two clicks that fast frequently land in the
        // same frame, where Bevy's `just_pressed` sees one press and the
        // second is simply lost; the DOM's own `dblclick` has no such
        // problem. The engine's job is the hit test, which the frontend
        // cannot do: it owns the camera and the true transforms.

        // Single click takes the whole stack. Dragging one token out of a
        // pile is the rarer intent — that is what the picker is for — while
        // "move these out of the doorway" is the common one.
        selected_token.select_stack(stack.clone());
        emit_token_selection(stack.first().map(String::as_str));
        emit_stack_selection(&stack);

        dragging.0 = token_query
            .iter()
            .filter(|(_, identity, _)| stack.contains(&identity.0))
            .map(|(transform, identity, _)| {
                (
                    identity.0.clone(),
                    transform.translation.truncate() - cursor_world,
                )
            })
            .collect();
        return;
    }

    if mouse_button.pressed(MouseButton::Left) {
        if dragging.0.is_empty() {
            return;
        }
        // Each member keeps its own offset, so a stack that was not
        // perfectly co-located stays in the arrangement it was picked up in.
        for (mut transform, identity, _) in token_query.iter_mut() {
            if let Some((_, offset)) = dragging.0.iter().find(|(id, _)| *id == identity.0) {
                let new_pos = cursor_world + *offset;
                transform.translation.x = new_pos.x;
                transform.translation.y = new_pos.y;
            }
        }
        return;
    }

    if mouse_button.just_released(MouseButton::Left) {
        let dragged = std::mem::take(&mut dragging.0);
        if dragged.is_empty() {
            return;
        }

        for (transform, identity, _) in token_query.iter() {
            if !dragged.iter().any(|(id, _)| *id == identity.0) {
                continue;
            }
            // Include scale/rotation, not just position: this event
            // fires on *every* select-click release, not only a real
            // drag (dragging is set on press, released here even
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
            //
            // One event per dragged token: the mutation bridge is
            // keyed by token id, and a stack move is genuinely N
            // separate persisted changes.
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
            let new_scale =
                (transform.scale.x + resize_delta).clamp(MIN_TOKEN_SCALE, MAX_TOKEN_SCALE);
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
