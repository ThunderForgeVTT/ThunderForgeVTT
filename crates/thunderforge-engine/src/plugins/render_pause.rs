//! Stop drawing, and start again, on the host's word (spec 090 FR-014).
//!
//! A canvas the viewer has scrolled past, or left in a background tab, has
//! no business drawing sixty frames a second. `set_rendering` with
//! `on: false` puts the event loop into a reactive mode that waits for
//! nothing but a wake-up; `on: true` puts it back on every frame.
//!
//! A paused loop runs no systems, so it would never drain the command queue
//! and never hear `on: true`. Every command the host sends therefore wakes
//! the loop for one update (`wake`), which applies whatever was queued.
//!
//! The command is generic plumbing: it says nothing about who is asking.
//! Removing this plugin leaves the engine drawing every frame, and
//! `set_rendering` changes a resource nobody reads (Constitution
//! Principle II).

use bevy::prelude::*;

/// Whether the host wants frames. Written by `set_rendering`.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rendering {
    pub on: bool,
}

impl Default for Rendering {
    fn default() -> Self {
        Self { on: true }
    }
}

pub struct RenderPausePlugin;

impl Plugin for RenderPausePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Rendering>();
        #[cfg(target_arch = "wasm32")]
        app.add_systems(Startup, web::keep_waker)
            .add_systems(Update, web::apply_rendering);
    }
}

/// Wake a paused loop for one update. Before the engine starts, and off the
/// web, there is no loop to wake and this does nothing.
pub fn wake() {
    #[cfg(target_arch = "wasm32")]
    web::wake();
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::Rendering;
    use bevy::prelude::*;
    use bevy::winit::{EventLoopProxyWrapper, UpdateMode, WinitSettings, WinitUserEvent};
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    type Waker = Box<dyn Fn() + Send + Sync>;
    static WAKER: OnceLock<Waker> = OnceLock::new();
    /// Whether the loop is paused. A running loop already updates every
    /// frame, and a wake-up sent to it cancels the frame it had asked the
    /// browser for: a host sending a command every frame (a token walking)
    /// would then never see a frame drawn.
    static PAUSED: AtomicBool = AtomicBool::new(false);

    pub(super) fn keep_waker(proxy: Option<Res<EventLoopProxyWrapper>>) {
        if let Some(proxy) = proxy {
            let proxy = (**proxy).clone();
            let _ = WAKER.set(Box::new(move || {
                let _ = proxy.send_event(WinitUserEvent::WakeUp);
            }));
        }
    }

    pub(super) fn wake() {
        if !PAUSED.load(Ordering::Relaxed) {
            return;
        }
        if let Some(wake) = WAKER.get() {
            wake();
        }
    }

    pub(super) fn apply_rendering(
        rendering: Res<Rendering>,
        settings: Option<ResMut<WinitSettings>>,
    ) {
        if !rendering.is_changed() || rendering.is_added() {
            return;
        }
        let Some(mut settings) = settings else {
            return;
        };
        PAUSED.store(!rendering.on, Ordering::Relaxed);
        if rendering.on {
            *settings = WinitSettings::game();
        } else {
            // Nothing but a wake-up: not the clock, not the pointer crossing
            // the canvas, not a resize. Those wait until the host says on.
            let paused = UpdateMode::Reactive {
                wait: Duration::MAX,
                react_to_device_events: false,
                react_to_user_events: true,
                react_to_window_events: false,
            };
            settings.focused_mode = paused;
            settings.unfocused_mode = paused;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_engine_draws_until_told_otherwise() {
        assert!(Rendering::default().on);
    }

    #[test]
    fn waking_before_the_engine_starts_does_nothing() {
        wake();
    }
}
