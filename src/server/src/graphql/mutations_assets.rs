//! Spec 002: GraphQL surface for canvas image assets — `uploadCanvasImage`
//! (FR-011..FR-017) and `canvasImageAssetsForScene` (FR-014, FR-019). See
//! `specs/002-canvas-authoring-asset-storage/contracts/asset-storage.graphql`.
//!
//! The `#[async_graphql::Object]`/`#[async_graphql::Object]` methods
//! below are thin wrappers around `upload_canvas_image_impl` /
//! `canvas_image_assets_for_scene_impl`, which take plain arguments
//! instead of a GraphQL `Context` — this keeps the authorization-then-
//! transcode-then-write ordering (FR-016) directly unit-testable without
//! constructing a full `async_graphql::Schema` execution.

use async_graphql::{Context, Enum, Error, ErrorExtensions, Result as GraphQLResult, SimpleObject, Upload};
use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::{require_world_member, WorldMembershipError};
use crate::db_types::CanvasImageAssetKindEnum;
use crate::graphql::{app_state, authenticated_user};
use crate::models::{CanvasImageAsset, NewCanvasImageAsset};
use crate::state::AppState;
use crate::storage::rustfs::{object_key, write_object, RustFsConfig};
use crate::storage::transcode::transcode_to_webp;

#[derive(Enum, Debug, Copy, Clone, Eq, PartialEq)]
pub enum GraphQLCanvasImageAssetKind {
    Background,
    Pasted,
}

