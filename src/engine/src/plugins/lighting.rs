use bevy::prelude::*;

use crate::plugins::authoring_mode::AuthoringMode;

use crate::resources::{IsGameMaster, LightSet, SelectedLight};
use crate::systems::lighting::{
    apply_light_illumination, handle_light_input, handle_light_keyboard_toggles,
    handle_light_resize, handle_light_undo, handle_switch_effects, init_lighting_systems_resources,
    sync_light_visuals,
};

/// Wires up light authoring (T036-T039, T041): the `LightSet` resource,
/// GM-only input systems (place/select/drag/resize/delete/toggle), undo,
/// the sprite-sync render pass into `CanvasLayer::Lighting`, and the
/// occlusion-aware illumination first-pass. Depends on `CanvasLayers`
/// existing (`CanvasLayerPlugin` must be added first — see lib.rs) since
/// `sync_light_visuals` reads `CanvasLayer::Lighting.z()`, and on
/// `WallSet`/`is_visible` (`resources::wall`) for occlusion — mirrors
/// `WallPlugin` exactly.
///
/// Independently addable/removable per Constitution Principle II: like
/// `WallPlugin`, this plugin registers its own `IsGameMaster` init (shared
/// with `WallPlugin`'s copy via Bevy's idempotent `init_resource`, not
/// duplicated data) so it doesn't strictly require `WallPlugin` to be
/// registered first for the GM flag to exist.
pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LightSet>()
            .init_resource::<SelectedLight>()
            .init_resource::<IsGameMaster>()
            // Registered here as well as in `InteractionPlugin`, idempotently.
            // A contributor that could only be added after the seam would not
            // be independently addable (Principle II).
            .add_message::<crate::plugins::interaction::InteractionActivated>();

        // Declared beside the system that performs it — see `plugins/wall.rs`.
        crate::plugins::interaction::contribute(
            app,
            thunderforge_canvas_core::lighting::interaction_effects(),
        );

        init_lighting_systems_resources(app);

        app.add_systems(OnExit(AuthoringMode::Lights), abandon_light_gesture);

        app.add_systems(
            Update,
            (
                // Only while its own tool is armed — see the note in
                // `plugins/wall.rs`. Every authoring system was previously
                // armed at once for a Game Master, so one click was offered to
                // all of them and whichever claimed it won (spec 031 FR-040a).
                handle_light_input.run_if(in_state(AuthoringMode::Lights)),
                handle_light_resize,
                handle_light_keyboard_toggles,
                handle_light_undo,
                // Spec 030: switching, contributed to the interaction seam.
                // Before `sync_light_visuals` so a lamp switched this frame is
                // drawn switched this frame, and before illumination so
                // shadows re-resolve in the same pass.
                handle_switch_effects,
                sync_light_visuals,
                apply_light_illumination,
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
fn abandon_light_gesture(mut drag: ResMut<crate::systems::lighting::LightDragState>) {
    drag.abandon();
}
