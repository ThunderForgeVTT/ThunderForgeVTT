use super::*;
use bevy::input::touch::{TouchInput, TouchPhase};

const A: Vec2 = Vec2::new(100.0, 100.0);

fn left() -> Option<MouseButton> {
    Some(MouseButton::Left)
}

#[test]
fn one_finger_is_the_left_button_where_the_finger_is() {
    let mut gesture = Gesture::default();

    let down = step(&mut gesture, &[(1, A)], 0.0);
    assert_eq!(down.pointer, Some(A));
    assert_eq!(down.press, left());
    assert_eq!(down.release, None);

    let to = Vec2::new(160.0, 100.0);
    let moved = step(&mut gesture, &[(1, to)], 0.1);
    assert_eq!(moved.pointer, Some(to));
    assert_eq!((moved.press, moved.release), (None, None));
}

#[test]
fn lifting_releases_where_the_finger_last_was_then_the_pointer_goes() {
    let mut gesture = Gesture::default();
    step(&mut gesture, &[(1, A)], 0.0);
    let to = Vec2::new(160.0, 100.0);
    step(&mut gesture, &[(1, to)], 0.1);

    let lifted = step(&mut gesture, &[], 0.2);
    // The release needs somewhere to land: a token is dropped at the pointer.
    assert_eq!(lifted.pointer, Some(to));
    assert_eq!(lifted.release, left());

    let after = step(&mut gesture, &[], 0.3);
    assert_eq!(after, Step::default());
    assert_eq!(gesture, Gesture::Idle);
}

#[test]
fn a_finger_held_still_becomes_a_right_click() {
    let mut gesture = Gesture::default();
    step(&mut gesture, &[(1, A)], 0.0);

    let early = step(&mut gesture, &[(1, A)], LONG_PRESS_SECS - 0.1);
    assert_eq!((early.press, early.release), (None, None));

    let held = step(&mut gesture, &[(1, A)], LONG_PRESS_SECS);
    assert_eq!(held.release, left());
    assert_eq!(held.press, Some(MouseButton::Right));

    // Once, not every frame after.
    let still = step(&mut gesture, &[(1, A)], LONG_PRESS_SECS + 0.1);
    assert_eq!((still.press, still.release), (None, None));

    let lifted = step(&mut gesture, &[], LONG_PRESS_SECS + 0.2);
    assert_eq!(lifted.release, Some(MouseButton::Right));
    assert_eq!(lifted.pointer, Some(A));
}

#[test]
fn a_finger_that_dragged_is_never_a_long_press_even_back_where_it_started() {
    let mut gesture = Gesture::default();
    step(&mut gesture, &[(1, A)], 0.0);
    step(&mut gesture, &[(1, A + Vec2::new(40.0, 0.0))], 0.1);

    let back = step(&mut gesture, &[(1, A)], LONG_PRESS_SECS + 1.0);
    assert_eq!((back.press, back.release), (None, None));
}

#[test]
fn two_fingers_pan_by_their_midpoint_and_zoom_by_their_spread() {
    let mut gesture = Gesture::default();
    let (a, b) = (Vec2::new(100.0, 100.0), Vec2::new(200.0, 100.0));

    let down = step(&mut gesture, &[(1, a), (2, b)], 0.0);
    // Landing is not moving, and two fingers are never a button.
    assert_eq!(down, Step::default());

    // Both slide 30 right and spread to twice as far apart.
    let (a2, b2) = (Vec2::new(80.0, 100.0), Vec2::new(280.0, 100.0));
    let moved = step(&mut gesture, &[(1, a2), (2, b2)], 0.1);
    assert_eq!(
        moved.pan,
        Some((Vec2::new(150.0, 100.0), Vec2::new(180.0, 100.0)))
    );
    assert_eq!(moved.pinch, Some((Vec2::new(180.0, 100.0), 2.0)));
    assert_eq!(
        (moved.pointer, moved.press, moved.release),
        (None, None, None)
    );
}

#[test]
fn a_second_finger_lets_go_of_what_the_first_was_holding() {
    let mut gesture = Gesture::default();
    step(&mut gesture, &[(1, A)], 0.0);

    let joined = step(&mut gesture, &[(1, A), (2, Vec2::new(200.0, 100.0))], 0.1);
    assert_eq!(joined.release, left());
    assert_eq!(joined.pointer, Some(A));
    assert_eq!(joined.pan, None);
    assert!(matches!(gesture, Gesture::Two { .. }));
}

