//! Per-token grid behaviour: how big a token is, and whether it snaps.
//!
//! Both default to the safe, expected thing — one cell, snapping on — so a
//! token created by any path that does not know about these lands correctly on
//! the grid rather than free-floating at whatever pixel it was dropped at.

use bevy::prelude::*;
use thunderforge_canvas_core::grid::Footprint;

/// A token's size and snapping, in grid terms.
///
/// Absent from an entity, `TokenGridBehaviour::default()` applies — see the
/// systems in `systems::token_grid`.
#[derive(Component, Clone, Copy, Debug)]
pub struct TokenGridBehaviour {
    /// Side length in cells. Clamped at half a cell by `Footprint`.
    pub footprint: Footprint,
    /// Whether this token snaps to the grid.
    ///
    /// Per-token rather than only scene-wide because the exceptions are real:
    /// a hazard marker, a spell template or a decorative prop often wants free
    /// placement while every creature on the same board stays locked.
    pub snap: bool,
}

impl Default for TokenGridBehaviour {
    /// One cell, snapping on — grid-locked by default.
    fn default() -> Self {
        Self {
            footprint: Footprint::default(),
            snap: true,
        }
    }
}

/// Scene-wide snapping master switch.
///
/// Separate from the per-token flag and ANDed with it: turning this off
/// suspends snapping for everything without editing every token, and turning
/// it back on restores each token's own setting rather than flattening them.
/// That is what makes it usable as a held-modifier "free placement" mode.
#[derive(Resource, Clone, Copy, Debug)]
pub struct GridSnapEnabled(pub bool);

impl Default for GridSnapEnabled {
    fn default() -> Self {
        Self(true)
    }
}
