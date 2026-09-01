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

        app.add_systems(OnExit(AuthoringMode::Shapes), abandon_shape_gesture);

        app.add_systems(
            Update,
            (
                handle_shape_tool_selection,
                // Only while its own tool is armed — see the note in
                // `plugins/wall.rs`. Every authoring system was previously
                // armed at once for a Game Master, so one click was offered to
                // all of them and whichever claimed it won (spec 031 FR-040a).
                handle_shape_input.run_if(in_state(AuthoringMode::Shapes)),
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
