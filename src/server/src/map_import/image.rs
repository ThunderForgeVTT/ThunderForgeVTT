//! Background image decode + save (T025).

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
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
    /// The **stored** image's cell size in pixels, which is the scene's
    /// `grid_size` and the scale every imported coordinate must use.
    ///
    /// Not the source file's `pixels_per_grid`. A map wider than the GPU
    /// texture cap is stored smaller than it arrived, and reporting the
    /// source's cell size for a resized image is what drew a 128px grid over
    /// a two-thirds-scale background.
    pub grid_size: i32,
    pub storage_path: String,
    pub original_format: String,
    pub width_px: i32,
    pub height_px: i32,
    pub byte_size: i64,
    /// Spec 028 FR-005: the hash of the bytes a client will actually receive.
    ///
    /// Absent here, this row reaches `world_sync_plan` with a NULL
    /// `content_hash`, which `compute_plan` turns into the placeholder
    /// `Fingerprint::of_bytes(&[])` — a fingerprint the fetched bytes can
    /// never match. The client then fetches the background on every open and
    /// refuses to store it every time, because `fetch_and_deliver` will not
    /// file bytes under a fingerprint it cannot reproduce. The effect is a
    /// background image that is permanently uncacheable, which is the largest
    /// asset in a world and the one the cache exists for.
    pub content_hash: String,
}

/// Spec 022 (FR-012): a scene's reduced-size preview/thumbnail image,
/// ready to be inserted as a `scene_preview_images` row and referenced
/// from `scenes::preview_asset_id`.
pub struct SavedScenePreview {
    pub asset_id: Uuid,
    pub byte_size: i64,
}

