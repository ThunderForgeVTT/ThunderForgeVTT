//! HTTP routes.
//!
//! All routing lives here rather than in the binary so tests can drive the
//! router in-process — the same split `thunderforge_crucible::server` uses.
//!
//! Routes:
//! - `GET /maps` — the corpus, with each map's pyramid
//! - `GET /maps/{name}` — one map's metadata
//! - `GET /maps/{name}/tile/{level}/{col}/{row}.webp` — one tile
//! - `GET /maps/{name}/full.webp?max=N` — a whole-image rendition, capped

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use image::DynamicImage;
use serde::{Deserialize, Serialize};

use crate::source::MapSource;
use crate::tiles::TileId;

/// Ceiling on a `full.webp` rendition, matching the main server's
/// `MAX_CANVAS_TEXTURE_DIMENSION`.
///
/// Duplicated rather than shared because the main server is a binary crate
/// with no library target to depend on. The number matters more than the
/// sharing: a rendition wider than the device's `MAX_TEXTURE_SIZE` fails to
/// upload and the map silently does not render, and 4096 is the common
/// integrated-GPU limit.
pub const MAX_FULL_DIMENSION: u32 = 4096;

#[derive(Serialize)]
struct MapSummary {
    name: String,
    width: u32,
    height: u32,
    pixels_per_grid: f32,
    tile_size: u32,
    levels: Vec<crate::tiles::LevelInfo>,
    total_tiles: u32,
}

#[derive(Deserialize)]
struct FullQuery {
    /// Longest permitted side. Clamped to `MAX_FULL_DIMENSION`.
    max: Option<u32>,
}

pub fn router(source: MapSource) -> Router {
    Router::new()
        .route("/maps", get(list_maps))
        .route("/maps/{name}", get(map_info))
        // The trailing segment carries an optional `.webp`, parsed off in the
        // handler rather than matched in the route: axum permits only one
        // parameter per path segment and will not mix it with literal text.
        // The extension is kept usable because Bevy's `AssetServer` resolves
        // an image loader by file extension.
        .route("/maps/{name}/tile/{level}/{col}/{row}", get(tile))
        .route("/maps/{name}/full", get(full))
        // Wide open: this service has no auth and no session, and the sandbox
        // it serves runs on a different port. It is a development tool and
        // must never be exposed beyond localhost.
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(source)
}

async fn list_maps(State(source): State<MapSource>) -> Response {
    let summaries: Vec<MapSummary> = source
        .list()
        .into_iter()
        .filter_map(|name| {
            let map = source.load(&name).ok()?;
            Some(summary(&map))
        })
        .collect();
    Json(summaries).into_response()
}

fn summary(map: &crate::source::LoadedMap) -> MapSummary {
    MapSummary {
        name: map.name.clone(),
        width: map.pyramid.width,
        height: map.pyramid.height,
        pixels_per_grid: map.pixels_per_grid,
        tile_size: map.pyramid.tile_size,
        levels: map.pyramid.levels.clone(),
        total_tiles: map.pyramid.total_tiles(),
    }
}

async fn map_info(State(source): State<MapSource>, Path(name): Path<String>) -> Response {
    match source.load(&name) {
        Ok(map) => Json(summary(&map)).into_response(),
        Err(err) => (StatusCode::NOT_FOUND, err.to_string()).into_response(),
    }
}

/// Default WebP quality for map imagery.
///
/// 82 is the usual sweet spot for photographic content: visually
/// indistinguishable from lossless at normal viewing distance, and a small
/// fraction of the bytes. Map art is exactly the content type lossy encoding
/// is good at — large areas of texture and gradient, no text or sharp UI
/// edges that would show ringing.
pub const WEBP_QUALITY: f32 = 82.0;

