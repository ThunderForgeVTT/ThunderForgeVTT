//! Bevy `Resource` shell for the active scene's background image (map
//! import's decoded PNG, see `scenes.background_image_path` /
//! `Scene.backgroundImagePath`). Mirrors `resources/wall.rs`'s "thin
//! resource, logic in systems" split, but this resource has no core-crate
//! counterpart since there's no pure geometry/logic to unit-test here —
//! it's just "what image, how big."

use bevy::prelude::*;

/// The currently active scene's background image and its pixel dimensions
/// (already computed server-side from `Scene.width`/`Scene.height` and
/// passed through by the `set_scene_background` external command — this
/// resource doesn't fetch scene metadata itself).
///
/// `path` is `None` when the active scene has no imported background
/// (`sync_scene_background` then despawns any existing background sprite,
/// leaving the canvas showing just its `ClearColor`).
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct SceneBackground {
    /// Asset-server-relative path, e.g. `"map-imports/{scene_id}/{uuid}.png"`
    /// (no leading slash, no `assets/` prefix — Bevy's default `AssetPlugin`
    /// root already resolves against `assets/`).
    pub path: Option<String>,
    pub width: f32,
    pub height: f32,
}
