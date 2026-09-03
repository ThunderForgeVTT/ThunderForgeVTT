//! Spec 002 (FR-012, FR-013): decode an arbitrary supported image format
//! and re-encode it to WebP server-side, before any RustFS write or DB
//! row is created — the only way to guarantee FR-013's "no
//! partial/corrupt asset persisted" for an oversized/malformed upload.
//!
//! Spec 012 (research.md §5): extends the above with a second output —
//! a normalized full-size rendition (capped at a max dimension, never
//! upscaled) and a fixed-size thumbnail, both WebP — for lore image
//! uploads (FR-009). Uses the `image` crate's own resize, already a
//! dependency for the transcode path above; no new dependency needed.

use image::DynamicImage;

/// Single source of truth for the upload size ceiling, reused (not
/// duplicated) from the original `map_import.rs` constant so map-import
/// and paste-to-canvas enforce the identical limit (FR-013, FR-018).
pub const MAX_UPLOAD_BYTES: usize = 50 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum TranscodeError {
    #[error("upload exceeds maximum size of {max} bytes (got {actual})")]
    TooLarge { max: usize, actual: usize },
    #[error("failed to decode image: {0}")]
    Decode(String),
    #[error("failed to encode image as WebP: {0}")]
    Encode(String),
}

#[derive(Debug)]
pub struct TranscodedImage {
    pub webp_bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Source format as uploaded (e.g. `"png"`, `"jpeg"`), for
    /// diagnostics only (data-model.md's `original_format` column).
    pub original_format: String,
}

/// Decodes `bytes` (any of the formats enabled on the `image` crate:
/// PNG, JPEG, WebP, GIF, BMP) and re-encodes as lossless WebP. Enforces
/// `MAX_UPLOAD_BYTES` before doing any decode work.
pub fn transcode_to_webp(bytes: &[u8]) -> Result<TranscodedImage, TranscodeError> {
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(TranscodeError::TooLarge {
            max: MAX_UPLOAD_BYTES,
            actual: bytes.len(),
        });
    }

    let format = image::guess_format(bytes).map_err(|e| TranscodeError::Decode(e.to_string()))?;
    let img = image::load_from_memory_with_format(bytes, format)
        .map_err(|e| TranscodeError::Decode(e.to_string()))?;

    // Capped so the result can actually be uploaded as a GPU texture on the
    // machines players use — see `MAX_CANVAS_TEXTURE_DIMENSION`.
    let img = resize_to_max_dimension(&img, MAX_CANVAS_TEXTURE_DIMENSION);

    let width = img.width();
    let height = img.height();

    let webp_bytes = encode_webp(&img)?;

    Ok(TranscodedImage {
        webp_bytes,
        width,
        height,
        original_format: format!("{format:?}").to_lowercase(),
    })
}

/// A map background, resized so a whole number of grid cells fits it.
///
/// # The bug this exists to prevent
///
/// `transcode_to_webp` caps every image at `MAX_CANVAS_TEXTURE_DIMENSION`,
/// because the result has to survive being uploaded as a GPU texture. The map
/// importer then stored the *source* file's `pixels_per_grid` as the scene's
/// `grid_size` — a number describing an image that no longer existed. A
/// 6144x3456 map became a 4096x2304 background under a 128px grid: thirty-two
/// cells drawn across a map with forty-eight, every square exactly 1.5 times
/// too large, and every wall, portal and light misplaced by the same factor.
///
/// Five of the eight bundled fixtures were affected. The two that were not are
/// the two small enough to skip the resize entirely, which is why nothing
/// caught it.
///
/// # Why the image is resized to fit the grid, rather than the grid to fit the
/// image
///
/// Because the honest scale factor is `4096/6144`, which turns a 128px cell
/// into 85.33px, and `scenes.grid_size` is an `i32`. Rounding it would put the
/// grid back out of step with the image — smaller error, same class of bug.
///
/// So the *cell* is chosen first, as the largest whole pixel size that keeps
/// the image within the cap, and the image is resized to an exact multiple of
/// it. 4080x2295 rather than 4096x2304: sixteen pixels narrower, and a grid
/// that lands exactly where the map says it should.
pub struct MapBackground {
    pub image: TranscodedImage,
    /// The stored image's cell size, in its own pixels. This is the scene's
    /// `grid_size`, and the scale every imported coordinate must use.
    pub grid_size: u32,
}

