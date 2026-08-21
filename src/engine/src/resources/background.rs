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

/// One placed (pasted) canvas image asset (spec 002, US3/FR-011),
/// generalizing `SceneBackground`'s single-slot shape to an
/// id-keyed set: unlike the scene background, a scene can have any
/// number of pasted images at once.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedCanvasImage {
    /// Same asset-server-relative-path convention as `SceneBackground.path`.
    /// The frontend resolves the `CanvasImageAsset.storagePath` returned by
    /// `uploadCanvasImage` to a fetchable URL before sending this command —
    /// this resource/system pair only knows "what image, where, how big",
    /// same division of responsibility as `SceneBackground`.
    pub path: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// All placed canvas images on the currently active scene, keyed by
/// `CanvasImageAsset.id`. `sync_placed_canvas_images`
/// (`systems/background.rs`) keeps one sprite entity per entry, adding
/// new ones and despawning removed ones — same "despawn-and-respawn on
/// change, simplicity over micro-optimization" call as
/// `sync_scene_background`.
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct PlacedCanvasImages(pub std::collections::HashMap<String, PlacedCanvasImage>);
