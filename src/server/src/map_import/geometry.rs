//! Grid-unit → target-scene-pixel coordinate conversion and the
//! wall/light "insert row" builders (T024).

use super::types::{UvttLight, UvttPoint, UvttPortal};

/// Convert a grid-unit coordinate from the source file into the target
/// scene's pixel space. `target_grid_size` is the *target* scene's
/// `grid_size` in pixels — callers importing a UVTT file pass the source
/// file's own `resolution.pixels_per_grid` here (adopted as the scene's
/// new `grid_size` by `import_uvtt_impl`), so imported geometry stays
/// aligned with the imported background image.
pub fn grid_units_to_scene_px(grid_units: f64, target_grid_size: f64) -> f64 {
    grid_units * target_grid_size
}

/// One `walls` table insert row's worth of plain values (no dependency
/// on any `models::Wall`-family struct — see T024's instructions).
#[derive(Debug, Clone, PartialEq)]
pub struct WallInsert {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub blocks_vision: bool,
    pub blocks_movement: bool,
    pub door_state: &'static str,
}

/// One `light_sources` table insert row's worth of plain values.
#[derive(Debug, Clone, PartialEq)]
pub struct LightInsert {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub intensity: f64,
    pub color: String,
    pub casts_shadows: bool,
}

/// (a) Each `line_of_sight`/`objects_line_of_sight` polygon's consecutive
/// point pairs become one ordinary (non-door) wall row each.
pub fn walls_from_line_of_sight(
    polygons: &[Vec<UvttPoint>],
    target_grid_size: f64,
) -> Vec<WallInsert> {
    let mut walls = Vec::new();
    for polygon in polygons {
        for pair in polygon.windows(2) {
            let a = pair[0];
            let b = pair[1];
            walls.push(WallInsert {
                x1: grid_units_to_scene_px(a.x, target_grid_size),
                y1: grid_units_to_scene_px(a.y, target_grid_size),
                x2: grid_units_to_scene_px(b.x, target_grid_size),
                y2: grid_units_to_scene_px(b.y, target_grid_size),
                blocks_vision: true,
                blocks_movement: false,
                door_state: "none",
            });
        }
    }
    walls
}

/// (b) Each `portals[]` entry becomes one wall row from its `bounds`
/// pair, with `door_state` derived from `closed`.
pub fn walls_from_portals(portals: &[UvttPortal], target_grid_size: f64) -> Vec<WallInsert> {
    portals
        .iter()
        .filter_map(|portal| {
            let a = *portal.bounds.first()?;
            let b = *portal.bounds.get(1)?;
            Some(WallInsert {
                x1: grid_units_to_scene_px(a.x, target_grid_size),
                y1: grid_units_to_scene_px(a.y, target_grid_size),
                x2: grid_units_to_scene_px(b.x, target_grid_size),
                y2: grid_units_to_scene_px(b.y, target_grid_size),
                blocks_vision: true,
                blocks_movement: false,
                door_state: if portal.closed { "closed" } else { "open" },
            })
        })
        .collect()
}

/// (c) Each `lights[]` entry becomes one `light_sources` insert row.
pub fn lights_from_uvtt(lights: &[UvttLight], target_grid_size: f64) -> Vec<LightInsert> {
    lights
        .iter()
        .map(|light| LightInsert {
            x: grid_units_to_scene_px(light.position.x, target_grid_size),
            y: grid_units_to_scene_px(light.position.y, target_grid_size),
            radius: grid_units_to_scene_px(light.range, target_grid_size),
            intensity: light.intensity,
            color: light.color.clone(),
            casts_shadows: light.shadows,
        })
        .collect()
}