impl From<GraphQLCanvasImageAssetKind> for CanvasImageAssetKindEnum {
    fn from(k: GraphQLCanvasImageAssetKind) -> Self {
        match k {
            GraphQLCanvasImageAssetKind::Background => CanvasImageAssetKindEnum::Background,
            GraphQLCanvasImageAssetKind::Pasted => CanvasImageAssetKindEnum::Pasted,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLCanvasImageAsset {
    pub id: Uuid,
    pub world_id: Uuid,
    pub scene_id: Option<Uuid>,
    pub owner_user_id: Uuid,
    pub storage_path: String,
    pub kind: GraphQLCanvasImageAssetKind,
    pub width_px: i32,
    pub height_px: i32,
    pub byte_size: i32,
    pub created_at: chrono::NaiveDateTime,
}

impl From<CanvasImageAsset> for GraphQLCanvasImageAsset {
    fn from(a: CanvasImageAsset) -> Self {
        Self {
            id: a.asset_id,
            world_id: a.world_id,
            scene_id: a.scene_id,
            owner_user_id: a.owner_user_id,
            storage_path: a.storage_path,
            kind: match a.kind {
                CanvasImageAssetKindEnum::Background => GraphQLCanvasImageAssetKind::Background,
                CanvasImageAssetKindEnum::Pasted => GraphQLCanvasImageAssetKind::Pasted,
            },
            width_px: a.width_px,
            height_px: a.height_px,
            byte_size: a.byte_size as i32,
            created_at: a.created_at,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UploadCanvasImageError {
    #[error("user is not a member of this world")]
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

/// Not a `From<UploadCanvasImageError> for Error` impl — async-graphql
/// already provides a blanket `From<T: Display + Send + Sync + 'static>`,
/// so a second impl would conflict (E0119). Called explicitly at each
/// GraphQL resolver call site instead, to attach the `FORBIDDEN`
/// extension code (FR-016) that a bare `?`-conversion would drop.
fn to_graphql_error(e: UploadCanvasImageError) -> Error {
    let msg = e.to_string();
    if matches!(e, UploadCanvasImageError::Forbidden) {
        Error::new(msg).extend_with(|_, ext| ext.set("code", "FORBIDDEN"))
    } else {
        Error::new(msg)
    }
}

/// FR-011, FR-012, FR-013, FR-015, FR-016, FR-017. Ordering is
/// deliberate and load-bearing: authorize (T037/FR-016: reject before
/// any object is created) → transcode (FR-012/FR-013: reject oversized
/// before any write) → write via a single-object-scoped, server-held
/// credential (FR-017) → persist the row.
pub async fn upload_canvas_image_impl(
    state: &AppState,
    user_id: Uuid,
    world_id: Uuid,
    scene_id: Uuid,
    kind: GraphQLCanvasImageAssetKind,
    file_bytes: Vec<u8>,
) -> Result<CanvasImageAsset, UploadCanvasImageError> {
    // 1. Authorize BEFORE any transcode/storage/DB work (FR-016).
    {
        let mut conn = state
            .db_pool
            .get()
            .map_err(|e| UploadCanvasImageError::Database(e.to_string()))?;
        require_world_member(&mut conn, user_id, world_id).map_err(|e| match e {
            WorldMembershipError::NotAMember => UploadCanvasImageError::Forbidden,
            WorldMembershipError::Database(msg) => UploadCanvasImageError::Database(msg),
        })?;
    }

    // 2. Decode + transcode to WebP, enforcing MAX_UPLOAD_BYTES (FR-012, FR-013).
    let transcoded = transcode_to_webp(&file_bytes).map_err(|e| match e {
        crate::storage::transcode::TranscodeError::TooLarge { max, actual } => {
            UploadCanvasImageError::TooLarge { max, actual }
        }
        other => UploadCanvasImageError::Transcode(other.to_string()),
    })?;

    // 3. Write via a single-object-scoped, server-held credential (FR-017).
    let asset_id = Uuid::now_v7();
    let key = object_key(user_id, world_id, Some(scene_id), asset_id);
    let byte_size = transcoded.webp_bytes.len() as i64;

    // Spec 028 FR-005: fingerprint the bytes we are about to STORE, not the
    // ones that were uploaded. The client never receives the original — it
    // receives this WebP — so hashing the upload would produce a value no
    // client could ever verify against what it actually holds.
    //
    // Computed here, while the bytes are already in memory, rather than on
    // demand later: hashing on read would mean pulling every object back out
    // of RustFS on every sync, which is the exact server load this feature
    // exists to remove.
    let content_hash =
        thunderforge_cache_core::Fingerprint::of_bytes(&transcoded.webp_bytes).to_hex();

    let cfg = RustFsConfig::from_env();
    write_object(&cfg, &key, transcoded.webp_bytes, "image/webp")
        .await
        .map_err(|e| UploadCanvasImageError::Storage(e.to_string()))?;

    // 4. Persist the row — only reached after a successful write, so no
    //    partial asset is ever recorded (FR-013, data-model.md).
    let now = Utc::now().naive_utc();
    let new_asset = NewCanvasImageAsset {
        asset_id,
        world_id,
        scene_id: Some(scene_id),
        owner_user_id: user_id,
        storage_path: key,
        original_format: transcoded.original_format,
        width_px: transcoded.width as i32,
        height_px: transcoded.height as i32,
        byte_size,
        kind: kind.into(),
        created_by: user_id,
        updated_by: user_id,
        created_at: now,
        updated_at: now,
        content_hash: Some(content_hash),
    };

    let mut conn = state
        .db_pool
        .get()
        .map_err(|e| UploadCanvasImageError::Database(e.to_string()))?;
    let inserted = tokio::task::spawn_blocking(move || {
        use crate::schema::canvas_image_assets;
        diesel::insert_into(canvas_image_assets::table)
            .values(&new_asset)
            .returning(CanvasImageAsset::as_returning())
            .get_result::<CanvasImageAsset>(&mut conn)
    })
    .await
    .map_err(|e| UploadCanvasImageError::Database(e.to_string()))?
    .map_err(|e| UploadCanvasImageError::Database(e.to_string()))?;

    Ok(inserted)
}

/// FR-014, FR-019: read-side authorization mirrors the write side —
/// rejects before any row is returned if the caller is not an
/// owner/accepted member of the scene's owning world.
pub async fn canvas_image_assets_for_scene_impl(
    state: &AppState,
    user_id: Uuid,
    scene_id: Uuid,
) -> Result<Vec<CanvasImageAsset>, UploadCanvasImageError> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|e| UploadCanvasImageError::Database(e.to_string()))?;
    let assets = tokio::task::spawn_blocking(move || {
        use crate::schema::scenes;
        let world_id = scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .select(scenes::world_id)
            .first::<Uuid>(&mut conn)
            .optional()?;

        let Some(world_id) = world_id else {
            return Ok::<_, diesel::result::Error>(Err(UploadCanvasImageError::Database(
                "scene not found".to_string(),
            )));
        };

        if let Err(e) = require_world_member(&mut conn, user_id, world_id) {
            return Ok(Err(match e {
                WorldMembershipError::NotAMember => UploadCanvasImageError::Forbidden,
                WorldMembershipError::Database(msg) => UploadCanvasImageError::Database(msg),
            }));
        }

        use crate::schema::canvas_image_assets;
        let rows = canvas_image_assets::table
            .filter(canvas_image_assets::scene_id.eq(scene_id))
            .select(CanvasImageAsset::as_select())
            .load::<CanvasImageAsset>(&mut conn)?;
        Ok(Ok(rows))
    })
    .await
    .map_err(|e| UploadCanvasImageError::Database(e.to_string()))?
    .map_err(|e| UploadCanvasImageError::Database(e.to_string()))??;

    Ok(assets)
}

#[derive(Default)]
pub struct AssetMutation;

#[async_graphql::Object]
impl AssetMutation {
    async fn upload_canvas_image(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        scene_id: Uuid,
        kind: GraphQLCanvasImageAssetKind,
        file: Upload,
    ) -> GraphQLResult<GraphQLCanvasImageAsset> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let upload_value = file.value(ctx).map_err(|e| Error::new(e.to_string()))?;
        let mut content = upload_value.into_read();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut content, &mut bytes)
            .map_err(|e| Error::new(format!("failed to read upload: {e}")))?;

        let asset = upload_canvas_image_impl(state, auth_user.user_id, world_id, scene_id, kind, bytes)
            .await
            .map_err(to_graphql_error)?;
        Ok(asset.into())
    }
}

#[derive(Default)]
pub struct AssetQuery;

#[async_graphql::Object]
impl AssetQuery {
    async fn canvas_image_assets_for_scene(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLCanvasImageAsset>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let assets = canvas_image_assets_for_scene_impl(state, auth_user.user_id, scene_id)
            .await
            .map_err(to_graphql_error)?;
        Ok(assets.into_iter().map(Into::into).collect())
    }
}

#[cfg(test)]
mod tests {
    //! Integration tests against a real Postgres (DATABASE_URL) and a
    //! real RustFS (`docker compose up -d rustfs`) — no mocks. Run with
    //! `cargo test`; each test creates its own throwaway user/world/scene
    //! fixtures via `crate::test_support`.

    use super::*;
    use crate::test_support::*;

    /// T021 (FR-011, FR-012, SC-005): happy path — a non-WebP (PNG)
    /// upload from the world's owner produces a CanvasImageAsset row and
    /// a WebP object in RustFS.
    #[tokio::test]
    async fn upload_canvas_image_happy_path_produces_webp_asset() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let asset = upload_canvas_image_impl(
            &state,
            owner_id,
            world_id,
            scene_id,
            GraphQLCanvasImageAssetKind::Pasted,
            tiny_png_bytes(),
        )
        .await
        .expect("owner's upload should succeed");

        assert_eq!(asset.world_id, world_id);
        assert_eq!(asset.scene_id, Some(scene_id));
        assert_eq!(asset.original_format, "png");
        assert!(asset.storage_path.ends_with(".webp"));
        assert_eq!(
            asset.storage_path,
            crate::storage::rustfs::object_key(owner_id, world_id, Some(scene_id), asset.asset_id)
        );

        // Verify the row is really persisted (SC-005: format is verified
        // by inspecting the stored asset).
        let mut conn = state.db_pool.get().unwrap();
        use crate::schema::canvas_image_assets;
        let reloaded = canvas_image_assets::table
            .filter(canvas_image_assets::asset_id.eq(asset.asset_id))
            .select(CanvasImageAsset::as_select())
            .first::<CanvasImageAsset>(&mut conn)
            .expect("row should exist");
        assert_eq!(reloaded.original_format, "png");
    }

    /// Spec 028 T019 (FR-005): the persisted `content_hash` is the hash of
    /// the bytes we STORED, not the bytes that were uploaded.
    ///
    /// This distinction is the whole reason the column exists. A client never
    /// receives the original — it receives the transcoded WebP — so a hash of
    /// the upload would be a value no client could ever verify against what
    /// it actually holds, and every cache check would miss forever while
    /// looking perfectly valid in the database.
    #[tokio::test]
    async fn upload_records_hash_of_stored_bytes_not_uploaded_bytes() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let uploaded = tiny_png_bytes();
        let asset = upload_canvas_image_impl(
            &state,
            owner_id,
            world_id,
            scene_id,
            GraphQLCanvasImageAssetKind::Pasted,
            uploaded.clone(),
        )
        .await
        .expect("owner's upload should succeed");

        let hash = asset
            .content_hash
            .as_deref()
            .expect("upload must record a fingerprint; NULL is only for un-backfilled rows");

        // Well-formed. The CHECK constraint enforces this at the database,
        // but asserting here localises a failure to the code that wrote it.
        assert_eq!(hash.len(), 64, "lowercase hex SHA-256");
        assert!(hash.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)));