/// Transcode a map background, keeping its grid and its pixels in step.
///
/// `pixels_per_grid` is the source file's own cell size. The returned
/// `grid_size` describes the image that was actually stored, and the two are
/// only equal when the image was small enough to need no resizing at all.
pub fn transcode_map_background(
    bytes: &[u8],
    pixels_per_grid: f64,
) -> Result<MapBackground, TranscodeError> {
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(TranscodeError::TooLarge {
            max: MAX_UPLOAD_BYTES,
            actual: bytes.len(),
        });
    }

    let format = image::guess_format(bytes).map_err(|e| TranscodeError::Decode(e.to_string()))?;
    let img = image::load_from_memory_with_format(bytes, format)
        .map_err(|e| TranscodeError::Decode(e.to_string()))?;

    let (source_width, source_height) = (img.width(), img.height());
    let cell = stored_cell_size(source_width, source_height, pixels_per_grid);

    let img = match cell {
        // Either no resize was needed, or the cell is unusable — see
        // `stored_cell_size`. Both fall back to the plain cap.
        None => resize_to_max_dimension(&img, MAX_CANVAS_TEXTURE_DIMENSION),
        Some(cell) => {
            // An exact multiple of the source, so the factor `cell /
            // pixels_per_grid` describes the stored image precisely rather
            // than approximately.
            let scale = f64::from(cell) / pixels_per_grid;
            let width = (f64::from(source_width) * scale).round().max(1.0) as u32;
            let height = (f64::from(source_height) * scale).round().max(1.0) as u32;
            img.resize_exact(width, height, image::imageops::FilterType::Lanczos3)
        }
    };

    let width = img.width();
    let height = img.height();
    let webp_bytes = encode_webp(&img)?;

    Ok(MapBackground {
        image: TranscodedImage {
            webp_bytes,
            width,
            height,
            original_format: format!("{format:?}").to_lowercase(),
        },
        grid_size: cell.unwrap_or_else(|| pixels_per_grid.round().max(1.0) as u32),
    })
}

/// The largest whole-pixel cell that keeps the image inside the texture cap.
///
/// `None` means leave it alone: either the image already fits — in which case
/// the source's own `pixels_per_grid` is exactly right and rescaling it would
/// introduce the error this function exists to avoid — or the numbers are
/// degenerate enough that no whole cell survives the cap, which would be a map
/// more than four thousand cells across.
fn stored_cell_size(width: u32, height: u32, pixels_per_grid: f64) -> Option<u32> {
    if width <= MAX_CANVAS_TEXTURE_DIMENSION && height <= MAX_CANVAS_TEXTURE_DIMENSION {
        return None;
    }
    if !pixels_per_grid.is_finite() || pixels_per_grid < 1.0 {
        return None;
    }

    let longest = f64::from(width.max(height));
    let scale = f64::from(MAX_CANVAS_TEXTURE_DIMENSION) / longest;

    // Floored, never rounded: a cell one pixel too large puts the image back
    // over the cap, which is the one thing the resize is for.
    let cell = (pixels_per_grid * scale).floor();
    if cell < 1.0 { None } else { Some(cell as u32) }
}

/// Spec 012 (FR-010): the lore-image-specific upload cap — distinct from
/// (and smaller than) `MAX_UPLOAD_BYTES` above, which stays 50 MB for
/// existing canvas-image uploads; lore images use the 25 MB fixed
/// default from spec.md's Clarifications.
pub const MAX_LORE_IMAGE_UPLOAD_BYTES: usize = 25 * 1024 * 1024;

/// research.md §5's chosen bounds: a normalized full-size rendition
/// capped at 2048px on its longest edge, and a 256px thumbnail. Neither
/// upscales an already-smaller image.
pub const LORE_IMAGE_MAX_DIMENSION: u32 = 2048;
pub const LORE_IMAGE_THUMBNAIL_DIMENSION: u32 = 256;

