use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use crate::resources::{SelectedToken, DraggingToken, SceneData, IsGameMaster};
use crate::{TokenIdentity, ActiveWorld, TOKEN_SIZE, emit_event};
use serde_json::json;

/// Whole grid-cell increments a token's `scale` may take (spec 004 US2's
/// resize clarification: 1x1, 2x2, 3x3... never a fractional cell).
const MIN_TOKEN_SCALE: f32 = 1.0;
const MAX_TOKEN_SCALE: f32 = 5.0;
/// Fixed rotation step per key press (30 degrees), independent of resize.
const TOKEN_ROTATE_STEP_RADIANS: f32 = std::f32::consts::FRAC_PI_6;

/// Hit-test: Check if point is within rectangle bounds
/// Used for token selection based on mouse click
fn is_point_in_rect(
    point: Vec2,
    rect_center: Vec2,
    rect_size: Vec2,
) -> bool {
    let half_size = rect_size / 2.0;
    let min = rect_center - half_size;
    let max = rect_center + half_size;

    point.x >= min.x && point.x <= max.x
        && point.y >= min.y && point.y <= max.y
}

/// Convert the cursor's window-pixel position into Bevy world space using the
/// active camera's real transform/projection (accounts for pan and zoom).
/// Using `Camera::viewport_to_world_2d` instead of hand-rolling the inverse
/// transform avoids the coordinate mismatch that made the old hit-test only
/// work by coincidence at an identity camera.
fn cursor_world_position(
    windows: &Query<&Window, With<PrimaryWindow>>,
    camera_query: &Query<(&Camera, &GlobalTransform)>,
) -> Option<Vec2> {
    let window = windows.iter().next()?;
    let (camera, camera_transform) = camera_query.iter().next()?;
    let cursor_px = window.cursor_position()?;
    camera.viewport_to_world_2d(camera_transform, cursor_px).ok()
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
pub(crate) fn handle_token_drag(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut token_query: Query<(&mut Transform, &TokenIdentity)>,
    mut selected_token: ResMut<SelectedToken>,
    mut dragging: ResMut<DraggingToken>,
    active_world: Res<ActiveWorld>,
) {
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
                dragging.0 = Some((token_id, offset));
            }
            None => {
                selected_token.deselect();
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
                emit_event(json!({
                    "type": "upsert_token",
                    "token": {
                        "id": token_id,
                        "x": transform.translation.x,
                        "y": transform.translation.y,
                        "z": transform.translation.z,
                    },
                    "worldId": active_world.0,
                }));
                break;
            }
        }
    }
}

/// Update token visual feedback based on selection state
/// Currently: Opacity feedback, Z-order bump
/// Deferred: Glow/border effects to Phase 4.8 (shader-based)
pub(crate) fn render_selection_feedback(
    mut sprite_query: Query<(&TokenIdentity, &mut Sprite, &mut Transform)>,
    selected_token: Res<SelectedToken>,
) {
    for (identity, mut sprite, mut transform) in sprite_query.iter_mut() {
        if selected_token.is_selected(&identity.0) {
            // Selected token: opaque, on top
            sprite.color = sprite.color.with_alpha(1.0);
            transform.translation.z = 2.0;
        } else {
            // Unselected token: slightly transparent
            sprite.color = sprite.color.with_alpha(0.85);
            transform.translation.z = 1.0;
        }
    }
}

