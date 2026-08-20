//! Universal VTT (`.dd2vtt`, format `0.3`) map import.
//!
//! Implements:
//! - T023: the UVTT JSON parser (`UvttFile` + `parse_uvtt`).
//! - T024: grid-unit → target-scene-pixel coordinate conversion and the
//!   wall/light "insert row" builders (`walls_from_line_of_sight`,
//!   `walls_from_portals`, `lights_from_uvtt`).
//! - T025: background image decode + save (`save_background_image`).
//! - T026: the `POST /api/scenes/{scene_id}/import/uvtt` REST endpoint.
//! - T027: best-effort NOTIFY emission for the whole import batch.
//!
//! See `specs/001-bevy-canvas-authoring/data-model.md`'s "Map Import"
//! section and `research.md` §7-9 for the design this implements, and
//! `examples/maps/README.md` for the exact source JSON shape.

use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::post;
use axum::{Extension, Router};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use chrono::Utc;
use diesel::prelude::*;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth_middleware::AuthenticatedUser;
use crate::state::AppState;
use crate::world_events::{record_world_event, EVENT_CODE_MAP_IMPORTED};

/// Only this UVTT format version is supported (research.md §7).
const SUPPORTED_FORMAT: f64 = 0.3;
/// Multipart upload size cap (T026b).
const MAX_UPLOAD_BYTES: usize = 50 * 1024 * 1024;
/// PNG file signature, used to sanity-check the decoded `image` field
/// without pulling in a full image-decoding crate (T025).
const PNG_MAGIC: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

// ---------------------------------------------------------------------
// T023: UVTT JSON shape + parser
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct UvttPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Deserialize)]
pub struct UvttResolution {
    #[serde(default)]
    pub map_origin: Option<UvttPoint>,
    pub map_size: UvttPoint,
    pub pixels_per_grid: f64,
}

#[derive(Debug, Deserialize)]
pub struct UvttPortal {
    #[serde(default)]
    pub position: Option<UvttPoint>,
    /// Expected to hold exactly two points (the door's endpoints).
    pub bounds: Vec<UvttPoint>,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub closed: bool,
    #[serde(default)]
    pub freestanding: bool,
}

#[derive(Debug, Default, Deserialize)]
pub struct UvttEnvironment {
    #[serde(default)]
    pub baked_lighting: bool,
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
    pub resolution: UvttResolution,
    #[serde(default)]
    pub line_of_sight: Vec<Vec<UvttPoint>>,
    #[serde(default)]
    pub objects_line_of_sight: Vec<Vec<UvttPoint>>,
    #[serde(default)]
    pub portals: Vec<UvttPortal>,
    #[serde(default)]
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
}

/// T023: parse and validate a raw UVTT JSON payload.
///
/// Rejects any `format` other than `0.3` outright. Skips (rather than
/// fails on) `line_of_sight`/`objects_line_of_sight` polygons with fewer
/// than 2 points, dropping them from the parsed document and reporting
/// how many were skipped.
pub fn parse_uvtt(raw: &[u8]) -> Result<ParsedUvtt, MapImportError> {
    let mut file: UvttFile = serde_json::from_slice(raw)?;

    if (file.format - SUPPORTED_FORMAT).abs() > f64::EPSILON {
        return Err(MapImportError::UnsupportedFormat { found: file.format });
    }

    let mut skipped = 0usize;

    file.line_of_sight.retain(|poly| {
        let keep = poly.len() >= 2;
        if !keep {
            skipped += 1;
        }
        keep
    });
    file.objects_line_of_sight.retain(|poly| {
        let keep = poly.len() >= 2;
        if !keep {
            skipped += 1;
        }
        keep
    });

    Ok(ParsedUvtt {
        file,
        skipped_degenerate_polygons: skipped,
    })
}

// ---------------------------------------------------------------------
// T024: coordinate scaling + wall/light row builders
// ---------------------------------------------------------------------

/// Convert a grid-unit coordinate from the source file into the target
/// scene's pixel space. Per research.md §8, the source file's own
/// `pixels_per_grid` cancels out once working in grid units — only the
/// *target* scene's `grid_size` matters.
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

