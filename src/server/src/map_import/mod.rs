//! Universal VTT (`.dd2vtt`, format `0.3`) map import.
//!
//! Implements:
//! - T023: the UVTT JSON parser (`UvttFile` + `parse_uvtt`) — `parse.rs`.
//! - T024: grid-unit → target-scene-pixel coordinate conversion and the
//!   wall/light "insert row" builders (`walls_from_line_of_sight`,
//!   `walls_from_portals`, `lights_from_uvtt`) — `geometry.rs`.
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
//! src/server test-coverage/file-size audit — internal reorganization
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
use chrono::Utc;
use diesel::prelude::*;
use serde_json::json;
use uuid::Uuid;

use crate::auth_middleware::AuthenticatedUser;
use crate::state::AppState;
use crate::world_events::{EVENT_CODE_MAP_IMPORTED, record_world_event};

pub mod alignment;
mod geometry;
mod image;
mod parse;
mod types;
mod warnings;

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
    let status = match err {
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
) -> Result<ImportResult, MapImportError> {
    // Parse + validate the whole file before touching the DB (T026c).
    let parsed = parse_uvtt(&file_bytes)?;

    let warnings: Vec<String> = [
        freestanding_portal_warning(&parsed.file.portals),
        ambient_light_warning(&parsed.file.environment),
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
        scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .select(scenes::world_id)
            .first::<Uuid>(&mut conn)
            .optional()?
            .ok_or(MapImportError::SceneNotOwned)
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
    let saved_background = save_background_image(
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
    let saved_preview = save_scene_preview_image(&parsed.file.image).await.ok();

    let walls: Vec<WallInsert> =
        walls_from_line_of_sight(&parsed.file.line_of_sight, target_grid_size)
            .into_iter()
            .chain(walls_from_line_of_sight(
                &parsed.file.objects_line_of_sight,
                target_grid_size,
            ))
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

            use crate::schema::canvas_image_assets;
            diesel::insert_into(canvas_image_assets::table)
                .values((
                    canvas_image_assets::asset_id.eq(saved_background.asset_id),
                    canvas_image_assets::world_id.eq(world_id),
                    canvas_image_assets::scene_id.eq(Some(scene_id)),
                    canvas_image_assets::owner_user_id.eq(user_id),
                    canvas_image_assets::storage_path.eq(&saved_background.storage_path),
                    canvas_image_assets::original_format.eq(&saved_background.original_format),
                    canvas_image_assets::width_px.eq(saved_background.width_px),
                    canvas_image_assets::height_px.eq(saved_background.height_px),
                    canvas_image_assets::byte_size.eq(saved_background.byte_size),
                    // Without this the row is NULL here and the background is
                    // permanently uncacheable; see `SavedBackgroundImage`.
                    canvas_image_assets::content_hash
                        .eq(Some(saved_background.content_hash.clone())),
                    canvas_image_assets::kind
                        .eq(crate::db_types::CanvasImageAssetKindEnum::Background),
                    canvas_image_assets::created_by.eq(user_id),
                    canvas_image_assets::updated_by.eq(user_id),
                    canvas_image_assets::created_at.eq(now),
                    canvas_image_assets::updated_at.eq(now),
                ))
                .execute(conn)?;

            // `width`/`height` are set from the imported art's real pixel
            // dimensions, not left at whatever the scene was created with.
            // The engine sizes the background sprite with
            // `custom_size: Vec2::new(scene.width, scene.height)`
            // (`systems/background.rs`), so a freshly-created scene's
            // default 100x100 rendered a 6144x3456 map as a 100-unit
            // sliver — a second, independent reason an imported dd2vtt
            // appeared not to render at all. Pixels are the right unit
            // here: `grid_size` above is already the file's own
            // `pixels_per_grid`, so image-pixel width/height makes the
            // grid line up 1:1 with the art.
            let existing_metadata: Option<serde_json::Value> = scenes::table
                .filter(scenes::scene_id.eq(scene_id))
                .select(scenes::metadata)
                .first::<Option<serde_json::Value>>(conn)?;

            diesel::update(scenes::table.filter(scenes::scene_id.eq(scene_id)))
                .set((
                    scenes::background_asset_id.eq(saved_background.asset_id),
                    scenes::grid_size.eq(new_grid_size),
                    scenes::grid_type.eq("square"),
                    scenes::width.eq(saved_background.width_px),
                    scenes::height.eq(saved_background.height_px),
                    // What the file said the map is, so a later disagreement
                    // between the grid and the background is answerable at all.
                    // Without it the worst case is undetectable: 4096/128 is
                    // exactly 32 and 2304/128 exactly 18, so a scene that is
                    // uniformly 1.5x wrong looks perfectly self-consistent.
                    scenes::metadata.eq(Some(crate::map_import::alignment::record_source_map(
                        existing_metadata,
                        source_map_cells_x,
                        source_map_cells_y,
                        source_pixels_per_grid,
                    ))),
                ))
                .execute(conn)?;

            if let Some(preview) = &saved_preview {
                use crate::schema::scene_preview_images;
                diesel::insert_into(scene_preview_images::table)
                    .values((
                        scene_preview_images::id.eq(preview.asset_id),
                        scene_preview_images::scene_id.eq(scene_id),
                        scene_preview_images::byte_size.eq(preview.byte_size),
                        scene_preview_images::created_at.eq(now),
                    ))
                    .execute(conn)?;
                diesel::update(scenes::table.filter(scenes::scene_id.eq(scene_id)))
                    .set(scenes::preview_asset_id.eq(preview.asset_id))
                    .execute(conn)?;
            }

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
    .map_err(|_| MapImportError::Io("Failed to spawn blocking task".to_string()))?;

    result.map_err(MapImportError::Database)?;

    Ok(ImportResult {
        walls_created,
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

    // Read the uploaded file field, enforcing the size cap as we go.
    let mut file_bytes: Option<Vec<u8>> = None;
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
            break;
        }
    }
    let Some(file_bytes) = file_bytes else {
        return Err(error_response(&MapImportError::MissingFileField));
    };

    let result = import_uvtt_impl(&state, user_id, auth_user.is_admin, scene_id, file_bytes)
        .await
        .map_err(|e| error_response(&e))?;

    Ok(Json(json!({
        "wallsCreated": result.walls_created,
        "doorsCreated": result.doors_created,
        "lightsCreated": result.lights_created,
        "backgroundImageSet": result.background_image_set,
        "skippedDegeneratePolygons": result.skipped_degenerate_polygons,
        "warnings": result.warnings,
    })))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "dedupe_integration_tests.rs"]
mod dedupe_integration_tests;
