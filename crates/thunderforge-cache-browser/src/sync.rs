//! Delta sync: ask the server what changed, act on the answer.
//!
//! Spec 028 T028, ADR-052. **Engine-driven by decision**: the manifest lives
//! here, in Rust, alongside the index that produces it — so the request is
//! built and the plan applied on this side of the WASM boundary rather than
//! in TypeScript.
//!
//! The alternative was orchestrating from TS and exposing `manifest()` /
//! `apply_plan()` through `wasm_bindgen`, which would have matched how
//! `apply_world_command` already works. It was rejected because TS would then
//! hold, even briefly, a second account of what is cached — and Constitution
//! Principle I exists to stop exactly that. Cache policy has one owner.
//!
//! TypeScript still triggers a sync and observes the result; it just never
//! decides anything.

use std::collections::BTreeMap;

use thunderforge_cache_core::delta::SyncPlan;
use thunderforge_cache_core::manifest::Manifest;
use thunderforge_cache_core::{Fingerprint, ItemId};
use uuid::Uuid;

use crate::index::IndexEntry;

/// The GraphQL document. Kept as a constant so the wire shape sits next to
/// the code that parses the reply, rather than being assembled at a call site
/// where a field could be added to one and not the other.
pub const WORLD_SYNC_PLAN_QUERY: &str = r"
query($worldId: UUID!, $held: [HeldItemInput!]!) {
  worldSyncPlan(worldId: $worldId, held: $held) {
    fetch { id fingerprint byteSize peerAvailable }
    evict
    canonicalVersion
  }
}";

/// Build the manifest to send for one world.
///
/// Pure: takes what the index already returned rather than reading it, so the
/// shape of a request is testable natively without a browser or a database.
pub fn manifest_for_world(world_id: Uuid, entries: &[(ItemId, IndexEntry)]) -> Manifest {
    let mut manifest = Manifest::new(world_id);
    for (id, entry) in entries {
        if entry.world_id == world_id {
            manifest.insert(*id, entry.fingerprint);
        }
    }
    manifest
}

/// What a sync produced, for the caller to act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncOutcome {
    pub plan: SyncPlan,
    /// The server's canonical-serialization version. A mismatch against what
    /// produced our stored scene fingerprints invalidates every one of them,
    /// so this is checked rather than assumed.
    pub canonical_version: u32,
}

/// Why a sync could not be completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    /// The request never reached the server, or the reply never arrived.
    Transport(String),
    /// The server answered with GraphQL errors.
    Server(String),
    /// The reply did not match the contract.
    Malformed(String),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(m) => write!(f, "sync transport failed: {m}"),
            Self::Server(m) => write!(f, "server rejected sync: {m}"),
            Self::Malformed(m) => write!(f, "malformed sync plan: {m}"),
        }
    }
}

impl std::error::Error for SyncError {}

/// Parse a `worldSyncPlan` reply.
///
/// Separated from the fetch so the contract's shape — including its failure
/// modes — is exercised natively. Every malformed field is an error rather
/// than a skipped entry: a plan that silently loses items would leave the
/// client believing it is current when it is not, which is the one failure
/// this whole feature must never produce.
pub fn parse_sync_plan(body: &str) -> Result<SyncOutcome, SyncError> {
    let root: serde_json::Value =
        serde_json::from_str(body).map_err(|e| SyncError::Malformed(e.to_string()))?;

    if let Some(errors) = root.get("errors").and_then(|e| e.as_array())
        && !errors.is_empty()
    {
        return Err(SyncError::Server(errors[0].to_string()));
    }

    let plan_json = root
        .pointer("/data/worldSyncPlan")
        .ok_or_else(|| SyncError::Malformed("missing data.worldSyncPlan".into()))?;

    let canonical_version = plan_json
        .get("canonicalVersion")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| SyncError::Malformed("missing canonicalVersion".into()))?
        as u32;

    let mut plan = SyncPlan::default();

    for item in plan_json
        .get("fetch")
        .and_then(|f| f.as_array())
        .ok_or_else(|| SyncError::Malformed("fetch is not a list".into()))?
    {
        let id = item
            .get("id")
            .and_then(serde_json::Value::as_str)
            .and_then(ItemId::from_wire)
            .ok_or_else(|| SyncError::Malformed(format!("bad fetch id in {item}")))?;
        let fingerprint = item
            .get("fingerprint")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| SyncError::Malformed(format!("missing fingerprint in {item}")))?;
        let fingerprint = Fingerprint::from_hex(fingerprint)
            .map_err(|e| SyncError::Malformed(format!("bad fingerprint: {e}")))?;
        let byte_size = item
            .get("byteSize")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        plan.fetch.push(thunderforge_cache_core::delta::PlanItem {
            id,
            fingerprint,
            byte_size,
        });
    }

    for id in plan_json
        .get("evict")
        .and_then(|e| e.as_array())
        .ok_or_else(|| SyncError::Malformed("evict is not a list".into()))?
    {
        let id = id
            .as_str()
            .and_then(ItemId::from_wire)
            .ok_or_else(|| SyncError::Malformed(format!("bad evict id {id}")))?;
        plan.evict.push(id);
    }

    Ok(SyncOutcome {
        plan,
        canonical_version,
    })
}

