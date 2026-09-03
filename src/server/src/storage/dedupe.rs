//! Store one copy of any given image, however many places refer to it.
//!
//! # The measurement this exists for
//!
//! Taken against the development database on 2026-09-03: **4,387 canvas image
//! assets, 61 distinct images.** 2,695 MB stored to hold 116 MB of content —
//! 96% of it byte-identical copies of something already present. Importing the
//! same map into two worlds wrote it twice; importing it into fifty wrote it
//! fifty times.
//!
//! Restricting the reuse to a single world would have reclaimed 3.8 MB of that
//! 2,579 MB, because the duplication is almost entirely *across* worlds — 3,815
//! of those rows share their bytes with a row in a different world. So the
//! lookup is instance-wide, deliberately, and the tradeoff is recorded below.
//!
//! # Why this is safe here, and what would make it unsafe
//!
//! Each asset keeps its **own row**, with its own `asset_id`, `world_id`,
//! `scene_id` and owner. Only `storage_path` is shared. `canvas_assets_serve`
//! authorises against the row it looked up and then reads whatever path that
//! row names, so two worlds pointing at one object are still two independent
//! permission checks. Nothing about who may see what changes.
//!
//! **Nothing in this product deletes stored objects.** `storage/rustfs.rs` has
//! no delete operation at all, which is what makes a shared path safe today: a
//! reference cannot dangle when references are never dropped.
//!
//! That is a load-bearing assumption, so it is written here rather than
//! discovered later. **Adding object deletion means adding reference counting
//! first** — deleting the object behind an asset row would silently blank the
//! background of every other scene sharing those bytes, and the failure would
//! appear as a missing image in a world nobody touched. The query to ask is
//! "does any other row name this `storage_path`", and it must be asked inside
//! the same transaction that removes the row.
//!
//! # What an instance-wide lookup reveals
//!
//! That someone else on this instance already holds a byte-identical file —
//! and only to a person who already has that exact file to upload. It is the
//! standard content-addressed-storage tradeoff, and on a self-hosted table
//! where uploads come from authenticated Game Masters it buys 96% of the
//! storage back. Recorded as a deliberate choice rather than an oversight: a
//! deployment that cannot accept it wants the world-scoped variant, which is
//! this query plus a `world_id` predicate and most of the benefit gone.

use diesel::prelude::*;

/// Where these exact bytes are already stored, if anywhere.
///
/// The hash is over the bytes as *stored* — the transcoded WebP, not whatever
/// was uploaded — which is what makes a match mean "the object at this path is
/// byte-for-byte what we were about to write".
///
/// A lookup failure answers `None`: the caller then writes its own copy, which
/// costs storage and is otherwise entirely correct. Deduplication is an
/// optimisation, and an optimisation that can fail an upload is a bad trade.
pub fn object_holding(conn: &mut PgConnection, content_hash: &str) -> Option<String> {
    use crate::schema::canvas_image_assets as assets;

    assets::table
        .filter(assets::content_hash.eq(content_hash))
        .select(assets::storage_path)
        // Oldest first: the copy most likely to have been backfilled, verified
        // and fetched by clients already. Deterministic rather than arbitrary,
        // so repeated uploads of one image converge on a single object instead
        // of scattering across whichever row the planner happened to return.
        .order(assets::created_at.asc())
        .first::<String>(conn)
        .optional()
        .ok()
        .flatten()
}

/// Two racing uploads of the same *new* image both miss this lookup and both
/// write, to different keys, and both insert valid rows.
///
/// That is a redundant object, not a broken one, and the next upload of those
/// bytes will reuse whichever won. Serialising uploads to prevent it would
/// trade a rare wasted write for a lock on every upload in the product.
#[cfg(test)]
#[path = "dedupe_tests.rs"]
mod tests;
