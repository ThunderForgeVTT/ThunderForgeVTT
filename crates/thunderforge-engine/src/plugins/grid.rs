//! Grid overlay, drawn with gizmos.
//!
//! Replaces a Startup system that spawned one `Sprite` entity per grid line
//! from a `SceneData` nothing ever populated. Three things were wrong with
//! that: it ran once, so switching scenes or importing a map never changed the
//! grid; it cost an entity per line, which a 48x27 map at 128px turns into
//! ~80 entities that exist forever; and hexagonal was an unimplemented `TODO`,
//! so hex scenes silently drew nothing.
//!
//! Gizmos fix all three by construction. They are immediate-mode: the grid is
//! re-emitted every frame from the current `SceneGrid`, so it always matches
//! the scene, costs no entities, and hex is just a different set of points.
//!
//! Only the cells currently on screen are emitted. Without that, a large map
//! at a zoomed-out camera would emit tens of thousands of line segments per
//! frame for lines that are sub-pixel anyway.

use bevy::prelude::*;

use crate::resources::{GridVisible, SceneGrid};
use thunderforge_canvas_core::grid::{Cell, GridKind};

/// Grid line colour. Deliberately low-contrast: the grid is a reference
/// overlay on top of map art, not a feature of it.
const GRID_COLOR: Color = Color::srgba(0.85, 0.87, 0.92, 0.28);

/// Below this on-screen cell size (in pixels), the grid stops drawing.
///
/// A grid whose cells are a few pixels across is a grey wash that hides the
/// map rather than helping read it — and it is exactly where the segment
/// count explodes. Dropping out at far zoom is what every VTT does.
const MIN_VISIBLE_CELL_PIXELS: f32 = 6.0;

/// Hard ceiling on cells drawn in one frame.
///
/// A backstop, not the primary limit — `MIN_VISIBLE_CELL_PIXELS` normally
/// keeps the count far below this. It exists so a pathological grid size
/// (say, arriving as 0.01 from a malformed scene) degrades to a partial grid
/// instead of locking the frame.
const MAX_CELLS_PER_FRAME: i32 = 20_000;

pub struct GridPlugin;

impl Plugin for GridPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SceneGrid>()
            .init_resource::<GridVisible>()
            .add_systems(Update, draw_grid);
    }
}

/// The world-space rectangle the camera can currently see.
fn visible_world_rect(camera: &Projection, transform: &GlobalTransform) -> Option<Rect> {
    let Projection::Orthographic(ortho) = camera else {
        return None;
    };
    let center = transform.translation().truncate();
    Some(Rect {
        min: center + ortho.area.min,
        max: center + ortho.area.max,
    })
}

fn draw_grid(
    grid: Res<SceneGrid>,
    visible: Res<GridVisible>,
    cameras: Query<(&Camera, &Projection, &GlobalTransform), With<Camera2d>>,
    mut gizmos: Gizmos,
) {
    if !visible.0 || grid.kind == GridKind::Gridless {
        return;
    }

    let Ok((camera, projection, camera_transform)) = cameras.single() else {
        return;
    };
    let Some(view) = visible_world_rect(projection, camera_transform) else {
        return;
    };

    // How large one cell is on screen. `area` is the world-space extent the
    // camera shows and already accounts for zoom, so the ratio against the
    // viewport's pixel width converts world units to pixels.
    if let Some(viewport) = camera.logical_viewport_size() {
        let world_width = view.width();
        if world_width > f32::EPSILON {
            let cell_pixels = grid.size * (viewport.x / world_width);
            if cell_pixels < MIN_VISIBLE_CELL_PIXELS {
                return;
            }
        }
    }

    // Corner cells of the visible rect, padded by one so partially-visible
    // cells at the edges still draw their outlines.
    let min_cell = grid.world_to_cell(view.min);
    let max_cell = grid.world_to_cell(view.max);

    let (q0, q1) = (min_cell.q - 1, max_cell.q + 1);
    // Hex rows shear horizontally as `r` grows, so the q-range that covers the
    // visible rect widens with the row span. Padding by the row count keeps
    // the far corners of a hex grid from being clipped away.
    let (r0, r1) = (min_cell.r - 1, max_cell.r + 1);
    let row_span = (r1 - r0).abs();
    let (q0, q1) = if grid.kind.is_hex() {
        (q0 - row_span, q1 + row_span)
    } else {
        (q0, q1)
    };

    let cell_count = (q1 - q0 + 1) as i64 * (r1 - r0 + 1) as i64;
    if cell_count > MAX_CELLS_PER_FRAME as i64 {
        return;
    }

    match grid.kind {
        GridKind::Gridless => {}
        GridKind::Square => {
            // Squares share edges, so drawing full-span lines emits roughly
            // 2N segments instead of the 4N a per-cell outline would.
            let size = grid.size.max(f32::EPSILON);
            let x0 = grid.origin.x + q0 as f32 * size;
            let x1 = grid.origin.x + (q1 + 1) as f32 * size;
            let y0 = grid.origin.y + r0 as f32 * size;
            let y1 = grid.origin.y + (r1 + 1) as f32 * size;

            for q in q0..=(q1 + 1) {
                let x = grid.origin.x + q as f32 * size;
                gizmos.line_2d(Vec2::new(x, y0), Vec2::new(x, y1), GRID_COLOR);
            }
            for r in r0..=(r1 + 1) {
                let y = grid.origin.y + r as f32 * size;
                gizmos.line_2d(Vec2::new(x0, y), Vec2::new(x1, y), GRID_COLOR);
            }
        }
        GridKind::HexPointyTop | GridKind::HexFlatTop => {
            // Hexes have no shared full-span lines, so each cell is outlined.
            // Neighbours redraw shared edges; at grid opacity that is not
            // visible, and de-duplicating edges costs more than it saves.
            for r in r0..=r1 {
                for q in q0..=q1 {
                    let outline = grid.cell_outline(Cell::new(q, r));
                    if outline.is_empty() {
                        continue;
                    }
                    // Cheap reject for the sheared corners the padded q-range
                    // over-covers.
                    let center = grid.cell_center(Cell::new(q, r));
                    if center.x < view.min.x - grid.size
                        || center.x > view.max.x + grid.size
                        || center.y < view.min.y - grid.size
                        || center.y > view.max.y + grid.size
                    {
                        continue;
                    }
                    gizmos.linestrip_2d(outline, GRID_COLOR);
                }
            }
        }
    }
}
