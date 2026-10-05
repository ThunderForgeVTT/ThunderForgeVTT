//! Fingers on the board (spec 069).
//!
//! Every system that points at the board was written for a mouse: it reads
//! `ButtonInput<MouseButton>` and the window's cursor. A browser does not
//! turn a finger into either — it arrives as a touch, which nothing here read,
//! so on a tablet the board could not be panned, zoomed or played on at all.
//!
//! This plugin does not teach each of those systems about touch. It
//! translates, once, in `PreUpdate`, into what they already read:
//!
//! | On the glass | What the board is told |
//! | --- | --- |
//! | One finger | The pointer is there, and the left button is down |
//! | One finger held still | The left button comes up and the right goes down, so lifting it is a right-click and opens the menu |
//! | Two fingers | Neither button. The camera pans by how their midpoint moved and zooms by how their spread changed |
//!
//! # Why the pointer is a resource and not the window's cursor
//!
//! Writing the touch into `Window`'s cursor position would have needed no
//! change anywhere else. But a cursor position set from inside the app is one
//! the window backend tries to *apply* — to move the real cursor there — and a
//! browser refuses, with an error logged for every frame of every drag. So
//! the touch goes into [`TouchPointer`], and [`Pointer`] is what a system
//! asks instead of the window: the finger if there is one, the cursor if not.

use bevy::ecs::system::SystemParam;
use bevy::input::InputSystems;
use bevy::input::touch::Touches;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use thunderforge_canvas_core::camera::{ZOOM_STEP, drag_pan, is_drag};

use crate::resources::CameraManager;

/// How long one still finger is held before it becomes a right-click.
pub(crate) const LONG_PRESS_SECS: f32 = 0.5;

/// Two fingers closer than this are not asked how their spread changed: the
/// ratio of two tiny distances is noise, and it would be read as a zoom.
const MIN_SPREAD_PX: f32 = 8.0;

/// Where a finger is pointing, in the window's logical pixels; `None` when
/// no finger is.
#[derive(Resource, Default, Debug, PartialEq)]
pub struct TouchPointer {
    pub position: Option<Vec2>,
}

/// Where the person is pointing: a finger if one is down, the mouse if not.
///
/// What a system takes in place of the primary window when all it wants from
/// the window is the cursor.
#[derive(SystemParam)]
pub struct Pointer<'w, 's> {
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    // `Option`: the plugin is independently addable, as every other is, and a
    // board without it is a board with a mouse.
    touch: Option<Res<'w, TouchPointer>>,
}

impl Pointer<'_, '_> {
    /// In the window's logical pixels, origin top left.
    pub fn position(&self) -> Option<Vec2> {
        self.touch
            .as_ref()
            .and_then(|touch| touch.position)
            .or_else(|| self.windows.iter().next()?.cursor_position())
    }
}

/// What the fingers are in the middle of.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) enum Gesture {
    #[default]
    Idle,
    /// One finger, holding a mouse button down for it.
    One {
        id: u64,
        pressed_at: Vec2,
        last: Vec2,
        since: f32,
        /// The furthest it got from where it landed, not where it is: a
        /// finger that wanders back was still dragging.
        travelled: f32,
        button: MouseButton,
    },
    /// The finger lifted last frame. The pointer stayed where it was for the
    /// frame the button came up in, so the release has somewhere to land.
    Lifted,
    /// Two fingers, moving the camera.
    Two { midpoint: Vec2, spread: f32 },
    /// A two-finger gesture one finger has left. The finger still down is the
    /// tail of a pinch, not a press, and does nothing until every finger is up.
    Spent,
}

/// What one frame of fingers asks of the board.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Step {
    pub pointer: Option<Vec2>,
    pub release: Option<MouseButton>,
    pub press: Option<MouseButton>,
    /// Screen pixels the two-finger midpoint moved, from and to.
    pub pan: Option<(Vec2, Vec2)>,
    /// The midpoint, and the spread now over the spread before.
    pub pinch: Option<(Vec2, f32)>,
}

fn two(fingers: &[(u64, Vec2)]) -> (Vec2, f32) {
    let (a, b) = (fingers[0].1, fingers[1].1);
    ((a + b) / 2.0, a.distance(b))
}