// Dimension/format fields are captured at transcode time but not currently
// persisted on `world_lore_image_assets` (which stores byte_size/content_type
// only) — kept on the struct since they're natural metadata a future pass
// may want to store, not dead in the sense of being wrong to have computed.
#[allow(dead_code)]
#[derive(Debug)]
pub struct LoreImageRenditions {
    pub full_webp_bytes: Vec<u8>,
    pub full_width: u32,
    pub full_height: u32,
    pub thumbnail_webp_bytes: Vec<u8>,
    pub thumbnail_width: u32,
    pub thumbnail_height: u32,
    pub original_format: String,
}

/// WebP quality for photographic canvas art (backgrounds, pasted images).
///
/// 82 is the usual sweet spot for photographic content: visually
/// indistinguishable at viewing distance, a fraction of the bytes. Map art is
/// what lossy encoding is good at — texture and gradient, no text or sharp UI
/// edges to ring.
pub const CANVAS_WEBP_QUALITY: f32 = 82.0;

/// WebP quality for lore imagery.
///
/// Higher than canvas art on purpose. A lore image is whatever a user
/// uploaded — a handout, a diagram, a screenshot with text — and lossy
/// artefacts around text are far more visible than in a painted map. 92 keeps
/// most of the saving while staying clear of that.
pub const LORE_WEBP_QUALITY: f32 = 92.0;

/// Encodes to WebP.
///
/// **Lossy**, via libwebp. This replaced `image`'s lossless-only WebP encoder
/// after measuring what lossless was costing on real map art. Against
/// `grassy-path-ambush` (6144x3456, 3.83MB as shipped), through the
/// `thunderforge_mapforge` harness:
///
/// | rendition             | lossless | lossy q82 |
/// |-----------------------|----------|-----------|
/// | capped 4096           |  7.90 MB |   1.45 MB |
/// | capped 2048           |  2.41 MB |   0.44 MB |
///
/// Lossless was producing renditions *larger than the source image* — a
/// capped-and-transcoded 4096 background cost 7.9MB where the original was
/// 3.8MB. Lossy at 4096 is 1.45MB: smaller than the original, and within
/// every GPU's texture limit.
///
/// Alpha is preserved only when the source actually has it. Encoding an
/// opaque image as RGBA spends bytes carrying a channel that is uniformly
/// 255 — which is most map art.
fn encode_webp_at(img: &DynamicImage, quality: f32) -> Result<Vec<u8>, TranscodeError> {
    let encoded = if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height()).encode(quality)
    } else {
        let rgb = img.to_rgb8();
        webp::Encoder::from_rgb(rgb.as_raw(), rgb.width(), rgb.height()).encode(quality)
    };

    if encoded.is_empty() {
        // libwebp signals failure by producing nothing.
        return Err(TranscodeError::Encode(
            "webp encoder produced no output".into(),
        ));
    }
    Ok(encoded.to_vec())
}

/// Canvas-quality WebP. See `encode_webp_at`.
fn encode_webp(img: &DynamicImage) -> Result<Vec<u8>, TranscodeError> {
    encode_webp_at(img, CANVAS_WEBP_QUALITY)
}

/// Scales `img` down to fit within `max_dimension` on its longest edge,
/// preserving aspect ratio; returns `img` unchanged if it already fits
/// (never upscales, per research.md §5).
fn resize_to_max_dimension(img: &DynamicImage, max_dimension: u32) -> DynamicImage {
    if img.width() <= max_dimension && img.height() <= max_dimension {
        img.clone()
    } else {
        img.resize(
            max_dimension,
            max_dimension,
            image::imageops::FilterType::Lanczos3,
        )
    }
}

/// Spec 022 (research.md §5): scene preview/thumbnail images use the same
/// max-dimension-capped WebP approach as lore thumbnails, reusing this
/// module's existing decode/resize/encode helpers rather than
/// reimplementing them. "Roughly 1/16 scale" (spec.md FR-012) is
/// interpreted as this capped-max-dimension approach, consistent with how
/// `LORE_IMAGE_THUMBNAIL_DIMENSION` above already handles the same
/// "small preview regardless of wildly varying source size" problem.
pub const SCENE_PREVIEW_MAX_DIMENSION: u32 = 256;