/// The request body for one world's manifest.
pub fn sync_request_body(manifest: &Manifest) -> String {
    let held: Vec<BTreeMap<&str, String>> = manifest
        .to_wire()
        .into_iter()
        .map(|h| {
            let mut m = BTreeMap::new();
            m.insert("id", h.id.to_wire());
            m.insert("fingerprint", h.fingerprint.to_hex());
            m
        })
        .collect();

    serde_json::json!({
        "query": WORLD_SYNC_PLAN_QUERY,
        "variables": { "worldId": manifest.world_id.to_string(), "held": held },
    })
    .to_string()
}

#[cfg(target_arch = "wasm32")]
pub use wasm::{
    ApplyOutcome, apply_plan, canvas_fingerprints, manifest_for_open_world, record_fetched,
};

/// The browser half of a sync: reading the manifest out of the index, and
/// putting the server's answer into effect.
///
/// This is where "policy has one owner" is actually enforced. Everything
/// above is pure and decides *what* the client holds and *what* the server
/// said; everything here performs it against OPFS and IndexedDB. Neither
/// half is reachable from TypeScript, which is the point of R1 — TS may ask
/// for a sync and read the summary, and that is the whole of its
/// involvement.
#[cfg(target_arch = "wasm32")]
mod wasm {
    use thunderforge_cache_core::delta::SyncPlan;
    use thunderforge_cache_core::manifest::Manifest;
    use thunderforge_cache_core::{Fingerprint, ItemId};
    use uuid::Uuid;

    use crate::Result;
    use crate::crypto::SessionKey;
    use crate::index::{IndexEntry, IndexStore};
    use crate::locks;
    use crate::opfs::OpfsStore;

    /// The manifest to send for the world being opened.
    ///
    /// An index we cannot read yields an *empty* manifest rather than an
    /// error, and an empty manifest is the cold-start case the contract
    /// already specifies: the server returns a full plan. So a broken index
    /// costs bandwidth, never correctness — it can never make the client
    /// believe it holds something it does not.
    pub async fn manifest_for_open_world(index: &IndexStore, world_id: Uuid) -> Manifest {
        let entries = index.for_world(world_id).await.unwrap_or_default();
        super::manifest_for_world(world_id, &entries)
    }

    /// What applying a plan actually managed to do.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
    pub struct ApplyOutcome {
        /// Items whose index row was dropped.
        pub evicted: usize,
        /// Blob files deleted. Lower than `evicted` when two items shared
        /// one blob, or when a row referred to a blob already gone.
        pub blobs_removed: usize,
        /// Evictions that hit a platform error. Reported rather than
        /// swallowed, because a failed eviction is the one failure in this
        /// path with a permissions consequence (FR-015).
        pub failed: usize,
    }

