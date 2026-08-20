use bevy::prelude::*;

use crate::resources::SceneBackground;
use crate::systems::background::sync_scene_background;

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
            .add_systems(Update, sync_scene_background);
    }
}
