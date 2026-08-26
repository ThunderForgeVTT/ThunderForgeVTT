use crate::resources::{CameraManager, SelectedLight};
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraManager>()
            .add_systems(Startup, setup_camera)
            .add_systems(
                Update,
                (
                    handle_mouse_wheel_zoom,
                    handle_keyboard_camera_shortcuts, // Phase 4.7.D2
                    // Last, so a zoom applied this frame reaches the camera
                    // in the same frame rather than a frame late.
                    update_camera_transform,
                )
                    .chain(),
            );
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((Camera2d, Transform::from_xyz(0.0, 0.0, 100.0)));
}

/// Pushes `CameraManager` onto the actual camera.
///
/// Zoom is applied to the **orthographic projection's** scale, not the
/// camera's `Transform.scale`. Both visually zoom, but only the projection is
/// reflected in `OrthographicProjection::area` — and `area` is what
/// `plugins::grid` and `plugins::darkness` use to work out which cells and
/// which lights are on screen. Zooming via the transform left both reading the
/// un-zoomed rectangle, so the grid would stop short of the viewport edge and
/// the darkness quad would be sized for the wrong area. Keeping zoom in the
/// projection makes those culls correct by construction.
fn update_camera_transform(
    mut cameras: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    mut camera_mgr: ResMut<CameraManager>,
    time: Res<Time>,
) {
    // Ease toward the requested camera before writing it out, so zoom and pan
    // glide instead of snapping. See `CameraManager::advance`.
    camera_mgr.advance(time.delta_secs());

    for (mut transform, mut projection) in cameras.iter_mut() {
        transform.translation = camera_mgr.translation.extend(100.0);
        // Left at identity deliberately — see above.
        transform.scale = Vec3::ONE;

        if let Projection::Orthographic(ortho) = projection.as_mut() {
            ortho.scale = camera_mgr.scale;
        }
    }
}

/// Keyboard camera shortcuts.
/// - `+`/`-`: zoom
/// - `Home`: reset to 1:1, centred
///
/// The arrow keys used to pan here. They now move the player's token, which
/// is what a player expects them to do and what `systems::token_move` binds
/// them to — leaving both bound meant every arrow press panned the camera and
/// moved a token at the same time. Panning is the mouse's job.
fn handle_keyboard_camera_shortcuts(
    mut camera_mgr: ResMut<CameraManager>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    // Zoom: +/- keys. Fractional steps because this fires every frame a key
    // is held; a full step per frame would cross the whole range in well
    // under a second.
    if keyboard.pressed(KeyCode::Equal) || keyboard.pressed(KeyCode::NumpadAdd) {
        camera_mgr.zoom_by(0.25);
    }
    if keyboard.pressed(KeyCode::Minus) || keyboard.pressed(KeyCode::NumpadSubtract) {
        camera_mgr.zoom_by(-0.25);
    }

    // Reset: Home key
    if keyboard.just_pressed(KeyCode::Home) {
        camera_mgr.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A prior version of this test tried to call `handle_keyboard_camera_shortcuts`
    // directly with `.into()`-converted `Res`/`ResMut` values, which isn't
    // constructible outside a running `App` — it asserted nothing and always
    // passed. `test_camera_keyboard_integration` below exercises the same
    // behavior through a real `App`, which is the only way to drive `Res<>`.

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

/// Mouse-wheel zoom, anchored at the cursor.
///
/// Yields the wheel entirely while a light is selected: `systems::lighting`'s
/// `handle_light_resize` uses the wheel to size that light, and both systems
/// read the same events independently (a `MessageReader` has a per-system
/// cursor, so one reading them does not consume them for the other). Without
/// this check a GM resizing a light would zoom the map at the same time.
fn handle_mouse_wheel_zoom(
    mut wheel_events: MessageReader<MouseWheel>,
    mut camera_mgr: ResMut<CameraManager>,
    selected_light: Res<SelectedLight>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
) {
    if selected_light.get_selected().is_some() {
        wheel_events.clear();
        return;
    }

    let scroll: f32 = wheel_events.read().map(|event| event.y).sum();
    if scroll == 0.0 {
        return;
    }

    // Anchor on the world point under the cursor so it stays put while
    // zooming. With no cursor position available — pointer outside the window,
    // or no window at all — fall back to a plain centre zoom rather than
    // skipping the input.
    let anchor = windows
        .single()
        .ok()
        .and_then(|window| window.cursor_position())
        .and_then(|cursor| {
            let (camera, camera_transform) = cameras.single().ok()?;
            camera.viewport_to_world_2d(camera_transform, cursor).ok()
        });

    match anchor {
        Some(anchor) => camera_mgr.zoom_toward(anchor, scroll),
        None => camera_mgr.zoom_by(scroll),
    }
}
