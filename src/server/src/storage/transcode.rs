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

use image::{DynamicImage, ExtendedColorType, ImageEncoder};

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
    let img =
        image::load_from_memory_with_format(bytes, format).map_err(|e| TranscodeError::Decode(e.to_string()))?;

    let width = img.width();
    let height = img.height();
    let color_type: ExtendedColorType = img.color().into();

    let mut webp_bytes: Vec<u8> = Vec::new();
    image::codecs::webp::WebPEncoder::new_lossless(&mut webp_bytes)
        .write_image(img.as_bytes(), width, height, color_type)
        .map_err(|e| TranscodeError::Encode(e.to_string()))?;

    Ok(TranscodedImage {
        webp_bytes,
        width,
        height,
        original_format: format!("{format:?}").to_lowercase(),
    })
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

fn encode_webp(img: &DynamicImage) -> Result<Vec<u8>, TranscodeError> {
    let color_type: ExtendedColorType = img.color().into();
    let mut bytes = Vec::new();
    image::codecs::webp::WebPEncoder::new_lossless(&mut bytes)
        .write_image(img.as_bytes(), img.width(), img.height(), color_type)
        .map_err(|e| TranscodeError::Encode(e.to_string()))?;
    Ok(bytes)
}

/// Scales `img` down to fit within `max_dimension` on its longest edge,
/// preserving aspect ratio; returns `img` unchanged if it already fits
/// (never upscales, per research.md §5).
fn resize_to_max_dimension(img: &DynamicImage, max_dimension: u32) -> DynamicImage {
    if img.width() <= max_dimension && img.height() <= max_dimension {
        img.clone()
    } else {
        img.resize(max_dimension, max_dimension, image::imageops::FilterType::Lanczos3)
    }
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
    let img =
        image::load_from_memory_with_format(bytes, format).map_err(|e| TranscodeError::Decode(e.to_string()))?;

    let full = resize_to_max_dimension(&img, LORE_IMAGE_MAX_DIMENSION);
    let full_webp_bytes = encode_webp(&full)?;

    let thumbnail = resize_to_max_dimension(&img, LORE_IMAGE_THUMBNAIL_DIMENSION);
    let thumbnail_webp_bytes = encode_webp(&thumbnail)?;

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
mod tests {
    use super::*;

    fn tiny_png() -> Vec<u8> {
        // 1x1 red pixel PNG.
        let img = image::RgbImage::from_pixel(1, 1, image::Rgb([255, 0, 0]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
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
            if (x + y) % 2 == 0 { image::Rgb([255, 0, 0]) } else { image::Rgb([0, 0, 255]) }
        });
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
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
        assert_eq!(renditions.thumbnail_height, LORE_IMAGE_THUMBNAIL_DIMENSION / 4);
    }

    /// FR-010: oversized uploads are rejected before any decode work.
    #[test]
    fn lore_renditions_reject_oversized_upload_before_decoding() {
        let oversized = vec![0u8; MAX_LORE_IMAGE_UPLOAD_BYTES + 1];
        let err = transcode_to_lore_renditions(&oversized).unwrap_err();
        assert!(matches!(err, TranscodeError::TooLarge { .. }));
    }
}
