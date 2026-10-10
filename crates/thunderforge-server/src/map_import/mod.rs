//! Universal VTT (`.dd2vtt`, format `0.3`) map import.
//!
//! Implements:
//! - T023: the UVTT JSON parser (`UvttFile` + `parse_uvtt`) — `parse.rs`.
//! - T024: grid-unit → scene coordinate conversion (`ScenePlacement`) and
//!   the wall/light "insert row" builders (`walls_from_line_of_sight`,
//!   `walls_from_portals`, `lights_from_uvtt`) — `geometry.rs`.
//! - The scene's ambient light, from the file's own (`ambient_level`) —
//!   `ambient.rs`.
//! - T025: background image decode + save (`save_background_image`) —
//!   `image.rs`.
//! - T026: the `POST /api/scenes/{scene_id}/import/uvtt` REST endpoint —
//!   this file.
//! - T027: best-effort NOTIFY emission for the whole import batch.
//!
//! See `specs/001-bevy-canvas-authoring/data-model.md`'s "Map Import"
//! section and `research.md` §7-9 for the design this implements, and
//! `examples/maps/README.md` for the exact source JSON shape.
//!
//! Split from a single flat `map_import.rs` into this directory module
//! (types.rs / parse.rs / geometry.rs / image.rs / warnings.rs, with the
//! HTTP endpoint + top-level orchestration staying here) per the
//! crates/thunderforge-server test-coverage/file-size audit — internal reorganization
//! only, `router()` remains the only symbol used outside this module
//! (`main.rs`).

use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::post;
use axum::{Extension, Router};
// Only used by `#[cfg(test)]` code below (`save_background_image` itself
// now lives in `image.rs`, with its own copy of these two imports) — cargo
// check (which skips `#[cfg(test)]`) would otherwise flag these unused.
#[cfg(test)]
use base64::Engine as _;
#[cfg(test)]
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
#[cfg(test)]
use chrono::Utc;
use diesel::prelude::*;
use serde_json::json;
use uuid::Uuid;

use crate::auth_middleware::AuthenticatedUser;
use crate::state::AppState;

pub mod alignment;
mod ambient;
mod geometry;
mod image;
mod offline;
mod parse;
pub mod perimeter;
mod scene_write;
mod types;
mod warnings;

pub use geometry::{LightInsert, ScenePlacement, WallInsert};
pub use image::{PreparedBackground, StoredPlace, store_background_webp, store_scene_preview_webp};
pub use offline::{OfflineImport, import_offline};
pub use scene_write::{MapWrite, SourceMap, stale_perimeter, write_map};
pub use types::MapImportError;

use ambient::ambient_level;
use geometry::*;
use image::*;
use parse::*;
use types::*;
use warnings::*;

/// Multipart upload size cap (T026b).
const MAX_UPLOAD_BYTES: usize = 50 * 1024 * 1024;

pub fn router() -> Router<AppState> {
    // Axum's `Multipart` extractor applies its own default body-size limit
    // (2MB) ahead of any handler code — well under MAX_UPLOAD_BYTES (50MB)
    // and under real-world map file sizes (examples/maps/demo.dd2vtt alone
    // is ~4.2MB), so without raising it here every non-trivial import fails
    // with a generic "error parsing multipart/form-data request" before
    // T026b's own size check ever runs. Cap it at MAX_UPLOAD_BYTES so our
    // own check (which returns a clean 413 with a real error body) is what
    // actually rejects oversized uploads.
    Router::new()
        .route("/scenes/{scene_id}/import/uvtt", post(import_uvtt))
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES))
}