/// Largest dimension, in pixels, of any image stored for display on the
/// canvas.
///
/// This is a hardware ceiling, not a preference. A canvas image becomes a GPU
/// texture, and WebGL2 only *guarantees* `MAX_TEXTURE_SIZE` of 2048 — 4096 is
/// the common real value on integrated and mobile GPUs, while a discrete card
/// reports 16384 or more. An image wider than the device's limit fails to
/// upload and the map silently does not render.
///
/// That failure mode is dangerous precisely because it is invisible during
/// development: measured on this project's own dev machine (RTX 3090) the
/// limit is 32768, so every one of the example maps works there — while five
/// of the seven exceed 4096 and would have failed on a player's laptop.
///
/// 4096 is the compromise: it clears the common ceiling, and the detail lost
/// only shows at maximum zoom-in. It also bounds VRAM at ~67MB per background
/// rather than the 81MB a 6144x3456 map costs. Serving larger art means
/// tiling, not raising this number.
pub const MAX_CANVAS_TEXTURE_DIMENSION: u32 = 4096;

/// Decodes an already-known-good image (`bytes`, e.g. a scene's own
/// background image already accepted by `transcode_to_webp` or the dd2vtt
/// importer) and produces a single max-dimension-capped WebP preview
/// rendition. Does not re-enforce an upload size ceiling — callers pass
/// bytes that already passed a ceiling check earlier in their own
/// pipeline (map import / canvas upload), so this never gates on size
/// again.
pub fn transcode_scene_preview(bytes: &[u8]) -> Result<TranscodedImage, TranscodeError> {
    let format = image::guess_format(bytes).map_err(|e| TranscodeError::Decode(e.to_string()))?;
    let img = image::load_from_memory_with_format(bytes, format)
        .map_err(|e| TranscodeError::Decode(e.to_string()))?;

    let preview = resize_to_max_dimension(&img, SCENE_PREVIEW_MAX_DIMENSION);
    let webp_bytes = encode_webp(&preview)?;

    Ok(TranscodedImage {
        webp_bytes,
        width: preview.width(),
        height: preview.height(),
        original_format: format!("{format:?}").to_lowercase(),
    })
}

/// Decodes `bytes` (enforcing `MAX_LORE_IMAGE_UPLOAD_BYTES` before any
/// decode work, per FR-010) and produces both a normalized full-size
/// WebP rendition and a WebP thumbnail (FR-009).
pub fn transcode_to_lore_renditions(bytes: &[u8]) -> Result<LoreImageRenditions, TranscodeError> {
    if bytes.len() > MAX_LORE_IMAGE_UPLOAD_BYTES {
        return Err(TranscodeError::TooLarge {
            max: MAX_LORE_IMAGE_UPLOAD_BYTES,
            actual: bytes.len(),
        });
    }

    let format = image::guess_format(bytes).map_err(|e| TranscodeError::Decode(e.to_string()))?;
    let img = image::load_from_memory_with_format(bytes, format)
        .map_err(|e| TranscodeError::Decode(e.to_string()))?;

    let full = resize_to_max_dimension(&img, LORE_IMAGE_MAX_DIMENSION);
    let full_webp_bytes = encode_webp_at(&full, LORE_WEBP_QUALITY)?;

    let thumbnail = resize_to_max_dimension(&img, LORE_IMAGE_THUMBNAIL_DIMENSION);
    let thumbnail_webp_bytes = encode_webp_at(&thumbnail, LORE_WEBP_QUALITY)?;

    Ok(LoreImageRenditions {
        full_width: full.width(),
        full_height: full.height(),
        full_webp_bytes,
        thumbnail_width: thumbnail.width(),
        thumbnail_height: thumbnail.height(),
        thumbnail_webp_bytes,
        original_format: format!("{format:?}").to_lowercase(),
    })
}

#[cfg(test)]
mod texture_cap_tests {
    use super::*;
    // Only the tests build source images now that encoding goes through
    // libwebp rather than `image`'s encoder traits.
    use image::{ExtendedColorType, ImageEncoder};