/// Decodes the same UVTT `image` field `save_background_image` decodes
/// (called alongside it, not instead of it) and writes a single
/// max-dimension-capped WebP preview rendition to RustFS, distinct from
/// the full-resolution background. Reuses `storage/transcode.rs`'s
/// `transcode_scene_preview` (research.md §5) — the same decode/resize/
/// encode machinery `save_background_image` already depends on.
pub async fn save_scene_preview_image(
    image_base64: &str,
) -> Result<SavedScenePreview, MapImportError> {
    let bytes = BASE64_STANDARD
        .decode(image_base64)
        .map_err(|e| MapImportError::InvalidImageBase64(e.to_string()))?;
    detect_image_extension(&bytes).ok_or(MapImportError::InvalidImageMagicBytes)?;

    let preview = crate::storage::transcode::transcode_scene_preview(&bytes)
        .map_err(|e| MapImportError::Storage(e.to_string()))?;

    let asset_id = Uuid::now_v7();
    let key = crate::scene_assets_serve::preview_key(asset_id);
    let byte_size = preview.webp_bytes.len() as i64;
    let cfg = crate::storage::rustfs::RustFsConfig::from_env();
    crate::storage::rustfs::write_object(&cfg, &key, preview.webp_bytes, "image/webp")
        .await
        .map_err(|e| MapImportError::Storage(e.to_string()))?;

    Ok(SavedScenePreview {
        asset_id,
        byte_size,
    })
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
    pixels_per_grid: f64,
) -> Result<SavedBackgroundImage, MapImportError> {
    let bytes = BASE64_STANDARD
        .decode(image_base64)
        .map_err(|e| MapImportError::InvalidImageBase64(e.to_string()))?;

    // Still sanity-checked up front (T025's original intent) before the
    // more expensive decode/transcode path runs.
    detect_image_extension(&bytes).ok_or(MapImportError::InvalidImageMagicBytes)?;

    // Grid-aware: the image and its cell size are decided together, so the
    // scene's `grid_size` describes the picture that was actually stored.
    let background = crate::storage::transcode::transcode_map_background(&bytes, pixels_per_grid)
        .map_err(|e| MapImportError::Storage(e.to_string()))?;
    let grid_size = i32::try_from(background.grid_size).unwrap_or(i32::MAX);
    let transcoded = background.image;

    let asset_id = Uuid::now_v7();
    let key = crate::storage::rustfs::object_key(owner_user_id, world_id, Some(scene_id), asset_id);
    let byte_size = transcoded.webp_bytes.len() as i64;

    // Spec 028 FR-005, the same rule and the same reasoning as
    // `upload_canvas_image_impl`: fingerprint the bytes about to be STORED,
    // not the ones that arrived. The client never receives the uploaded
    // image — it receives this WebP — so hashing the input would produce a
    // value no client could verify against what it holds. Computed here
    // while the bytes are already in memory, because hashing on read would
    // mean pulling every object back out of RustFS on every sync, which is
    // the load this feature exists to remove.
    let content_hash =
        thunderforge_cache_core::Fingerprint::of_bytes(&transcoded.webp_bytes).to_hex();

    let cfg = crate::storage::rustfs::RustFsConfig::from_env();
    crate::storage::rustfs::write_object(&cfg, &key, transcoded.webp_bytes, "image/webp")
        .await
        .map_err(|e| MapImportError::Storage(e.to_string()))?;

    Ok(SavedBackgroundImage {
        asset_id,
        grid_size,
        storage_path: key,
        original_format: transcoded.original_format,
        width_px: transcoded.width as i32,
        height_px: transcoded.height as i32,
        byte_size,
        content_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 028 FR-005 / T019, for the *other* writer of `content_hash`.
    ///
    /// `upload_canvas_image_impl` has had this covered since T019. Map import
    /// did not, and it did not set the column at all — so every scene
    /// background imported from a UVTT file reached `world_sync_plan` with a
    /// NULL hash. `compute_plan`'s "no server fingerprint yet" branch turns
    /// that into `Fingerprint::of_bytes(&[])`, a promise the real bytes can
    /// never satisfy, and the client's `fetch_and_deliver` then refuses to
    /// store what it fetched because it cannot reproduce the fingerprint.
    /// The background — the largest asset in a world, and the one the cache
    /// exists for — was therefore re-downloaded on every single open while
    /// the cache reported itself healthy.
    ///
    /// The bug was invisible from either side alone: the server was honestly
    /// reporting "I have no fingerprint for this", and the client was
    /// honestly refusing to file bytes it could not verify. Only the pair is
    /// wrong. Hence a test on the value itself rather than on either
    /// behaviour.
    #[tokio::test]
    async fn background_records_hash_of_stored_bytes_not_decoded_bytes() {
        let owner_id = Uuid::now_v7();
        let world_id = Uuid::now_v7();
        let scene_id = Uuid::now_v7();

        let source = crate::test_support::tiny_png_bytes();
        let encoded = BASE64_STANDARD.encode(&source);

        let saved = save_background_image(owner_id, world_id, scene_id, &encoded, 128.0)
            .await
            .expect("saving a valid png background should succeed");

        assert_eq!(saved.content_hash.len(), 64, "lowercase hex SHA-256");
        assert!(
            saved
                .content_hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
            "content_hash must be lowercase hex"
        );

        // The empty-input digest is what a NULL column became downstream, so
        // it is worth naming: seeing it here would mean the column is being
        // filled with the very placeholder this fix exists to remove.
        assert_ne!(
            saved.content_hash,
            thunderforge_cache_core::Fingerprint::of_bytes(&[]).to_hex(),
            "an empty-bytes fingerprint is the unset placeholder, not a hash"
        );

        // Not the decoded upload's hash — the WebP transcode makes these
        // differ, and hashing the input is the mistake this test forbids.
        assert_ne!(
            saved.content_hash,
            thunderforge_cache_core::Fingerprint::of_bytes(&source).to_hex(),
            "hashing the decoded upload would produce a value no client could verify"
        );

        // The stored object's hash: what a client computes over what it
        // actually receives, and therefore the only value that can ever hit.
        let stored = crate::storage::rustfs::read_object(
            &crate::storage::rustfs::RustFsConfig::from_env(),
            &saved.storage_path,
        )
        .await
        .expect("stored object should be readable");
        assert_eq!(
            saved.content_hash,
            thunderforge_cache_core::Fingerprint::of_bytes(&stored).to_hex(),
            "content_hash must match the bytes the client will actually receive"
        );
        assert_eq!(
            saved.byte_size,
            stored.len() as i64,
            "byte_size and content_hash must describe the same bytes"
        );
    }
}