fn error_response(err: &MapImportError) -> (StatusCode, Json<serde_json::Value>) {
    use crate::play_pause::gate::{GateError, WORLD_PLAY_PAUSED};
    let status = match err {
        // Spec 051 (contracts/live-play-lock.md): the one REST refusal with a
        // code, because the client routes on it as it does on the GraphQL one.
        MapImportError::Paused(GateError::Paused(_)) => {
            return (
                StatusCode::LOCKED,
                Json(json!({ "code": WORLD_PLAY_PAUSED })),
            );
        }
        MapImportError::Paused(GateError::Unreadable(_)) => StatusCode::SERVICE_UNAVAILABLE,
        MapImportError::InvalidJson(_)
        | MapImportError::UnsupportedFormat { .. }
        | MapImportError::InvalidImageBase64(_)
        | MapImportError::InvalidImageMagicBytes => StatusCode::BAD_REQUEST,
        MapImportError::SceneNotOwned => StatusCode::FORBIDDEN,
        MapImportError::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        MapImportError::MissingFileField => StatusCode::BAD_REQUEST,
        MapImportError::Database(_) | MapImportError::Io(_) | MapImportError::Storage(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };
    (status, Json(json!({ "error": err.to_string() })))
}

/// Core import logic, independent of the HTTP/multipart layer, so tests
/// can call it directly and re-query the DB afterward (research.md §4's
/// round-trip pattern) — mirrors `mutations_assets.rs`'s
/// `upload_canvas_image_impl` shape.
pub async fn import_uvtt_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    scene_id: Uuid,
    file_bytes: Vec<u8>,
    // Spec 088 FR-090: wall the map's edges, replacing the edge walls a
    // previous import left.
    wall_edges: bool,
) -> Result<ImportResult, MapImportError> {
    // Parse + validate the whole file before touching the DB (T026c).
    let parsed = parse_uvtt(&file_bytes)?;

    let warnings: Vec<String> = [
        freestanding_portal_warning(&parsed.file.portals),
        objects_line_of_sight_warning(&parsed.file.objects_line_of_sight),
    ]
    .into_iter()
    .flatten()
    .collect();

    let db_pool = state.db_pool.clone();

    // Authority check (T026a) — importing a map writes walls, doors, lights
    // and a background onto a scene, so it is content authoring and follows
    // the world role: the Owner and any GM may import onto any scene in
    // their world, a Player or non-member onto none. It used to require
    // `scenes.owner_id == caller`, which locked a co-GM out of every scene
    // they had not personally created. We only need world_id back from this
    // (the scene's *existing* grid_size no longer matters: the import now
    // adopts the source file's own grid, below).
    let ownership_pool = db_pool.clone();
    let world_id = tokio::task::spawn_blocking(move || -> Result<Uuid, MapImportError> {
        use crate::schema::scenes;
        let mut conn = ownership_pool
            .get()
            .map_err(|e| MapImportError::Io(format!("Failed to get DB connection: {e}")))?;
        if !crate::auth::world_membership::is_dm_of_scene(&mut conn, user_id, is_admin, scene_id)? {
            return Err(MapImportError::SceneNotOwned);
        }
        let world_id = scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .select(scenes::world_id)
            .first::<Uuid>(&mut conn)
            .optional()?
            .ok_or(MapImportError::SceneNotOwned)?;
        // Spec 051: before the background is stored or a wall is written.
        crate::play_pause::gate::refuse_if_paused(&mut conn, world_id)
            .map_err(MapImportError::Paused)?;
        Ok(world_id)
    })
    .await
    .map_err(|_| MapImportError::Io("Failed to spawn blocking task".to_string()))??;

    // Adopt the source file's own grid instead of the scene's existing
    // one: a UVTT file's line_of_sight/portal/light coordinates are in
    // *that file's* grid units, and its background image's own squares
    // are `resolution.pixels_per_grid` pixels apart. Converting those
    // grid-unit coordinates using anything other than the file's own
    // pixels_per_grid would place walls/doors/lights out of alignment
    // with the very background image being imported alongside them
    // whenever the file's native grid differs from whatever grid the
    // target scene happened to have before (every bundled example map is
    // coincidentally 128px/256px square, which made this invisible until
    // a real map with a different native grid was tried). The scene's
    // `grid_size` is updated to match, below, so the frontend's grid
    // overlay stays aligned with the newly imported background too. Any
    // tokens already placed on the scene keep their absolute pixel
    // position — only the grid overlay/new geometry moves to the file's
    // native scale.
    // Decode + transcode + write the background image to RustFS outside
    // the DB transaction (the RustFS write isn't transactional with
    // Postgres anyway); if it fails we bail before any DB writes happen.
    //
    // This happens **before** the grid is decided, and that ordering is the
    // fix rather than an accident. A background wider than the GPU texture cap
    // is stored smaller than it arrived, so the file's own `pixels_per_grid`
    // describes an image that no longer exists: a 6144x3456 map became a
    // 4096x2304 background under a 128px grid — thirty-two cells drawn across
    // a map with forty-eight, and every wall, portal and light out by the same
    // 1.5x. `transcode_map_background` picks the stored cell size and the
    // stored image together, and reports the one that survived.
    // Resolved once for the whole import, which is also the only sane place
    // for it: the background and its preview must land in the same store even
    // if an operator changes the answer halfway through an upload.
    let storage_cfg = crate::storage::rustfs::RustFsConfig::resolve(state).await;
    let saved_background = save_background_image(
        &storage_cfg,
        user_id,
        world_id,
        scene_id,
        &parsed.file.image,
        parsed.file.resolution.pixels_per_grid,
        Some(state.db_pool.clone()),
    )
    .await?;

    let new_grid_size = saved_background.grid_size;
    let target_grid_size = f64::from(new_grid_size);

    // Recorded alongside the scene below. Read from the file rather than
    // derived from the stored image, because the point of keeping it is to
    // have a statement of what the map *is* that is independent of whatever
    // the storage path did to the picture.
    let source_map_cells_x = parsed.file.resolution.map_size.x;
    let source_map_cells_y = parsed.file.resolution.map_size.y;
    let source_pixels_per_grid = parsed.file.resolution.pixels_per_grid;
    // Spec 022 (FR-012): a preview/thumbnail rendition, generated
    // alongside the full-resolution background from the same source
    // bytes. Best-effort — a preview-generation failure must not fail the
    // whole import (the map itself already saved successfully above).
    let saved_preview = save_scene_preview_image(&storage_cfg, &parsed.file.image)
        .await
        .ok();

    // Walls, doors and lights are placed on the background as stored: centred
    // on the origin, y up — see `ScenePlacement`.
    let placement = ScenePlacement {
        grid_size: target_grid_size,
        width: f64::from(saved_background.width_px),
        height: f64::from(saved_background.height_px),
    };
    let walls: Vec<WallInsert> = walls_from_line_of_sight(&parsed.file.line_of_sight, &placement)
        .into_iter()
        .chain(walls_from_line_of_sight(
            &parsed.file.objects_line_of_sight,
            &placement,
        ))
        .collect();
    let doors: Vec<WallInsert> = walls_from_portals(&parsed.file.portals, &placement);
    let lights: Vec<LightInsert> = lights_from_uvtt(&parsed.file.lights, &placement);

    // Walled where the file's own walls and doors leave an edge open.
    let mut walls = walls;
    let perimeter_walls_created = if wall_edges {
        let file_walls: Vec<WallInsert> = walls.iter().chain(doors.iter()).cloned().collect();
        let edges = perimeter::perimeter_walls(&placement, &file_walls);
        let count = edges.len();
        walls.extend(edges);
        count
    } else {
        0
    };

    let walls_created = walls.len();
    let doors_created = doors.len();
    let lights_created = lights.len();
    // The light the map was drawn in, so a night map arrives dark and its
    // walls cast shadows from the first frame. A re-import resets a level the
    // Game Master chose, as it resets the walls: the map is being replaced.
    let ambient_light = ambient_level(&parsed.file.environment);

    let write = MapWrite {
        world_id,
        scene_id,
        user_id,
        background: saved_background,
        preview: saved_preview,
        walls,
        doors,
        lights,
        ambient_light: ambient_light.to_string(),
        source_map: Some(SourceMap {
            cells_x: source_map_cells_x,
            cells_y: source_map_cells_y,
            pixels_per_grid: source_pixels_per_grid,
        }),
        base_map_id: None,
        replace_perimeter: wall_edges,
    };
    let result = tokio::task::spawn_blocking(move || -> Result<(), diesel::result::Error> {
        let mut conn = db_pool
            .get()
            .expect("Failed to get DB connection for map import transaction");
        conn.transaction(|conn| write_map(conn, &write))
    })
    .await
    .map_err(|_| MapImportError::Io("Failed to spawn blocking task".to_string()))?;

    result.map_err(MapImportError::Database)?;

    Ok(ImportResult {
        walls_created,
        perimeter_walls_created,
        doors_created,
        lights_created,
        background_image_set: true,
        skipped_degenerate_polygons: parsed.skipped_degenerate_polygons,
        warnings,
    })
}

