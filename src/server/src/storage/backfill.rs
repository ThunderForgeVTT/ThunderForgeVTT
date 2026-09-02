//! Spec 028 T125 (FR-005): backfill `content_hash` for canvas image assets
//! written before the column existed.
//!
//! ## Why this can be a background job at all
//!
//! A NULL `content_hash` is not a broken row. Every read of the column
//! already treats NULL as "the client must fetch this" — see
//! `graphql/queries/world_sync_plan.rs` and
//! `thunderforge_cache_core::delta::compute_plan` — so an un-backfilled
//! asset is *correct*, merely *wasteful*: the client refetches bytes it
//! could have recognised it already held. Nothing observable breaks while
//! this job is halfway done, which is precisely what lets it run behind
//! live traffic instead of blocking a release.
//!
//! ## What the work actually is
//!
//! Sized against the dev database on 2026-08-28: 413 `Background` rows,
//! ~1432 MB, average 3.5 MB, largest 13 MB, left behind by the map-import
//! bug fixed in `4ea412e` (nothing has accrued since — the upload path in
//! `graphql/mutations_assets.rs` has always written a hash). The bytes are
//! only in object storage, so the only way to learn their digest is to
//! read all 1.4 GB back out of RustFS. That is why this is paced: one
//! object at a time, a small batch per tick, never a single sweep
//! competing with real requests.
//!
//! ## Safety properties
//!
//! - **Resumable.** Each row is hashed and written on its own, so stopping
//!   anywhere — crash, deploy, SIGKILL — leaves the database consistent.
//!   The next start simply finds fewer NULLs.
//! - **Idempotent.** The UPDATE is guarded by `content_hash IS NULL`, so a
//!   second run (or two servers running at once) can only ever fill a hole,
//!   never overwrite a hash somebody else already computed.
//! - **Never invents a value.** The hash is always
//!   `Fingerprint::of_bytes` over the bytes actually read back from
//!   storage, via the same `thunderforge_cache_core` entry point the upload
//!   path uses. If the two ever disagreed, every backfilled asset would
//!   fail the client's integrity check forever and become permanently
//!   unfetchable, so they share one implementation rather than two that
//!   look alike.
//! - **Tolerates missing objects.** An object that cannot be read — gone
//!   from storage, permission denied, truncated — leaves its row NULL (the
//!   safe value) and the run moves on. A restart retries it, in case
//!   storage was repaired in between.

use diesel::prelude::*;
use uuid::Uuid;

use crate::state::DbPool;
use crate::storage::rustfs::{RustFsConfig, read_object};

/// Candidates fetched per cycle. Small on purpose: the loop is a trickle
/// behind live traffic, not a migration.
const BATCH_SIZE: i64 = 8;

/// Pause between two object reads. At ~3.5 MB average this holds the job
/// to a few MB/s of storage reads — a rounding error next to a single
/// player loading a scene.
const PAUSE_BETWEEN_ASSETS: std::time::Duration = std::time::Duration::from_millis(500);

/// Pause between batches, on top of the per-asset pause.
const PAUSE_BETWEEN_BATCHES: std::time::Duration = std::time::Duration::from_secs(5);

/// One asset needing a hash: its id and where its bytes live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackfillCandidate {
    pub asset_id: Uuid,
    pub storage_path: String,
}

/// What happened to one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackfillOutcome {
    /// The bytes were read, hashed, and the hash written.
    Hashed,
    /// The row already had a hash by the time the UPDATE ran, so nothing
    /// was written. Not an error — this is the idempotency guard doing its
    /// job.
    AlreadyHashed,
    /// The object could not be read. The row is still NULL, which still
    /// means "fetch", which is still correct.
    Unreadable,
}