    /// Put the server's plan into effect: discard what it evicted.
    ///
    /// Only the eviction half is performed here. `fetch` is deliberately not
    /// acted on at this layer — obtaining bytes needs an HTTP client and a
    /// URL scheme, neither of which belongs to a storage crate — so the
    /// caller fetches and calls [`record_fetched`] per item.
    ///
    /// **Blobs are deleted only when nothing else refers to them.** Content
    /// is addressed by fingerprint, so two items in a world can legitimately
    /// share one file (`opfs.rs`, "identical content is stored once");
    /// deleting on the strength of one row would silently take the other
    /// item's bytes with it. The index row goes first regardless, because
    /// that is what makes the item unreachable — which is the property
    /// revocation actually needs.
    pub async fn apply_plan(
        store: &OpfsStore,
        index: &IndexStore,
        world_id: Uuid,
        plan: &SyncPlan,
    ) -> ApplyOutcome {
        // FR-021c. Held for the whole pass, and released when this returns.
        //
        // Without it, a second tab that has just fetched and stored an item
        // can have its blob deleted out from under it here: this tab decided
        // what to evict from a manifest taken before that fetch existed. The
        // damage is repairable (FR-018/FR-019 notice the missing blob and
        // refetch) but it is a wasted round trip in exactly the situation the
        // cache is supposed to make fast.
        //
        // Not getting the lock is not a reason to skip the eviction. An
        // eviction that never runs leaves content the server said we should
        // no longer hold, and FR-015 cares about that far more than about a
        // duplicated fetch. So this degrades to today's behaviour — the race
        // is back, and it was always survivable (FR-021d).
        let _lock = locks::acquire_exclusive(
            &locks::world_sync_lock(world_id),
            locks::WORLD_LOCK_TIMEOUT_MS,
        )
        .await;

        let mut outcome = ApplyOutcome::default();

        for id in &plan.evict {
            let fingerprint = match index.get(*id).await {
                Ok(Some(entry)) => Some(entry.fingerprint),
                // Nothing indexed under this id: already absent, which is
                // the postcondition. Not a failure.
                Ok(None) => None,
                Err(_) => {
                    outcome.failed += 1;
                    continue;
                }
            };

            if index.remove(*id).await.is_err() {
                outcome.failed += 1;
                continue;
            }
            outcome.evicted += 1;

            let Some(fingerprint) = fingerprint else {
                continue;
            };
            if still_referenced(index, world_id, &fingerprint).await {
                continue;
            }
            match store.remove_blob(world_id, &fingerprint).await {
                Ok(()) => outcome.blobs_removed += 1,
                Err(_) => outcome.failed += 1,
            }
        }

        outcome
    }

    /// Whether any surviving row in this world still points at a blob.
    ///
    /// Errs on the side of keeping the file: an index we cannot read is a
    /// reason to leave bytes alone, not to delete another item's content.
    /// The bytes are encrypted and unreferenced-but-present is a state the
    /// FR-019 repair pass already collects.
    async fn still_referenced(
        index: &IndexStore,
        world_id: Uuid,
        fingerprint: &Fingerprint,
    ) -> bool {
        match index.for_world(world_id).await {
            Ok(rows) => rows
                .iter()
                .any(|(_, entry)| entry.fingerprint == *fingerprint),
            Err(_) => true,
        }
    }

    /// Store bytes obtained for a planned fetch, and index them.
    ///
    /// `write_blob` verifies against `fingerprint` before anything is
    /// encrypted, so this cannot file bytes that are not what the server
    /// promised — the check is not repeated here precisely so there is only
    /// one of it.
    pub async fn record_fetched(
        store: &OpfsStore,
        index: &mut IndexStore,
        key: &SessionKey,
        world_id: Uuid,
        id: ItemId,
        fingerprint: &Fingerprint,
        bytes: &[u8],
    ) -> Result<()> {
        // The other half of FR-021c: the same lock [`apply_plan`] takes, so
        // a blob and its index row land together rather than straddling
        // another tab's eviction pass. Short-waited on purpose — there is an
        // asset waiting behind this, and a write that goes ahead unlocked is
        // no worse than every write was before this lock existed.
        let _lock = locks::acquire_exclusive(
            &locks::world_sync_lock(world_id),
            locks::WRITE_LOCK_TIMEOUT_MS,
        )
        .await;

        store.write_blob(world_id, fingerprint, bytes, key).await?;
        let seq = index.tick();
        index
            .put(
                id,
                &IndexEntry::new(*fingerprint, bytes.len() as u64, world_id, seq),
            )
            .await
    }

    /// The canvas assets this client currently believes it holds for a
    /// world, as `(asset id, fingerprint)`.
    ///
    /// The engine's read path needs a server-promised fingerprint per asset
    /// before it will consult the cache at all, and after a sync the index
    /// *is* that promise: rows the server did not contradict are current by
    /// the contract's rule that silence means unchanged.
    pub async fn canvas_fingerprints(
        index: &IndexStore,
        world_id: Uuid,
    ) -> Vec<(Uuid, Fingerprint)> {
        index
            .for_world(world_id)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(id, entry)| match id {
                ItemId::CanvasAsset(asset_id) => Some((asset_id, entry.fingerprint)),
                ItemId::SceneState(_) => None,
            })
            .collect()
    }
}
