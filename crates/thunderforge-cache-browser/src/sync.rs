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
