//! Background image decode + save (T025).

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use uuid::Uuid;

use super::types::MapImportError;

/// PNG file signature, used to sanity-check the decoded `image` field
/// without pulling in a full image-decoding crate (T025).
const PNG_MAGIC: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
/// WebP's container is a RIFF chunk (`RIFF????WEBP`) — bytes 0-3 are
/// "RIFF", bytes 4-7 are a little-endian file-size field (varies per
/// file, not checked), bytes 8-11 are "WEBP". DungeonDraft's own UVTT
/// exporter uses WebP for the background image (verified against
/// examples/maps/demo.dd2vtt, ~4.2MB, vs. chamber-of-echoing-grief.dd2vtt's
/// genuine PNG) — both are valid per the format, so both must be accepted.
const RIFF_MAGIC: [u8; 4] = [0x52, 0x49, 0x46, 0x46];
const WEBP_MAGIC: [u8; 4] = [0x57, 0x45, 0x42, 0x50];

/// Returns the file extension to save with (`"png"`/`"webp"`) if `bytes`
/// starts with a recognized image magic-byte signature, or `None` if it
/// looks like neither. Extension matters here beyond cosmetics: Bevy's
/// `AssetServer` (the engine-side background renderer) picks its decoder
/// by file extension, so saving WebP bytes under a `.png` name would
/// fail to load correctly downstream.
pub(super) fn detect_image_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= PNG_MAGIC.len() && bytes[..PNG_MAGIC.len()] == PNG_MAGIC {
        return Some("png");
    }
    if bytes.len() >= 12 && bytes[0..4] == RIFF_MAGIC && bytes[8..12] == WEBP_MAGIC {
        return Some("webp");
    }
    None
}

/// One saved background image, ready to be inserted as a
/// `canvas_image_assets` row (`kind = Background`) and referenced from
/// `scenes::background_asset_id` (FR-018 migration).
pub struct SavedBackgroundImage {
    pub asset_id: Uuid,
    pub storage_path: String,
    pub original_format: String,
    pub width_px: i32,
    pub height_px: i32,
    pub byte_size: i64,
}

/// Decode the UVTT file's base64 `image` field, sanity-check it looks
/// like a PNG or WebP file (both are valid per the format), transcode it
/// to WebP, and write it to RustFS via a single-object-scoped,
/// server-held credential (spec 002, FR-018 — the same
/// `storage/transcode.rs` + `storage/rustfs.rs` path `uploadCanvasImage`
/// uses, so map-import and paste-to-canvas share one storage mechanism,
/// not two). Superseded the earlier local-filesystem write this
/// function did in spec 001.
pub async fn save_background_image(
    owner_user_id: Uuid,
    world_id: Uuid,
    scene_id: Uuid,
    image_base64: &str,
) -> Result<SavedBackgroundImage, MapImportError> {
    let bytes = BASE64_STANDARD
        .decode(image_base64)
        .map_err(|e| MapImportError::InvalidImageBase64(e.to_string()))?;

    // Still sanity-checked up front (T025's original intent) before the
    // more expensive decode/transcode path runs.
    detect_image_extension(&bytes).ok_or(MapImportError::InvalidImageMagicBytes)?;

    let transcoded = crate::storage::transcode::transcode_to_webp(&bytes)
        .map_err(|e| MapImportError::Storage(e.to_string()))?;

    let asset_id = Uuid::now_v7();
    let key = crate::storage::rustfs::object_key(owner_user_id, world_id, Some(scene_id), asset_id);
    let byte_size = transcoded.webp_bytes.len() as i64;
    let cfg = crate::storage::rustfs::RustFsConfig::from_env();
    crate::storage::rustfs::write_object(&cfg, &key, transcoded.webp_bytes, "image/webp")
        .await
        .map_err(|e| MapImportError::Storage(e.to_string()))?;

    Ok(SavedBackgroundImage {
        asset_id,
        storage_path: key,
        original_format: transcoded.original_format,
        width_px: transcoded.width as i32,
        height_px: transcoded.height as i32,
        byte_size,
    })
}
