//! Scene-background render sync: keeps a single background `Sprite` in
//! `CanvasLayer::Background` matching the current `SceneBackground`
//! resource. Wiring: see `plugins/background.rs`'s `BackgroundPlugin`.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::resources::{CanvasLayer, PlacedCanvasImages, SceneBackground};

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
    existing: Query<Entity, With<BackgroundSprite>>,
) {
    if !background.is_changed() {
        return;
    }

    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }

    let Some(path) = background.path.clone() else {
        return;
    };

    let image: Handle<Image> = asset_server.load(&path);

    commands.spawn((
        Sprite {
            image,
            custom_size: Some(Vec2::new(background.width, background.height)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, CanvasLayer::Background.z()),
        BackgroundSprite,
    ));
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
    existing: Query<(Entity, &PlacedCanvasImageSprite)>,
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

        let handle: Handle<Image> = asset_server.load(&image.path);
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
