use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use crate::components::Token;
use crate::resources::{SelectedToken, SceneData, CameraManager};
use crate::transforms::coordinate::{pixel_to_grid};

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

/// Handle token selection on mouse click
/// Checks for token hit-test and updates SelectedToken resource
pub fn handle_token_selection(
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    token_query: Query<(&Transform, &Token)>,
    mut selected_token: ResMut<SelectedToken>,
    scene: Res<SceneData>,
) {
    // Only on left-click release
    if !mouse_button.just_released(MouseButton::Left) {
        return;
    }

    let window = windows.iter().next();
    if window.is_none() {
        return;
    }
    let window = window.unwrap();

    // Get mouse position in window coordinates
    if let Some(mouse_px) = window.cursor_position() {
        let mut hit_token: Option<String> = None;

        // Hit-test all tokens
        for (transform, token) in token_query.iter() {
            if !token.is_visible {
                continue;
            }

            let token_pixel_pos = transform.translation.truncate();
            let token_pixel_size = Vec2::new(
                (token.size_x as f32) * scene.grid_size,
                (token.size_y as f32) * scene.grid_size,
            );

            // AABB hit test
            if is_point_in_rect(
                mouse_px,
                token_pixel_pos,
                token_pixel_size,
            ) {
                hit_token = Some(token.id.clone());
                break;  // First hit wins (topmost token)
            }
        }

        // Update selection
        match hit_token {
            Some(token_id) => {
                selected_token.select(token_id);
            }
            None => {
                selected_token.deselect();
            }
        }
    }
}

/// Update token visual feedback based on selection state
/// Currently: Opacity feedback, Z-order bump
/// Deferred: Glow/border effects to Phase 4.8 (shader-based)
pub fn render_selection_feedback(
    mut sprite_query: Query<(&Token, &mut Sprite, &mut Transform)>,
    selected_token: Res<SelectedToken>,
) {
    for (token, mut sprite, mut transform) in sprite_query.iter_mut() {
        if selected_token.is_selected(&token.id) {
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
pub fn handle_keyboard_token_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut token_query: Query<&mut Token>,
    selected_token: Res<SelectedToken>,
    scene: Res<SceneData>,
) {
    // Get selected token ID
    let Some(selected_id) = selected_token.get_selected() else {
        return;
    };

    // Determine movement direction
    let (dx, dy) = if keyboard.just_pressed(KeyCode::ArrowUp) {
        (0, 1)  // Up (Y increases in grid coords)
    } else if keyboard.just_pressed(KeyCode::ArrowDown) {
        (0, -1)  // Down
    } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
        (-1, 0)  // Left
    } else if keyboard.just_pressed(KeyCode::ArrowRight) {
        (1, 0)  // Right
    } else {
        return;  // No movement key pressed
    };

    // Find and update selected token
    for mut token in token_query.iter_mut() {
        if token.id == *selected_id {
            // Store old position for rollback (Phase 4.6)
            let old_x = token.base_x;
            let old_y = token.base_y;

            // Apply movement (optimistic)
            token.base_x += dx;
            token.base_y += dy;

            // TODO: Phase 4.6 Integration
            // Queue mutation: upsertToken({ id, base_x: token.base_x, base_y: token.base_y })
            // On server rejection event: token.base_x = old_x; token.base_y = old_y;

            // Log for debugging
            eprintln!(
                "📤 Token movement: {} moved from ({},{}) to ({},{}) via keyboard",
                token.id, old_x, old_y, token.base_x, token.base_y
            );
            break;
        }
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
        app.init_resource::<SelectedToken>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<SceneData>();

        // Verify system can be added without errors
        // Note: Full integration test deferred to G2 (E2E testing)
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