/// Rows still missing a hash, oldest first, starting strictly after
/// `after` (asset ids are UUIDv7, so id order is creation order).
///
/// The cursor is what keeps a permanently unreadable object from wedging
/// the loop: a failed row stays NULL but the cursor still advances past
/// it, so the next batch is genuinely new work rather than the same
/// failure forever.
pub fn select_candidates(
    conn: &mut PgConnection,
    after: Option<Uuid>,
    limit: i64,
) -> QueryResult<Vec<BackfillCandidate>> {
    use crate::schema::canvas_image_assets::dsl as a;

    let mut query = a::canvas_image_assets
        .filter(a::content_hash.is_null())
        .into_boxed();
    if let Some(cursor) = after {
        query = query.filter(a::asset_id.gt(cursor));
    }
    query
        .order(a::asset_id.asc())
        .limit(limit)
        .select((a::asset_id, a::storage_path))
        .load::<(Uuid, String)>(conn)
        .map(|rows| {
            rows.into_iter()
                .map(|(asset_id, storage_path)| BackfillCandidate {
                    asset_id,
                    storage_path,
                })
                .collect()
        })
}

/// Read one asset's stored bytes, hash them, and record the digest.
///
/// This is the whole job, for one row. It is deliberately the only place
/// that writes a backfilled hash, so "we never wrote a hash we did not
/// compute from the bytes" is a claim about a single function.
pub async fn backfill_asset(
    pool: &DbPool,
    cfg: &RustFsConfig,
    candidate: &BackfillCandidate,
) -> BackfillOutcome {
    let bytes = match read_object(cfg, &candidate.storage_path).await {
        Ok(bytes) => bytes,
        Err(err) => {
            // Leaving the row NULL is the conservative choice: NULL costs a
            // refetch, a wrong hash costs the client the asset permanently.
            eprintln!(
                "[content-hash backfill] ⚠️  skipping asset {} ({}): {err}",
                candidate.asset_id, candidate.storage_path
            );
            return BackfillOutcome::Unreadable;
        }
    };

    // The same call the upload path makes over the same (stored, WebP)
    // bytes. Not a reimplementation — a reuse, because a second hasher
    // that drifted by one byte would be undetectable here and fatal at
    // the client.
    let hash = thunderforge_cache_core::Fingerprint::of_bytes(&bytes).to_hex();

    let asset_id = candidate.asset_id;
    let pool = pool.clone();
    let updated = tokio::task::spawn_blocking(move || {
        use crate::schema::canvas_image_assets::dsl as a;
        let mut conn = pool.get()?;
        diesel::update(
            a::canvas_image_assets
                .filter(a::asset_id.eq(asset_id))
                // The idempotency guard: only ever fill a hole.
                .filter(a::content_hash.is_null()),
        )
        .set(a::content_hash.eq(Some(hash)))
        .execute(&mut conn)
        .map_err(Box::<dyn std::error::Error + Send + Sync>::from)
    })
    .await;

    match updated {
        Ok(Ok(1)) => BackfillOutcome::Hashed,
        Ok(Ok(_)) => BackfillOutcome::AlreadyHashed,
        Ok(Err(err)) => {
            eprintln!("[content-hash backfill] ⚠️  failed to record hash for {asset_id}: {err}");
            BackfillOutcome::Unreadable
        }
        Err(err) => {
            eprintln!("[content-hash backfill] ⚠️  hash write task for {asset_id} panicked: {err}");
            BackfillOutcome::Unreadable
        }
    }
}

