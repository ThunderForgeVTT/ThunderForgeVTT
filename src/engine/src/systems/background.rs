//! Scene-background render sync: keeps a single background `Sprite` in
//! `CanvasLayer::Background` matching the current `SceneBackground`
//! resource. Wiring: see `plugins/background.rs`'s `BackgroundPlugin`.

use bevy::prelude::*;

use crate::resources::{CanvasLayer, SceneBackground};

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
