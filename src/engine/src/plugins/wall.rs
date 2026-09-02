use bevy::prelude::*;

use crate::plugins::authoring_mode::AuthoringMode;

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

        // What this subsystem contributes to the seam's vocabulary, declared
        // beside the systems that perform it so the two cannot drift apart.
        crate::plugins::interaction::contribute(
            app,
            thunderforge_canvas_core::wall::interaction_effects(),
        );

        init_wall_systems_resources(app);

        app.add_systems(OnExit(AuthoringMode::Walls), abandon_wall_gesture);

        app.add_systems(
            Update,
            (
                // Only while the wall tool is armed.
                //
                // This system used to be gated on `IsGameMaster` alone, which
                // meant it competed for every Game Master click with token
                // dragging, shapes and lighting — all of which were armed at
                // the same time, because the engine had no idea which tool the
                // rail was showing. Spec 031 FR-040a: exactly one authority
                // decides the mode, and it is the engine.
                //
                // `IsGameMaster` stays as the inner check. The mode says *what*
                // a click means; the role says whether this person may author
                // at all, and that is not the mode's business.
                handle_wall_input.run_if(in_state(AuthoringMode::Walls)),
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
fn abandon_wall_gesture(
    mut drag: ResMut<crate::systems::wall::WallDragState>,
    mut chain: ResMut<crate::systems::wall::WallChainState>,
) {
    drag.abandon();
    chain.abandon();
}
