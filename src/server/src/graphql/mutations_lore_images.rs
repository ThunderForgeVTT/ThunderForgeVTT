//! Spec 012: `uploadLoreImage` — paste/drop image upload for lore
//! entries (FR-008/009/010). See
//! `specs/012-lore-wiki/contracts/lore-images.md`. Mirrors
//! `mutations_assets.rs`'s `uploadCanvasImage` shape/ordering (authorize
//! → transcode → write → persist), reusing the existing RustFS
//! `write_object` path unchanged (ADR-039) and the extended
//! `transcode_to_lore_renditions` (spec 012, research.md §5).

use async_graphql::{Context, Error, ErrorExtensions, Result as GraphQLResult, Upload};
use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::assets_serve::lore::{full_key, thumb_key};
use crate::auth::lore_permissions::require_lore_permission;
use crate::graphql::types::{ActorPermissionLevel, GraphQLLoreImageAsset};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{LoreImageAsset, NewLoreImageAsset};
use crate::schema::world_lore_image_assets;
use crate::state::AppState;
use crate::storage::rustfs::{RustFsConfig, write_object};
use crate::storage::transcode::{TranscodeError, transcode_to_lore_renditions};

#[derive(Debug, thiserror::Error)]
pub enum UploadLoreImageError {
    #[error("insufficient permission to upload images to this lore entry")]
    Forbidden,
    #[error("upload exceeds maximum size of {max} bytes (got {actual})")]
    TooLarge { max: usize, actual: usize },
    #[error("failed to decode/transcode image: {0}")]
    Transcode(String),
    #[error("failed to write object to storage: {0}")]
    Storage(String),
    #[error("database error: {0}")]
    Database(String),
}

fn to_graphql_error(e: UploadLoreImageError) -> Error {
    let msg = e.to_string();
    if matches!(e, UploadLoreImageError::Forbidden) {
        Error::new(msg).extend_with(|_, ext| ext.set("code", "FORBIDDEN"))
    } else {
        Error::new(msg)
    }
}

/// FR-008/009/010. Ordering is deliberate and load-bearing, mirroring
/// `upload_canvas_image_impl`: authorize (Editor-or-Owner, same edit
/// gate as `updateLoreEntry`) → transcode both renditions (reject
/// oversized/undecodable before any write) → write both objects via the
/// existing per-object STS-scoped `write_object` path → persist the row
/// only after both writes succeed, so the entry's content is never left
/// referencing a missing asset.
pub async fn upload_lore_image_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    lore_entry_id: Uuid,
    original_filename: Option<String>,
    content_type: String,
    file_bytes: Vec<u8>,
) -> Result<LoreImageAsset, UploadLoreImageError> {
    // 1. Authorize BEFORE any transcode/storage/DB work.
    require_lore_permission(
        state,
        user_id,
        is_admin,
        lore_entry_id,
        ActorPermissionLevel::Editor,
    )
    .await
    .map_err(|_| UploadLoreImageError::Forbidden)?;

    // 2. Decode + produce both WebP renditions, enforcing
    //    MAX_LORE_IMAGE_UPLOAD_BYTES (FR-010).
    let renditions = transcode_to_lore_renditions(&file_bytes).map_err(|e| match e {
        TranscodeError::TooLarge { max, actual } => UploadLoreImageError::TooLarge { max, actual },
        other => UploadLoreImageError::Transcode(other.to_string()),
    })?;

    // 3. Write both objects via the existing per-object-scoped credential path.
    let asset_id = Uuid::now_v7();
    let cfg = RustFsConfig::from_env();
    write_object(
        &cfg,
        &full_key(asset_id),
        renditions.full_webp_bytes,
        "image/webp",
    )
    .await
    .map_err(|e| UploadLoreImageError::Storage(e.to_string()))?;
    write_object(
        &cfg,
        &thumb_key(asset_id),
        renditions.thumbnail_webp_bytes,
        "image/webp",
    )
    .await
    .map_err(|e| UploadLoreImageError::Storage(e.to_string()))?;

    // 4. Persist the row — only reached after both writes succeed, so no
    //    partial asset is ever recorded.
    let byte_size = file_bytes.len() as i64;
    let new_asset = NewLoreImageAsset {
        id: asset_id,
        lore_entry_id,
        uploaded_by: user_id,
        original_filename,
        content_type,
        byte_size,
        created_at: Utc::now().naive_utc(),
    };

    let mut conn = state
        .db_pool
        .get()
        .map_err(|e| UploadLoreImageError::Database(e.to_string()))?;
    tokio::task::spawn_blocking(move || {
        diesel::insert_into(world_lore_image_assets::table)
            .values(&new_asset)
            .returning(LoreImageAsset::as_returning())
            .get_result::<LoreImageAsset>(&mut conn)
    })
    .await
    .map_err(|e| UploadLoreImageError::Database(e.to_string()))?
    .map_err(|e| UploadLoreImageError::Database(e.to_string()))
}