        // Not the upload's hash — the transcode makes these differ.
        let uploaded_hash = thunderforge_cache_core::Fingerprint::of_bytes(&uploaded).to_hex();
        assert_ne!(
            hash, uploaded_hash,
            "hashing the uploaded bytes would produce a value no client could verify"
        );

        // The stored object's hash: what a client computes over what it
        // actually receives, and therefore the only value that can ever hit.
        let stored = crate::storage::rustfs::read_object(
            &crate::storage::rustfs::RustFsConfig::from_env(),
            &asset.storage_path,
        )
        .await
        .expect("stored object should be readable");
        assert_eq!(
            hash,
            thunderforge_cache_core::Fingerprint::of_bytes(&stored).to_hex(),
            "content_hash must match the bytes the client will actually receive"
        );
    }

    /// Spec 028 T019: the same image uploaded twice fingerprints identically
    /// under two different asset ids.
    ///
    /// Content addressing depends on this: it is what lets a peer serve a
    /// blob by hash, and what lets a client recognise it already holds bytes
    /// another scene introduced.
    #[tokio::test]
    async fn identical_content_produces_identical_fingerprints() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let bytes = tiny_png_bytes();
        let first = upload_canvas_image_impl(
            &state,
            owner_id,
            world_id,
            scene_id,
            GraphQLCanvasImageAssetKind::Pasted,
            bytes.clone(),
        )
        .await
        .expect("first upload should succeed");
        let second = upload_canvas_image_impl(
            &state,
            owner_id,
            world_id,
            scene_id,
            GraphQLCanvasImageAssetKind::Pasted,
            bytes,
        )
        .await
        .expect("second upload should succeed");

        assert_ne!(first.asset_id, second.asset_id, "distinct rows");
        assert_eq!(
            first.content_hash, second.content_hash,
            "identical content must fingerprint identically, or dedup and peer transfer cannot work"
        );
    }

    /// T022 (FR-013): oversized upload is rejected before any row or
    /// object is created — no partial asset persisted.
    #[tokio::test]
    async fn upload_canvas_image_rejects_oversized_upload_before_persisting() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        drop(conn);

        let oversized = vec![0u8; crate::storage::transcode::MAX_UPLOAD_BYTES + 1];
        let result = upload_canvas_image_impl(
            &state,
            owner_id,
            world_id,
            scene_id,
            GraphQLCanvasImageAssetKind::Pasted,
            oversized,
        )
        .await;

        assert!(matches!(result, Err(UploadCanvasImageError::TooLarge { .. })));

        let mut conn = state.db_pool.get().unwrap();
        use crate::schema::canvas_image_assets;
        let count: i64 = canvas_image_assets::table
            .filter(canvas_image_assets::world_id.eq(world_id))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(count, 0, "no partial asset row should be persisted");
    }

    /// T033 (US4 Acceptance Scenario 1, SC-006): a non-member's write
    /// attempt is rejected before any object/row is created.
    #[tokio::test]
    async fn upload_canvas_image_rejects_non_member_before_any_write() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let outsider_id = insert_test_user(&mut conn); // owns nothing, invited to nothing
        drop(conn);

        let result = upload_canvas_image_impl(
            &state,
            outsider_id,
            world_id,
            scene_id,
            GraphQLCanvasImageAssetKind::Pasted,
            tiny_png_bytes(),
        )
        .await;

        assert!(matches!(result, Err(UploadCanvasImageError::Forbidden)));

        let mut conn = state.db_pool.get().unwrap();
        use crate::schema::canvas_image_assets;
        let count: i64 = canvas_image_assets::table
            .filter(canvas_image_assets::world_id.eq(world_id))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(count, 0, "no object should be created for a rejected write");
    }

    /// T034 (US4 Acceptance Scenarios 2-3, SC-007): after invite-accept
    /// (simulated: a world_members row is inserted), the same user's
    /// write succeeds; after membership removal, a subsequent write is
    /// rejected — both within one request each, no stale-permission
    /// window.
    #[tokio::test]
    async fn upload_canvas_image_respects_membership_grant_and_revoke() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let invited_id = insert_test_user(&mut conn);

        // Not yet a member: rejected.
        drop(conn);
        let result = upload_canvas_image_impl(
            &state,
            invited_id,
            world_id,
            scene_id,
            GraphQLCanvasImageAssetKind::Pasted,
            tiny_png_bytes(),
        )
        .await;
        assert!(matches!(result, Err(UploadCanvasImageError::Forbidden)));

        // Grant membership (simulated invite-accept): write succeeds.
        let mut conn = state.db_pool.get().unwrap();
        insert_test_world_member(&mut conn, world_id, invited_id, "Player");
        drop(conn);
        let result = upload_canvas_image_impl(
            &state,
            invited_id,
            world_id,
            scene_id,
            GraphQLCanvasImageAssetKind::Pasted,
            tiny_png_bytes(),
        )
        .await;
        assert!(result.is_ok(), "write should succeed once a member: {result:?}");

        // Revoke membership: next write is rejected again.
        let mut conn = state.db_pool.get().unwrap();
        remove_test_world_member(&mut conn, world_id, invited_id);
        drop(conn);
        let result = upload_canvas_image_impl(
            &state,
            invited_id,
            world_id,
            scene_id,
            GraphQLCanvasImageAssetKind::Pasted,
            tiny_png_bytes(),
        )
        .await;
        assert!(matches!(result, Err(UploadCanvasImageError::Forbidden)));
    }

    /// T036 (FR-014, FR-019): a non-member cannot read a world's assets
    /// via canvas_image_assets_for_scene_impl — rejected before any row
    /// is returned.
    #[tokio::test]
    async fn canvas_image_assets_for_scene_rejects_non_member_read() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let outsider_id = insert_test_user(&mut conn);
        drop(conn);

        // Owner can read (even with zero assets, this must not error).
        let owner_read = canvas_image_assets_for_scene_impl(&state, owner_id, scene_id).await;
        assert!(owner_read.is_ok());

        let outsider_read = canvas_image_assets_for_scene_impl(&state, outsider_id, scene_id).await;
        assert!(matches!(outsider_read, Err(UploadCanvasImageError::Forbidden)));
    }
}
