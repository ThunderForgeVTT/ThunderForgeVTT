use bevy::prelude::*;

use crate::resources::{BackgroundTextureCache, PlacedCanvasImages, SceneBackground};
use crate::systems::background::{
    sync_placed_canvas_images, sync_scene_background, trace_background_asset_load,
};

/// Wires up the scene-background render: the `SceneBackground` resource
/// and the sprite-sync system that keeps `CanvasLayer::Background` showing
/// the active scene's imported map image (or nothing, if it has none).
/// Depends on `CanvasLayers` existing (`CanvasLayerPlugin` must be added
/// first — see lib.rs) since `sync_scene_background` reads
/// `CanvasLayer::Background.z()`.
///
/// Independently addable/removable per Constitution Principle II: nothing
/// outside this plugin depends on a background existing —
/// `apply_external_commands` in lib.rs degrades gracefully if this plugin
/// isn't registered (a `set_scene_background` command would simply not be
/// dispatched, same pattern as `WallPlugin`'s `WallSet`).
pub struct BackgroundPlugin;

impl Plugin for BackgroundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SceneBackground>()
            // Keeps recently-shown map textures resident; see
            // `resources/background_cache.rs` for the measurements that
            // motivated it.
            .init_resource::<BackgroundTextureCache>()
            .add_systems(Update, sync_scene_background)
            // Timing instrumentation for map switches; see
            // `plugins/frame_trace.rs`. Reads asset events only, writes
            // nothing the rest of the engine can observe.
            .add_systems(Update, trace_background_asset_load)
            // Spec 002 (US3): placed (pasted) canvas images — a
            // resource/system pair added to this existing plugin rather
            // than a new plugin type, per Constitution Principle II's
            // "independently addable/removable" still holding (nothing
            // outside this plugin depends on placed images existing) and
            // the plan's explicit call to generalize BackgroundPlugin's
            // pattern instead of introducing a new plugin for one more
            // scene-image concept.
            .init_resource::<PlacedCanvasImages>()
            .add_systems(Update, sync_placed_canvas_images);
    }
}
