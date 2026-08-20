//! Spec 002 (FR-012, FR-013): decode an arbitrary supported image format
//! and re-encode it to WebP server-side, before any RustFS write or DB
//! row is created — the only way to guarantee FR-013's "no
//! partial/corrupt asset persisted" for an oversized/malformed upload.

use image::{ExtendedColorType, ImageEncoder};

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
}
