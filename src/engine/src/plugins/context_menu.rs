//! Right-clicking the map.
//!
//! Spec 031 FR-029. Two things have to be true for a right-click on the canvas
//! to be usable: the browser's own menu must not open over it, and something
//! has to know *what* was right-clicked. Neither is chrome's to do alone — the
//! browser menu is suppressed on a DOM element chrome does not own, and the hit
//! test needs the camera.
//!
//! # Where the listener goes, and why it is not obvious
//!
//! The real `<canvas>` is inserted by Bevy/winit as a direct child of
//! `<body>`. It is **not** inside the React container whose id was handed to
//! `start()` — that div only reserves layout space, and the canvas is
//! positioned over it. A `contextmenu` listener bound to the React container
//! therefore never fires, which is a mistake this project has already made
//! once; `apps/web/src/engine/canvasKeyboard.ts` carries the same note about
//! the same element.
//!
//! So this queries for the canvas element itself and binds there. Not
//! `document`, not `window`: research R6 is explicit that whatever suppresses
//! the menu must be scoped to the canvas surface. A document-level handler
//! would take the browser menu away from the tool rail, the actors pane, the
//! chat and every text field on the page — surfaces where the browser menu is
//! the *right* answer, and where taking it away would be a regression nobody
//! asked for.
//!
//! # Why the listener is not `preventDefault` on everything
//!
//! It suppresses exactly one event type on exactly one element, and does not
//! stop propagation. R6's diagnosis was that a Game Master's click was offered
//! to every authoring system at once because the engine had no notion of an
//! active tool; the fix for that class was `AuthoringMode`, and nothing here
//! should add a second thing that intercepts pointer input. This listener
//! cancels a browser default and forwards nothing.
//!
//! # What the engine reports, and what it does not
//!
//! A single `canvas_context_menu` event per right-click, carrying where it
//! happened and which tokens were under it. Chrome opens the menu, because a
//! menu is chrome.
//!
//! The event carries the pointer's screen position as well as its world
//! position, which is the one place this plugin comes close to Constitution
//! Principle I's line about screen coordinates. It stays on the right side of
//! it: this is one position at the moment of one gesture, not a per-frame
//! stream, and it is not a projection chrome could compute — a DOM menu has to
//! open at the pointer, and only the engine knows where the pointer was when
//! the engine decided a right-click had happened.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use serde_json::json;
use thunderforge_canvas_core::grid::Footprint;
use thunderforge_canvas_core::token_stack::{StackCandidate, tokens_at};

use crate::emit_event;
use crate::resources::{SceneGrid, TokenGridBehaviour};
use crate::{TOKEN_SIZE, TokenIdentity};

/// Whether the browser menu has already been suppressed.
///
/// The canvas does not exist when the app is built — winit inserts it while
/// the engine starts — so this is retried each frame until it lands, then
/// never again.
#[derive(Resource, Default)]
struct MenuSuppressed(bool);

/// Bind the `contextmenu` handler to the canvas element, once it exists.
///
/// Returns whether it was bound. `false` simply means "not yet": on the frames
/// before winit has inserted the canvas there is nothing to bind to, and that
/// is a normal state rather than a failure.
#[cfg(target_arch = "wasm32")]
fn bind_context_menu_suppression() -> bool {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return false;
    };
    // The element itself, found by tag. The selector `start()` was given names
    // the React container, which does not contain the canvas — see the module
    // docs. `canvasKeyboard.ts` finds the same element the same way.
    let Ok(Some(canvas)) = document.query_selector("canvas") else {
        return false;
    };

    let handler = Closure::<dyn FnMut(web_sys::Event)>::new(|event: web_sys::Event| {
        // Cancel the browser's menu and nothing else. Propagation is left
        // alone deliberately: this is not a router.
        event.prevent_default();
    });

    let bound = canvas
        .add_event_listener_with_callback("contextmenu", handler.as_ref().unchecked_ref())
        .is_ok();

    // Leaked on purpose. The listener has to outlive this call and lives as
    // long as the canvas does, which is as long as the page; dropping the
    // closure would unbind the handler the moment it was installed.
    handler.forget();

    bound
}

/// Native builds have no DOM and no browser menu to suppress.
///
/// The engine only ships as wasm; this exists so the module reads the same on
/// both and a native `cargo check` does not need `web-sys` in the graph.
#[cfg(not(target_arch = "wasm32"))]
fn bind_context_menu_suppression() -> bool {
    false
}

/// Keep trying until the canvas exists.
fn suppress_browser_menu(mut suppressed: ResMut<MenuSuppressed>) {
    if suppressed.0 {
        return;
    }
    suppressed.0 = bind_context_menu_suppression();
}

/// Report a right-click, with what was under it.
fn report_right_click(
    mouse_button: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    tokens: Query<(&Transform, &TokenIdentity, Option<&TokenGridBehaviour>)>,
    grid: Option<Res<SceneGrid>>,
) {
    if !mouse_button.just_pressed(MouseButton::Right) {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(screen) = window.cursor_position() else {
        return;
    };
    let Some((camera, camera_transform)) = camera_query.iter().next() else {
        return;
    };
    let Ok(world) = camera.viewport_to_world_2d(camera_transform, screen) else {
        return;
    };

    // The same hit test a left-click drag uses (`systems::token.rs`), so
    // right-clicking a token and left-clicking it agree about which token that
    // is. Anything else would be a second answer to the same question.
    let candidates: Vec<StackCandidate> = tokens
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

    emit_event(json!({
        "type": "canvas_context_menu",
        "worldX": world.x,
        "worldY": world.y,
        "screenX": screen.x,
        "screenY": screen.y,
        // Empty for a right-click on bare map, which is a meaningful answer
        // rather than a missing one: it is what tells chrome to offer the
        // scene's menu instead of a token's.
        "tokenIds": tokens_at(&candidates, world),
    }));
}

/// Right-click on the canvas: no browser menu, and one event saying what was
/// clicked.
///
/// Independently addable and removable (Constitution Principle II). Delete the
/// line that adds it and the browser menu comes back and the event stops being
/// emitted; nothing else changes, because nothing else reads either.
pub struct ContextMenuPlugin;

impl Plugin for ContextMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuSuppressed>()
            .add_systems(Update, (suppress_browser_menu, report_right_click));
    }
}
