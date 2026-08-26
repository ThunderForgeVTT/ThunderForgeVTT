//! Keeps tokens sized to the grid and, unless told otherwise, snapped to it.

use bevy::prelude::*;

use crate::resources::{GridSnapEnabled, SceneGrid, TokenGridBehaviour};
use crate::TokenIdentity;
use thunderforge_canvas_core::grid::{Footprint, GridKind};

/// Sizes every token's sprite to its footprint.
///
/// A token's on-screen size is derived from the grid, never stored: change the
/// scene's `grid_size` — or import a map with a different `pixels_per_grid` —
/// and every token resizes with it. Storing a pixel size instead is what makes
/// tokens the wrong size after an import, which is the bug this avoids by
/// construction.
pub(crate) fn size_tokens_to_grid(
    grid: Res<SceneGrid>,
    mut tokens: Query<(&mut Sprite, Option<&TokenGridBehaviour>), With<TokenIdentity>>,
) {
    for (mut sprite, behaviour) in tokens.iter_mut() {
        let footprint = behaviour.map_or_else(Footprint::default, |b| b.footprint);
        let side = footprint.world_size(grid.size);
        let size = Vec2::splat(side);

        // Only write when it actually changed: `Sprite` is change-detected, and
        // touching it every frame would re-extract every token to the render
        // world for nothing.
        if sprite.custom_size != Some(size) {
            sprite.custom_size = Some(size);
        }
    }
}

/// Snaps tokens to the grid.
///
/// Runs only on tokens whose transform changed, so a settled board costs
/// nothing. Snapping is skipped entirely on a gridless scene and whenever the
/// scene-wide switch is off.
pub(crate) fn snap_tokens_to_grid(
    grid: Res<SceneGrid>,
    enabled: Res<GridSnapEnabled>,
    mut tokens: Query<
        (&mut Transform, Option<&TokenGridBehaviour>),
        (With<TokenIdentity>, Changed<Transform>),
    >,
) {
    if !enabled.0 || grid.kind == GridKind::Gridless {
        return;
    }

    for (mut transform, behaviour) in tokens.iter_mut() {
        let behaviour = behaviour.copied().unwrap_or_default();
        if !behaviour.snap {
            continue;
        }

        let current = transform.translation.truncate();
        let snapped = grid.snap_footprint(current, behaviour.footprint);

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