    /// Builds a solid-colour PNG of the given size.
    fn png_of(width: u32, height: u32) -> Vec<u8> {
        let img = DynamicImage::new_rgba8(width, height);
        let mut bytes: Vec<u8> = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(img.as_bytes(), width, height, ExtendedColorType::Rgba8)
            .expect("encoding a blank png should succeed");
        bytes
    }

    /// The bug the cap above quietly caused.
    ///
    /// Capping the image was right; keeping the source file's cell size beside
    /// the capped image was not. A 6144x3456 map stored at 4096x2304 under a
    /// 128px grid draws thirty-two cells across a map with forty-eight — every
    /// square 1.5x too large, and every wall, portal and light out by the same
    /// factor. The test directly above proved the resize happened and never
    /// asked what it invalidated.
    #[test]
    fn a_capped_map_background_keeps_its_grid_in_step_with_its_pixels() {
        // grassy-path-ambush, little-fish-academy and road-side-in's shape:
        // 48 x 27 cells of 128px.
        let source = png_of(6144, 3456);
        let background = transcode_map_background(&source, 128.0).expect("should transcode");

        assert!(
            background.image.width <= MAX_CANVAS_TEXTURE_DIMENSION
                && background.image.height <= MAX_CANVAS_TEXTURE_DIMENSION,
            "the cap still holds: stored {}x{}",
            background.image.width,
            background.image.height
        );

        // The property: the stored image is still exactly the map's size in
        // cells. 48 x 85 = 4080, 27 x 85 = 2295.
        assert_eq!(background.grid_size, 85);
        assert_eq!(background.image.width, 4080);
        assert_eq!(background.image.height, 2295);
        assert_eq!(background.image.width / background.grid_size, 48);
        assert_eq!(background.image.height / background.grid_size, 27);
    }

    /// A map that needs no resizing must not be rescaled at all.
    ///
    /// Its `pixels_per_grid` already describes the stored image exactly, and
    /// recomputing a cell size for it could only introduce the error this
    /// whole path exists to avoid.
    #[test]
    fn a_background_under_the_cap_keeps_the_files_own_grid_untouched() {
        // azheim-meeting's shape: 8 x 8 cells of 256px.
        let source = png_of(2048, 2048);
        let background = transcode_map_background(&source, 256.0).expect("should transcode");

        assert_eq!(background.grid_size, 256);
        assert_eq!(background.image.width, 2048);
        assert_eq!(background.image.height, 2048);
    }

    /// Every bundled fixture's shape, as one property.
    ///
    /// Stated as a ratio rather than as expected numbers, because the numbers
    /// are what was wrong: an assertion naming 128 would have passed against
    /// the bug. What must hold is that the stored image is the same number of
    /// cells across as the source was.
    #[test]
    fn a_stored_background_is_the_same_number_of_cells_across_as_its_source() {
        // (source width, source height, pixels_per_grid) for the real fixtures
        // plus the user-reported Simple Beach, which shares the 6144x3456 shape.
        for (width, height, ppg) in [
            (6144u32, 3456u32, 128.0f64), // three fixtures, and Simple Beach
            (4480, 2560, 128.0),          // demo
            (3072, 4608, 128.0),          // dwarven-forge
            (2048, 2048, 256.0),          // azheim-meeting, under the cap
            (1280, 1280, 128.0),          // chamber-of-echoing-grief, under the cap
        ] {
            let background = transcode_map_background(&png_of(width, height), ppg)
                .unwrap_or_else(|e| panic!("{width}x{height} should transcode: {e:?}"));

            let source_cells_x = f64::from(width) / ppg;
            let stored_cells_x =
                f64::from(background.image.width) / f64::from(background.grid_size);

            assert!(
                (source_cells_x - stored_cells_x).abs() < 0.5,
                "{width}x{height} @ {ppg}ppg: source is {source_cells_x:.2} cells wide, \
                 stored is {stored_cells_x:.2} ({}px / {}px)",
                background.image.width,
                background.grid_size
            );
            assert!(
                background.image.width <= MAX_CANVAS_TEXTURE_DIMENSION
                    && background.image.height <= MAX_CANVAS_TEXTURE_DIMENSION,
                "{width}x{height} exceeded the cap after resizing"
            );
        }
    }

