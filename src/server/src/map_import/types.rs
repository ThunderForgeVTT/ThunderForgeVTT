//! UVTT JSON shape, parser output, and error/result types for map import.
//! Split out of the former flat `map_import.rs` (T023's original module)
//! per the src/server test-coverage/file-size audit.

use serde::Deserialize;

/// Only this UVTT format version is supported (research.md §7).
pub(super) const SUPPORTED_FORMAT: f64 = 0.3;

// ---------------------------------------------------------------------
// T023: UVTT JSON shape
//
// These structs deliberately mirror the full documented UVTT shape
// (examples/maps/README.md), including fields not yet consumed by T024's
// conversion logic (`map_origin`, `resolution`/`environment` as wholes,
// a portal's `position`/`rotation`/`freestanding`) — kept for parser
// correctness/round-tripping and as the natural place to add
// ambient-light or portal-orientation handling later, rather than
// silently dropping fields the source format defines. `#[allow(dead_code)]`
// documents that gap instead of hiding it behind an `_`-prefixed name.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct UvttPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Deserialize)]
pub struct UvttResolution {
    /// Parsed for round-trip fidelity; not read downstream.
    #[serde(default)]
    #[allow(dead_code)]
    pub map_origin: Option<UvttPoint>,
    /// Parsed for round-trip fidelity; not read downstream.
    #[allow(dead_code)]
    pub map_size: UvttPoint,
    /// The source file's own grid pitch, in pixels — adopted directly as
    /// the target scene's `grid_size` by `import_uvtt_impl`, so imported
    /// geometry/lights and the imported background image stay aligned
    /// regardless of whatever grid the scene had before.
    pub pixels_per_grid: f64,
}

#[derive(Debug, Deserialize)]
pub struct UvttPortal {
    #[serde(default)]
    #[allow(dead_code)]
    pub position: Option<UvttPoint>,
    /// Expected to hold exactly two points (the door's endpoints).
    pub bounds: Vec<UvttPoint>,
    #[serde(default)]
    #[allow(dead_code)]
    pub rotation: f64,
    #[serde(default)]
    pub closed: bool,
    /// A portal not attached to any wall/door geometry. Read by
    /// `freestanding_portal_warning` (User Story 3) to disclose that it
    /// wasn't turned into a usable door/wall by this import.
    #[serde(default)]
    pub freestanding: bool,
}

#[derive(Debug, Default, Deserialize)]
pub struct UvttEnvironment {
    /// Parsed for round-trip fidelity; not read downstream (baked
    /// lighting itself is out of this feature's scope, unlike
    /// `ambient_light` below).
    #[serde(default)]
    #[allow(dead_code)]
    pub baked_lighting: bool,
    /// Read by `ambient_light_warning` (User Story 3) — present-and-set
    /// values aren't applied to scene lighting by this import yet.
    #[serde(default)]
    pub ambient_light: Option<String>,
}

fn default_light_intensity() -> f64 {
    1.0
}

#[derive(Debug, Deserialize)]
pub struct UvttLight {
    pub position: UvttPoint,
    pub range: f64,
    #[serde(default = "default_light_intensity")]
    pub intensity: f64,
    pub color: String,
    #[serde(default)]
    pub shadows: bool,
}

#[derive(Debug, Deserialize)]
pub struct UvttFile {
    pub format: f64,
    /// `resolution.pixels_per_grid` is adopted as the target scene's own
    /// `grid_size` by `import_uvtt_impl`, so `grid_units_to_scene_px`'s
    /// `target_grid_size` argument is this value, not whatever grid the
    /// scene had before import.
    pub resolution: UvttResolution,
    #[serde(default)]
    pub line_of_sight: Vec<Vec<UvttPoint>>,
    #[serde(default)]
    pub objects_line_of_sight: Vec<Vec<UvttPoint>>,
    #[serde(default)]
    pub portals: Vec<UvttPortal>,
    /// Parsed but not yet wired to a scene-level ambient-light concept.
    #[serde(default)]
    #[allow(dead_code)]
    pub environment: UvttEnvironment,
    #[serde(default)]
    pub lights: Vec<UvttLight>,
    pub image: String,
}

/// Result of successfully parsing + validating a UVTT file: the parsed
/// document plus a count of any degenerate (< 2 point) line-of-sight
/// polygons that were skipped rather than causing a hard failure.
#[derive(Debug)]
pub struct ParsedUvtt {
    pub file: UvttFile,
    pub skipped_degenerate_polygons: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum MapImportError {
    #[error("invalid UVTT JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("unsupported UVTT format version {found}; only {SUPPORTED_FORMAT} is supported")]
    UnsupportedFormat { found: f64 },
    #[error("image field is not valid base64: {0}")]
    InvalidImageBase64(String),
    #[error("decoded image does not look like a PNG file")]
    InvalidImageMagicBytes,
    #[error("database error: {0}")]
    Database(#[from] diesel::result::Error),
    #[error("io error: {0}")]
    Io(String),
    #[error("scene not found or not owned by caller")]
    SceneNotOwned,
    #[error("upload exceeds the maximum allowed size")]
    PayloadTooLarge,
    #[error("no file field found in multipart upload")]
    MissingFileField,
    #[error("storage error: {0}")]
    Storage(String),
}

/// T020 (User Story 3): the shape of a successful import's response.
/// `warnings` discloses source-file field categories that were parsed but
/// silently not applied — see `research.md` §5-6 — so a GM is never
/// unknowingly missing part of their map. Empty for every fixture that
/// doesn't use those fields (FR-014).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportResult {
    pub walls_created: usize,
    pub doors_created: usize,
    pub lights_created: usize,
    pub background_image_set: bool,
    pub skipped_degenerate_polygons: usize,
    pub warnings: Vec<String>,
}