/// Advance the gesture by one frame.
///
/// `fingers` are the touches down this frame, in a stable order. Pure, so the
/// rules in the module's table can be tested without a browser to touch.
pub(crate) fn step(gesture: &mut Gesture, fingers: &[(u64, Vec2)], now: f32) -> Step {
    let mut out = Step::default();
    let begin_two = |fingers: &[(u64, Vec2)]| {
        let (midpoint, spread) = two(fingers);
        Gesture::Two { midpoint, spread }
    };

    *gesture = match (gesture.clone(), fingers.len()) {
        (
            Gesture::One {
                id,
                pressed_at,
                since,
                travelled,
                button,
                ..
            },
            1,
        ) if fingers[0].0 == id => {
            let at = fingers[0].1;
            let travelled = travelled.max(at.distance(pressed_at));
            out.pointer = Some(at);
            let mut button = button;
            if button == MouseButton::Left && !is_drag(travelled) && now - since >= LONG_PRESS_SECS
            {
                out.release = Some(MouseButton::Left);
                out.press = Some(MouseButton::Right);
                button = MouseButton::Right;
            }
            Gesture::One {
                id,
                pressed_at,
                last: at,
                since,
                travelled,
                button,
            }
        }
        // The finger lifted, was replaced, or was joined by another. Whatever
        // it was holding is let go where it last was.
        (Gesture::One { last, button, .. }, n) => {
            out.pointer = Some(last);
            out.release = Some(button);
            match n {
                0 => Gesture::Lifted,
                1 => Gesture::Spent,
                _ => begin_two(fingers),
            }
        }
        (Gesture::Two { midpoint, spread }, n) if n >= 2 => {
            let (now_mid, now_spread) = two(fingers);
            out.pan = Some((midpoint, now_mid));
            if spread >= MIN_SPREAD_PX && now_spread >= MIN_SPREAD_PX {
                out.pinch = Some((now_mid, now_spread / spread));
            }
            Gesture::Two {
                midpoint: now_mid,
                spread: now_spread,
            }
        }
        (Gesture::Two { .. } | Gesture::Spent, 1) => Gesture::Spent,
        (Gesture::Idle | Gesture::Lifted, 1) => {
            let (id, at) = fingers[0];
            out.pointer = Some(at);
            out.press = Some(MouseButton::Left);
            Gesture::One {
                id,
                pressed_at: at,
                last: at,
                since: now,
                travelled: 0.0,
                button: MouseButton::Left,
            }
        }
        (_, 0) => Gesture::Idle,
        (_, _) => begin_two(fingers),
    };
    out
}

/// A pinch's ratio as zoom steps, positive being in: fingers spreading apart
/// bring the map closer, by as much as they spread.
pub(crate) fn pinch_steps(ratio: f32) -> f32 {
    ratio.ln() / ZOOM_STEP.ln()
}

fn translate_touches(
    touches: Res<Touches>,
    time: Res<Time>,
    mut gesture: Local<Gesture>,
    mut pointer: ResMut<TouchPointer>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    // `Option`: no camera plugin, nothing to pan. The one-finger half still
    // works.
    camera_mgr: Option<ResMut<CameraManager>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
) {
    if *gesture == Gesture::Idle && touches.iter().next().is_none() {
        // The common frame on a desktop: no finger, nothing to write, and no
        // change detection tripped on the resources below.
        return;
    }

    let mut fingers: Vec<(u64, Vec2)> = touches
        .iter()
        .map(|touch| (touch.id(), touch.position()))
        .collect();
    fingers.sort_by_key(|(id, _)| *id);

    let step = step(&mut gesture, &fingers, time.elapsed_secs());

    if pointer.position != step.pointer {
        pointer.position = step.pointer;
    }
    if let Some(button) = step.release {
        mouse.release(button);
    }
    if let Some(button) = step.press {
        mouse.press(button);
    }

    let Some(mut camera_mgr) = camera_mgr else {
        return;
    };
    if let Some((from, to)) = step.pan {
        let delta = drag_pan(from, to, camera_mgr.scale);
        if delta != Vec2::ZERO {
            camera_mgr.drag_by(delta);
        }
    }
    if let Some((midpoint, ratio)) = step.pinch
        && ratio != 1.0
    {
        let steps = pinch_steps(ratio);
        // Anchored on the point between the fingers, so the map under them
        // stays under them — the wheel does the same for the cursor.
        let anchor = cameras
            .single()
            .ok()
            .and_then(|(camera, transform)| camera.viewport_to_world_2d(transform, midpoint).ok());
        match anchor {
            Some(anchor) => camera_mgr.zoom_toward(anchor, steps),
            None => camera_mgr.zoom_by(steps),
        }
    }
}

pub struct TouchInputPlugin;

impl Plugin for TouchInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TouchPointer>()
            // After the frame's input has been read, so the buttons this
            // presses are not cleared by the system that reads the mouse; and
            // in `PreUpdate`, so every system in `Update` sees a finger the
            // way it would see a mouse, in the frame it happened.
            .add_systems(PreUpdate, translate_touches.after(InputSystems));
    }
}

#[cfg(test)]
#[path = "touch_tests.rs"]
mod tests;