    /// A file claiming a nonsensical cell size must not panic or divide by it.
    #[test]
    fn a_degenerate_pixels_per_grid_falls_back_rather_than_failing() {
        let source = png_of(6144, 3456);
        for ppg in [0.0, -12.0, f64::NAN, f64::INFINITY] {
            let background = transcode_map_background(&source, ppg)
                .unwrap_or_else(|e| panic!("{ppg} should still transcode: {e:?}"));
            assert!(background.grid_size >= 1, "grid_size must stay usable");
            assert!(background.image.width <= MAX_CANVAS_TEXTURE_DIMENSION);
        }
    }

    /// The regression this guards: five of this project's seven example maps
    /// exceed 4096px, which is a common WebGL2 `MAX_TEXTURE_SIZE`. Stored at
    /// full size they upload fine on a discrete GPU and fail silently on an
    /// integrated one.
    #[test]
    fn an_oversized_background_is_capped_to_a_uploadable_texture() {
        // The shape of grassy-path-ambush and two other example maps.
        let source = png_of(6144, 3456);
        let transcoded = transcode_to_webp(&source).expect("should transcode");

        assert!(
            transcoded.width <= MAX_CANVAS_TEXTURE_DIMENSION
                && transcoded.height <= MAX_CANVAS_TEXTURE_DIMENSION,
            "stored {}x{} exceeds the {MAX_CANVAS_TEXTURE_DIMENSION}px ceiling",
            transcoded.width,
            transcoded.height,
        );

        // Aspect ratio must survive, or the grid stops matching the art.
        let source_aspect = 6144.0 / 3456.0;
        let stored_aspect = transcoded.width as f64 / transcoded.height as f64;
        assert!(
            (source_aspect - stored_aspect).abs() < 0.01,
            "aspect changed: {source_aspect} -> {stored_aspect}",
        );
    }

    /// Reports what the real import path produces for every example map.
    ///
    /// Not an assertion of specific sizes — those depend on the encoder and
    /// would be brittle. It exists to make the cost of a delivery change
    /// visible: run it before and after touching the encoder or the ceiling
    /// and the table moves.
    #[test]
    #[ignore = "reporting benchmark; run with --ignored --nocapture"]
    // The table *is* the output. `print_stdout` is warned on workspace-wide
    // because a server should log through `tracing`; a reporting benchmark
    // read by a person at a terminal is the case that rule is not about.
    #[allow(clippy::print_stdout)]
    fn report_import_rendition_sizes() {
        use base64::Engine as _;

        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/maps");
        let mut entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("examples/maps should exist")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("dd2vtt"))
            .collect();
        entries.sort();

        println!();
        println!(
            "{:<46}{:>12}{:>12}{:>10}{:>14}",
            "map", "source", "stored", "ratio", "stored px"
        );
        println!("{}", "-".repeat(94));

        let (mut total_source, mut total_stored) = (0usize, 0usize);

        for path in entries {
            let name = path.file_stem().unwrap().to_string_lossy().to_string();
            let raw = std::fs::read_to_string(&path).expect("readable fixture");
            let parsed: serde_json::Value = serde_json::from_str(&raw).expect("valid json");
            let encoded = parsed["image"].as_str().expect("image field");
            let source = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .expect("valid base64");

            let transcoded = transcode_to_webp(&source).expect("should transcode");

            total_source += source.len();
            total_stored += transcoded.webp_bytes.len();

            println!(
                "{:<46}{:>9.2} MB{:>9.2} MB{:>9.2}x{:>14}",
                name,
                source.len() as f64 / 1024.0 / 1024.0,
                transcoded.webp_bytes.len() as f64 / 1024.0 / 1024.0,
                transcoded.webp_bytes.len() as f64 / source.len() as f64,
                format!("{}x{}", transcoded.width, transcoded.height),
            );
        }