#[test]
fn the_finger_left_after_a_pinch_presses_nothing() {
    let mut gesture = Gesture::default();
    step(&mut gesture, &[(1, A), (2, Vec2::new(200.0, 100.0))], 0.0);

    let one_left = step(&mut gesture, &[(2, Vec2::new(210.0, 100.0))], 0.1);
    assert_eq!(one_left, Step::default());
    let still = step(&mut gesture, &[(2, Vec2::new(260.0, 100.0))], 5.0);
    assert_eq!(still, Step::default());

    step(&mut gesture, &[], 5.1);
    assert_eq!(gesture, Gesture::Idle);
    // And the next finger is a press again.
    assert_eq!(step(&mut gesture, &[(3, A)], 5.2).press, left());
}

#[test]
fn spreading_zooms_in_and_closing_zooms_out_by_the_same_amount() {
    assert!(pinch_steps(2.0) > 0.0);
    assert!((pinch_steps(2.0) + pinch_steps(0.5)).abs() < 1e-5);
    assert_eq!(pinch_steps(1.0), 0.0);
    // One step is one `ZOOM_STEP`.
    assert!((pinch_steps(ZOOM_STEP) - 1.0).abs() < 1e-5);
}

/// A headless app that is sent touches the way the window backend sends them.
fn app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::input::InputPlugin);
    app.init_resource::<CameraManager>();
    app.add_plugins(TouchInputPlugin);
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    app.update();
    (app, window)
}

fn touch(app: &mut App, window: Entity, id: u64, phase: TouchPhase, position: Vec2) {
    app.world_mut().write_message(TouchInput {
        phase,
        position,
        window,
        force: None,
        id,
    });
}

#[test]
fn a_touch_reaches_update_as_a_left_press_at_the_finger() {
    let (mut app, window) = app();

    #[derive(Resource, Default)]
    struct Seen(Vec<(bool, bool, Option<Vec2>)>);
    app.init_resource::<Seen>();
    app.add_systems(
        Update,
        |mouse: Res<ButtonInput<MouseButton>>, pointer: Pointer, mut seen: ResMut<Seen>| {
            seen.0.push((
                mouse.just_pressed(MouseButton::Left),
                mouse.just_released(MouseButton::Left),
                pointer.position(),
            ));
        },
    );

    touch(&mut app, window, 7, TouchPhase::Started, A);
    app.update();
    let to = Vec2::new(140.0, 120.0);
    touch(&mut app, window, 7, TouchPhase::Moved, to);
    app.update();
    touch(&mut app, window, 7, TouchPhase::Ended, to);
    app.update();
    app.update();

    let seen = &app.world().resource::<Seen>().0;
    assert_eq!(
        seen.as_slice(),
        [
            (true, false, Some(A)),
            (false, false, Some(to)),
            (false, true, Some(to)),
            (false, false, None),
        ]
    );
    assert!(
        !app.world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left)
    );
}

#[test]
fn two_fingers_dragging_move_the_camera_and_press_nothing() {
    let (mut app, window) = app();
    let (a, b) = (Vec2::new(100.0, 100.0), Vec2::new(200.0, 100.0));

    touch(&mut app, window, 1, TouchPhase::Started, a);
    touch(&mut app, window, 2, TouchPhase::Started, b);
    app.update();
    let by = Vec2::new(50.0, 0.0);
    touch(&mut app, window, 1, TouchPhase::Moved, a + by);
    touch(&mut app, window, 2, TouchPhase::Moved, b + by);
    app.update();

    let camera = app.world().resource::<CameraManager>();
    // Fingers right, map right, camera left: the point grabbed stays grabbed.
    assert_eq!(camera.translation, Vec2::new(-50.0, 0.0));
    assert_eq!(camera.target_scale, 1.0);
    assert!(
        !app.world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left)
    );
    assert_eq!(app.world().resource::<TouchPointer>().position, None);
}

#[test]
fn with_no_finger_the_pointer_is_the_mouse() {
    let (mut app, window) = app();
    let at = Vec2::new(33.0, 44.0);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(at));

    #[derive(Resource, Default)]
    struct Seen(Option<Vec2>);
    app.init_resource::<Seen>();
    app.add_systems(Update, |pointer: Pointer, mut seen: ResMut<Seen>| {
        seen.0 = pointer.position();
    });
    app.update();
    assert_eq!(app.world().resource::<Seen>().0, Some(at));

    // A finger wins while it is down.
    touch(&mut app, window, 1, TouchPhase::Started, A);
    app.update();
    assert_eq!(app.world().resource::<Seen>().0, Some(A));
}
