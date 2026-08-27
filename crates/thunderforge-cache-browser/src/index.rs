//! The IndexedDB `index` store: what the cache believes it holds (T026).
//!
//! Spec 028 FR-019/FR-022/FR-023, data-model.md "IndexedDB stores".
//!
//! ```text
//! index : ItemId -> (fingerprint, byte size, last read seq, world id)
//! ```
//!
//! # The index is a belief, not the truth
//!
//! OPFS holds the bytes; this store holds an account of them. The two can
//! disagree, and given a crash between the blob write and the index write,
//! eventually they will. That is expected rather than exceptional, and it is
//! why the fingerprint is the blob's filename: the disk can be re-read and
//! the disagreement settled without downloading anything (FR-019). Where they
//! differ, **OPFS wins** — a file that exists exists, and an index row
//! claiming otherwise is stale bookkeeping.
//!
//! So nothing here may be treated as proof that content is readable. The
//! index answers "what should I have, and how big was it"; only a verified
//! read answers "do I have it".
//!
//! # Why `last_read` is a sequence number and not a time
//!
//! LRU eviction needs an order, not a date. A client clock is forgeable and
//! routinely wrong — the same reason FR-040a forbids timestamps in conflict
//! resolution — and a machine whose clock jumps backwards would otherwise
//! evict its hottest content. A monotonic counter owned by this store is
//! immune to that, is smaller to store, and is exactly as useful, because no
//! rule in this feature asks *when* something was read.

use serde::{Deserialize, Serialize};
use thunderforge_cache_core::{Fingerprint, ItemId};
use thunderforge_opfs::store::BlobShape;
use uuid::Uuid;

/// Monotonic counter standing in for "when", for LRU ordering only.
///
/// Never compared across devices and never sent to the server; it is a local
/// ordering device, not a clock.
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct ReadSeq(pub u64);

impl ReadSeq {
    /// The seq to hand the next read.
    ///
    /// Saturating rather than wrapping: at one read per nanosecond this
    /// exhausts in about 584 years, and wrapping would silently invert the
    /// eviction order, which is worse than freezing it.
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// One row of the `index` store.
///
/// `world_id` is carried per entry even though the OPFS path already encodes
/// it, because eviction has to answer "what does this world cost me" and
/// "which world is the open one" (FR-023) without walking the filesystem.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct IndexEntry {
    /// The fingerprint of the content, and therefore its blob filename.
    pub fingerprint: Fingerprint,
    /// Size of the *plaintext*, which is what the budget is expressed in.
    ///
    /// Not the encrypted size: AES-GCM adds a tag and we prepend a nonce, so
    /// the two differ by a constant. Budgets and server-reported sizes both
    /// speak in plaintext bytes, and mixing the two units would make the
    /// budget quietly wrong by that constant times the item count.
    pub byte_size: u64,
    /// LRU position. See the module docs on why this is not a timestamp.
    pub last_read: ReadSeq,
    /// The world this content was cached for.
    pub world_id: Uuid,
}

impl IndexEntry {
    /// A freshly written entry, counted as read at `seq` — a write is an
    /// acquisition, and an item nobody has touched since fetching it should
    /// not be first out the door on the strength of never having been read.
    pub fn new(fingerprint: Fingerprint, byte_size: u64, world_id: Uuid, seq: ReadSeq) -> Self {
        Self {
            fingerprint,
            byte_size,
            last_read: seq,
            world_id,
        }
    }

    /// Mark this entry as just read.
    pub fn touched(self, seq: ReadSeq) -> Self {
        Self {
            last_read: seq,
            ..self
        }
    }

    /// Whether this entry still describes the fingerprint the server says is
    /// current.
    ///
    /// A mismatch means superseded, and the state machine has only one exit:
    /// the entry becomes `Absent` and is refetched. There is no in-place
    /// update of content under an unchanged identity.
    pub fn is_current(&self, server: &Fingerprint) -> bool {
        self.fingerprint == *server
    }
}

/// The highest seq any entry has been read at, which is the value to count on
/// from after a reload.
///
/// Derived from the entries rather than stored separately, so the counter can
/// never drift out of step with the rows it orders — a separately persisted
/// counter that was written and then lost in a crash would hand out seqs that
/// compare as older than existing rows.
pub fn high_water(entries: &[IndexEntry]) -> ReadSeq {
    entries
        .iter()
        .map(|entry| entry.last_read)
        .max()
        .unwrap_or_default()
}

/// Total plaintext bytes the index accounts for.
pub fn total_bytes(entries: &[IndexEntry]) -> u64 {
    entries.iter().map(|entry| entry.byte_size).sum()
}

