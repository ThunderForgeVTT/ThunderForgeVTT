use bevy::prelude::*;

use crate::resources::{IsGameMaster, SelectedWall, WallSet};
use crate::systems::wall::{
    handle_door_effects, handle_wall_input, handle_wall_keyboard_toggles, handle_wall_undo,
    init_wall_systems_resources, sync_wall_visuals,
};

/// Wires up wall authoring (T011-T014): the `WallSet` resource, GM-only
/// input systems (create/select/move-endpoint/delete/toggle), undo, the
/// sprite-sync render pass into `CanvasLayer::Walls`, and the vision-
/// occlusion first-pass. Depends on `CanvasLayers` existing
/// (`CanvasLayerPlugin` must be added first — see lib.rs) since
/// `sync_wall_visuals` reads `CanvasLayer::Walls.z()`.
///
/// Independently addable/removable per Constitution Principle II: nothing
/// outside this plugin depends on walls existing (`apply_external_commands`
/// in lib.rs degrades gracefully — an `upsert_wall`/`remove_wall` command
/// arriving with no `WallSet` present would simply not be dispatched,
/// since wall command handling is registered by this plugin, not lib.rs's
/// core command loop).
pub struct WallPlugin;

impl Plugin for WallPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WallSet>()
            .init_resource::<SelectedWall>()
            .init_resource::<IsGameMaster>()
            // Registered here as well as in `InteractionPlugin`, and
            // `add_message` is idempotent. A contributor that could only be
            // added *after* the seam would not be independently addable, and
            // Principle II asks for plugins that are.
            .add_message::<crate::plugins::interaction::InteractionActivated>();

        init_wall_systems_resources(app);

        app.add_systems(
            Update,
            (
                handle_wall_input,
                handle_wall_keyboard_toggles,
                handle_wall_undo,
                // Spec 030: doors, contributed to the interaction seam. Reads
                // the activation message; nothing in the interaction plugin
                // knows this system exists (FR-039, FR-040). Placed before
                // `sync_wall_visuals` so a door that opened this frame is
                // drawn open this frame.
                handle_door_effects,
                sync_wall_visuals,
            )
                .chain(),
        );
    }
}
