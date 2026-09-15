//! Keeps tokens sized to the grid and, unless told otherwise, snapped to it.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::TokenIdentity;
use crate::resources::{GridSnapEnabled, SceneGrid, TokenGridBehaviour};
use thunderforge_canvas_core::grid::{Footprint, GridKind};
use thunderforge_canvas_core::token_art::fit_within_footprint;

/// Sizes every token's sprite to its footprint.
///
/// A token's on-screen size is derived from the grid, never stored: change the
/// scene's `grid_size` — or import a map with a different `pixels_per_grid` —
/// and every token resizes with it. Storing a pixel size instead is what makes
/// tokens the wrong size after an import, which is the bug this avoids by
/// construction.
///
/// A token with art keeps that art's aspect ratio inside the footprint
/// instead of being stretched to fill it (see
/// `thunderforge_canvas_core::token_art`). A flat colour swatch has no
/// aspect to preserve and fills the square exactly, as it always has.
///
/// Runs every frame rather than on change, because an image's dimensions
/// are not known when the token spawns: `Assets<Image>` reports nothing
/// until the load completes, so the correct size can only be applied once
/// it arrives. The write below is guarded, so the extra frames cost a
/// comparison and nothing else.
pub(crate) fn size_tokens_to_grid(
    grid: Res<SceneGrid>,
    images: Res<Assets<Image>>,
    mut tokens: Query<(&mut Sprite, Option<&TokenGridBehaviour>), With<TokenIdentity>>,
) {
    for (mut sprite, behaviour) in tokens.iter_mut() {
        let footprint = behaviour.map_or_else(Footprint::default, |b| b.footprint);
        let side = footprint.world_size(grid.size);

        let size = match images.get(&sprite.image) {
            Some(image) => fit_within_footprint(side, image.size_f32()),
            // No art, or art still loading. `fit_within_footprint` would
            // return the same square for zero dimensions; this skips the
            // lookup for the colour-swatch case entirely.
            None => Vec2::splat(side),
        };

        // Only write when it actually changed: `Sprite` is change-detected, and
        // touching it every frame would re-extract every token to the render
        // world for nothing.
        if sprite.custom_size != Some(size) {
            sprite.custom_size = Some(size);
        }
    }
}

/// A token whose art failed to load draws its colour swatch instead of
/// nothing (playtest 2026-09-10 P1).
///
/// `app.rs` falls back to the swatch only when a token has no photo URL. A
/// URL that 404s, is refused, or will not decode left a sprite whose image
/// never arrives, and Bevy draws nothing for that — so the token was
/// invisible, which is worse than the square it was meant to improve on. Runs
/// every frame because a load can fail at any point after the token spawns;
/// once a swatch is swapped in its image is the default handle, which never
/// reports a failure, so it is not swapped again.
///
/// Before `size_tokens_to_grid`, so the swatch is sized in the frame it
/// appears.
pub(crate) fn fall_back_when_token_art_fails(
    asset_server: Res<AssetServer>,
    mut tokens: Query<(&mut Sprite, &crate::components::Token), With<TokenIdentity>>,
) {
    for (mut sprite, token) in tokens.iter_mut() {
        if matches!(
            asset_server.load_state(sprite.image.id()),
            bevy::asset::LoadState::Failed(_)
        ) {
            *sprite = Sprite::from_color(token.color, Vec2::ONE);
        }
    }
}

/// A token whose position or footprint changed this frame.
type MovedOrResized = (
    With<TokenIdentity>,
    Or<(Changed<Transform>, Changed<TokenGridBehaviour>)>,
);