/// Entries belonging to one world.
pub fn for_world(entries: &[IndexEntry], world_id: Uuid) -> Vec<IndexEntry> {
    entries
        .iter()
        .copied()
        .filter(|entry| entry.world_id == world_id)
        .collect()
}

/// Item ids in least-recently-read order — the order eviction consumes.
///
/// Ties break on the item id so the order is total and reproducible. A
/// partial order here would make eviction non-deterministic, and a
/// non-deterministic eviction is one that cannot be tested.
pub fn lru_order(entries: &[(ItemId, IndexEntry)]) -> Vec<ItemId> {
    let mut ordered: Vec<_> = entries.to_vec();
    ordered
        .sort_by(|(a_id, a), (b_id, b)| a.last_read.cmp(&b.last_read).then_with(|| a_id.cmp(b_id)));
    ordered.into_iter().map(|(id, _)| id).collect()
}

/// Item ids whose blob is not actually on disk.
///
/// Half of the FR-019 repair: these rows are lies and must be dropped, which
/// turns the item back into `Absent` and therefore refetchable.
pub fn missing_blobs(entries: &[(ItemId, IndexEntry)], on_disk: &[Fingerprint]) -> Vec<ItemId> {
    entries
        .iter()
        .filter(|(_, entry)| !on_disk.contains(&entry.fingerprint))
        .map(|(id, _)| *id)
        .collect()
}

/// Fingerprints on disk that no index row refers to.
///
/// The other half of the repair: bytes nothing can reach. They are safe to
/// delete precisely because content is addressed by fingerprint — an
/// unreferenced blob cannot be some other item's copy under a different name.
pub fn orphaned_blobs(
    entries: &[(ItemId, IndexEntry)],
    on_disk: &[Fingerprint],
) -> Vec<Fingerprint> {
    on_disk
        .iter()
        .filter(|fingerprint| {
            !entries
                .iter()
                .any(|(_, entry)| entry.fingerprint == **fingerprint)
        })
        .copied()
        .collect()
}

/// Split unreferenced blobs into the ones a repair may delete and the ones it
/// must leave alone.
///
/// The second half is the whole point, and it is the T055 rule arriving from
/// the other direction. A blob with no index row is *also* exactly what an
/// in-flight write looks like from outside — `record_fetched` writes the
/// bytes first and the row second — so "unreferenced" on its own is not
/// evidence that anything may be deleted. An unfinished file is never
/// reclaimed, whoever asks.
///
/// Leaving one costs nothing worth counting: an unfinished file is empty, so
/// deleting it would free no space, and it self-heals — the next write of
/// that content targets the same name.
///
/// Returns `(reclaimable, kept)`.
pub fn partition_orphans(
    orphans: &[(Fingerprint, BlobShape)],
) -> (Vec<Fingerprint>, Vec<Fingerprint>) {
    let mut reclaimable = Vec::new();
    let mut kept = Vec::new();
    for (fingerprint, shape) in orphans {
        if shape.is_reclaimable() {
            reclaimable.push(*fingerprint);
        } else {
            kept.push(*fingerprint);
        }
    }
    (reclaimable, kept)
}

