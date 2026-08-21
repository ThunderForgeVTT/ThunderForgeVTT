use bevy::prelude::*;

use crate::resources::{IsGameMaster, LightSet, SelectedLight};
use crate::systems::lighting::{
    apply_light_illumination, handle_light_input, handle_light_keyboard_toggles,
    handle_light_resize, handle_light_undo, init_lighting_systems_resources, sync_light_visuals,
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
            .init_resource::<IsGameMaster>();

        init_lighting_systems_resources(app);

        app.add_systems(
            Update,
            (
                handle_light_input,
                handle_light_resize,
                handle_light_keyboard_toggles,
                handle_light_undo,
                sync_light_visuals,
                apply_light_illumination,
            )
                .chain(),
        );
    }
}