/// Encodes to WebP.
///
/// Lossy, via libwebp. `image`'s own WebP encoder implements lossless only,
/// and lossless on photographic map art is badly counterproductive. Measured
/// against `grassy-path-ambush` (6144x3456, ~3.8MB as shipped):
///
/// | delivery              | lossless | lossy q82 |      |
/// |-----------------------|----------|-----------|------|
/// | 84 level-0 tiles      | 12.3 MB  | 2.9 MB    | 4.2x |
/// | capped 4096 rendition |  7.9 MB  | 1.45 MB   | 5.4x |
/// | capped 2048 rendition |  2.4 MB  | 0.44 MB   | 5.4x |
///
/// The conclusion that falls out of this is the important part: **a single
/// capped 4096 rendition at 1.45MB is smaller than shipping the original
/// 3.8MB image, and fits on any GPU.** It solves the texture-ceiling problem
/// while *reducing* bandwidth, with none of tiling's complexity — and tiling,
/// even lossy, still costs 2x that for the same view.
///
/// So the encoder, not the delivery strategy, was dominating the byte count.
/// No amount of tiling or capping could have fixed a 3x lossless penalty,
/// which is why this crate takes a C dependency it would otherwise avoid.
/// Tiling earns its keep only past what one capped texture can serve.
fn encode_webp(img: &DynamicImage) -> Result<Vec<u8>, String> {
    // RGB rather than RGBA: map backgrounds are opaque, and carrying a
    // pointless alpha channel through a lossy encoder costs bytes for nothing.
    let rgb = img.to_rgb8();
    let encoder = webp::Encoder::from_rgb(rgb.as_raw(), rgb.width(), rgb.height());
    Ok(encoder.encode(WEBP_QUALITY).to_vec())
}

fn webp_response(bytes: Vec<u8>) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "image/webp".to_string()),
            // Tiles are immutable for a given (map, level, col, row): the
            // corpus is static and a tile's content is a pure function of its
            // address. That makes them safe to cache hard, which is exactly
            // the property the app's service worker relies on for its own
            // asset route.
            (
                header::CACHE_CONTROL,
                "public, max-age=31536000, immutable".to_string(),
            ),
        ],
        bytes,
    )
        .into_response()
}

/// Strips an optional `.webp` off a path segment and parses the number.
///
/// Anything else — notably Bevy's `.meta` probe, which it issues before every
/// image load — returns `None` so the route 404s. Serving image bytes for a
/// meta request hands Bevy something it fails to parse as RON and fails the
/// load with it.
fn parse_indexed_segment(segment: &str) -> Option<u32> {
    let value = match segment.split_once('.') {
        None => segment,
        Some((value, "webp")) => value,
        Some(_) => return None,
    };
    value.parse().ok()
}

async fn tile(
    State(source): State<MapSource>,
    Path((name, level, col, row)): Path<(String, u32, u32, String)>,
) -> Response {
    let Ok(map) = source.load(&name) else {
        return (StatusCode::NOT_FOUND, "map not found").into_response();
    };

    let Some(row) = parse_indexed_segment(&row) else {
        return (StatusCode::NOT_FOUND, "not a tile").into_response();
    };

    let id = TileId { level, col, row };
    let Some((x, y, width, height)) = map.pyramid.tile_rect(id) else {
        return (StatusCode::NOT_FOUND, "tile out of range").into_response();
    };

    // Downscaled once per level and cached, not per tile — see
    // `LoadedMap::level_image` for the measurement that forced this.
    let Some(level_image) = map.level_image(level) else {
        return (StatusCode::NOT_FOUND, "level out of range").into_response();
    };

    let cropped = level_image.crop_imm(x, y, width, height);
    match encode_webp(&cropped) {
        Ok(bytes) => webp_response(bytes),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
    }
}

async fn full(
    State(source): State<MapSource>,
    Path(name): Path<String>,
    Query(query): Query<FullQuery>,
) -> Response {
    let Ok(map) = source.load(&name) else {
        return (StatusCode::NOT_FOUND, "map not found").into_response();
    };

    // Clamped, not merely defaulted: a caller asking for the full 6144px would
    // otherwise get something a majority of GPUs cannot upload.
    let max = query.max.unwrap_or(MAX_FULL_DIMENSION).min(MAX_FULL_DIMENSION);

    let image = if map.image.width() <= max && map.image.height() <= max {
        map.image.clone()
    } else {
        map.image
            .resize(max, max, image::imageops::FilterType::Lanczos3)
    };

    match encode_webp(&image) {
        Ok(bytes) => webp_response(bytes),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
    }
}