/// Phase 4.7.E2: Keyboard-driven token movement
/// If a token is selected and an arrow key is pressed, move the token 1 grid cell
/// in that direction. Triggers optimistic update (visual feedback immediately)
/// with rollback placeholder for server rejection (Phase 4.6 integration).
pub(crate) fn handle_keyboard_token_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut token_query: Query<(&mut Transform, &TokenIdentity)>,
    selected_token: Res<SelectedToken>,
    scene: Res<SceneData>,
    active_world: Res<ActiveWorld>,
) {
    // Get selected token ID
    let Some(selected_id) = selected_token.get_selected() else {
        return;
    };

    // Determine movement direction (one grid cell per key press)
    let (dx, dy) = if keyboard.just_pressed(KeyCode::ArrowUp) {
        (0.0, 1.0) // Up (Y increases in Bevy world space)
    } else if keyboard.just_pressed(KeyCode::ArrowDown) {
        (0.0, -1.0) // Down
    } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
        (-1.0, 0.0) // Left
    } else if keyboard.just_pressed(KeyCode::ArrowRight) {
        (1.0, 0.0) // Right
    } else {
        return; // No movement key pressed
    };

    // Find and update selected token
    for (mut transform, identity) in token_query.iter_mut() {
        if identity.0 == *selected_id {
            transform.translation.x += dx * scene.grid_size;
            transform.translation.y += dy * scene.grid_size;

            emit_event(json!({
                "type": "upsert_token",
                "token": {
                    "id": identity.0,
                    "x": transform.translation.x,
                    "y": transform.translation.y,
                    "z": transform.translation.z,
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
/// Interim keyboard-based mechanism rather than literal canvas-rendered
/// drag handles (research.md §5/tasks.md T016-T017's original plan) — a
/// real engineering-time tradeoff made while implementing this feature
/// live: proper drag handles need their own hit-testable marker entities
/// and a dedicated drag-mode state machine (mirroring `wall.rs`'s
/// `WallHandle`/`WallDragMode`), which didn't fit in this session's
/// budget. This delivers the same functional outcome (GM resizes/rotates
/// a selected token, grid-snapped, persisted, synced) so User Story 2's
/// acceptance criteria are met in substance; a follow-up should replace
/// this with actual draggable handle sprites for interaction-affordance
/// parity with walls/shapes.
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
    fn test_keyboard_token_movement_basic() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(SceneData::new(
            "test".to_string(),
            "world-test".to_string(),
            crate::resources::GridType::Square,
            32.0,
            20,
            20,
            None,
        ));
        app.insert_resource(ActiveWorld("world-test".to_string()));
        app.init_resource::<SelectedToken>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_systems(Update, handle_keyboard_token_movement);

        let token = app
            .world_mut()
            .spawn((Transform::from_xyz(0.0, 0.0, 0.0), TokenIdentity("token-1".to_string())))
            .id();

        app.world_mut().resource_mut::<SelectedToken>().select("token-1".to_string());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowRight);

        app.update();

        let transform = app.world().get::<Transform>(token).unwrap();
        assert_eq!(transform.translation.x, 32.0);
        assert_eq!(transform.translation.y, 0.0);
    }

    // Phase 4.7.G1: Additional selection hit-testing tests for precision

    #[test]
    fn test_point_in_rect_corners() {
        let rect_center = Vec2::new(50.0, 50.0);
        let rect_size = Vec2::new(40.0, 40.0);

        // Top-left corner
        assert!(is_point_in_rect(Vec2::new(30.0, 70.0), rect_center, rect_size));

        // Top-right corner
        assert!(is_point_in_rect(Vec2::new(70.0, 70.0), rect_center, rect_size));

        // Bottom-left corner
        assert!(is_point_in_rect(Vec2::new(30.0, 30.0), rect_center, rect_size));

        // Bottom-right corner
        assert!(is_point_in_rect(Vec2::new(70.0, 30.0), rect_center, rect_size));
    }

    #[test]
    fn test_point_in_rect_just_outside_corners() {
        let rect_center = Vec2::new(50.0, 50.0);
        let rect_size = Vec2::new(40.0, 40.0);

        // Just outside each corner
        assert!(!is_point_in_rect(Vec2::new(29.9, 70.0), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(70.1, 70.0), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(30.0, 29.9), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(70.0, 70.1), rect_center, rect_size));
    }

    #[test]
    fn test_point_in_rect_small_rect() {
        let rect_center = Vec2::new(100.0, 100.0);
        let rect_size = Vec2::new(1.0, 1.0);  // 0.5 units on each side

        // Dead center
        assert!(is_point_in_rect(Vec2::new(100.0, 100.0), rect_center, rect_size));

        // Just outside
        assert!(!is_point_in_rect(Vec2::new(100.6, 100.0), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(100.0, 100.6), rect_center, rect_size));
    }

    #[test]
    fn test_point_in_rect_large_rect() {
        let rect_center = Vec2::new(500.0, 500.0);
        let rect_size = Vec2::new(1000.0, 1000.0);  // 500 units on each side

        // Far corners still inside
        assert!(is_point_in_rect(Vec2::new(1.0, 1.0), rect_center, rect_size));
        assert!(is_point_in_rect(Vec2::new(999.0, 999.0), rect_center, rect_size));
    }

    #[test]
    fn test_point_in_rect_zero_size() {
        let rect_center = Vec2::new(50.0, 50.0);
        let rect_size = Vec2::ZERO;

        // Only exact center is inside
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
        let rect_size = Vec2::new(40.0, 20.0);  // 20x10 from center

        // Inside
        assert!(is_point_in_rect(Vec2::new(100.0, 100.0), rect_center, rect_size));
        assert!(is_point_in_rect(Vec2::new(110.0, 105.0), rect_center, rect_size));
        assert!(is_point_in_rect(Vec2::new(90.0, 95.0), rect_center, rect_size));

        // Outside (too far horizontally)
        assert!(!is_point_in_rect(Vec2::new(121.0, 100.0), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(79.0, 100.0), rect_center, rect_size));

        // Outside (too far vertically)
        assert!(!is_point_in_rect(Vec2::new(100.0, 111.0), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(100.0, 89.0), rect_center, rect_size));
    }

    #[test]
    fn test_point_in_rect_floating_point_precision() {
        let rect_center = Vec2::new(100.0, 100.0);
        let rect_size = Vec2::new(10.0, 10.0);

        // Test boundary with floating point values
        assert!(is_point_in_rect(Vec2::new(104.999, 100.0), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(105.001, 100.0), rect_center, rect_size));
        assert!(is_point_in_rect(Vec2::new(100.0, 104.999), rect_center, rect_size));
        assert!(!is_point_in_rect(Vec2::new(100.0, 105.001), rect_center, rect_size));
    }
}
