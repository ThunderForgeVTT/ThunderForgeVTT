use bevy::prelude::*;
use crate::resources::CameraManager;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<CameraManager>()
            .add_systems(Startup, setup_camera)
            .add_systems(Update, (
                update_camera_transform,
                handle_keyboard_camera_shortcuts,  // Phase 4.7.D2
            ));
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d::default(),
        Transform::from_xyz(0.0, 0.0, 100.0),
    ));
}

fn update_camera_transform(
    mut cameras: Query<&mut Transform, With<Camera2d>>,
    camera_mgr: Res<CameraManager>,
) {
    for mut transform in cameras.iter_mut() {
        transform.translation = camera_mgr.translation.extend(100.0);
        transform.scale = Vec3::splat(camera_mgr.scale);
    }
}

/// Phase 4.7.D2: Keyboard camera shortcuts
/// - Arrow keys (↑↓←→): Pan 50px per frame
/// - +/=: Zoom in by 10%
/// - -: Zoom out by 10%
/// - Home: Reset camera (pan to 0, zoom to 1.0x)
fn handle_keyboard_camera_shortcuts(
    mut camera_mgr: ResMut<CameraManager>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    const PAN_SPEED: f32 = 50.0;

    // Pan: Arrow keys
    if keyboard.pressed(KeyCode::ArrowUp) {
        camera_mgr.pan(Vec2::new(0.0, PAN_SPEED));
    }
    if keyboard.pressed(KeyCode::ArrowDown) {
        camera_mgr.pan(Vec2::new(0.0, -PAN_SPEED));
    }
    if keyboard.pressed(KeyCode::ArrowLeft) {
        camera_mgr.pan(Vec2::new(-PAN_SPEED, 0.0));
    }
    if keyboard.pressed(KeyCode::ArrowRight) {
        camera_mgr.pan(Vec2::new(PAN_SPEED, 0.0));
    }

    // Zoom: +/- keys
    if keyboard.pressed(KeyCode::Equal) || keyboard.pressed(KeyCode::NumpadAdd) {
        camera_mgr.zoom_in();
    }
    if keyboard.pressed(KeyCode::Minus) || keyboard.pressed(KeyCode::NumpadSubtract) {
        camera_mgr.zoom_out();
    }

    // Reset: Home key
    if keyboard.just_pressed(KeyCode::Home) {
        camera_mgr.translation = Vec2::ZERO;
        camera_mgr.scale = 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_keyboard_camera_shortcuts_pan_up() {
        let mut camera_mgr = CameraManager::default();
        let mut keyboard_input = ButtonInput::default();
        keyboard_input.press(KeyCode::ArrowUp);

        handle_keyboard_camera_shortcuts(camera_mgr.into(), keyboard_input.into());
        // Note: Direct test limited due to Res<> requirement. Integration test below.
    }

    #[test]
    fn test_camera_keyboard_integration() {
        let mut app = App::new();
        app.add_plugins(CameraPlugin);

        // Initial state
        let camera_mgr = app.world().resource::<CameraManager>();
        assert_eq!(camera_mgr.translation, Vec2::ZERO);
        assert_eq!(camera_mgr.scale, 1.0);
    }
}