        println!("{}", "-".repeat(94));
        println!(
            "{:<46}{:>9.2} MB{:>9.2} MB{:>9.2}x",
            "TOTAL",
            total_source as f64 / 1024.0 / 1024.0,
            total_stored as f64 / 1024.0 / 1024.0,
            total_stored as f64 / total_source as f64,
        );
    }

    #[test]
    fn an_image_already_within_the_ceiling_is_untouched() {
        // Resizing what already fits would throw away detail for nothing.
        let transcoded = transcode_to_webp(&png_of(1280, 1280)).expect("should transcode");
        assert_eq!((transcoded.width, transcoded.height), (1280, 1280));
    }

    #[test]
    fn a_very_tall_narrow_image_is_capped_on_its_long_axis() {
        let transcoded = transcode_to_webp(&png_of(512, 8192)).expect("should transcode");
        assert_eq!(transcoded.height, MAX_CANVAS_TEXTURE_DIMENSION);
        assert!(transcoded.width <= MAX_CANVAS_TEXTURE_DIMENSION);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_png() -> Vec<u8> {
        // 1x1 red pixel PNG.
        let img = image::RgbImage::from_pixel(1, 1, image::Rgb([255, 0, 0]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }

    #[test]
    fn transcodes_png_to_webp() {
        let png = tiny_png();
        let result = transcode_to_webp(&png).expect("transcode should succeed");
        assert_eq!(result.width, 1);
        assert_eq!(result.height, 1);
        assert_eq!(result.original_format, "png");
        // WebP files start with "RIFF"...."WEBP".
        assert_eq!(&result.webp_bytes[0..4], b"RIFF");
        assert_eq!(&result.webp_bytes[8..12], b"WEBP");
    }

    #[test]
    fn rejects_oversized_upload_before_decoding() {
        let oversized = vec![0u8; MAX_UPLOAD_BYTES + 1];
        let err = transcode_to_webp(&oversized).unwrap_err();
        assert!(matches!(err, TranscodeError::TooLarge { .. }));
    }

    fn checkerboard_png(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(width, height, |x, y| {
            if (x + y) % 2 == 0 {
                image::Rgb([255, 0, 0])
            } else {
                image::Rgb([0, 0, 255])
            }
        });
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }

    /// FR-009: a small image is not upscaled — both renditions keep its
    /// original dimensions.
    #[test]
    fn lore_renditions_do_not_upscale_a_small_image() {
        let png = checkerboard_png(10, 10);
        let renditions = transcode_to_lore_renditions(&png).expect("transcode should succeed");
        assert_eq!(renditions.full_width, 10);
        assert_eq!(renditions.full_height, 10);
        assert_eq!(renditions.thumbnail_width, 10);
        assert_eq!(renditions.thumbnail_height, 10);
        assert_eq!(&renditions.full_webp_bytes[0..4], b"RIFF");
        assert_eq!(&renditions.thumbnail_webp_bytes[0..4], b"RIFF");
    }

    /// FR-009: an oversized image is downscaled to fit within each
    /// rendition's max dimension, preserving aspect ratio.
    #[test]
    fn lore_renditions_downscale_a_large_image_to_max_dimensions() {
        let png = checkerboard_png(4000, 1000);
        let renditions = transcode_to_lore_renditions(&png).expect("transcode should succeed");
        // 4000x1000 is aspect ratio 4:1; fitting within a 2048/256 box
        // preserving aspect ratio scales height to max_dimension / 4.
        assert_eq!(renditions.full_width, LORE_IMAGE_MAX_DIMENSION);
        assert_eq!(renditions.full_height, LORE_IMAGE_MAX_DIMENSION / 4);
        assert_eq!(renditions.thumbnail_width, LORE_IMAGE_THUMBNAIL_DIMENSION);
        assert_eq!(
            renditions.thumbnail_height,
            LORE_IMAGE_THUMBNAIL_DIMENSION / 4
        );
    }

    /// FR-010: oversized uploads are rejected before any decode work.
    #[test]
    fn lore_renditions_reject_oversized_upload_before_decoding() {
        let oversized = vec![0u8; MAX_LORE_IMAGE_UPLOAD_BYTES + 1];
        let err = transcode_to_lore_renditions(&oversized).unwrap_err();
        assert!(matches!(err, TranscodeError::TooLarge { .. }));
    }
}