/// Snaps tokens to the grid.
///
/// Runs only on tokens whose transform or footprint changed, so a settled
/// board costs nothing. Snapping is skipped entirely on a gridless scene and
/// whenever the scene-wide switch is off.
///
/// A changed footprint re-snaps (spec 046 T076): a token arrives, is snapped
/// as one square, and is told a moment later by `set_token_grid` that it is a
/// Large ogre. Keyed on the transform alone, it would stay centred in one cell
/// — straddling half of each neighbour — until somebody moved it.
///
/// And it re-snaps **from where the token was put, not from where the first
/// snap left it**. Snapping is not idempotent across footprints: an ogre the
/// server placed on a vertex, snapped first as one square, moves half a square
/// to a cell's centre, and snapped from there as two squares moves half a
/// square again — a whole square from where every other board and the server
/// have it. So each token's unsnapped position is remembered beside the
/// position snapping gave it; while the transform still holds that snapped
/// position, nothing has moved the token and a new footprint snaps the
/// remembered one. Found by `combat-reach.spec.ts`, which drew the ogre one
/// square up and right of the server's.
pub(crate) fn snap_tokens_to_grid(
    grid: Res<SceneGrid>,
    enabled: Res<GridSnapEnabled>,
    mut placed: Local<HashMap<Entity, (Vec2, Vec2)>>,
    mut removed: RemovedComponents<TokenIdentity>,
    mut tokens: Query<(Entity, &mut Transform, Option<&TokenGridBehaviour>), MovedOrResized>,
) {
    for entity in removed.read() {
        placed.remove(&entity);
    }
    if !enabled.0 || grid.kind == GridKind::Gridless {
        return;
    }

    for (entity, mut transform, behaviour) in tokens.iter_mut() {
        let behaviour = behaviour.copied().unwrap_or_default();
        if !behaviour.snap {
            continue;
        }

        let current = transform.translation.truncate();
        let put_at = match placed.get(&entity) {
            Some((raw, snapped)) if snapped.distance_squared(current) <= 0.0001 => *raw,
            _ => current,
        };
        let snapped = grid.snap_footprint(put_at, behaviour.footprint);
        placed.insert(entity, (put_at, snapped));

        // Guarded because this query is driven by `Changed<Transform>` and
        // writing the transform re-triggers it. Without the comparison an
        // already-snapped token would mark itself changed every frame, and the
        // system would never go quiet.
        if current.distance_squared(snapped) > 0.0001 {
            transform.translation.x = snapped.x;
            transform.translation.y = snapped.y;
        }
    }
}

/// What each token fills on this board, for [`token_footprints`].
type FootprintList = Vec<serde_json::Value>;

static TOKEN_FOOTPRINTS: std::sync::OnceLock<std::sync::Mutex<FootprintList>> =
    std::sync::OnceLock::new();

/// Mirrors, a few times a second, what every token fills as drawn: its
/// footprint, its centre, its sprite's size, where its name sits and how wide
/// its bars are. For [`token_footprints`], so a test can ask a player's engine
/// whether the ogre is two squares by two in every respect the board shows —
/// not only whether the command arrived.
#[allow(clippy::type_complexity)]
pub(crate) fn mirror_token_footprints(
    mut frame: Local<u32>,
    tokens: Query<(
        &TokenIdentity,
        &Transform,
        &Sprite,
        Option<&TokenGridBehaviour>,
        Option<&Children>,
    )>,
    names: Query<(&Transform, &crate::plugins::nameplate::Nameplate)>,
    bars: Query<&Sprite, With<crate::plugins::status_display::StatusGeometry>>,
) {
    *frame = frame.wrapping_add(1);
    if *frame % 10 != 1 {
        return;
    }
    let list: FootprintList = tokens
        .iter()
        .map(|(identity, transform, sprite, behaviour, children)| {
            let footprint = behaviour.map_or_else(Footprint::default, |b| b.footprint);
            let size = sprite.custom_size.unwrap_or(Vec2::ZERO);
            let children = children
                .map(|c| c.iter().collect::<Vec<_>>())
                .unwrap_or_default();
            let name_y = children
                .iter()
                .filter_map(|child| names.get(*child).ok())
                .find(|(_, plate)| !plate.shadow)
                .map(|(t, _)| t.translation.y * transform.scale.y);
            let bar_width = children
                .iter()
                .filter_map(|child| bars.get(*child).ok())
                .filter_map(|bar| bar.custom_size.map(|s| s.x))
                .fold(None, |widest: Option<f32>, w| {
                    Some(widest.map_or(w, |x| x.max(w)))
                });
            serde_json::json!({
                "tokenId": identity.0,
                "footprint": footprint.cells(),
                "x": transform.translation.x,
                "y": transform.translation.y,
                "width": size.x * transform.scale.x,
                "height": size.y * transform.scale.y,
                "nameY": name_y,
                "barWidth": bar_width.map(|w| w * transform.scale.x),
            })
        })
        .collect();
    let slot = TOKEN_FOOTPRINTS.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    if let Ok(mut current) = slot.lock() {
        *current = list;
    }
}

/// Every token this engine draws and what it fills, as
/// `[{"tokenId","footprint","x","y","width","height","nameY","barWidth"}]`
/// in world units. Read-only, for tests (spec 046 US4).
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn token_footprints() -> String {
    let list = TOKEN_FOOTPRINTS
        .get()
        .and_then(|slot| slot.lock().ok().map(|l| l.clone()))
        .unwrap_or_default();
    serde_json::Value::from(list).to_string()
}
