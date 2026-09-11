use bevy::prelude::*;

use crate::plugins::authoring_mode::AuthoringMode;

use crate::resources::{ActiveShapeTool, IsGameMaster, SelectedShape, ShapeSet};
use crate::systems::shape::{
    handle_shape_input, handle_shape_keyboard_toggles, handle_shape_tool_selection,
    handle_shape_undo, init_shape_systems_resources, sync_shape_visuals,
};

/// Wires up shape/annotation authoring (T052-T055): the `ShapeSet`
/// resource, GM-only input systems (tool selection/create/select/move/
/// restyle/delete), undo, and the render-sync pass into
/// `CanvasLayer::Shapes`. Depends on `CanvasLayers` existing
/// (`CanvasLayerPlugin` must be added first — see lib.rs) since
/// `sync_shape_visuals` reads `CanvasLayer::Shapes.z()`.
///
/// Independently addable/removable per Constitution Principle II: nothing
/// outside this plugin depends on shapes existing (`apply_external_commands`
/// in lib.rs degrades gracefully — an `upsert_shape`/`remove_shape` command
/// arriving with no `ShapeSet` present would simply not be dispatched,
/// since shape command handling is registered by this plugin, not lib.rs's
/// core command loop).
///
/// Reuses `IsGameMaster` from `resources::wall` rather than duplicating a
/// GM-role flag (per the task brief) — `init_resource` is a no-op if
/// `WallPlugin` already registered it, and works standalone if not,
/// since plugin registration order doesn't matter here.
pub struct ShapePlugin;

impl Plugin for ShapePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShapeSet>()
            .init_resource::<SelectedShape>()
            .init_resource::<ActiveShapeTool>()
            .init_resource::<IsGameMaster>();

        init_shape_systems_resources(app);

        app.add_systems(
            OnExit(AuthoringMode::Shapes),
            (abandon_shape_gesture, clear_active_shape_tool),
        );

        app.add_systems(
            Update,
            (
                // Before the keys: a toolbar click and a key press in the
                // same frame resolve to the key, which is the later intent.
                apply_requested_shape_tool,
                handle_shape_tool_selection,
                // Only while its own tool is armed — see the note in
                // `plugins/wall.rs`. Every authoring system was previously
                // armed at once for a Game Master, so one click was offered to
                // all of them and whichever claimed it won (spec 031 FR-040a).
                handle_shape_input
                    .run_if(in_state(AuthoringMode::Shapes))
                    // And only while this viewer may use the tool at all.
                    // The mode gate above cannot cover a revocation: the
                    // state change that takes a lost tool away lands a frame
                    // later, and a click in that frame would still draw
                    // (spec 031 SC-012).
                    .run_if(crate::plugins::authoring_mode::authoring_tool_allowed(
                        AuthoringMode::Shapes,
                    )),
                handle_shape_keyboard_toggles,
                handle_shape_undo,
                sync_shape_visuals,
            )
                .chain(),
        );
    }
}

/// Discard this tool's unfinished gesture when its mode is left.
///
/// `OnExit` is the whole reason the authoring mode is a Bevy state rather than
/// a resource holding a tool name: there is exactly one place where leaving a
/// mode happens, so "abandon whatever was in progress" is written once instead
/// of at every path that could change tools.
///
/// Spec 031 FR-040a and its edge case: a drag begun under one tool must not
/// complete under another's rules. The user changed what a click means partway
/// through; reinterpreting the half-finished gesture would be guessing.
fn abandon_shape_gesture(mut drag: ResMut<crate::systems::shape::ShapeDragState>) {
    drag.abandon();
}

/// Leaving the Shapes tool leaves no shape kind armed, so coming back to it
/// is a fresh choice — the same reason the gesture above is abandoned.
fn clear_active_shape_tool(mut active: ResMut<ActiveShapeTool>) {
    active.0 = None;
}

use thunderforge_canvas_core::shape::ShapeKind;

/// A shape kind the web asked for, waiting for the next frame. The outer
/// `Option` is "is there a request"; the inner one is the kind, `None` being
/// select-and-move.
type ShapeToolRequest = Option<Option<ShapeKind>>;

static REQUESTED_SHAPE_TOOL: std::sync::OnceLock<std::sync::Mutex<ShapeToolRequest>> =
    std::sync::OnceLock::new();

fn requested_shape_tool_slot() -> &'static std::sync::Mutex<ShapeToolRequest> {
    REQUESTED_SHAPE_TOOL.get_or_init(|| std::sync::Mutex::new(None))
}

/// The web's name for a shape kind, as the Shapes panel's buttons say it.
///
/// `"text"` is select-and-move to the engine: text is placed by `ShapeTool`
/// in the DOM, and a drag with the text button pressed must not draw a box.
fn shape_tool_from_web(kind: &str) -> Option<Option<ShapeKind>> {
    match kind {
        "freehand" => Some(Some(ShapeKind::Stroke)),
        "rect" => Some(Some(ShapeKind::Rect)),
        "ellipse" => Some(Some(ShapeKind::Ellipse)),
        "line" => Some(Some(ShapeKind::Line)),
        "text" | "none" => Some(None),
        _ => None,
    }
}

/// Arm the shape a drag in the Shapes tool draws — playtest 2026-09-10 P10.
///
/// The Shapes panel's buttons used to change only React state; the engine's
/// `ActiveShapeTool` was reachable from the number keys alone, and only while
/// the canvas had keyboard focus, so a drag after clicking "Rectangle" drew
/// nothing and said nothing. Queued like `set_authoring_mode`, and applied at
/// the start of the next frame. Returns whether the kind was recognised; an
/// unknown one changes nothing.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_active_shape_tool(kind: &str) -> bool {
    let Some(request) = shape_tool_from_web(kind) else {
        return false;
    };
    if let Ok(mut slot) = requested_shape_tool_slot().lock() {
        *slot = Some(request);
    }
    true
}

fn apply_requested_shape_tool(mut active: ResMut<ActiveShapeTool>) {
    let requested = requested_shape_tool_slot()
        .lock()
        .ok()
        .and_then(|mut slot| slot.take());
    if let Some(kind) = requested {
        active.0 = kind;
    }
}

#[cfg(test)]
mod shape_tool_tests {
    use super::*;

    /// The toolbar's buttons reach the engine (P10), `"text"` and `"none"`
    /// disarm it, and a name it does not know changes nothing.
    #[test]
    fn the_toolbar_arms_the_shape_a_drag_draws() {
        let mut app = App::new();
        app.init_resource::<ActiveShapeTool>();
        app.add_systems(Update, apply_requested_shape_tool);

        assert!(set_active_shape_tool("rect"));
        app.update();
        assert_eq!(
            app.world().resource::<ActiveShapeTool>().0,
            Some(ShapeKind::Rect)
        );

        assert!(set_active_shape_tool("freehand"));
        app.update();
        assert_eq!(
            app.world().resource::<ActiveShapeTool>().0,
            Some(ShapeKind::Stroke)
        );

        assert!(!set_active_shape_tool("hexagon"));
        app.update();
        assert_eq!(
            app.world().resource::<ActiveShapeTool>().0,
            Some(ShapeKind::Stroke),
            "an unknown kind must not disarm what the GM chose",
        );

        assert!(set_active_shape_tool("text"));
        app.update();
        assert_eq!(app.world().resource::<ActiveShapeTool>().0, None);
    }
}
