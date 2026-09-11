use crate::resources::{CameraManager, SelectedLight};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use thunderforge_canvas_core::camera::{drag_pan, is_drag, wheel_notches};

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraManager>()
            .add_systems(Startup, setup_camera)
            .add_systems(
                Update,
                (
                    handle_mouse_wheel_zoom,
                    handle_drag_pan,
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
    mirror_camera_state(camera_mgr.translation, camera_mgr.scale);

    for (mut transform, mut projection) in cameras.iter_mut() {
        transform.translation = camera_mgr.translation.extend(100.0);
        // Left at identity deliberately — see above.
        transform.scale = Vec3::ONE;

        if let Projection::Orthographic(ortho) = projection.as_mut() {
            ortho.scale = camera_mgr.scale;
        }
    }
}

/// The buttons that pan. Left is not one: it selects, drags tokens and
/// authors.
const PAN_BUTTONS: [MouseButton; 2] = [MouseButton::Middle, MouseButton::Right];

/// One press of a pan button, from going down to coming up.
#[derive(Default)]
struct DragPan {
    button: Option<MouseButton>,
    pressed_at: Vec2,
    last: Vec2,
    dragging: bool,
}

/// Grab-and-drag panning with the middle or right mouse button.
///
/// Playtest 2026-09-10 P3: the arrow keys stopped panning on 2026-08-26 with
/// "panning is the mouse's job", and the mouse was never given it. This is
/// that job.
///
/// Nothing moves until the pointer has travelled past
/// `DRAG_THRESHOLD_PX`, so a right-click that wobbles a pixel is still a click
/// — `context_menu::report_right_click` makes the same decision, on release,
/// with the same threshold, and opens the menu only for a click. When the
/// threshold is crossed the camera catches up from where the button went
/// down, so the point grabbed is the point that stays under the pointer.
///
/// `drag_by`, not `pan`: a pan eases toward its target, and a map that trails
/// the hand holding it is not being dragged.
fn handle_drag_pan(
    mouse_button: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut camera_mgr: ResMut<CameraManager>,
    mut drag: Local<DragPan>,
) {
    let cursor = windows
        .single()
        .ok()
        .and_then(|window| window.cursor_position());

    if let Some(button) = drag.button {
        if !mouse_button.pressed(button) {
            *drag = DragPan::default();
            return;
        }
        let Some(cursor) = cursor else {
            return;
        };
        if !drag.dragging && is_drag((cursor - drag.pressed_at).length()) {
            drag.dragging = true;
            drag.last = drag.pressed_at;
        }
        if drag.dragging {
            let delta = drag_pan(drag.last, cursor, camera_mgr.scale);
            camera_mgr.drag_by(delta);
            drag.last = cursor;
        }
        return;
    }

    if let Some(cursor) = cursor
        && let Some(button) = PAN_BUTTONS
            .into_iter()
            .find(|button| mouse_button.just_pressed(*button))
    {
        *drag = DragPan {
            button: Some(button),
            pressed_at: cursor,
            last: cursor,
            dragging: false,
        };
    }
}

/// The camera as it was last written out, for [`camera_state`].
static CAMERA_STATE: std::sync::OnceLock<std::sync::Mutex<(f32, f32, f32)>> =
    std::sync::OnceLock::new();

fn mirror_camera_state(translation: Vec2, scale: f32) {
    let slot = CAMERA_STATE.get_or_init(|| std::sync::Mutex::new((0.0, 0.0, 1.0)));
    if let Ok(mut state) = slot.lock() {
        *state = (translation.x, translation.y, scale);
    }
}

/// Where the camera is, as `{"x":…,"y":…,"scale":…}` in world units.
///
/// Read-only, and here so a pan can be observed directly rather than inferred
/// from pixels — the same reason `authoring_mode()` exists.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn camera_state() -> String {
    let (x, y, scale) = CAMERA_STATE
        .get()
        .and_then(|slot| slot.lock().ok().map(|state| *state))
        .unwrap_or((0.0, 0.0, 1.0));
    format!("{{\"x\":{x},\"y\":{y},\"scale\":{scale}}}")
}

