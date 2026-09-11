//! Grid-unit → scene coordinate conversion and the wall/light "insert row"
//! builders (T024).

use super::types::{UvttLight, UvttPoint, UvttPortal};

/// Convert a grid-unit length from the source file into scene pixels.
/// `target_grid_size` is the *stored* scene's `grid_size` in pixels — the
/// cell size of the background as it was actually saved, which is the source
/// file's own `pixels_per_grid` unless the image had to be shrunk to fit a GPU
/// texture (see `transcode_map_background`).
pub fn grid_units_to_scene_px(grid_units: f64, target_grid_size: f64) -> f64 {
    grid_units * target_grid_size
}

/// Where an imported map sits in the scene.
///
/// Playtest 2026-09-10 P9. A UVTT file counts grid units from the image's
/// top-left corner with y growing down. The engine draws the background
/// centred on the origin with y growing *up* — as it does the grid, tokens and
/// every hand-drawn wall. Scaling alone, which is all this used to do, put
/// imported walls, doors and lights half a map away from the art and mirrored
/// top to bottom, so every wall shaded the wrong part of the map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScenePlacement {
    /// The stored background's cell size, in its own pixels.
    pub grid_size: f64,
    /// The stored background's size, in pixels — the scene's `width` and
    /// `height`.
    pub width: f64,
    pub height: f64,
}

impl ScenePlacement {
    /// A point in the file's grid units, in the scene's coordinates.
    pub fn point(&self, p: UvttPoint) -> (f64, f64) {
        (
            grid_units_to_scene_px(p.x, self.grid_size) - self.width / 2.0,
            self.height / 2.0 - grid_units_to_scene_px(p.y, self.grid_size),
        )
    }

    /// A length in the file's grid units, in scene pixels. Lengths do not
    /// care where the origin is.
    pub fn length(&self, grid_units: f64) -> f64 {
        grid_units_to_scene_px(grid_units, self.grid_size)
    }
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

fn wall(
    placement: &ScenePlacement,
    a: UvttPoint,
    b: UvttPoint,
    door_state: &'static str,
) -> WallInsert {
    let (x1, y1) = placement.point(a);
    let (x2, y2) = placement.point(b);
    WallInsert {
        x1,
        y1,
        x2,
        y2,
        blocks_vision: true,
        blocks_movement: false,
        door_state,
    }
}

/// (a) Each `line_of_sight`/`objects_line_of_sight` polygon's consecutive
/// point pairs become one ordinary (non-door) wall row each.
pub fn walls_from_line_of_sight(
    polygons: &[Vec<UvttPoint>],
    placement: &ScenePlacement,
) -> Vec<WallInsert> {
    polygons
        .iter()
        .flat_map(|polygon| polygon.windows(2))
        .map(|pair| wall(placement, pair[0], pair[1], "none"))
        .collect()
}

/// (b) Each `portals[]` entry becomes one wall row from its `bounds`
/// pair, with `door_state` derived from `closed`.
pub fn walls_from_portals(portals: &[UvttPortal], placement: &ScenePlacement) -> Vec<WallInsert> {
    portals
        .iter()
        .filter_map(|portal| {
            let a = *portal.bounds.first()?;
            let b = *portal.bounds.get(1)?;
            let state = if portal.closed { "closed" } else { "open" };
            Some(wall(placement, a, b, state))
        })
        .collect()
}

/// (c) Each `lights[]` entry becomes one `light_sources` insert row.
pub fn lights_from_uvtt(lights: &[UvttLight], placement: &ScenePlacement) -> Vec<LightInsert> {
    lights
        .iter()
        .map(|light| {
            let (x, y) = placement.point(light.position);
            LightInsert {
                x,
                y,
                radius: placement.length(light.range),
                intensity: light.intensity,
                color: light.color.clone(),
                casts_shadows: light.shadows,
            }
        })
        .collect()
}

#[cfg(test)]
mod placement_tests {
    use super::*;

    /// A 10×6-cell map at 128px: 1280×768 pixels, drawn from (-640, 384) at
    /// its top-left to (640, -384) at its bottom-right.
    const MAP: ScenePlacement = ScenePlacement {
        grid_size: 128.0,
        width: 1280.0,
        height: 768.0,
    };

    #[test]
    fn the_files_corners_land_on_the_backgrounds_corners() {
        assert_eq!(MAP.point(UvttPoint { x: 0.0, y: 0.0 }), (-640.0, 384.0));
        assert_eq!(MAP.point(UvttPoint { x: 10.0, y: 6.0 }), (640.0, -384.0));
        assert_eq!(MAP.point(UvttPoint { x: 5.0, y: 3.0 }), (0.0, 0.0));
    }

    #[test]
    fn down_in_the_file_is_down_on_the_map() {
        let (_, upper) = MAP.point(UvttPoint { x: 1.0, y: 1.0 });
        let (_, lower) = MAP.point(UvttPoint { x: 1.0, y: 2.0 });
        assert!(lower < upper, "a larger file y is further down the map");
    }

    #[test]
    fn a_lights_radius_is_scaled_but_not_moved() {
        assert_eq!(MAP.length(2.5), 320.0);
    }
}
