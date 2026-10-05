//! What an import would store, worked out with no database and no object
//! store.
//!
//! Spec 074: the demo ships the example maps as scenes of a world that lives
//! in a browser tab. Its build could have parsed the `.dd2vtt` files itself,
//! and would then have been a second importer — one that had not learned
//! about the texture cap, the grid that follows it, or which way up the file
//! counts. So the demo's build calls this instead, and what a visitor sees is
//! what `import_uvtt_impl` would have put in a real scene.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde_json::json;

use super::ambient::ambient_level;
use super::geometry::{
    LightInsert, ScenePlacement, WallInsert, lights_from_uvtt, walls_from_line_of_sight,
    walls_from_portals,
};
use super::image::detect_image_extension;
use super::parse::parse_uvtt;
use super::types::MapImportError;

/// One map, as a scene would hold it.
pub struct OfflineImport {
    /// The background as it would be stored: WebP, inside the texture cap.
    pub background_webp: Vec<u8>,
    /// The scene's thumbnail, when one could be made.
    pub preview_webp: Option<Vec<u8>>,
    /// The stored background's size in pixels: the scene's width and height.
    pub width: u32,
    pub height: u32,
    /// The stored background's cell size: the scene's `grid_size`.
    pub grid_size: u32,
    /// Walls first, then doors, in the order an import inserts them.
    pub walls: Vec<WallInsert>,
    pub lights: Vec<LightInsert>,
    pub ambient_light: &'static str,
}

impl OfflineImport {
    /// Everything but the pictures, in the field names the GraphQL API uses.
    pub fn geometry_json(&self) -> serde_json::Value {
        json!({
            "width": self.width,
            "height": self.height,
            "gridSize": self.grid_size,
            "ambientLight": self.ambient_light,
            "walls": self.walls.iter().map(|wall| json!({
                "x1": wall.x1,
                "y1": wall.y1,
                "x2": wall.x2,
                "y2": wall.y2,
                "blocksVision": wall.blocks_vision,
                "blocksMovement": wall.blocks_movement,
                "doorState": wall.door_state,
            })).collect::<Vec<_>>(),
            "lights": self.lights.iter().map(|light| json!({
                "x": light.x,
                "y": light.y,
                "radius": light.radius,
                // The same rule the import applies: a file's light has one
                // range (spec 045 FR-062).
                "brightRadius": light.radius * 0.5,
                "intensity": light.intensity,
                "color": light.color,
                "castsShadows": light.casts_shadows,
            })).collect::<Vec<_>>(),
        })
    }
}

/// Runs the import's own steps on a `.dd2vtt` file's bytes and keeps the
/// results instead of writing them anywhere.
pub fn import_offline(raw: &[u8]) -> Result<OfflineImport, MapImportError> {
    let parsed = parse_uvtt(raw)?;

    let bytes = BASE64_STANDARD
        .decode(&parsed.file.image)
        .map_err(|e| MapImportError::InvalidImageBase64(e.to_string()))?;
    detect_image_extension(&bytes).ok_or(MapImportError::InvalidImageMagicBytes)?;

    let background = crate::storage::transcode::transcode_map_background(
        &bytes,
        parsed.file.resolution.pixels_per_grid,
    )
    .map_err(|e| MapImportError::Storage(e.to_string()))?;
    let preview_webp = crate::storage::transcode::transcode_scene_preview(&bytes)
        .ok()
        .map(|preview| preview.webp_bytes);

    let placement = ScenePlacement {
        grid_size: f64::from(background.grid_size),
        width: f64::from(background.image.width),
        height: f64::from(background.image.height),
    };
    let walls = walls_from_line_of_sight(&parsed.file.line_of_sight, &placement)
        .into_iter()
        .chain(walls_from_line_of_sight(
            &parsed.file.objects_line_of_sight,
            &placement,
        ))
        .chain(walls_from_portals(&parsed.file.portals, &placement))
        .collect();
    let lights = lights_from_uvtt(&parsed.file.lights, &placement);

    Ok(OfflineImport {
        width: background.image.width,
        height: background.image.height,
        grid_size: background.grid_size,
        background_webp: background.image.webp_bytes,
        preview_webp,
        walls,
        lights,
        ambient_light: ambient_level(&parsed.file.environment),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example(name: &str) -> Vec<u8> {
        let path = format!(
            "{}/../../examples/maps/{name}.dd2vtt",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    #[test]
    fn a_map_inside_the_cap_keeps_its_own_grid() {
        let map = import_offline(&example("chamber-of-echoing-grief")).expect("imports");
        assert_eq!((map.width, map.height, map.grid_size), (1280, 1280, 128));
        // Four walls round a ten-cell room, centred on the origin, y up.
        assert_eq!(map.walls.len(), 4);
        assert!(
            map.walls
                .iter()
                .all(|wall| wall.x1.abs() == 640.0 && wall.y1.abs() == 640.0)
        );
        assert!(map.preview_webp.is_some());
    }

    #[test]
    fn a_map_over_the_cap_is_stored_smaller_with_a_grid_that_still_fits_it() {
        let map = import_offline(&example("road-side-in")).expect("imports");
        // 48x27 cells at 128px is 6144x3456; stored at 85px a cell.
        assert_eq!((map.width, map.height, map.grid_size), (4080, 2295, 85));
        let doors = map
            .walls
            .iter()
            .filter(|wall| wall.door_state != "none")
            .count();
        assert_eq!(doors, 16);
        assert_eq!(map.lights.len(), 4);
        let json = map.geometry_json();
        assert_eq!(json["lights"][0]["brightRadius"], 5.0 * 85.0 * 0.5);
    }
}