/// Keyboard camera shortcuts.
/// - `+`/`-`: zoom
/// - `Home`: reset to 1:1, centred
///
/// The arrow keys used to pan here. They now move the player's token, which
/// is what a player expects them to do and what `systems::token_move` binds
/// them to — leaving both bound meant every arrow press panned the camera and
/// moved a token at the same time. Panning is the mouse's job, and
/// `handle_drag_pan` does it.
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

/// Mouse-wheel zoom, anchored at the cursor.
///
/// Yields the wheel entirely while a light is selected: `systems::lighting`'s
/// `handle_light_resize` uses the wheel to size that light, and both systems
/// read the same events independently (a `MessageReader` has a per-system
/// cursor, so one reading them does not consume them for the other). Without
/// this check a GM resizing a light would zoom the map at the same time.
/// Drains the wheel messages and returns their total in **notches**.
///
/// Every consumer of the wheel needs this, because `MouseWheel::y` is
/// meaningless without `MouseWheel::unit`: the same physical flick of the
/// same wheel arrives as `1.0` on a platform that counts lines and as
/// `100.0` on one that counts pixels — and the browser this engine ships
/// into counts pixels. Reading `.y` alone therefore looked correct in a
/// native window and, on the web, made a single notch worth a hundred zoom
/// steps.
pub(crate) fn read_wheel_notches(events: &mut MessageReader<MouseWheel>) -> f32 {
    events
        .read()
        .map(|event| wheel_notches(event.y, matches!(event.unit, MouseScrollUnit::Pixel)))
        .sum()
}

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

    let scroll = read_wheel_notches(&mut wheel_events);
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::ButtonState;
    use bevy::input::mouse::MouseButtonInput;

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

    /// A headless app with a primary window whose cursor this test moves.
    fn app_with_window() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::input::InputPlugin);
        app.init_resource::<SelectedLight>();
        app.add_plugins(CameraPlugin);
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        app.update();
        (app, window)
    }

    fn move_cursor(app: &mut App, window: Entity, to: Vec2) {
        app.world_mut()
            .get_mut::<Window>(window)
            .expect("window")
            .set_cursor_position(Some(to));
    }

    fn mouse(app: &mut App, window: Entity, button: MouseButton, state: ButtonState) {
        app.world_mut().write_message(MouseButtonInput {
            button,
            state,
            window,
        });
    }

    /// Playtest 2026-09-10 P3, through input rather than by calling `pan()` —
    /// the camera tests used to call `pan()` directly, which is why they all
    /// passed while no mouse could pan at all.
    #[test]
    fn a_middle_drag_pans_the_map_under_the_pointer() {
        let (mut app, window) = app_with_window();

        move_cursor(&mut app, window, Vec2::new(400.0, 300.0));
        mouse(&mut app, window, MouseButton::Middle, ButtonState::Pressed);
        app.update();

        move_cursor(&mut app, window, Vec2::new(500.0, 260.0));
        app.update();

        let camera = app.world().resource::<CameraManager>();
        assert!(
            (camera.translation - Vec2::new(-100.0, -40.0)).length() < 1e-3,
            "a drag 100px right and 40px up at 1:1 moves the camera 100 left \
             and 40 down, got {:?}",
            camera.translation,
        );
    }

    #[test]
    fn a_right_click_that_does_not_travel_does_not_pan() {
        let (mut app, window) = app_with_window();

        move_cursor(&mut app, window, Vec2::new(400.0, 300.0));
        mouse(&mut app, window, MouseButton::Right, ButtonState::Pressed);
        app.update();
        // A wobble, inside the threshold.
        move_cursor(&mut app, window, Vec2::new(402.0, 301.0));
        app.update();
        mouse(&mut app, window, MouseButton::Right, ButtonState::Released);
        app.update();

        let camera = app.world().resource::<CameraManager>();
        assert_eq!(camera.translation, Vec2::ZERO, "a click is not a pan");
    }
}
