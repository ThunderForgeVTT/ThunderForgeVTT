//! What a player's token has seen, remembered.
//!
//! Spec 045 US7, owner decision 3: with exploration on, everywhere a player's
//! own token has been able to see stays faintly on *their* map, and a Game
//! Master can reset it.
//!
//! # Why the engine accumulates it
//!
//! Because the engine is the only thing that knows. Whether a point is
//! visible is decided by the lighting pass every frame — walls, doors,
//! darkness, the viewer's own sight — and nothing outside the ECS can answer
//! it (Principle I). The web persists what this accumulates and hands it back
//! on the next visit; it never decides what was seen.
//!
//! # Why cells rather than a mask
//!
//! A bitmap of a scene is large, has to be scaled when the grid changes, and
//! means nothing to anything but a shader. A set of cells is small enough to
//! keep in a browser, survives a change of zoom, and is the same vocabulary
//! movement and walls already speak.
//!
//! # Why it draws into `CanvasLayer::Fog`
//!
//! That layer has existed since spec 001 with a visibility toggle and nothing
//! drawing into it. Reusing it means nothing about z-ordering or the darkness
//! pass has to be re-litigated.

use bevy::prelude::*;
use std::collections::HashSet;

use crate::resources::{CanvasLayer, SceneGrid, WallSet};
use crate::systems::lighting_vision::ViewerToken;
use thunderforge_canvas_core::grid::Cell;

/// How faint a remembered area is drawn.
///
/// Visible enough to read as "you have been here", faint enough that it is
/// never mistaken for somewhere currently lit. A player who cannot tell the
/// difference will walk into a room they think they can see.
const REMEMBERED_ALPHA: f32 = 0.22;
const REMEMBERED_COLOR: Color = Color::srgba(0.62, 0.68, 0.82, REMEMBERED_ALPHA);

/// The furthest a token's memory reaches from it, in cells.
///
/// A bound rather than a rule about sight: the accumulator walks the cells
/// around the viewer each frame, and without a limit a scene with a huge grid
/// would walk the whole board every frame to find the handful it can see.
const REACH: i32 = 24;

/// The most cells one scene may remember.
///
/// Data-model.md: a record past a size bound is dropped rather than grown.
/// A player who walks a very large map for hours must not fill their own
/// browser storage; at this bound the memory stops growing and what is
/// already remembered stays.
const MAX_CELLS: usize = 20_000;

/// Whether this scene remembers, as the Game Master set it.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExplorationEnabled(pub bool);

/// The cells this viewer's token has seen.
#[derive(Resource, Default, Debug, Clone)]
pub struct ExploredCells(pub HashSet<(i32, i32)>);

impl ExploredCells {
    pub fn is_full(&self) -> bool {
        self.0.len() >= MAX_CELLS
    }
}

/// Marks a sprite drawn for a remembered cell.
#[derive(Component)]
pub(crate) struct RememberedCell;

/// Accumulate what the viewer's token can see.
///
/// Runs after the lighting pass, and asks the same question it does, through
/// the same shared function — a second notion of "visible" would drift, and
/// the first thing a player would notice is fog that does not match what they
/// can see.
pub(crate) fn accumulate_explored(
    enabled: Res<ExplorationEnabled>,
    mut explored: ResMut<ExploredCells>,
    grid: Option<Res<SceneGrid>>,
    walls: Res<WallSet>,
    viewer: Option<Res<ViewerToken>>,
    tokens: Query<(&Transform, &crate::TokenIdentity)>,
) {
    if !enabled.0 || explored.is_full() {
        return;
    }
    let Some(grid) = grid else {
        // No grid, no cells. A gridless scene has nothing to remember in
        // this vocabulary, and inventing one would not match what the board
        // draws.
        return;
    };
    let Some(viewer_id) = viewer.as_ref().and_then(|viewer| viewer.0.as_deref()) else {
        // A Game Master sees through no token and explores nothing: the fog
        // is the player's own memory, not the table's.
        return;
    };
    let Some((transform, _)) = tokens.iter().find(|(_, identity)| identity.0 == viewer_id) else {
        return;
    };

    let from = transform.translation.truncate();
    let here = grid.world_to_cell(from);
    for dq in -REACH..=REACH {
        for dr in -REACH..=REACH {
            let cell = Cell::new(here.q + dq, here.r + dr);
            if explored.0.contains(&(cell.q, cell.r)) {
                continue;
            }
            if thunderforge_canvas_core::wall::is_visible(from, grid.cell_center(cell), &walls) {
                explored.0.insert((cell.q, cell.r));
                if explored.is_full() {
                    return;
                }
            }
        }
    }
}