#[cfg(target_arch = "wasm32")]
pub use wasm::IndexStore;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use thunderforge_cache_core::ItemId;
    use uuid::Uuid;
    use wasm_bindgen::JsValue;

    use super::{IndexEntry, ReadSeq, high_water};
    use crate::idb::Db;
    use crate::{CacheError, Result, STORE_INDEX};

    /// The `index` object store.
    ///
    /// Holds an in-memory high-water mark for [`ReadSeq`], seeded from the
    /// stored rows on open so that seqs keep increasing across a reload.
    pub struct IndexStore {
        db: Db,
        seq: ReadSeq,
    }

    impl IndexStore {
        /// Open the store and recover the read counter.
        pub async fn open() -> Result<Self> {
            let db = Db::open().await?;
            let mut store = Self {
                db,
                seq: ReadSeq::default(),
            };
            let entries: Vec<IndexEntry> = store.all().await?.into_iter().map(|(_, e)| e).collect();
            store.seq = high_water(&entries);
            Ok(store)
        }

        /// Allocate the next read seq.
        pub fn tick(&mut self) -> ReadSeq {
            self.seq = self.seq.next();
            self.seq
        }

        /// Look one item up.
        pub async fn get(&self, id: ItemId) -> Result<Option<IndexEntry>> {
            let Some(value) = self.db.get(STORE_INDEX, &id.to_wire()).await? else {
                return Ok(None);
            };
            decode(&value).map(Some)
        }

        /// Record (or replace) what is held for an item.
        pub async fn put(&self, id: ItemId, entry: &IndexEntry) -> Result<()> {
            let encoded =
                serde_json::to_string(entry).map_err(|err| CacheError::Corrupt(err.to_string()))?;
            self.db
                .put(STORE_INDEX, &id.to_wire(), &JsValue::from_str(&encoded))
                .await
        }

        /// Note that an item was just read, moving it to the back of the LRU
        /// queue. A no-op if the item is not indexed — a read of something we
        /// have no row for is a miss, not a bug.
        pub async fn touch(&mut self, id: ItemId) -> Result<()> {
            let Some(entry) = self.get(id).await? else {
                return Ok(());
            };
            let seq = self.tick();
            self.put(id, &entry.touched(seq)).await
        }

        /// Forget one item. Absent is success.
        pub async fn remove(&self, id: ItemId) -> Result<()> {
            self.db.delete(STORE_INDEX, &id.to_wire()).await
        }

        /// Every row. The repair pass and the budget both need the whole set;
        /// nothing in this feature queries the index by anything but item id,
        /// which is why R2 chose a key-value store over SQLite.
        pub async fn all(&self) -> Result<Vec<(ItemId, IndexEntry)>> {
            let mut out = Vec::new();
            for (key, value) in self.db.entries(STORE_INDEX).await? {
                // A key we cannot parse is not ours. Skipping is safer than
                // failing the whole listing, which would take the cache down
                // over one bad row.
                let Some(id) = ItemId::from_wire(&key) else {
                    continue;
                };
                if let Ok(entry) = decode(&value) {
                    out.push((id, entry));
                }
            }
            Ok(out)
        }

        /// Rows for one world.
        pub async fn for_world(&self, world_id: Uuid) -> Result<Vec<(ItemId, IndexEntry)>> {
            Ok(self
                .all()
                .await?
                .into_iter()
                .filter(|(_, entry)| entry.world_id == world_id)
                .collect())
        }

        /// Drop every row for a world, alongside `OpfsStore::remove_world`.
        pub async fn remove_world(&self, world_id: Uuid) -> Result<()> {
            for (id, _) in self.for_world(world_id).await? {
                self.remove(id).await?;
            }
            Ok(())
        }

        /// Empty the index. Paired with discarding the key on sign-out: the
        /// rows disclose which worlds and items a user held, so they go too.
        pub async fn clear(&self) -> Result<()> {
            self.db.clear(STORE_INDEX).await
        }
    }

    fn decode(value: &JsValue) -> Result<IndexEntry> {
        let text = value
            .as_string()
            .ok_or_else(|| CacheError::Corrupt("index row was not a string".into()))?;
        serde_json::from_str(&text).map_err(|err| CacheError::Corrupt(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(byte: u8) -> Fingerprint {
        Fingerprint::of_bytes(&[byte])
    }

    fn entry(byte: u8, size: u64, seq: u64, world: u128) -> IndexEntry {
        IndexEntry::new(fp(byte), size, Uuid::from_u128(world), ReadSeq(seq))
    }

    #[test]
    fn read_seq_is_monotonic() {
        let mut seq = ReadSeq::default();
        let mut seen = Vec::new();
        for _ in 0..5 {
            seq = seq.next();
            seen.push(seq);
        }
        assert_eq!(
            seen,
            vec![ReadSeq(1), ReadSeq(2), ReadSeq(3), ReadSeq(4), ReadSeq(5)]
        );
    }

    #[test]
    fn read_seq_saturates_rather_than_inverting_the_order() {
        assert_eq!(ReadSeq(u64::MAX).next(), ReadSeq(u64::MAX));
    }

    #[test]
    fn high_water_survives_a_reload() {
        let entries = [entry(1, 10, 4, 1), entry(2, 10, 9, 1), entry(3, 10, 2, 1)];
        assert_eq!(high_water(&entries), ReadSeq(9));
        assert_eq!(high_water(&entries).next(), ReadSeq(10));
    }

    #[test]
    fn high_water_of_an_empty_index_is_zero() {
        assert_eq!(high_water(&[]), ReadSeq(0));
    }

    #[test]
    fn touching_moves_an_entry_to_the_back_of_the_queue() {
        let before = entry(1, 10, 3, 1);
        let after = before.touched(ReadSeq(11));
        assert_eq!(after.last_read, ReadSeq(11));
        assert_eq!(after.fingerprint, before.fingerprint);
        assert_eq!(after.byte_size, before.byte_size);
        assert_eq!(after.world_id, before.world_id);
    }

    #[test]
    fn a_new_entry_is_not_immediately_the_lru_victim() {
        let old = entry(1, 10, 1, 1);
        let fresh = IndexEntry::new(fp(2), 10, Uuid::from_u128(1), ReadSeq(7));
        assert!(fresh.last_read > old.last_read);
    }

    #[test]
    fn supersession_is_detected_by_fingerprint() {
        let held = entry(1, 10, 1, 1);
        assert!(held.is_current(&fp(1)));
        assert!(!held.is_current(&fp(2)));
    }

    #[test]
    fn total_bytes_sums_plaintext_sizes() {
        assert_eq!(total_bytes(&[entry(1, 10, 1, 1), entry(2, 32, 1, 2)]), 42);
        assert_eq!(total_bytes(&[]), 0);
    }

    #[test]
    fn entries_partition_by_world() {
        let entries = [entry(1, 1, 1, 1), entry(2, 1, 1, 2), entry(3, 1, 1, 1)];
        assert_eq!(for_world(&entries, Uuid::from_u128(1)).len(), 2);
        assert_eq!(for_world(&entries, Uuid::from_u128(3)).len(), 0);
    }

    #[test]
    fn lru_order_is_total_and_deterministic() {
        let a = ItemId::CanvasAsset(Uuid::from_u128(1));
        let b = ItemId::CanvasAsset(Uuid::from_u128(2));
        let c = ItemId::SceneState(Uuid::from_u128(3));
        // `a` and `b` tie on last_read; the item id must break it the same
        // way every time.
        let rows = vec![
            (c, entry(3, 1, 9, 1)),
            (b, entry(2, 1, 1, 1)),
            (a, entry(1, 1, 1, 1)),
        ];
        assert_eq!(lru_order(&rows), vec![a, b, c]);

        let mut shuffled = rows;
        shuffled.reverse();
        assert_eq!(lru_order(&shuffled), vec![a, b, c]);
    }

    #[test]
    fn repair_finds_rows_whose_blob_is_gone() {
        let a = ItemId::CanvasAsset(Uuid::from_u128(1));
        let b = ItemId::CanvasAsset(Uuid::from_u128(2));
        let rows = vec![(a, entry(1, 1, 1, 1)), (b, entry(2, 1, 1, 1))];
        assert_eq!(missing_blobs(&rows, &[fp(1)]), vec![b]);
        assert_eq!(missing_blobs(&rows, &[fp(1), fp(2)]), Vec::<ItemId>::new());
    }

    #[test]
    fn a_repair_never_reclaims_a_file_nobody_finished_writing() {
        // The T055 rule, arriving from the repair side. An unreferenced blob
        // is indistinguishable from a write whose index row has not landed
        // yet, so the shape is what decides — not the missing row.
        let (reclaimable, kept) =
            partition_orphans(&[(fp(1), BlobShape::Complete), (fp(2), BlobShape::Incomplete)]);
        assert_eq!(reclaimable, vec![fp(1)], "a finished orphan is dead weight");
        assert_eq!(
            kept,
            vec![fp(2)],
            "an unfinished orphan may be another tab's write in progress"
        );
    }

    #[test]
    fn a_repair_with_nothing_to_do_deletes_nothing() {
        let (reclaimable, kept) = partition_orphans(&[]);
        assert!(reclaimable.is_empty());
        assert!(kept.is_empty());
    }

    #[test]
    fn repair_finds_blobs_no_row_refers_to() {
        let a = ItemId::CanvasAsset(Uuid::from_u128(1));
        let rows = vec![(a, entry(1, 1, 1, 1))];
        assert_eq!(orphaned_blobs(&rows, &[fp(1), fp(5)]), vec![fp(5)]);
    }

    #[test]
    fn deduplicated_content_is_not_reported_as_orphaned() {
        // Two items sharing one blob: dropping either row must not make the
        // shared file look unreferenced.
        let a = ItemId::CanvasAsset(Uuid::from_u128(1));
        let b = ItemId::SceneState(Uuid::from_u128(2));
        let shared = entry(7, 1, 1, 1);
        let rows = vec![(a, shared), (b, shared)];
        assert_eq!(orphaned_blobs(&rows, &[fp(7)]), Vec::<Fingerprint>::new());
    }

    #[test]
    fn entries_round_trip_through_json() {
        // The wasm store persists rows as JSON strings; a silent shape change
        // would empty every cache on upgrade.
        let original = entry(3, 4096, 12, 99);
        let text = serde_json::to_string(&original).expect("serializable");
        let decoded: IndexEntry = serde_json::from_str(&text).expect("round trip");
        assert_eq!(decoded, original);
    }
}