// ---------------------------------------------------------------------
// T025: background image decode + save
// ---------------------------------------------------------------------

/// Decode the UVTT file's base64 `image` field, sanity-check it looks
/// like a PNG, and save it under `asset_directory` at a scene-scoped,
/// collision-safe relative path. Returns the path relative to
/// `asset_directory` (suitable for `scenes::background_image_path`,
/// servable at `/assets/<that path>`).
pub async fn save_background_image(
    asset_directory: &str,
    scene_id: Uuid,
    image_base64: &str,
) -> Result<String, MapImportError> {
    let bytes = BASE64_STANDARD
        .decode(image_base64)
        .map_err(|e| MapImportError::InvalidImageBase64(e.to_string()))?;

    if bytes.len() < PNG_MAGIC.len() || bytes[..PNG_MAGIC.len()] != PNG_MAGIC {
        return Err(MapImportError::InvalidImageMagicBytes);
    }

    let relative_path = format!("map-imports/{scene_id}/{}.png", Uuid::now_v7());
    let full_path = std::path::Path::new(asset_directory).join(&relative_path);

    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| MapImportError::Io(e.to_string()))?;
    }
    tokio::fs::write(&full_path, &bytes)
        .await
        .map_err(|e| MapImportError::Io(e.to_string()))?;

    Ok(relative_path)
}

// ---------------------------------------------------------------------
// T026: REST endpoint
// ---------------------------------------------------------------------

pub fn router() -> Router<AppState> {
    Router::new().route("/scenes/{scene_id}/import/uvtt", post(import_uvtt))
}

fn error_response(err: &MapImportError) -> (StatusCode, Json<serde_json::Value>) {
    let status = match err {
        MapImportError::InvalidJson(_)
        | MapImportError::UnsupportedFormat { .. }
        | MapImportError::InvalidImageBase64(_)
        | MapImportError::InvalidImageMagicBytes => StatusCode::BAD_REQUEST,
        MapImportError::SceneNotOwned => StatusCode::FORBIDDEN,
        MapImportError::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        MapImportError::MissingFileField => StatusCode::BAD_REQUEST,
        MapImportError::Database(_) | MapImportError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(json!({ "error": err.to_string() })))
}