/// Draw what is remembered, and nothing else.
///
/// Redrawn only when the set changes. Rebuilding twenty thousand sprites a
/// frame would cost more than everything else the canvas does.
pub(crate) fn draw_explored(
    mut commands: Commands,
    explored: Res<ExploredCells>,
    enabled: Res<ExplorationEnabled>,
    grid: Option<Res<SceneGrid>>,
    drawn: Query<Entity, With<RememberedCell>>,
) {
    if !explored.is_changed() && !enabled.is_changed() {
        return;
    }
    for entity in drawn.iter() {
        commands.entity(entity).despawn();
    }
    if !enabled.0 {
        return;
    }
    let Some(grid) = grid else {
        return;
    };

    let size = Vec2::splat(grid.size);
    for (q, r) in explored.0.iter() {
        let centre = grid.cell_center(Cell::new(*q, *r));
        commands.spawn((
            Sprite::from_color(REMEMBERED_COLOR, size),
            Transform::from_translation(centre.extend(CanvasLayer::Fog.z())),
            RememberedCell,
        ));
    }
}

/// Forget everything — a Game Master's reset, or a scene change.
pub fn clear_explored(explored: &mut ExploredCells) {
    explored.0.clear();
}

/// The engine's side of the exploration boundary: what the application last
/// asked for, and the memory mirrored back out.
///
/// # Why a value, with one instance
///
/// The application speaks through `wasm_bindgen` free functions, which have no
/// handle on the `World`, so its requests wait here until
/// [`reconcile_exploration`] takes them up. The instance the web app talks to
/// is process-wide ([`BOUNDARY`]). The systems reach it through the
/// [`ExplorationLink`] resource rather than naming the static, so a test gives
/// its app a boundary of its own. When they named the static, parallel tests
/// drained each other's requests and the module failed nearly every run.
pub struct ExplorationBoundary {
    /// Whether the scene remembers, as the application last said.
    pending_enabled: std::sync::Mutex<Option<bool>>,
    /// Cells handed back from the browser, waiting to be taken up. An empty
    /// list is a reset, which is not the same as nothing pending.
    pending_cells: std::sync::Mutex<Option<Vec<(i32, i32)>>>,
    /// What is remembered, as the JSON [`explored_cells`] returns; `None`
    /// until the first frame, which reads as `[]`.
    mirror: std::sync::Mutex<Option<String>>,
}

impl Default for ExplorationBoundary {
    fn default() -> Self {
        Self::new()
    }
}

impl ExplorationBoundary {
    pub const fn new() -> Self {
        Self {
            pending_enabled: std::sync::Mutex::new(None),
            pending_cells: std::sync::Mutex::new(None),
            mirror: std::sync::Mutex::new(None),
        }
    }

    /// See [`set_exploration`].
    pub fn set_enabled(&self, enabled: bool) -> bool {
        if let Ok(mut pending) = self.pending_enabled.lock() {
            *pending = Some(enabled);
            return true;
        }
        false
    }

    /// See [`set_explored_cells`].
    pub fn set_cells(&self, cells_json: &str) -> bool {
        let parsed = if cells_json.is_empty() {
            Some(Vec::new())
        } else {
            serde_json::from_str::<Vec<(i32, i32)>>(cells_json).ok()
        };
        let Some(cells) = parsed else {
            return false;
        };
        if let Ok(mut pending) = self.pending_cells.lock() {
            *pending = Some(cells);
            return true;
        }
        false
    }

