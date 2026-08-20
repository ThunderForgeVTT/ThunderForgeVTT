use bevy::prelude::*;

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
            Update,
            (
                handle_shape_tool_selection,
                handle_shape_input,
                handle_shape_keyboard_toggles,
                handle_shape_undo,
                sync_shape_visuals,
            )
                .chain(),
        );
    }
}