async fn import_uvtt(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(scene_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;

    // Read the uploaded file field, enforcing the size cap as we go.
    let mut file_bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error": format!("Failed to read multipart field: {e}")}))))?
    {
        if field.name() == Some("file") {
            let bytes = field
                .bytes()
                .await
                .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({"error": format!("Failed to read file bytes: {e}")}))))?;
            if bytes.len() > MAX_UPLOAD_BYTES {
                return Err(error_response(&MapImportError::PayloadTooLarge));
            }
            file_bytes = Some(bytes.to_vec());
            break;
        }
    }
    let Some(file_bytes) = file_bytes else {
        return Err(error_response(&MapImportError::MissingFileField));
    };

    // Parse + validate the whole file before touching the DB (T026c).
    let parsed = parse_uvtt(&file_bytes).map_err(|e| error_response(&e))?;

    let asset_directory = state.directories.asset_directory.clone();
    let db_pool = state.db_pool.clone();

    // Ownership check (T026a) — resolve the scene's grid_size + world_id
    // at the same time, since we need both regardless.
    let ownership_pool = db_pool.clone();
    let scene_row = tokio::task::spawn_blocking(move || -> Result<(i32, Uuid), MapImportError> {
        use crate::schema::scenes;
        let mut conn = ownership_pool
            .get()
            .map_err(|e| MapImportError::Io(format!("Failed to get DB connection: {e}")))?;
        scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .filter(scenes::owner_id.eq(user_id))
            .select((scenes::grid_size, scenes::world_id))
            .first::<(i32, Uuid)>(&mut conn)
            .optional()?
            .ok_or(MapImportError::SceneNotOwned)
    })
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to spawn blocking task"})),
        )
    })?
    .map_err(|e| error_response(&e))?;

    let (grid_size, world_id) = scene_row;
    let target_grid_size = grid_size as f64;

    // Decode + save the background image outside the DB transaction
    // (filesystem writes aren't transactional anyway); if it fails we
    // bail before any DB writes happen.
    let background_path = save_background_image(&asset_directory, scene_id, &parsed.file.image)
        .await
        .map_err(|e| error_response(&e))?;

    let walls: Vec<WallInsert> = walls_from_line_of_sight(&parsed.file.line_of_sight, target_grid_size)
        .into_iter()
        .chain(walls_from_line_of_sight(&parsed.file.objects_line_of_sight, target_grid_size))
        .collect();
    let doors: Vec<WallInsert> = walls_from_portals(&parsed.file.portals, target_grid_size);
    let lights: Vec<LightInsert> = lights_from_uvtt(&parsed.file.lights, target_grid_size);

    let walls_created = walls.len();
    let doors_created = doors.len();
    let lights_created = lights.len();

    let result = tokio::task::spawn_blocking(move || -> Result<(), diesel::result::Error> {
        let mut conn = db_pool
            .get()
            .expect("Failed to get DB connection for map import transaction");
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            use crate::schema::{light_sources, scenes, walls as walls_table};

            let now = Utc::now().naive_utc();

            for wall in walls.iter().chain(doors.iter()) {
                diesel::insert_into(walls_table::table)
                    .values((
                        walls_table::wall_id.eq(Uuid::now_v7()),
                        walls_table::scene_id.eq(scene_id),
                        walls_table::x1.eq(wall.x1),
                        walls_table::y1.eq(wall.y1),
                        walls_table::x2.eq(wall.x2),
                        walls_table::y2.eq(wall.y2),
                        walls_table::blocks_vision.eq(wall.blocks_vision),
                        walls_table::blocks_movement.eq(wall.blocks_movement),
                        walls_table::door_state.eq(wall.door_state),
                        walls_table::created_by.eq(user_id),
                        walls_table::updated_by.eq(user_id),
                        walls_table::created_at.eq(now),
                        walls_table::updated_at.eq(now),
                    ))
                    .execute(conn)?;
            }

            for light in &lights {
                diesel::insert_into(light_sources::table)
                    .values((
                        light_sources::light_id.eq(Uuid::now_v7()),
                        light_sources::scene_id.eq(scene_id),
                        light_sources::x.eq(light.x),
                        light_sources::y.eq(light.y),
                        light_sources::radius.eq(light.radius),
                        light_sources::intensity.eq(light.intensity),
                        light_sources::color.eq(&light.color),
                        light_sources::attached_token_id.eq(None::<Uuid>),
                        light_sources::casts_shadows.eq(light.casts_shadows),
                        light_sources::metadata.eq(None::<serde_json::Value>),
                        light_sources::created_by.eq(user_id),
                        light_sources::updated_by.eq(user_id),
                        light_sources::created_at.eq(now),
                        light_sources::updated_at.eq(now),
                    ))
                    .execute(conn)?;
            }

            diesel::update(scenes::table.filter(scenes::scene_id.eq(scene_id)))
                .set(scenes::background_image_path.eq(&background_path))
                .execute(conn)?;

            // T027: best-effort NOTIFY for the whole batch — do not fail
            // the import if this fails.
            let _ = record_world_event(
                conn,
                world_id,
                EVENT_CODE_MAP_IMPORTED,
                Some(json!({
                    "scene_id": scene_id,
                    "walls_created": walls_created,
                    "doors_created": doors_created,
                    "lights_created": lights_created,
                    "background_image_set": true,
                })),
                user_id,
            );

            Ok(())
        })
    })
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to spawn blocking task"})),
        )
    })?;

    result.map_err(|e| error_response(&MapImportError::Database(e)))?;

    Ok(Json(json!({
        "wallsCreated": walls_created,
        "doorsCreated": doors_created,
        "lightsCreated": lights_created,
        "backgroundImageSet": true,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_fixture(name: &str) -> Vec<u8> {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        // CARGO_MANIFEST_DIR is src/server; fixtures live at repo root.
        let path = std::path::Path::new(manifest_dir)
            .join("../../examples/maps")
            .join(name);
        std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {path:?}: {e}"))
    }

    #[test]
    fn parses_demo_fixture_with_expected_counts() {
        let raw = read_fixture("demo.dd2vtt");
        let parsed = parse_uvtt(&raw).expect("demo.dd2vtt should parse");

        assert_eq!(parsed.skipped_degenerate_polygons, 0);
        assert_eq!(parsed.file.line_of_sight.len(), 8, "8 line_of_sight polygons");
        assert_eq!(parsed.file.portals.len(), 2, "2 doors/portals");
        assert_eq!(parsed.file.lights.len(), 12, "12 lights");

        let target_grid_size = 128.0; // matches source pixels_per_grid for a 1:1 sanity check
        let walls = walls_from_line_of_sight(&parsed.file.line_of_sight, target_grid_size);
        // Sum of (points-1) per polygon for consecutive-pair walls.
        let expected_wall_count: usize = parsed
            .file
            .line_of_sight
            .iter()
            .map(|p| p.len() - 1)
            .sum();
        assert_eq!(walls.len(), expected_wall_count);
        assert_eq!(walls.len(), 31);

        let doors = walls_from_portals(&parsed.file.portals, target_grid_size);
        assert_eq!(doors.len(), 2);
        assert!(doors.iter().any(|d| d.door_state == "closed"));

        let lights = lights_from_uvtt(&parsed.file.lights, target_grid_size);
        assert_eq!(lights.len(), 12);

        // 4.5 grid units * 128 px/grid == 576 px, sanity-checking the
        // coordinate scale math against a known fixture value.
        assert!((lights[0].x - 576.0).abs() < 1e-6);
    }

    #[test]
    fn parses_chamber_fixture_walls_only() {
        let raw = read_fixture("chamber-of-echoing-grief.dd2vtt");
        let parsed = parse_uvtt(&raw).expect("chamber fixture should parse");

        assert_eq!(parsed.file.line_of_sight.len(), 1);
        assert_eq!(parsed.file.portals.len(), 0);
        assert_eq!(parsed.file.lights.len(), 0);

        let walls = walls_from_line_of_sight(&parsed.file.line_of_sight, 128.0);
        assert_eq!(walls.len(), 4, "5-point polygon yields 4 consecutive-pair walls");

        let doors = walls_from_portals(&parsed.file.portals, 128.0);
        assert_eq!(doors.len(), 0);
        let lights = lights_from_uvtt(&parsed.file.lights, 128.0);
        assert_eq!(lights.len(), 0);
    }

    #[test]
    fn rejects_unsupported_format_version() {
        let json = r#"{
            "format": 9.9,
            "resolution": { "map_size": { "x": 1, "y": 1 }, "pixels_per_grid": 128 },
            "image": ""
        }"#;

        let err = parse_uvtt(json.as_bytes()).expect_err("format 9.9 must be rejected");
        match err {
            MapImportError::UnsupportedFormat { found } => assert_eq!(found, 9.9),
            other => panic!("expected UnsupportedFormat, got {other:?}"),
        }
    }

    #[test]
    fn skips_degenerate_line_of_sight_polygons_without_crashing() {
        let json = r#"{
            "format": 0.3,
            "resolution": { "map_size": { "x": 10, "y": 10 }, "pixels_per_grid": 128 },
            "line_of_sight": [
                [ { "x": 1, "y": 1 } ],
                [ { "x": 0, "y": 0 }, { "x": 5, "y": 5 } ]
            ],
            "image": ""
        }"#;

        let parsed = parse_uvtt(json.as_bytes()).expect("should parse despite degenerate polygon");
        assert_eq!(parsed.skipped_degenerate_polygons, 1);
        assert_eq!(parsed.file.line_of_sight.len(), 1);
        let walls = walls_from_line_of_sight(&parsed.file.line_of_sight, 100.0);
        assert_eq!(walls.len(), 1);
    }
}
