//! Scene-background render sync: keeps a single background `Sprite` in
//! `CanvasLayer::Background` matching the current `SceneBackground`
//! resource. Wiring: see `plugins/background.rs`'s `BackgroundPlugin`.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::plugins::cached_assets::{CanvasAssetCache, load_canvas_image};
use crate::plugins::mark_frame;
use crate::resources::{BackgroundTextureCache, CanvasLayer, PlacedCanvasImages, SceneBackground};

/// Marker on the sprite entity spawned for the active scene's background
/// image. There is at most one at a time (despawn-and-respawn on change,
/// same "simplicity over micro-optimization" call as `resources/wall.rs`).
#[derive(Component)]
pub(crate) struct BackgroundSprite;

/// T(new): despawns the previous background sprite (if any) and, when
/// `SceneBackground.path` is `Some`, spawns a fresh one sized to
/// `width`x`height` at the scene origin, loading the image via
/// `AssetServer::load` against Bevy's default asset root ("assets"),
/// which the dev proxy and production server both map to `/assets/...`
/// (see `apps/web/vite.config.mts` and `src/server/src/serve/mod.rs`).
pub(crate) fn sync_scene_background(
    mut commands: Commands,
    background: Res<SceneBackground>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    existing: Query<Entity, With<BackgroundSprite>>,
    // Optional, like every other plugin-owned resource this loop touches:
    // without `BackgroundPlugin`'s cache the background still renders, it
    // just pays the upload again on every visit.
    mut texture_cache: Option<ResMut<BackgroundTextureCache>>,
    // Spec 028 (T027): absent unless `CachedAssetsPlugin` is registered, in
    // which case `load_canvas_image` is `asset_server.load` verbatim.
    mut asset_cache: Option<ResMut<CanvasAssetCache>>,
) {
    if !background.is_changed() {
        return;
    }

    let despawned = existing.iter().count();
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }

    let Some(path) = background.path.clone() else {
        info!(
            target: "render_probe",
            "background: cleared (despawned {despawned} sprite(s))",
        );
        return;
    };

    let image: Handle<Image> = load_canvas_image(
        &path,
        asset_cache.as_deref_mut(),
        &mut images,
        &asset_server,
    );

    // Hold a strong handle past the sprite's lifetime. Despawning the old
    // background above dropped the only handle to its image, which freed
    // the texture and made returning to that scene re-upload it in full —
    // seconds of frozen frame for a map the GPU had already seen. See
    // `resources/background_cache.rs`.
    if let Some(cache) = texture_cache.as_deref_mut() {
        let pixels = (background.width.max(0.0) as u64) * (background.height.max(0.0) as u64);
        cache.touch(&path, image.clone(), pixels);
    }

    commands.spawn((
        Sprite {
            image,
            custom_size: Some(Vec2::new(background.width, background.height)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, CanvasLayer::Background.z()),
        BackgroundSprite,
    ));

    // Start of the interval anyone measuring a map switch cares about. The
    // matching `background_loaded` mark is left by
    // `trace_background_asset_load` once the bytes are decoded.
    mark_frame(format!("background_spawn {path}"));

    if let Some(cache) = texture_cache.as_deref() {
        debug!(
            target: "background",
            "background cache: {} resident ({:.1}MP)",
            cache.resident_paths().len(),
            cache.resident_pixels() as f64 / 1.0e6,
        );
    }

    // Traced unconditionally rather than behind the render probe: a scene
    // background changing is a rare, deliberate event, and knowing when the
    // sprite was replaced — and at what size — is the first thing anyone
    // asks when a map does not appear.
    info!(
        target: "background",
        "background: spawned sprite for {path} at {}x{} (z={}), replacing {despawned}",
        background.width,
        background.height,
        CanvasLayer::Background.z(),
    );
}

/// Marks the frame on which the background image finished loading.
///
/// This is the frame to look at when asking whether a map switch hitches.
/// `AssetServer::load` returns immediately with an unloaded handle; the
/// fetch is asynchronous, but the **decode** is not offloaded on wasm —
/// Bevy is single-threaded there regardless of features (real threads need
/// `SharedArrayBuffer` and cross-origin isolation), so decoding a map runs
/// on the same thread as the frame loop. The GPU upload then happens in the
/// render world the first time the image is prepared, i.e. on this frame or
/// the one after it.
pub(crate) fn trace_background_asset_load(
    mut events: MessageReader<AssetEvent<Image>>,
    background_sprites: Query<&Sprite, With<BackgroundSprite>>,
) {
    for event in events.read() {
        let AssetEvent::LoadedWithDependencies { id } = event else {
            continue;
        };
        if background_sprites
            .iter()
            .any(|sprite| sprite.image.id() == *id)
        {
            mark_frame("background_loaded");
        }
    }
}

/// Marker + id on a placed (pasted) canvas image's sprite entity (spec
/// 002, US3). Unlike `BackgroundSprite` there can be many at once, so
/// each carries the `CanvasImageAsset.id` it was spawned for.
#[derive(Component)]
pub(crate) struct PlacedCanvasImageSprite(pub String);

/// Keeps one sprite per `PlacedCanvasImages` entry: despawns entities for
/// ids no longer present, spawns entities for new ids. Renders in
/// `CanvasLayer::Shapes` (same layer as hand-drawn annotations — pasted
/// images are an annotation-adjacent, GM-placed element, not part of the
/// base map) — one z-height above `Background`, below `Tokens`.
///
/// Does not currently diff-update an existing entry's position/size in
/// place (despawn-and-respawn on any change to the whole set, same
/// "simplicity over micro-optimization" call `sync_scene_background`
/// documents) — acceptable for spec 002's scope since pasted images are
/// placed once and not repositioned by any authoring tool yet.
pub(crate) fn sync_placed_canvas_images(
    mut commands: Commands,
    placed: Res<PlacedCanvasImages>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    existing: Query<(Entity, &PlacedCanvasImageSprite)>,
    // See `sync_scene_background`: `None` when the plugin is not registered.
    mut asset_cache: Option<ResMut<CanvasAssetCache>>,
) {
    if !placed.is_changed() {
        return;
    }

    let existing_ids: HashMap<String, Entity> = existing
        .iter()
        .map(|(entity, marker)| (marker.0.clone(), entity))
        .collect();

    for (id, entity) in &existing_ids {
        if !placed.0.contains_key(id) {
            commands.entity(*entity).despawn();
        }
    }

    for (id, image) in placed.0.iter() {
        if existing_ids.contains_key(id) {
            continue;
        }

        let handle: Handle<Image> = load_canvas_image(
            &image.path,
            asset_cache.as_deref_mut(),
            &mut images,
            &asset_server,
        );
        commands.spawn((
            Sprite {
                image: handle,
                custom_size: Some(Vec2::new(image.width, image.height)),
                ..default()
            },
            Transform::from_xyz(image.x, image.y, CanvasLayer::Shapes.z()),
            PlacedCanvasImageSprite(id.clone()),
        ));
    }
}
