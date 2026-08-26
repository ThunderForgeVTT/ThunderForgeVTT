use crate::resources::{SceneData, SelectedToken};
use crate::{ActiveWorld, TokenIdentity, emit_event};
use bevy::prelude::*;
use serde_json::json;

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

#[cfg(test)]
mod tests {
    use super::*;

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
            .spawn((
                Transform::from_xyz(0.0, 0.0, 0.0),
                TokenIdentity("token-1".to_string()),
            ))
            .id();

        app.world_mut()
            .resource_mut::<SelectedToken>()
            .select("token-1".to_string());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowRight);

        app.update();

        let transform = app.world().get::<Transform>(token).unwrap();
        assert_eq!(transform.translation.x, 32.0);
        assert_eq!(transform.translation.y, 0.0);
    }
}
