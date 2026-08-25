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

    // Ownership check (T026a) — we only need world_id back from this
    // (the scene's *existing* grid_size no longer matters: the import now
    // adopts the source file's own grid, below).
    let ownership_pool = db_pool.clone();
    let world_id = tokio::task::spawn_blocking(move || -> Result<Uuid, MapImportError> {
        use crate::schema::scenes;
        let mut conn = ownership_pool
            .get()
            .map_err(|e| MapImportError::Io(format!("Failed to get DB connection: {e}")))?;
        scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .filter(scenes::owner_id.eq(user_id))
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
    let target_grid_size = parsed.file.resolution.pixels_per_grid;
    let new_grid_size = target_grid_size.round() as i32;

    // Decode + transcode + write the background image to RustFS outside
    // the DB transaction (the RustFS write isn't transactional with
    // Postgres anyway); if it fails we bail before any DB writes happen.
    let saved_background =
        save_background_image(user_id, world_id, scene_id, &parsed.file.image).await?;
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
                    canvas_image_assets::kind
                        .eq(crate::db_types::CanvasImageAssetKindEnum::Background),
                    canvas_image_assets::created_by.eq(user_id),
                    canvas_image_assets::updated_by.eq(user_id),
                    canvas_image_assets::created_at.eq(now),
                    canvas_image_assets::updated_at.eq(now),
                ))
                .execute(conn)?;

            diesel::update(scenes::table.filter(scenes::scene_id.eq(scene_id)))
                .set((
                    scenes::background_asset_id.eq(saved_background.asset_id),
                    scenes::grid_size.eq(new_grid_size),
                    scenes::grid_type.eq("square"),
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

    let result = import_uvtt_impl(&state, user_id, scene_id, file_bytes)
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

    /// Regression test: examples/maps/demo.dd2vtt's `image` field is
    /// genuinely WebP (DungeonDraft's own exporter choice), not PNG —
    /// discovered when a PNG-only magic-byte check rejected it with
    /// "decoded image does not look like a PNG file" during a real
    /// end-to-end import. chamber-of-echoing-grief.dd2vtt's image is a
    /// genuine PNG, so both fixtures together cover both accepted formats.
    #[test]
    fn detect_image_extension_accepts_both_real_fixture_formats() {
        let demo = parse_uvtt(&read_fixture("demo.dd2vtt")).expect("demo.dd2vtt should parse");
        let demo_bytes = BASE64_STANDARD
            .decode(&demo.file.image)
            .expect("demo.dd2vtt image should be valid base64");
        assert_eq!(detect_image_extension(&demo_bytes), Some("webp"));

        let chamber = parse_uvtt(&read_fixture("chamber-of-echoing-grief.dd2vtt"))
            .expect("chamber fixture should parse");
        let chamber_bytes = BASE64_STANDARD
            .decode(&chamber.file.image)
            .expect("chamber fixture image should be valid base64");
        assert_eq!(detect_image_extension(&chamber_bytes), Some("png"));
    }

    #[test]
    fn detect_image_extension_rejects_garbage() {
        assert_eq!(detect_image_extension(b"not an image"), None);
        assert_eq!(detect_image_extension(b""), None);
        // RIFF container present but not WEBP (e.g. a WAV file) must not
        // be misdetected as an image.
        assert_eq!(
            detect_image_extension(b"RIFF\x00\x00\x00\x00WAVEfmt "),
            None
        );
    }

    #[test]
    fn parses_demo_fixture_with_expected_counts() {
        let raw = read_fixture("demo.dd2vtt");
        let parsed = parse_uvtt(&raw).expect("demo.dd2vtt should parse");

        assert_eq!(parsed.skipped_degenerate_polygons, 0);
        assert_eq!(
            parsed.file.line_of_sight.len(),
            8,
            "8 line_of_sight polygons"
        );
        assert_eq!(parsed.file.portals.len(), 2, "2 doors/portals");
        assert_eq!(parsed.file.lights.len(), 12, "12 lights");

        let target_grid_size = 128.0; // matches source pixels_per_grid for a 1:1 sanity check
        let walls = walls_from_line_of_sight(&parsed.file.line_of_sight, target_grid_size);
        // Sum of (points-1) per polygon for consecutive-pair walls.
        let expected_wall_count: usize =
            parsed.file.line_of_sight.iter().map(|p| p.len() - 1).sum();
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
        assert_eq!(
            walls.len(),
            4,
            "5-point polygon yields 4 consecutive-pair walls"
        );

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

    /// T066: the import endpoint's ownership check (T026a) is the exact
    /// same `scenes::table.filter(scene_id).filter(owner_id.eq(user_id))`
    /// shape as `mutations_walls.rs`'s `wall_mutations_are_scoped_to_scene_owner`
    /// — verified directly at the Diesel-query level (rather than through
    /// the full Axum multipart handler) for the same reason that test
    /// does: it's the actual authorization boundary, and a live-DB
    /// `test_transaction` exercises it without needing HTTP/multipart
    /// scaffolding.
    #[test]
    fn import_ownership_check_is_scoped_to_scene_owner() {
        use diesel::PgConnection;

        fn try_connect() -> Option<PgConnection> {
            dotenvy::dotenv().ok();
            let url = std::env::var("DATABASE_URL").ok()?;
            PgConnection::establish(&url).ok()
        }

        let Some(mut conn) = try_connect() else {
            eprintln!(
                "skipping import_ownership_check_is_scoped_to_scene_owner: no DATABASE_URL/dev DB reachable"
            );
            return;
        };

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            use crate::schema::{scenes, users, worlds};

            let owner_id = uuid::Uuid::now_v7();
            let intruder_id = uuid::Uuid::now_v7();
            let world_id = uuid::Uuid::now_v7();
            let scene_id = uuid::Uuid::now_v7();
            let now = chrono::Utc::now().naive_utc();

            for (id, username) in [
                (owner_id, "import-test-owner"),
                (intruder_id, "import-test-intruder"),
            ] {
                diesel::insert_into(users::table)
                    .values((
                        users::id.eq(id),
                        users::username.eq(format!("{username}-{id}")),
                        users::password_hash.eq("test-hash"),
                        users::email.eq(format!("{username}-{id}@example.test")),
                        users::created_at.eq(now),
                        users::updated_at.eq(now),
                    ))
                    .execute(conn)?;
            }

            diesel::insert_into(worlds::table)
                .values((
                    worlds::id.eq(world_id),
                    worlds::name.eq("Import Test World"),
                    worlds::created_by.eq(owner_id),
                    worlds::updated_by.eq(owner_id),
                    worlds::created_at.eq(now),
                    worlds::updated_at.eq(now),
                ))
                .execute(conn)?;

            diesel::insert_into(scenes::table)
                .values((
                    scenes::scene_id.eq(scene_id),
                    scenes::world_id.eq(world_id),
                    scenes::name.eq("Import Test Scene"),
                    scenes::type_.eq("battlemap"),
                    scenes::grid_size.eq(32),
                    scenes::grid_type.eq("square"),
                    scenes::width.eq(1000),
                    scenes::height.eq(1000),
                    scenes::owner_id.eq(owner_id),
                    scenes::created_at.eq(now),
                    scenes::updated_at.eq(now),
                ))
                .execute(conn)?;

            // Same query shape as import_uvtt's ownership check.
            let intruder_result = scenes::table
                .filter(scenes::scene_id.eq(scene_id))
                .filter(scenes::owner_id.eq(intruder_id))
                .select((scenes::grid_size, scenes::world_id))
                .first::<(i32, uuid::Uuid)>(conn)
                .optional()?;
            assert!(
                intruder_result.is_none(),
                "a non-owner's import ownership check must not match another owner's scene"
            );

            let owner_result = scenes::table
                .filter(scenes::scene_id.eq(scene_id))
                .filter(scenes::owner_id.eq(owner_id))
                .select((scenes::grid_size, scenes::world_id))
                .first::<(i32, uuid::Uuid)>(conn)
                .optional()?;
            assert_eq!(
                owner_result,
                Some((32, world_id)),
                "the scene owner's import ownership check must match their own scene"
            );

            Ok(())
        });
    }

    /// T025 (FR-018): map-import's background image path now produces a
    /// WebP object in RustFS via the same `storage/transcode.rs` +
    /// `storage/rustfs.rs` mechanism `uploadCanvasImage` uses — not a
    /// write to the local filesystem — using the real `demo.dd2vtt`
    /// fixture's embedded image. Requires DATABASE_URL is unused here
    /// (save_background_image doesn't touch Postgres); requires a
    /// reachable RustFS (`docker compose up -d rustfs`).
    #[tokio::test]
    async fn save_background_image_writes_webp_to_rustfs_not_filesystem() {
        let demo = parse_uvtt(&read_fixture("demo.dd2vtt")).expect("demo.dd2vtt should parse");
        let owner_user_id = Uuid::now_v7();
        let world_id = Uuid::now_v7();
        let scene_id = Uuid::now_v7();

        let saved = save_background_image(owner_user_id, world_id, scene_id, &demo.file.image)
            .await
            .expect("save_background_image should succeed against a reachable RustFS");

        assert!(saved.storage_path.ends_with(".webp"));
        assert_eq!(
            saved.storage_path,
            crate::storage::rustfs::object_key(
                owner_user_id,
                world_id,
                Some(scene_id),
                saved.asset_id
            )
        );
        // demo.dd2vtt's source image is WebP already (see the regression
        // test above); either way the *stored* format must be WebP.
        assert!(saved.width_px > 0 && saved.height_px > 0);
        assert!(saved.byte_size > 0);

        // Confirm it's really in RustFS (not a local file) by reading it
        // back with the same S3 client machinery, using the root
        // credential directly (proving the object exists at that key —
        // T025's "produces ... a RustFS object, not a local-filesystem
        // file").
        let cfg = crate::storage::rustfs::RustFsConfig::from_env();
        let creds = aws_sdk_s3::config::Credentials::new(
            &cfg.root_access_key,
            &cfg.root_secret_key,
            None,
            None,
            "test-root",
        );
        let conf = aws_sdk_s3::config::Builder::new()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .region(aws_sdk_s3::config::Region::new(cfg.region.clone()))
            .endpoint_url(&cfg.endpoint)
            .credentials_provider(creds)
            .force_path_style(true)
            .build();
        let client = aws_sdk_s3::Client::from_conf(conf);
        let head = client
            .head_object()
            .bucket(&cfg.bucket)
            .key(&saved.storage_path)
            .send()
            .await
            .expect("uploaded object should exist in RustFS");
        assert_eq!(head.content_type(), Some("image/webp"));

        // And that no local-filesystem write happened under the old
        // map-imports/ convention.
        assert!(!std::path::Path::new("map-imports").exists());
    }

    // -------------------------------------------------------------
    // User Story 2: round-trip persistence (T010-T015)
    //
    // Model: `mutations_assets.rs`'s `upload_canvas_image_happy_path_
    // produces_webp_asset` (research.md §4) — build fixtures via
    // `test_support`, perform the write, then re-query via a *fresh*
    // `SELECT` (not the mutation's return value) and assert field-for-
    // field equality against what the source file actually specifies.
    // -------------------------------------------------------------

    /// A wall's round-trip-relevant fields, order-independent (DB row
    /// order is not guaranteed without an explicit ORDER BY, and none of
    /// this feature's FRs care about insertion order — only "is every
    /// field present and correct"). Coordinates compared bit-for-bit
    /// since both sides go through the same `grid_units_to_scene_px`
    /// multiplication with no intervening rounding.
    #[derive(Debug, Clone, PartialEq, PartialOrd)]
    struct WallSignature {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        blocks_vision: bool,
        blocks_movement: bool,
        door_state: String,
    }

    impl From<crate::models::Wall> for WallSignature {
        fn from(w: crate::models::Wall) -> Self {
            WallSignature {
                x1: w.x1,
                y1: w.y1,
                x2: w.x2,
                y2: w.y2,
                blocks_vision: w.blocks_vision,
                blocks_movement: w.blocks_movement,
                door_state: w.door_state,
            }
        }
    }

    impl From<WallInsert> for WallSignature {
        fn from(w: WallInsert) -> Self {
            WallSignature {
                x1: w.x1,
                y1: w.y1,
                x2: w.x2,
                y2: w.y2,
                blocks_vision: w.blocks_vision,
                blocks_movement: w.blocks_movement,
                door_state: w.door_state.to_string(),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, PartialOrd)]
    struct LightSignature {
        x: f64,
        y: f64,
        radius: f64,
        intensity: f64,
        color: Option<String>,
        casts_shadows: bool,
    }

    impl From<crate::models::LightSource> for LightSignature {
        fn from(l: crate::models::LightSource) -> Self {
            LightSignature {
                x: l.x,
                y: l.y,
                radius: l.radius,
                intensity: l.intensity,
                color: l.color,
                casts_shadows: l.casts_shadows,
            }
        }
    }

    impl From<LightInsert> for LightSignature {
        fn from(l: LightInsert) -> Self {
            LightSignature {
                x: l.x,
                y: l.y,
                radius: l.radius,
                intensity: l.intensity,
                color: Some(l.color),
                casts_shadows: l.casts_shadows,
            }
        }
    }

    fn sorted<T: PartialOrd>(mut v: Vec<T>) -> Vec<T> {
        v.sort_by(|a, b| {
            a.partial_cmp(b)
                .expect("no NaN coordinates in test fixtures")
        });
        v
    }

    /// Re-queries every wall/light for `scene_id` from a fresh DB
    /// connection (not the import's in-memory return value) and asserts
    /// exact field equality against what the source fixture specifies,
    /// plus a background asset actually being set on the scene. Shared by
    /// T010-T012's three fixtures.
    async fn assert_round_trip_matches_fixture(fixture_name: &str) {
        use crate::test_support::*;

        // Loading `.env` here (rather than relying on another test having
        // already done so first) avoids a real ordering hazard:
        // `test_app_state()` reads `DATABASE_URL` directly from the
        // process environment with no dotenv fallback of its own, so
        // whichever test runs first in this binary is the one that
        // actually needs it loaded.
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let raw = read_fixture(fixture_name);
        let parsed = parse_uvtt(&raw).expect("fixture should parse");

        // Import now adopts the source file's own grid (regardless of
        // `test_support::insert_test_scene`'s fixed grid_size of 5) — see
        // `import_uvtt_impl`'s comment on why.
        let target_grid_size = parsed.file.resolution.pixels_per_grid;

        let mut expected_walls: Vec<WallSignature> =
            walls_from_line_of_sight(&parsed.file.line_of_sight, target_grid_size)
                .into_iter()
                .chain(walls_from_line_of_sight(
                    &parsed.file.objects_line_of_sight,
                    target_grid_size,
                ))
                .chain(walls_from_portals(&parsed.file.portals, target_grid_size))
                .map(WallSignature::from)
                .collect();
        expected_walls = sorted(expected_walls);

        let mut expected_lights: Vec<LightSignature> =
            lights_from_uvtt(&parsed.file.lights, target_grid_size)
                .into_iter()
                .map(LightSignature::from)
                .collect();
        expected_lights = sorted(expected_lights);

        let result = import_uvtt_impl(&state, owner_id, scene_id, raw)
            .await
            .expect("import should succeed");

        assert_eq!(
            result.walls_created + result.doors_created,
            expected_walls.len()
        );
        assert_eq!(result.lights_created, expected_lights.len());

        // Re-query from a fresh connection — the point of this test.
        let mut conn = state.db_pool.get().unwrap();
        use crate::schema::{light_sources, scenes, walls as walls_table};

        let reloaded_walls: Vec<WallSignature> = walls_table::table
            .filter(walls_table::scene_id.eq(scene_id))
            .select(crate::models::Wall::as_select())
            .load::<crate::models::Wall>(&mut conn)
            .expect("walls should reload")
            .into_iter()
            .map(WallSignature::from)
            .collect();
        assert_eq!(
            sorted(reloaded_walls),
            expected_walls,
            "reloaded walls must exactly match {fixture_name}'s source geometry"
        );

        let reloaded_lights: Vec<LightSignature> = light_sources::table
            .filter(light_sources::scene_id.eq(scene_id))
            .select(crate::models::LightSource::as_select())
            .load::<crate::models::LightSource>(&mut conn)
            .expect("lights should reload")
            .into_iter()
            .map(LightSignature::from)
            .collect();
        assert_eq!(
            sorted(reloaded_lights),
            expected_lights,
            "reloaded lights must exactly match {fixture_name}'s source lights"
        );

        let (background_asset_id, reloaded_grid_size) = scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .select((scenes::background_asset_id, scenes::grid_size))
            .first::<(Option<Uuid>, i32)>(&mut conn)
            .expect("scene should reload");
        assert!(
            background_asset_id.is_some(),
            "reloaded scene must reference the background image asset created by import"
        );
        assert_eq!(
            reloaded_grid_size,
            target_grid_size.round() as i32,
            "import must adopt {fixture_name}'s own pixels_per_grid as the scene's grid_size"
        );
    }

    /// T010: `road-side-in.dd2vtt` — 24 line-of-sight polygons / 16
    /// portals / 4 lights, the richest real fixture — is the primary
    /// round-trip stress test (FR-008, FR-009, FR-010).
    #[tokio::test]
    async fn round_trip_road_side_in_matches_fixture_exactly() {
        assert_round_trip_matches_fixture("road-side-in.dd2vtt").await;
    }

    /// T011: `dwarven-forge.dd2vtt` — walls-only (no doors/lights) —
    /// confirms the walls-only path is equally durable.
    #[tokio::test]
    async fn round_trip_dwarven_forge_walls_only_matches_fixture_exactly() {
        assert_round_trip_matches_fixture("dwarven-forge.dd2vtt").await;
    }

    /// T012: `demo.dd2vtt` — walls, doors, lights, baked lighting — the
    /// broadest-coverage existing fixture.
    #[tokio::test]
    async fn round_trip_demo_matches_fixture_exactly() {
        assert_round_trip_matches_fixture("demo.dd2vtt").await;
    }

    /// T013 (FR-011, spec.md US2 Acceptance Scenario 3): hand-built
    /// changes applied *on top of* an import — a new wall, a passability
    /// toggle on an imported wall, and a new light — must persist exactly
    /// as durably as the import itself. Exercises the same
    /// `walls`/`light_sources` update path `update_wall`/
    /// `create_light_source` use (research.md §3's ownership-filter
    /// pattern), at the Diesel level directly rather than through the
    /// full GraphQL resolver (consistent with this file's/mutations_
    /// walls.rs's existing test style).
    #[tokio::test]
    async fn hand_built_edits_on_top_of_an_import_persist_exactly() {
        use crate::test_support::*;

        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let raw = read_fixture("dwarven-forge.dd2vtt");
        import_uvtt_impl(&state, owner_id, scene_id, raw)
            .await
            .expect("import should succeed");

        let mut conn = state.db_pool.get().unwrap();
        use crate::schema::{light_sources, scenes, walls as walls_table};

        // Pick an arbitrary imported wall to toggle passability on, same
        // ownership-scoped update shape as `update_wall`.
        let target_wall_id = walls_table::table
            .filter(walls_table::scene_id.eq(scene_id))
            .select(walls_table::wall_id)
            .first::<Uuid>(&mut conn)
            .expect("import should have created at least one wall");

        diesel::update(
            walls_table::table
                .filter(walls_table::wall_id.eq(target_wall_id))
                .filter(
                    walls_table::scene_id.eq_any(
                        scenes::table
                            .filter(scenes::owner_id.eq(owner_id))
                            .select(scenes::scene_id),
                    ),
                ),
        )
        .set(walls_table::blocks_movement.eq(true))
        .execute(&mut conn)
        .expect("passability toggle should succeed");

        // A brand-new hand-drawn wall, same insert shape create_wall uses.
        let new_wall_id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(walls_table::table)
            .values((
                walls_table::wall_id.eq(new_wall_id),
                walls_table::scene_id.eq(scene_id),
                walls_table::x1.eq(1.0),
                walls_table::y1.eq(2.0),
                walls_table::x2.eq(3.0),
                walls_table::y2.eq(4.0),
                walls_table::blocks_vision.eq(true),
                walls_table::blocks_movement.eq(false),
                walls_table::door_state.eq("none"),
                walls_table::created_by.eq(owner_id),
                walls_table::updated_by.eq(owner_id),
                walls_table::created_at.eq(now),
                walls_table::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("hand-drawn wall insert should succeed");

        // A hand-placed light ("torch"), same insert shape
        // create_light_source uses.
        let new_light_id = Uuid::now_v7();
        diesel::insert_into(light_sources::table)
            .values((
                light_sources::light_id.eq(new_light_id),
                light_sources::scene_id.eq(scene_id),
                light_sources::x.eq(10.0),
                light_sources::y.eq(20.0),
                light_sources::radius.eq(100.0),
                light_sources::intensity.eq(1.0),
                light_sources::casts_shadows.eq(true),
                light_sources::created_by.eq(owner_id),
                light_sources::updated_by.eq(owner_id),
                light_sources::created_at.eq(now),
                light_sources::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("hand-placed light insert should succeed");
        drop(conn);

        // Reload from a fresh connection and confirm all three edits
        // (not just the originally-imported state) are exactly present.
        let mut conn = state.db_pool.get().unwrap();
        let reloaded_wall = walls_table::table
            .filter(walls_table::wall_id.eq(target_wall_id))
            .select(crate::models::Wall::as_select())
            .first::<crate::models::Wall>(&mut conn)
            .expect("toggled wall should reload");
        assert!(
            reloaded_wall.blocks_movement,
            "passability toggle must survive reload"
        );

        let reloaded_new_wall = walls_table::table
            .filter(walls_table::wall_id.eq(new_wall_id))
            .select(crate::models::Wall::as_select())
            .first::<crate::models::Wall>(&mut conn)
            .expect("hand-drawn wall should reload");
        assert_eq!(
            (
                reloaded_new_wall.x1,
                reloaded_new_wall.y1,
                reloaded_new_wall.x2,
                reloaded_new_wall.y2
            ),
            (1.0, 2.0, 3.0, 4.0)
        );

        let reloaded_new_light = light_sources::table
            .filter(light_sources::light_id.eq(new_light_id))
            .select(crate::models::LightSource::as_select())
            .first::<crate::models::LightSource>(&mut conn)
            .expect("hand-placed light should reload");
        assert_eq!(
            (
                reloaded_new_light.x,
                reloaded_new_light.y,
                reloaded_new_light.radius
            ),
            (10.0, 20.0, 100.0)
        );
    }

    /// T015 (SC-006): a documented, one-time verification that the
    /// round-trip check above actually has teeth — i.e. it would fail if
    /// a real fidelity bug were introduced, not just pass vacuously.
    ///
    /// Verification actually performed during this feature's
    /// implementation (not just described): temporarily changed
    /// `WallSignature::from(crate::models::Wall)` above to
    /// `x1: w.x1 + 1.0` (simulating a coordinate-fidelity bug on
    /// reload). Result: `cargo test map_import::tests::round_trip`
    /// failed immediately and specifically — every one of
    /// `round_trip_demo_matches_fixture_exactly`,
    /// `round_trip_dwarven_forge_walls_only_matches_fixture_exactly`,
    /// and `round_trip_road_side_in_matches_fixture_exactly` failed at
    /// their `assert_eq!(sorted(reloaded_walls), expected_walls, ...)`
    /// line with a clear "reloaded walls must exactly match ...'s source
    /// geometry" message showing the off-by-one `x1` values — not a
    /// silent pass. (An earlier attempt hard-coding
    /// `blocks_movement: false` in the same spot did *not* catch
    /// anything, because every wall this importer produces already has
    /// `blocks_movement: false` — a useful negative-control finding in
    /// its own right, showing the check's teeth are in the coordinate/
    /// door-state fields, not the always-false import-time
    /// `blocks_movement` default.) The change was then reverted; `cargo
    /// test map_import::tests::round_trip` passed again with the fix
    /// undone. This confirms the round-trip tests genuinely detect
    /// fidelity regressions rather than trivially passing regardless of
    /// what's persisted.
    #[test]
    fn round_trip_tests_have_teeth_verification_is_documented_not_a_live_test() {
        // Intentionally a no-op: see this test's doc comment above for
        // the one-time verification this documents.
    }

    // -------------------------------------------------------------
    // User Story 3: import result field-gap disclosure (T017-T019)
    // -------------------------------------------------------------

    async fn import_and_get_warnings(fixture_name: &str) -> Vec<String> {
        use crate::test_support::*;

        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let raw = read_fixture(fixture_name);
        let result = import_uvtt_impl(&state, owner_id, scene_id, raw)
            .await
            .unwrap_or_else(|e| panic!("{fixture_name} should import successfully: {e}"));
        result.warnings
    }

    /// T017: `little-fish-academy.dd2vtt`'s non-default `ambient_light`
    /// must be disclosed (FR-012, FR-013).
    #[tokio::test]
    async fn warnings_disclose_non_default_ambient_light() {
        let warnings = import_and_get_warnings("little-fish-academy.dd2vtt").await;
        assert!(
            warnings
                .iter()
                .any(|w| w.to_lowercase().contains("ambient")),
            "expected an ambient_light warning, got: {warnings:?}"
        );
    }

    /// T018: the synthetic fixture's freestanding portal and
    /// `objects_line_of_sight` polygon must both be disclosed (FR-012,
    /// FR-013).
    #[tokio::test]
    async fn warnings_disclose_freestanding_portal_and_objects_line_of_sight() {
        let warnings =
            import_and_get_warnings("synthetic-freestanding-portal-and-object-los.dd2vtt").await;
        assert!(
            warnings
                .iter()
                .any(|w| w.to_lowercase().contains("freestanding")),
            "expected a freestanding-portal warning, got: {warnings:?}"
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.to_lowercase().contains("objects_line_of_sight")),
            "expected an objects_line_of_sight warning, got: {warnings:?}"
        );
    }

    /// T019 (FR-014, SC-004): every fixture that doesn't use the three
    /// unhandled field categories must produce an empty `warnings` — no
    /// new noise for the common case.
    #[tokio::test]
    async fn warnings_are_empty_for_fixtures_without_unhandled_fields() {
        for fixture in [
            "demo.dd2vtt",
            "chamber-of-echoing-grief.dd2vtt",
            "grassy-path-ambush.dd2vtt",
            "azheim-meeting.dd2vtt",
            "road-side-in.dd2vtt",
            "dwarven-forge.dd2vtt",
        ] {
            let warnings = import_and_get_warnings(fixture).await;
            assert!(
                warnings.is_empty(),
                "{fixture} should produce no warnings, got: {warnings:?}"
            );
        }
    }

    // -------------------------------------------------------------
    // User Story 3: pure-function warning-builder unit tests
    // (T020-T024, no DB required)
    // -------------------------------------------------------------

    #[test]
    fn freestanding_portal_warning_fires_only_when_present() {
        assert!(freestanding_portal_warning(&[]).is_none());
        let none_freestanding = vec![UvttPortal {
            position: None,
            bounds: vec![UvttPoint { x: 0.0, y: 0.0 }, UvttPoint { x: 1.0, y: 0.0 }],
            rotation: 0.0,
            closed: true,
            freestanding: false,
        }];
        assert!(freestanding_portal_warning(&none_freestanding).is_none());

        let one_freestanding = vec![UvttPortal {
            position: None,
            bounds: vec![UvttPoint { x: 0.0, y: 0.0 }, UvttPoint { x: 1.0, y: 0.0 }],
            rotation: 0.0,
            closed: true,
            freestanding: true,
        }];
        assert!(freestanding_portal_warning(&one_freestanding).is_some());
    }

    #[test]
    fn ambient_light_warning_ignores_the_exporter_default() {
        assert!(
            ambient_light_warning(&UvttEnvironment {
                baked_lighting: false,
                ambient_light: None,
            })
            .is_none()
        );
        assert!(
            ambient_light_warning(&UvttEnvironment {
                baked_lighting: false,
                ambient_light: Some("ffffffff".to_string()),
            })
            .is_none()
        );
        assert!(
            ambient_light_warning(&UvttEnvironment {
                baked_lighting: false,
                ambient_light: Some("fffff7e4".to_string()),
            })
            .is_some()
        );
    }

    #[test]
    fn objects_line_of_sight_warning_fires_only_when_non_empty() {
        assert!(objects_line_of_sight_warning(&[]).is_none());
        assert!(
            objects_line_of_sight_warning(&[vec![
                UvttPoint { x: 0.0, y: 0.0 },
                UvttPoint { x: 1.0, y: 1.0 },
            ]])
            .is_some()
        );
    }
}
