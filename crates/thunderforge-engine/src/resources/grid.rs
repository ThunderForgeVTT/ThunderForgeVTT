//! The active scene's grid, as a Bevy resource.
//!
//! A thin newtype over `thunderforge_canvas_core::grid::GridSpec`, following
//! this crate's convention of keeping tested geometry in the pure core crate
//! and wrapping it here for the ECS (see `resources/wall.rs`).
//!
//! This replaces two separate sources of grid truth that had drifted apart:
//! `plugins/grid.rs` drew lines from a `SceneData` that nothing ever populated
//! with real values, while `movement::apply_grid_snapping` snapped to a
//! hardcoded 32.0. Neither reflected the `pixels_per_grid` a dd2vtt import
//! recorded, so on a 128px imported map the visible grid, the snap lattice and
//! the actual art were three different grids.

use bevy::prelude::*;
use thunderforge_canvas_core::grid::{GridKind, GridSpec};

/// The grid every system snaps, measures and draws against.
#[derive(Resource, Clone, Copy, Debug, Default, Deref, DerefMut)]
pub struct SceneGrid(pub GridSpec);

impl SceneGrid {
    /// Builds a grid from the server's scene fields.
    ///
    /// `grid_type` is the raw `scenes.grid_type` string; unknown values fall
    /// back to square rather than failing the scene load.
    pub fn from_server(grid_type: &str, grid_size: f32, origin: Vec2) -> Self {
        Self(GridSpec {
            kind: GridKind::from_server_str(grid_type),
            size: grid_size,
            origin,
        })
    }

    /// Builds a grid aligned to a map of `map_size`, which is what a caller
    /// almost always wants: imported art has its own grid painted on, and ours
    /// has to sit on top of it.
    ///
    /// The caller supplies the map's dimensions and nothing else — it does not
    /// have to know that `sync_scene_background` centres the sprite on the
    /// world origin, which is the fact that decides where cell (0,0) goes.
    pub fn anchored_to_map(grid_type: &str, grid_size: f32, map_size: Vec2) -> Self {
        Self(GridSpec::anchored_to_map(
            GridKind::from_server_str(grid_type),
            grid_size,
            map_size,
        ))
    }
}

/// Whether the grid overlay is drawn.
///
/// Separate from the grid itself: a GM hiding the grid must not change where
/// tokens snap, so visibility is its own piece of state rather than a
/// `GridKind::Gridless` masquerade.
#[derive(Resource, Clone, Copy, Debug)]
pub struct GridVisible(pub bool);

impl Default for GridVisible {
    fn default() -> Self {
        Self(true)
    }
}
