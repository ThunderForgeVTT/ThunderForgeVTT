use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum GridType {
    #[default]
    Square,
    Hexagonal,
    Gridless,
}

impl std::fmt::Display for GridType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Square => write!(f, "square"),
            Self::Hexagonal => write!(f, "hexagonal"),
            Self::Gridless => write!(f, "gridless"),
        }
    }
}

/// Phase 4.7.A1: Scene Data Resource
///
/// Holds game board metadata including grid type, dimensions, and background.
/// This resource is inserted at startup and accessed by all rendering systems.
///
/// # Coordinate System
///
/// - **Database/External**: Top-left origin (0,0), Y increases downward
/// - **Bevy/Rendering**: Centered origin, Y increases upward
///
/// Conversion is handled by `database_y_to_bevy_y()` and `bevy_y_to_database_y()`
#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct SceneData {
    /// Unique ID of this scene in the database
    pub scene_id: String,

    /// World this scene belongs to
    pub world_id: String,

    /// Grid rendering type (square, hexagonal, or gridless)
    pub grid_type: GridType,

    /// Pixels per grid cell (e.g., 32.0 for 32x32 cells)
    pub grid_size: f32,

    /// Scene width in grid cells
    pub width: i32,

    /// Scene height in grid cells
    pub height: i32,

    /// Optional background image URL
    pub background_url: Option<String>,

    /// Derived: pixel dimensions (width_cells * grid_size)
    #[serde(skip)]
    pub pixel_width: f32,

    /// Derived: pixel height (height_cells * grid_size)
    #[serde(skip)]
    pub pixel_height: f32,
}

impl SceneData {
    /// Create a new SceneData resource
    pub fn new(
        scene_id: String,
        world_id: String,
        grid_type: GridType,
        grid_size: f32,
        width: i32,
        height: i32,
        background_url: Option<String>,
    ) -> Self {
        let pixel_width = width as f32 * grid_size;
        let pixel_height = height as f32 * grid_size;

        Self {
            scene_id,
            world_id,
            grid_type,
            grid_size,
            width,
            height,
            background_url,
            pixel_width,
            pixel_height,
        }
    }

    /// Convert database Y coordinate (top-left origin, Y-down) to Bevy Y (center origin, Y-up)
    ///
    /// Database coordinate system:
    /// ```text
    /// (0,0) ─────────────────────→ (+X)
    ///   │
    ///   │ (0, height-1)
    ///   ↓
    ///  (+Y)
    /// ```
    ///
    /// Bevy coordinate system:
    /// ```text
    ///           ↑ (+Y)
    ///           │
    ///   (0, 0) ──────────────────→ (+X)
    /// ```
    ///
    /// # Arguments
    ///
    /// * `database_y` - Y coordinate in database space (0 at top)
    ///
    /// # Returns
    ///
    /// Y coordinate in Bevy space
    pub fn database_y_to_bevy_y(&self, database_y: f32) -> f32 {
        self.pixel_height - (database_y * self.grid_size)
    }

    /// Convert Bevy Y coordinate (center origin, Y-up) to database Y (top-left origin, Y-down)
    ///
    /// Reverse of `database_y_to_bevy_y()`
    pub fn bevy_y_to_database_y(&self, bevy_y: f32) -> f32 {
        (self.pixel_height - bevy_y) / self.grid_size
    }

    /// Get bounds in pixel space
    pub fn bounds(&self) -> (f32, f32, f32, f32) {
        (0.0, 0.0, self.pixel_width, self.pixel_height)
    }

    /// Static helper for Y conversion (used in grid generation before resource access)
    pub fn database_y_to_bevy_y_static(database_y: f32, pixel_height: f32) -> f32 {
        pixel_height - (database_y * 32.0) // Assumes grid_size=32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_data_creation() {
        let scene = SceneData::new(
            "scene-1".to_string(),
            "world-1".to_string(),
            GridType::Square,
            32.0,
            100,
            100,
            None,
        );

        assert_eq!(scene.pixel_width, 3200.0);
        assert_eq!(scene.pixel_height, 3200.0);
        assert_eq!(scene.grid_size, 32.0);
        assert_eq!(scene.width, 100);
        assert_eq!(scene.height, 100);
    }

    #[test]
    fn test_y_axis_conversion() {
        let scene = SceneData::new(
            "scene-1".to_string(),
            "world-1".to_string(),
            GridType::Square,
            32.0,
            10,
            10,
            None,
        );

        // Top-left in database (y=0) maps to top of Bevy viewport (y=pixel_height)
        let bevy_y = scene.database_y_to_bevy_y(0.0);
        assert_eq!(
            bevy_y, 320.0,
            "Database Y=0 should map to Bevy Y=pixel_height"
        );

        // Bottom in database (y=10) maps to bottom of Bevy (y=0)
        let bevy_y = scene.database_y_to_bevy_y(10.0);
        assert_eq!(bevy_y, 0.0, "Database Y=height should map to Bevy Y=0");

        // Middle should map correctly
        let bevy_y = scene.database_y_to_bevy_y(5.0);
        assert_eq!(bevy_y, 160.0, "Database Y=5 should map to Bevy Y=160");

        // Reverse conversion should work
        let db_y = scene.bevy_y_to_database_y(320.0);
        assert_eq!(db_y, 0.0, "Bevy Y=pixel_height should convert back to Y=0");

        let db_y = scene.bevy_y_to_database_y(160.0);
        assert_eq!(db_y, 5.0, "Bevy Y=160 should convert back to Y=5");

        let db_y = scene.bevy_y_to_database_y(0.0);
        assert_eq!(db_y, 10.0, "Bevy Y=0 should convert back to Y=height");
    }

    #[test]
    fn test_grid_type_display() {
        assert_eq!(GridType::Square.to_string(), "square");
        assert_eq!(GridType::Hexagonal.to_string(), "hexagonal");
        assert_eq!(GridType::Gridless.to_string(), "gridless");
    }

    #[test]
    fn test_bounds() {
        let scene = SceneData::new(
            "scene-1".to_string(),
            "world-1".to_string(),
            GridType::Square,
            32.0,
            20,
            30,
            None,
        );

        let (x, y, w, h) = scene.bounds();
        assert_eq!(x, 0.0);
        assert_eq!(y, 0.0);
        assert_eq!(w, 640.0); // 20 * 32
        assert_eq!(h, 960.0); // 30 * 32
    }
}
