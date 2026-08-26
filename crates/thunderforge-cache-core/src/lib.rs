//! Shared cache and sync policy for ThunderForgeVTT clients and server.
//!
//! Spec `028-client-world-cache`, ADR-052.
//!
//! # Why this crate exists
//!
//! The same rules are needed on both sides of the wire, and both sides are
//! Rust. The server computes fingerprints, decides what a client is missing,
//! and adjudicates conflicting offline edits. The client computes
//! fingerprints to verify what it received, decides what to evict, and
//! predicts what the server will say. Those are the same rules.
//!
//! Implemented twice — once in Rust on the server, once in TypeScript in the
//! browser — they drift. Drift here is not cosmetic: a client whose notion of
//! "current" disagrees with the server's believes it is up to date when it is
//! not, and that presents as missing map art and silently lost edits.
//!
//! So the rules live here once, and this crate depends on nothing
//! platform-specific: no `web-sys`, no Diesel, no network, no clock. That is
//! what lets every rule below be exercised by plain `cargo test` rather than
//! only inside a browser — the same reasoning ADR-038 used to split
//! `thunderforge-canvas-core` out for native testability.
//!
//! **If something here needs I/O, it belongs in `thunderforge-cache-browser`
//! or the server crate instead.**

pub mod budget;
pub mod conflict;
pub mod delta;
pub mod fingerprint;
pub mod manifest;
pub mod queue;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use fingerprint::{Fingerprint, IntegrityError};

/// One cacheable thing, independent of where it happens to be stored.
///
/// Deliberately a closed enum. Compendium content, system packs and world
/// documents are out of scope for this feature (spec 028 Assumptions), and
/// keeping the set closed means extending it is a decision someone makes
/// rather than something that happens by accident.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum ItemId {
    /// A scene's logical state, fingerprinted over its canonical form.
    SceneState(Uuid),
    /// One canvas image asset — a map background or token art.
    CanvasAsset(Uuid),
}

impl ItemId {
    /// The wire encoding: `"scene:<uuid>"` or `"asset:<uuid>"`.
    pub fn to_wire(&self) -> String {
        match self {
            Self::SceneState(id) => format!("scene:{id}"),
            Self::CanvasAsset(id) => format!("asset:{id}"),
        }
    }

    /// Parse the wire encoding. Unknown prefixes are rejected rather than
    /// guessed at — a client and server disagreeing about what an id means
    /// is worse than failing to parse it.
    pub fn from_wire(s: &str) -> Option<Self> {
        let (kind, id) = s.split_once(':')?;
        let uuid = Uuid::parse_str(id).ok()?;
        match kind {
            "scene" => Some(Self::SceneState(uuid)),
            "asset" => Some(Self::CanvasAsset(uuid)),
            _ => None,
        }
    }

    /// The world this item belongs to is not encoded in the id; callers hold
    /// that separately. Exposed so eviction can group by world without
    /// re-parsing.
    pub fn uuid(&self) -> Uuid {
        match self {
            Self::SceneState(id) | Self::CanvasAsset(id) => *id,
        }
    }
}

/// What a client says it holds.
///
/// **A claim of possession, never of entitlement.** The server must not infer
/// any permission from an item appearing here: a client claiming an item it
/// may not see receives that item in neither the fetch nor the evict list,
/// which discloses nothing about whether it exists (FR-014, FR-047).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct HeldItem {
    pub id: ItemId,
    pub fingerprint: Fingerprint,
}