/// Spawn the paced backfill.
///
/// Runs until it has walked every row that was missing a hash at the time
/// it reached it, then exits — there is no steady-state work here, because
/// the upload path has always written a hash. Restarting the server
/// restarts the walk, which is also how objects that were unreadable once
/// get another chance.
pub fn spawn_content_hash_backfill_task(pool: DbPool) {
    tokio::spawn(async move {
        let cfg = RustFsConfig::from_env();
        let mut cursor: Option<Uuid> = None;
        let mut hashed = 0usize;
        let mut skipped = 0usize;

        loop {
            let batch = {
                let pool = pool.clone();
                match tokio::task::spawn_blocking(move || {
                    let mut conn = pool.get().map_err(|e| e.to_string())?;
                    select_candidates(&mut conn, cursor, BATCH_SIZE).map_err(|e| e.to_string())
                })
                .await
                {
                    Ok(Ok(batch)) => batch,
                    Ok(Err(err)) => {
                        eprintln!("[content-hash backfill] ⚠️  candidate query failed: {err}");
                        return;
                    }
                    Err(err) => {
                        eprintln!("[content-hash backfill] ⚠️  candidate query panicked: {err}");
                        return;
                    }
                }
            };

            if batch.is_empty() {
                if hashed > 0 || skipped > 0 {
                    eprintln!(
                        "[content-hash backfill] ✅ done: {hashed} hashed, {skipped} left NULL (unreadable)"
                    );
                }
                return;
            }

            for candidate in batch {
                cursor = Some(candidate.asset_id);
                match backfill_asset(&pool, &cfg, &candidate).await {
                    BackfillOutcome::Hashed => hashed += 1,
                    BackfillOutcome::AlreadyHashed => {}
                    BackfillOutcome::Unreadable => skipped += 1,
                }
                tokio::time::sleep(PAUSE_BETWEEN_ASSETS).await;
            }

            tokio::time::sleep(PAUSE_BETWEEN_BATCHES).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_assets::{GraphQLCanvasImageAssetKind, upload_canvas_image_impl};
    use crate::storage::rustfs::{object_key, write_object};
    use crate::test_support::{
        insert_test_scene, insert_test_user, insert_test_world, test_app_state,
    };

    /// A minimal 1x1 PNG, same fixture the upload tests use.
    fn tiny_png_bytes() -> Vec<u8> {
        // 1x1 transparent PNG.
        base64_decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
        )
    }

    fn base64_decode(s: &str) -> Vec<u8> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        STANDARD.decode(s).expect("fixture must be valid base64")
    }

    /// Inserts a row with an explicit `content_hash`, mirroring the shape a
    /// pre-backfill row has (`None`) or a done one (`Some`).
    fn insert_asset_row(
        conn: &mut PgConnection,
        owner_id: Uuid,
        world_id: Uuid,
        scene_id: Uuid,
        storage_path: String,
        content_hash: Option<String>,
    ) -> Uuid {
        use crate::schema::canvas_image_assets;
        let asset_id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(canvas_image_assets::table)
            .values((
                canvas_image_assets::asset_id.eq(asset_id),
                canvas_image_assets::world_id.eq(world_id),
                canvas_image_assets::scene_id.eq(Some(scene_id)),
                canvas_image_assets::owner_user_id.eq(owner_id),
                canvas_image_assets::storage_path.eq(storage_path),
                canvas_image_assets::original_format.eq("png"),
                canvas_image_assets::width_px.eq(1),
                canvas_image_assets::height_px.eq(1),
                canvas_image_assets::byte_size.eq(1i64),
                canvas_image_assets::kind.eq(crate::db_types::CanvasImageAssetKindEnum::Background),
                canvas_image_assets::created_by.eq(owner_id),
                canvas_image_assets::updated_by.eq(owner_id),
                canvas_image_assets::created_at.eq(now),
                canvas_image_assets::updated_at.eq(now),
                canvas_image_assets::content_hash.eq(content_hash),
            ))
            .execute(conn)
            .expect("failed to insert test asset row");
        asset_id
    }

    fn stored_hash(conn: &mut PgConnection, asset_id: Uuid) -> Option<String> {
        use crate::schema::canvas_image_assets::dsl as a;
        a::canvas_image_assets
            .filter(a::asset_id.eq(asset_id))
            .select(a::content_hash)
            .first::<Option<String>>(conn)
            .expect("row should exist")
    }

    /// The job's whole purpose: a row left NULL by the map-import bug ends
    /// up holding the digest of the bytes that are really in storage — not
    /// of anything else, and not of nothing.
    #[tokio::test]
    async fn a_row_with_no_hash_gets_the_digest_of_its_actual_bytes() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);

        let asset_id = Uuid::now_v7();
        let key = object_key(owner_id, world_id, Some(scene_id), asset_id);
        let bytes = b"not really a webp, but these are the stored bytes".to_vec();
        let cfg = RustFsConfig::from_env();
        write_object(&cfg, &key, bytes.clone(), "image/webp")
            .await
            .expect("test object write should succeed");

        let row_id = insert_asset_row(&mut conn, owner_id, world_id, scene_id, key.clone(), None);
        drop(conn);

        let outcome = backfill_asset(
            &state.db_pool,
            &cfg,
            &BackfillCandidate {
                asset_id: row_id,
                storage_path: key,
            },
        )
        .await;
        assert_eq!(outcome, BackfillOutcome::Hashed);

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            stored_hash(&mut conn, row_id).as_deref(),
            Some(thunderforge_cache_core::Fingerprint::of_bytes(&bytes).to_hex()).as_deref(),
            "the backfilled value must be the digest of the bytes in storage"
        );
    }

    /// Re-running the job must never disturb a hash that is already there.
    /// The guard is what makes it safe to restart the server mid-run, and
    /// safe for two servers to run the job at once: the worst case is a
    /// wasted object read, never a clobbered digest.
    #[tokio::test]
    async fn a_row_that_already_has_a_hash_is_left_alone() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);

        let asset_id = Uuid::now_v7();
        let key = object_key(owner_id, world_id, Some(scene_id), asset_id);
        let cfg = RustFsConfig::from_env();
        write_object(
            &cfg,
            &key,
            b"different bytes entirely".to_vec(),
            "image/webp",
        )
        .await
        .expect("test object write should succeed");

        // A well-formed hash that deliberately does NOT match the object:
        // if the job overwrote existing values, this test would notice.
        let pre_existing = "a".repeat(64);
        let row_id = insert_asset_row(
            &mut conn,
            owner_id,
            world_id,
            scene_id,
            key.clone(),
            Some(pre_existing.clone()),
        );
        drop(conn);

        let outcome = backfill_asset(
            &state.db_pool,
            &cfg,
            &BackfillCandidate {
                asset_id: row_id,
                storage_path: key,
            },
        )
        .await;
        assert_eq!(outcome, BackfillOutcome::AlreadyHashed);

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            stored_hash(&mut conn, row_id).as_deref(),
            Some(pre_existing.as_str()),
            "an existing hash must survive the job untouched"
        );
    }

    /// An object that is gone from storage must not poison the row and must
    /// not end the run. NULL costs one refetch; a guessed hash would cost
    /// the client that asset forever, and a panic would cost every later
    /// row its backfill.
    #[tokio::test]
    async fn an_unreadable_object_leaves_the_row_null_and_the_run_continues() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let cfg = RustFsConfig::from_env();

        // Row 1: points at a key that was never written.
        let missing_key = object_key(owner_id, world_id, Some(scene_id), Uuid::now_v7());
        let missing_row = insert_asset_row(
            &mut conn,
            owner_id,
            world_id,
            scene_id,
            missing_key.clone(),
            None,
        );

        // Row 2: a perfectly good asset that must still get hashed.
        let good_asset_id = Uuid::now_v7();
        let good_key = object_key(owner_id, world_id, Some(scene_id), good_asset_id);
        let good_bytes = b"readable stored bytes".to_vec();
        write_object(&cfg, &good_key, good_bytes.clone(), "image/webp")
            .await
            .expect("test object write should succeed");
        let good_row = insert_asset_row(
            &mut conn,
            owner_id,
            world_id,
            scene_id,
            good_key.clone(),
            None,
        );
        drop(conn);

        let missing_outcome = backfill_asset(
            &state.db_pool,
            &cfg,
            &BackfillCandidate {
                asset_id: missing_row,
                storage_path: missing_key,
            },
        )
        .await;
        assert_eq!(missing_outcome, BackfillOutcome::Unreadable);

        let good_outcome = backfill_asset(
            &state.db_pool,
            &cfg,
            &BackfillCandidate {
                asset_id: good_row,
                storage_path: good_key,
            },
        )
        .await;
        assert_eq!(
            good_outcome,
            BackfillOutcome::Hashed,
            "one unreadable object must not stop the rows after it"
        );

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            stored_hash(&mut conn, missing_row),
            None,
            "an unreadable object must leave NULL — which still means 'fetch' — not a guess"
        );
        assert_eq!(
            stored_hash(&mut conn, good_row).as_deref(),
            Some(thunderforge_cache_core::Fingerprint::of_bytes(&good_bytes).to_hex()).as_deref()
        );
    }

    /// The property everything else rests on: the digest this job writes is
    /// byte-for-byte the digest the upload path would have written for the
    /// same asset. If these two ever disagreed, every backfilled asset would
    /// fail the client's integrity check and become permanently unfetchable
    /// — a failure that looks like a healthy database.
    ///
    /// Proven the only way that admits no drift: upload for real, remember
    /// the hash the upload path recorded, blank the column to recreate a
    /// pre-backfill row, run the job, and require the same value back.
    #[tokio::test]
    async fn the_backfilled_digest_equals_what_the_upload_path_would_have_written() {
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
            GraphQLCanvasImageAssetKind::Background,
            tiny_png_bytes(),
        )
        .await
        .expect("owner's upload should succeed");
        let upload_path_hash = asset
            .content_hash
            .clone()
            .expect("the upload path always records a hash");

        // Recreate the pre-backfill state this job exists for.
        {
            use crate::schema::canvas_image_assets::dsl as a;
            let mut conn = state.db_pool.get().unwrap();
            diesel::update(a::canvas_image_assets.filter(a::asset_id.eq(asset.asset_id)))
                .set(a::content_hash.eq(None::<String>))
                .execute(&mut conn)
                .expect("blanking the hash should succeed");
        }

        let outcome = backfill_asset(
            &state.db_pool,
            &RustFsConfig::from_env(),
            &BackfillCandidate {
                asset_id: asset.asset_id,
                storage_path: asset.storage_path.clone(),
            },
        )
        .await;
        assert_eq!(outcome, BackfillOutcome::Hashed);

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            stored_hash(&mut conn, asset.asset_id).as_deref(),
            Some(upload_path_hash.as_str()),
            "backfill and upload must agree, or backfilled assets are unfetchable forever"
        );
    }

    /// Candidate selection is the other half of "never overwrite": the job
    /// only ever looks at rows that are missing a hash, so a row with one is
    /// never even read from storage.
    #[tokio::test]
    async fn candidate_selection_sees_only_rows_missing_a_hash() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);

        let cursor = Uuid::now_v7();
        let without = insert_asset_row(
            &mut conn,
            owner_id,
            world_id,
            scene_id,
            object_key(owner_id, world_id, Some(scene_id), Uuid::now_v7()),
            None,
        );
        let with = insert_asset_row(
            &mut conn,
            owner_id,
            world_id,
            scene_id,
            object_key(owner_id, world_id, Some(scene_id), Uuid::now_v7()),
            Some("b".repeat(64)),
        );

        // Anchored after a cursor minted just before both rows, so this sees
        // exactly the rows this test created and nothing else in the database.
        let candidates =
            select_candidates(&mut conn, Some(cursor), 100).expect("candidate query should run");
        let ids: Vec<Uuid> = candidates.iter().map(|c| c.asset_id).collect();
        assert!(ids.contains(&without), "a NULL-hash row is work to be done");
        assert!(
            !ids.contains(&with),
            "a row that already has a hash must never be re-read from storage"
        );

        let _ = state;
    }

    /// The cursor is what keeps a permanently unreadable object from wedging
    /// the loop: rows before it are never revisited within a run, even
    /// though a failed row is still NULL and would otherwise match forever.
    #[tokio::test]
    async fn the_cursor_advances_past_a_row_that_stayed_null() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);

        let start = Uuid::now_v7();
        let first = insert_asset_row(
            &mut conn,
            owner_id,
            world_id,
            scene_id,
            object_key(owner_id, world_id, Some(scene_id), Uuid::now_v7()),
            None,
        );
        let second = insert_asset_row(
            &mut conn,
            owner_id,
            world_id,
            scene_id,
            object_key(owner_id, world_id, Some(scene_id), Uuid::now_v7()),
            None,
        );

        // Both rows are ahead of `start` and still NULL, so both are work.
        // (Other tests share this database, so assert on membership rather
        // than on exact pages — the ordering claim is what matters.)
        let batch = select_candidates(&mut conn, Some(start), 1000).expect("candidate query");
        let ids: Vec<Uuid> = batch.iter().map(|c| c.asset_id).collect();
        let first_at = ids
            .iter()
            .position(|id| *id == first)
            .expect("first is work");
        let second_at = ids
            .iter()
            .position(|id| *id == second)
            .expect("second is work");
        assert!(first_at < second_at, "oldest first");

        // Even with `first` still NULL — as it would be after an unreadable
        // object — a cursor past it never offers it again.
        let next = select_candidates(&mut conn, Some(first), 1000).expect("candidate query");
        let next_ids: Vec<Uuid> = next.iter().map(|c| c.asset_id).collect();
        assert!(
            !next_ids.contains(&first),
            "a row the run already tried must not come back within that run"
        );
        assert!(next_ids.contains(&second), "the run keeps moving forward");

        let _ = state;
    }
}