/// Thin Axum handler: reads the multipart body, delegates to
/// `import_uvtt_impl`, and serializes the result to JSON. Kept separate
/// from `import_uvtt_impl` so tests can call the core logic directly
/// without HTTP/multipart scaffolding (research.md §4).
async fn import_uvtt(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(scene_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;

    // Read the uploaded file field, enforcing the size cap as we go, and
    // `wallEdges` (spec 088 FR-091), which is on unless it says otherwise.
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut wall_edges = true;
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("Failed to read multipart field: {e}")})),
        )
    })? {
        if field.name() == Some("file") {
            let bytes = field.bytes().await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": format!("Failed to read file bytes: {e}")})),
                )
            })?;
            if bytes.len() > MAX_UPLOAD_BYTES {
                return Err(error_response(&MapImportError::PayloadTooLarge));
            }
            file_bytes = Some(bytes.to_vec());
        } else if field.name() == Some("wallEdges") {
            let value = field.text().await.unwrap_or_default();
            wall_edges = wall_edges_from(&value);
        }
    }
    let Some(file_bytes) = file_bytes else {
        return Err(error_response(&MapImportError::MissingFileField));
    };

    let result = import_uvtt_impl(
        &state,
        user_id,
        auth_user.is_admin,
        scene_id,
        file_bytes,
        wall_edges,
    )
    .await
    .map_err(|e| error_response(&e))?;

    Ok(Json(json!({
        "wallsCreated": result.walls_created,
        "perimeterWallsCreated": result.perimeter_walls_created,
        "doorsCreated": result.doors_created,
        "lightsCreated": result.lights_created,
        "backgroundImageSet": result.background_image_set,
        "skippedDegeneratePolygons": result.skipped_degenerate_polygons,
        "warnings": result.warnings,
    })))
}

/// The multipart `wallEdges` field: `false`, `0` or `off` turn it off;
/// anything else, or no field at all, leaves it on.
fn wall_edges_from(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off"
    )
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "dedupe_integration_tests.rs"]
mod dedupe_integration_tests;

#[cfg(test)]
#[path = "scene_write_tests.rs"]
mod scene_write_tests;

#[cfg(test)]
#[path = "perimeter_import_tests.rs"]
mod perimeter_import_tests;
