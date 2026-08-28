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
    /// Whether anyone else is live in this world right now (T086's
    /// `peerAvailable`).
    ///
    /// **Reachability, not holdings.** It says a peer exists, never that any
    /// peer has the bytes — so it may be used to skip asking when this client
    /// is alone, and a `false` must never suppress a server fetch. Reported
    /// rather than acted on here: peer transfer already falls back to the
    /// server whenever no channel is open, so gating on this would only add
    /// a second way to say the same thing, and one that goes stale the
    /// moment somebody joins mid-session.
    pub peer_available: bool,
}

/// Why a sync could not be completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    /// The request never reached the server, or the reply never arrived.
    Transport(String),
    /// The server answered, and refused this caller access to the world
    /// (FR-014/FR-015). Categorically different from every other variant:
    /// this is the only one that means "you have lost access", and so the
    /// only one that justifies discarding a world's cached content.
    ///
    /// See [`is_authorization_refusal`] for exactly what earns this variant
    /// — deliberately a machine-readable `extensions.code`, never the
    /// human-readable message.
    Forbidden(String),
    /// The server answered with GraphQL errors.
    Server(String),
    /// The reply did not match the contract.
    Malformed(String),
}

/// The `extensions.code` `worldSyncPlan` attaches when it refuses a caller.
///
/// Set by `to_graphql_error` in `src/server/src/graphql/queries/world_sync_plan.rs`
/// and asserted by that module's own tests, so it is part of the contract
/// rather than an accident of formatting.
const FORBIDDEN_CODE: &str = "FORBIDDEN";

/// The root field this client asks for. Used only to confirm a refusal is
/// about the world we asked about.
const SYNC_PLAN_FIELD: &str = "worldSyncPlan";