#[derive(Default)]
pub struct LoreImageMutation;

#[async_graphql::Object]
impl LoreImageMutation {
    async fn upload_lore_image(
        &self,
        ctx: &Context<'_>,
        lore_entry_id: Uuid,
        file: Upload,
    ) -> GraphQLResult<GraphQLLoreImageAsset> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let upload_value = file.value(ctx).map_err(|e| Error::new(e.to_string()))?;
        let content_type = upload_value
            .content_type
            .clone()
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let original_filename = Some(upload_value.filename.clone());
        let mut content = upload_value.into_read();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut content, &mut bytes)
            .map_err(|e| Error::new(format!("failed to read upload: {e}")))?;

        let asset = upload_lore_image_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            lore_entry_id,
            original_filename,
            content_type,
            bytes,
        )
        .await
        .map_err(to_graphql_error)?;
        Ok(asset.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_lore::{CreateLoreEntryInput, create_lore_entry_impl};
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
        tiny_png_bytes,
    };

    /// FR-008/009: a happy-path upload produces a `LoreImageAsset` row.
    #[tokio::test]
    async fn upload_lore_image_happy_path_produces_asset() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let entry = create_lore_entry_impl(
            &state,
            owner_id,
            false,
            CreateLoreEntryInput {
                world_id,
                title: "Entry".to_string(),
                content: None,
            },
        )
        .await
        .unwrap();

        let asset = upload_lore_image_impl(
            &state,
            owner_id,
            false,
            entry.id,
            Some("map.png".to_string()),
            "image/png".to_string(),
            tiny_png_bytes(),
        )
        .await
        .expect("owner's (implicit Owner) upload should succeed");

        assert_eq!(asset.lore_entry_id, entry.id);
        assert_eq!(asset.content_type, "image/png");

        let mut conn = state.db_pool.get().unwrap();
        let reloaded = world_lore_image_assets::table
            .filter(world_lore_image_assets::id.eq(asset.id))
            .select(LoreImageAsset::as_select())
            .first::<LoreImageAsset>(&mut conn)
            .expect("row should exist");
        assert_eq!(reloaded.id, asset.id);
    }

    /// FR-010: a Viewer-level (default) caller is rejected before any
    /// write.
    #[tokio::test]
    async fn upload_lore_image_rejects_viewer_level_caller() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let viewer_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, viewer_id, "Player");
        drop(conn);

        let entry = create_lore_entry_impl(
            &state,
            owner_id,
            false,
            CreateLoreEntryInput {
                world_id,
                title: "Entry".to_string(),
                content: None,
            },
        )
        .await
        .unwrap();

        let result = upload_lore_image_impl(
            &state,
            viewer_id,
            false,
            entry.id,
            None,
            "image/png".to_string(),
            tiny_png_bytes(),
        )
        .await;
        assert!(matches!(result, Err(UploadLoreImageError::Forbidden)));

        let mut conn = state.db_pool.get().unwrap();
        let count: i64 = world_lore_image_assets::table
            .filter(world_lore_image_assets::lore_entry_id.eq(entry.id))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(
            count, 0,
            "no partial asset row should be persisted for a rejected upload"
        );
    }

    /// FR-010: an oversized upload is rejected before any row is
    /// created.
    #[tokio::test]
    async fn upload_lore_image_rejects_oversized_upload() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let entry = create_lore_entry_impl(
            &state,
            owner_id,
            false,
            CreateLoreEntryInput {
                world_id,
                title: "Entry".to_string(),
                content: None,
            },
        )
        .await
        .unwrap();

        let oversized = vec![0u8; crate::storage::transcode::MAX_LORE_IMAGE_UPLOAD_BYTES + 1];
        let result = upload_lore_image_impl(
            &state,
            owner_id,
            false,
            entry.id,
            None,
            "image/png".to_string(),
            oversized,
        )
        .await;
        assert!(matches!(result, Err(UploadLoreImageError::TooLarge { .. })));
    }
}