    /// See [`explored_cells`].
    pub fn cells(&self) -> String {
        self.mirror
            .lock()
            .ok()
            .and_then(|held| held.clone())
            .unwrap_or_else(|| String::from("[]"))
    }
}

/// The one boundary the web app talks to.
static BOUNDARY: ExplorationBoundary = ExplorationBoundary::new();

/// Which boundary an app's exploration systems answer to: [`BOUNDARY`] unless
/// a test inserts another.
#[derive(Resource, Clone, Copy)]
pub struct ExplorationLink(pub &'static ExplorationBoundary);

impl Default for ExplorationLink {
    fn default() -> Self {
        Self(&BOUNDARY)
    }
}

/// What has been remembered, as a JSON array of `[q, r]` pairs.
///
/// The web persists this in the browser and hands it back on the next visit.
/// Sorted, so two reads of the same memory are the same string and a caller
/// can tell whether anything changed without comparing sets.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn explored_cells() -> String {
    BOUNDARY.cells()
}

/// Turn a scene's memory on or off, as the server says.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_exploration(enabled: bool) -> bool {
    BOUNDARY.set_enabled(enabled)
}

/// Hand back what this player's browser remembered, or `""` to forget.
///
/// A reset arrives as the empty string rather than as an absent call, because
/// "the Game Master cleared your map" and "the application has not told me
/// anything yet" must not look the same — the first has to survive a frame in
/// which nothing else happens.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_explored_cells(cells_json: &str) -> bool {
    BOUNDARY.set_cells(cells_json)
}

/// Apply what the application asked for, and mirror what is remembered.
///
/// A system rather than direct mutation, for the reason
/// `reconcile_controlled_token` is: the application speaks before the scene
/// necessarily exists, and a request applied once and dropped is lost exactly
/// when it arrives first.
pub(crate) fn reconcile_exploration(
    link: Res<ExplorationLink>,
    mut enabled: ResMut<ExplorationEnabled>,
    mut explored: ResMut<ExploredCells>,
) {
    let boundary = link.0;
    if let Ok(mut pending) = boundary.pending_enabled.lock()
        && let Some(requested) = pending.take()
    {
        enabled.set_if_neq(ExplorationEnabled(requested));
    }

    if let Ok(mut pending) = boundary.pending_cells.lock()
        && let Some(cells) = pending.take()
    {
        explored.0 = cells.into_iter().collect();
    }

    // Mirrored every frame rather than on change: the mirror is what the web
    // reads to decide whether to persist, and a memory that stopped being
    // reported would be a memory that stopped being saved.
    let mut sorted: Vec<(i32, i32)> = explored.0.iter().copied().collect();
    sorted.sort_unstable();
    let json = serde_json::to_string(&sorted).unwrap_or_else(|_| String::from("[]"));
    if let Ok(mut held) = boundary.mirror.lock()
        && held.as_deref() != Some(json.as_str())
    {
        *held = Some(json);
    }
}

pub struct ExplorationPlugin;

impl Plugin for ExplorationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ExplorationEnabled>()
            .init_resource::<ExploredCells>()
            .init_resource::<ExplorationLink>()
            .add_systems(
                Update,
                (
                    // Before accumulating, so a memory handed back from the
                    // browser is in place before this frame adds to it.
                    reconcile_exploration,
                    // After the lighting pass, so the question "can the viewer
                    // see this" is answered against the frame the player is
                    // actually looking at.
                    accumulate_explored
                        .after(crate::systems::lighting::apply_light_illumination)
                        .after(reconcile_exploration),
                    draw_explored.after(accumulate_explored),
                ),
            );
    }
}

#[cfg(test)]
#[path = "exploration_tests.rs"]
mod tests;