/// Whether a GraphQL `errors` array says *this caller may not have this
/// world*, as opposed to anything else that can go wrong.
///
/// # Why the code, and not the message
///
/// The message is deliberately ambiguous — `NOT_A_MEMBER` is worded
/// identically whether the world exists or not, precisely so a non-member
/// learns nothing (FR-014, FR-047). Wording chosen for non-disclosure is
/// wording nobody promised to keep stable, and matching a substring of it
/// would mean a copy-edit on the server silently turns revocation back into
/// the bug this function exists to fix. `extensions.code` is the field the
/// server sets on purpose, for machines.
///
/// # Why the failure direction is "keep the cache"
///
/// Anything unrecognised — a bare error, a different code, a reply that
/// never arrived — is *not* a refusal. Discarding on a transient failure
/// would throw away a user's whole cache every time their wifi dropped,
/// which is strictly worse than holding bytes a moment too long: the server
/// and the byte route have already stopped serving them (FR-014), so the
/// held copy is unusable in the meantime, and the next successful sync
/// discards it. So this answers "yes" only on positive evidence.
///
/// The `path` check is belt-and-braces. This client sends exactly one root
/// field for exactly one world, so a `FORBIDDEN` in the reply cannot be
/// about anything else; requiring the path to name that field when a path is
/// present makes that reasoning explicit rather than assumed.
pub fn is_authorization_refusal(errors: &[serde_json::Value]) -> bool {
    errors.iter().any(|err| {
        let code_matches = err
            .pointer("/extensions/code")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|code| code == FORBIDDEN_CODE);
        if !code_matches {
            return false;
        }
        match err.get("path").and_then(|p| p.as_array()) {
            Some(path) => path
                .first()
                .and_then(serde_json::Value::as_str)
                .is_some_and(|field| field == SYNC_PLAN_FIELD),
            // No path at all: a request-level error on a request that only
            // ever asks about one world.
            None => true,
        }
    })
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(m) => write!(f, "sync transport failed: {m}"),
            Self::Forbidden(m) => write!(f, "server refused access to this world: {m}"),
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
        return Err(if is_authorization_refusal(errors) {
            SyncError::Forbidden(errors[0].to_string())
        } else {
            SyncError::Server(errors[0].to_string())
        });
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
    let mut peer_available = false;

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
        peer_available |= item
            .get("peerAvailable")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

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
        peer_available,
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
    ApplyOutcome, BudgetOutcome, DiscardOutcome, RepairOutcome, apply_plan, canvas_fingerprints,
    discard_world, enforce_budget, manifest_for_open_world, record_fetched, repair_world,
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
    use std::collections::BTreeMap;

    use wasm_bindgen::JsCast as _;

    use thunderforge_cache_core::budget;
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

    /// What discarding a refused world managed to do.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
    pub struct DiscardOutcome {
        /// Index rows this world had when the discard began — the number
        /// dropped, when `failed` is zero.
        pub rows: usize,
        /// Whether the index rows are gone.
        pub index_cleared: bool,
        /// Whether the world's blob directory is gone.
        pub blobs_cleared: bool,
    }

    impl DiscardOutcome {
        /// Both halves succeeded, so nothing for this world remains.
        #[must_use]
        pub fn complete(&self) -> bool {
            self.index_cleared && self.blobs_cleared
        }
    }

    /// Discard everything held for one world, because the server refused it.
    ///
    /// The counterpart to [`apply_plan`] for the case the plan never
    /// arrives. A per-item `evict` list can only discard items the server
    /// was willing to talk about; when the answer is "you may not have this
    /// world at all" there is no list, and FR-015 still requires the bytes
    /// to go. Whole-world revocation is therefore its own path rather than a
    /// plan with everything in it.
    ///
    /// **Scoped to `world_id` and nothing else.** Both calls below are
    /// per-world — `IndexStore::remove_world` filters rows by `world_id` and
    /// `OpfsStore::remove_world` removes that one directory — so a refusal
    /// for one world can never take another world's content with it, which
    /// matters because a user may be a member of many and lose one.
    ///
    /// **Never returns an error.** A discard that cannot complete is
    /// reported in the outcome and left to the sign-out reclamation and the
    /// FR-019 repair pass; raising here would turn a cleanup failure into a
    /// failure to open a world, and the caller
    /// (`sync_world_cache`) is contractually incapable of throwing.
    ///
    /// Ordering mirrors `apply_plan`: index rows first, because that is what
    /// makes the content unreachable, then the bytes.
    pub async fn discard_world(
        store: &OpfsStore,
        index: &IndexStore,
        world_id: Uuid,
    ) -> DiscardOutcome {
        // FR-021c: the same per-world lock `apply_plan` and `record_fetched`
        // take, so another tab's in-flight fetch cannot interleave a fresh
        // blob and index row into the middle of this pass and leave content
        // behind for a world we have just been refused.
        //
        // As in `apply_plan`, failing to get the lock is not a reason to
        // skip: content the server says we may no longer have outweighs a
        // race that was always survivable (FR-021d).
        let _lock = locks::acquire_exclusive(
            &locks::world_sync_lock(world_id),
            locks::WORLD_LOCK_TIMEOUT_MS,
        )
        .await;

        let rows = index.for_world(world_id).await.map_or(0, |rows| rows.len());

        DiscardOutcome {
            rows,
            index_cleared: index.remove_world(world_id).await.is_ok(),
            // Attempted whether or not the index rows went: bytes on disk
            // are the thing FR-015 is about, and an index we could not write
            // is no reason to leave them.
            blobs_cleared: store.remove_world(world_id).await.is_ok(),
        }
    }

    /// What a repair pass found and put right.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
    pub struct RepairOutcome {
        /// Index rows dropped because the blob they name is not on disk.
        pub rows_dropped: usize,
        /// Complete blob files deleted because no row referred to them.
        pub blobs_reclaimed: usize,
        /// Unreferenced files left alone because they are not finished, and
        /// so may be another tab's write in progress.
        pub unfinished_kept: usize,
        /// Steps that hit a platform error. Reported rather than swallowed —
        /// a repair that silently failed would leave the store diverging and
        /// look like it had converged.
        pub failed: usize,
    }

    /// Reconcile the index against what is actually on disk (FR-019).
    ///
    /// The two stores drift for ordinary reasons and always will: a blob is
    /// written before its index row (`record_fetched`), a row is removed
    /// before its blob (`apply_plan`), and a tab can be closed between any
    /// two awaits. `index.rs` states the rule for resolving it — **where
    /// they differ, OPFS wins** — and the pure halves of the diff
    /// (`missing_blobs`, `orphaned_blobs`) have been sitting there tested and
    /// uncalled. This is the caller.
    ///
    /// Both directions are repaired, and they are not symmetrical:
    ///
    /// - A row naming a blob that is gone is a **lie**. It makes the client
    ///   tell the server "I hold this" in its manifest, and the server then
    ///   says nothing about it, because silence means unchanged. The item is
    ///   then never fetched and never displayed from cache. Dropping the row
    ///   turns it back into `Absent`, which is refetchable.
    /// - A blob no row names is **unreachable bytes**. Nothing can read it,
    ///   and it counts against the space budget forever.
    ///
    /// # Why an unfinished file is never reclaimed here
    ///
    /// A blob that exists without an index row is exactly what an in-flight
    /// write looks like from the outside — `record_fetched` writes the bytes
    /// first and the row second. Deleting on that evidence alone would
    /// reintroduce, from the repair side, the bug T055 fixed on the read
    /// side.
    ///
    /// Two things keep it safe. The world lock, which every writer also
    /// takes, so a *locked* write cannot be interleaved with this pass. And
    /// the shape check: a file that is not finished is never deleted, whether
    /// or not the lock was granted. That costs nothing to leave — an
    /// unfinished file is zero bytes, so reclaiming it would free nothing —
    /// and it self-heals, because the next write of that content targets the
    /// same name.
    ///
    /// The residual is a *complete* orphan belonging to a writer that was
    /// refused the lock (it is best-effort, 250ms) and has not yet written
    /// its row. Deleting it costs that tab one re-fetch and nothing else,
    /// which is the same trade `apply_plan` already documents.
    ///
    /// Never fails as a whole: every step is independently recoverable, and
    /// a repair that could not run leaves the store exactly as divergent as
    /// it found it, which is where it started.
    pub async fn repair_world(
        store: &OpfsStore,
        index: &IndexStore,
        world_id: Uuid,
    ) -> RepairOutcome {
        let _lock = locks::acquire_exclusive(
            &locks::world_sync_lock(world_id),
            locks::WORLD_LOCK_TIMEOUT_MS,
        )
        .await;

        let mut outcome = RepairOutcome::default();

        let Ok(entries) = index.for_world(world_id).await else {
            // An index we cannot read is not evidence that anything on disk
            // is unreferenced. Deleting on the strength of it would be
            // deleting the whole world.
            outcome.failed += 1;
            return outcome;
        };
        let Ok(on_disk) = store.list_fingerprints(world_id).await else {
            outcome.failed += 1;
            return outcome;
        };

        // `on_disk` is every file present, finished or not, and this half
        // deliberately does not ask which. A row pointing at an *unfinished*
        // file is technically a lie too — the item cannot be read — but
        // reaching that verdict would cost a `getFile()` per blob per open,
        // on every world, to catch a state that essentially does not occur:
        // rewriting an existing blob does not truncate it (the writable
        // buffers into a swap file), so a row and an empty file together
        // require a truncation nothing here performs. The orphan half below
        // pays for shapes because orphans are few.
        for id in crate::index::missing_blobs(&entries, &on_disk) {
            match index.remove(id).await {
                Ok(()) => outcome.rows_dropped += 1,
                Err(_) => outcome.failed += 1,
            }
        }

        // Shapes first, then one pure decision over them — the same split
        // the rest of this crate uses, and what lets the rule that actually
        // matters here be tested without a browser.
        let mut shaped = Vec::new();
        for fingerprint in crate::index::orphaned_blobs(&entries, &on_disk) {
            match store.blob_shape(world_id, &fingerprint).await {
                Ok(shape) => shaped.push((fingerprint, shape)),
                Err(_) => outcome.failed += 1,
            }
        }
        let (reclaimable, kept) = crate::index::partition_orphans(&shaped);
        outcome.unfinished_kept = kept.len();
        for fingerprint in reclaimable {
            match store.remove_blob(world_id, &fingerprint).await {
                Ok(()) => outcome.blobs_reclaimed += 1,
                Err(_) => outcome.failed += 1,
            }
        }

        outcome
    }

    /// What a budget pass found and released.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
    pub struct BudgetOutcome {
        /// Bytes the browser says are available to this origin, halved and
        /// capped by [`budget::limit_bytes`]. Zero when the platform will not
        /// say, which is treated as "do not evict", not as "no room".
        pub limit_bytes: u64,
        /// Plaintext bytes the index accounts for, before this pass.
        pub in_use_bytes: u64,
        /// Index rows released.
        pub evicted: usize,
        /// Blob files deleted. Lower than `evicted` when two items share
        /// content, since the file goes only with the last row naming it.
        pub blobs_removed: usize,
        /// Steps that hit a platform error, reported rather than swallowed.
        pub failed: usize,
        /// Even releasing everything permissible leaves too little room. The
        /// caller fetches without storing (FR-024) — degraded, never failed.
        pub insufficient: bool,
        /// The platform declined to estimate, so no plan was made.
        pub unknown_quota: bool,
    }

    /// Recompute the budget against the browser's *current* quota and release
    /// what no longer fits (FR-022, FR-023).
    ///
    /// # Why this runs on every world open
    ///
    /// The quota is not a constant and is not ours. `navigator.storage
    /// .estimate()` answers from whatever the browser presently thinks the
    /// origin may have, and that figure moves — the disk fills, the user
    /// clears other sites, the browser revises its per-origin share, or the
    /// profile moves to a smaller machine. A budget computed once at install
    /// would be a number about a machine that no longer exists.
    ///
    /// So the limit is derived fresh each open and the store is **shrunk**
    /// when the quota has dropped. The asymmetry is deliberate: growing needs
    /// no action, because a larger limit simply admits the next write, while
    /// shrinking needs eviction or the store sits permanently over a limit
    /// nothing will ever bring it under.
    ///
    /// # Why a refused estimate evicts nothing
    ///
    /// `estimate()` is absent in some contexts and can reject in others. The
    /// tempting reading of "no answer" is zero, and zero would mean a limit
    /// of zero, and a limit of zero means evict everything the user has —
    /// destroying a working cache because a diagnostic API was unavailable.
    /// `unknown_quota` says so and the pass does nothing, which leaves the
    /// store exactly as it was: possibly over an unknown limit, which the
    /// next successful estimate corrects.
    ///
    /// # Eviction is per victim world, not per open world
    ///
    /// Unlike [`apply_plan`], what this releases belongs to *other* worlds by
    /// construction — FR-023 forbids touching the open one. Their blobs live
    /// in their own directories and their writers take their own locks, so
    /// the victims are grouped by world and each group is released under that
    /// world's lock. Taking the open world's lock here would serialise
    /// against the wrong tab entirely and protect nothing.
    pub async fn enforce_budget(
        store: &OpfsStore,
        index: &IndexStore,
        open_world: Uuid,
        incoming_bytes: u64,
    ) -> BudgetOutcome {
        let mut outcome = BudgetOutcome::default();

        let Some(quota) = storage_quota().await else {
            outcome.unknown_quota = true;
            return outcome;
        };
        let limit = budget::limit_bytes(quota);
        outcome.limit_bytes = limit;

        let Ok(rows) = index.all().await else {
            outcome.failed += 1;
            return outcome;
        };
        let entries = crate::index::budget_entries(&rows);
        let plan = budget::plan_eviction(&entries, limit, incoming_bytes, open_world);
        outcome.in_use_bytes = plan.in_use_bytes;
        outcome.insufficient = plan.insufficient;

        if plan.evict.is_empty() {
            return outcome;
        }

        // Group first, then take one lock per world. `plan.evict` is ordered
        // by the planner and that order is not grouped, so evicting in it
        // directly would mean acquiring and releasing the same world's lock
        // repeatedly, with other tabs free to interleave in the gaps.
        let mut by_world: BTreeMap<Uuid, Vec<ItemId>> = BTreeMap::new();
        for id in &plan.evict {
            let Ok(Some(entry)) = index.get(*id).await else {
                // The row went between planning and here — another tab, or a
                // repair pass. Already absent is the postcondition.
                continue;
            };
            by_world.entry(entry.world_id).or_default().push(*id);
        }

        for (world_id, ids) in by_world {
            let _lock = locks::acquire_exclusive(
                &locks::world_sync_lock(world_id),
                locks::WORLD_LOCK_TIMEOUT_MS,
            )
            .await;

            for id in ids {
                let fingerprint = match index.get(id).await {
                    Ok(Some(entry)) => Some(entry.fingerprint),
                    Ok(None) => None,
                    Err(_) => {
                        outcome.failed += 1;
                        continue;
                    }
                };

                if index.remove(id).await.is_err() {
                    outcome.failed += 1;
                    continue;
                }
                outcome.evicted += 1;

                let Some(fingerprint) = fingerprint else {
                    continue;
                };
                // Deduplicated content: the file goes with the last row that
                // names it, never with the first.
                if still_referenced(index, world_id, &fingerprint).await {
                    continue;
                }
                match store.remove_blob(world_id, &fingerprint).await {
                    Ok(()) => outcome.blobs_removed += 1,
                    Err(_) => outcome.failed += 1,
                }
            }
        }

        outcome
    }

    /// What the browser says this origin may store, in bytes.
    ///
    /// `None` when the platform will not answer — no `navigator.storage`, no
    /// `estimate`, a rejected promise, or a result with no numeric `quota`.
    /// Every one of those is "we do not know", and the caller must not read
    /// any of them as "no space" (see [`enforce_budget`]).
    async fn storage_quota() -> Option<u64> {
        let navigator = crate::global_property("navigator").ok()?;
        let storage =
            js_sys::Reflect::get(&navigator, &wasm_bindgen::JsValue::from_str("storage")).ok()?;
        if storage.is_undefined() || storage.is_null() {
            return None;
        }
        let estimate =
            js_sys::Reflect::get(&storage, &wasm_bindgen::JsValue::from_str("estimate")).ok()?;
        let estimate: js_sys::Function = estimate.dyn_into().ok()?;
        let promise = estimate.call0(&storage).ok()?;
        let promise: js_sys::Promise = promise.dyn_into().ok()?;
        let result = wasm_bindgen_futures::JsFuture::from(promise).await.ok()?;
        let quota =
            js_sys::Reflect::get(&result, &wasm_bindgen::JsValue::from_str("quota")).ok()?;
        // `as_f64` rejects a missing or non-numeric quota, which is the same
        // "we do not know" as the platform having no API at all.
        let quota = quota.as_f64()?;
        if !quota.is_finite() || quota < 0.0 {
            return None;
        }
        Some(quota as u64)
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

/// Native tests for the one decision in this module that has a permissions
/// consequence: telling "you have lost access to this world" apart from
/// "something went wrong".
///
/// These live here rather than in `tests/sync.rs` because they are about an
/// internal classification rule, and because getting the direction wrong in
/// either sense is a bug worth pinning down next to the code that decides
/// it: a missed refusal leaves cached content a revoked member still holds
/// (the FR-015 bug), and a false positive throws away a working cache every
/// time a server hiccups.
#[cfg(test)]
mod tests {
    use super::{SyncError, is_authorization_refusal, parse_sync_plan};

    fn errors(json: &str) -> Vec<serde_json::Value> {
        serde_json::from_str(json).expect("test fixture should be a json array")
    }

    /// Exactly what `world_sync_plan.rs`'s `to_graphql_error` produces for a
    /// non-member, as its own tests assert: the ambiguous message plus a
    /// `FORBIDDEN` code.
    const NON_MEMBER: &str = r#"{
      "errors": [{
        "message": "user is not a member of this world",
        "path": ["worldSyncPlan"],
        "extensions": { "code": "FORBIDDEN" }
      }],
      "data": null
    }"#;

    #[test]
    fn a_forbidden_code_is_an_authorization_refusal() {
        match parse_sync_plan(NON_MEMBER) {
            Err(SyncError::Forbidden(msg)) => assert!(msg.contains("not a member")),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn an_uncoded_server_error_is_not_a_refusal() {
        // The same *message*, without the code. This is the case that must
        // not discard: matching on wording would make a server-side copy
        // edit — or any resolver that happens to mention membership —
        // destroy a user's cache.
        let body = r#"{"errors":[{"message":"user is not a member of this world"}]}"#;
        assert!(matches!(parse_sync_plan(body), Err(SyncError::Server(_))));
    }

    #[test]
    fn other_codes_are_not_refusals() {
        for code in ["INTERNAL_SERVER_ERROR", "BAD_USER_INPUT", "forbidden"] {
            let body =
                format!(r#"{{"errors":[{{"message":"nope","extensions":{{"code":"{code}"}}}}]}}"#);
            assert!(
                matches!(parse_sync_plan(&body), Err(SyncError::Server(_))),
                "code {code} must not be read as a refusal",
            );
        }
    }

    #[test]
    fn a_refusal_must_be_about_the_field_we_asked_for() {
        // Defence in depth: this client only ever asks about one world, but
        // a FORBIDDEN attributed to some other field is not evidence about
        // that world's membership.
        let elsewhere = errors(
            r#"[{"message":"nope","path":["someOtherField"],"extensions":{"code":"FORBIDDEN"}}]"#,
        );
        assert!(!is_authorization_refusal(&elsewhere));

        let pathless = errors(r#"[{"message":"nope","extensions":{"code":"FORBIDDEN"}}]"#);
        assert!(is_authorization_refusal(&pathless));
    }

    #[test]
    fn a_refusal_anywhere_in_the_list_counts() {
        let mixed = errors(
            r#"[{"message":"slow"},
                {"message":"nope","path":["worldSyncPlan"],"extensions":{"code":"FORBIDDEN"}}]"#,
        );
        assert!(is_authorization_refusal(&mixed));
    }

    #[test]
    fn nothing_transient_is_ever_a_refusal() {
        // The whole safety argument in one assertion. A transport failure
        // never reaches `parse_sync_plan` at all — it is constructed by the
        // caller — and no reply that fails to parse can produce `Forbidden`
        // either, so an offline client, a 500, or a truncated body all keep
        // their cache.
        assert!(!is_authorization_refusal(&[]));
        for body in ["", "not json", "{}", r#"{"data":null}"#, r#"{"errors":[]}"#] {
            assert!(
                !matches!(parse_sync_plan(body), Err(SyncError::Forbidden(_))),
                "{body:?} must not be read as a refusal",
            );
        }
    }
}
