//! What a client holds, and the canonical form scene fingerprints are taken
//! over.
//!
//! Spec 028 FR-005/FR-007, data-model.md.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Fingerprint, HeldItem, ItemId};

/// A client's account of one world.
///
/// Backed by `BTreeMap` rather than `HashMap` deliberately: two clients in
/// identical states must serialize identically. With hash ordering the wire
/// bytes vary run to run, which makes the protocol nondeterministic and its
/// tests unable to assert on anything but set equality.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub world_id: Uuid,
    items: BTreeMap<ItemId, Fingerprint>,
}

impl Manifest {
    pub fn new(world_id: Uuid) -> Self {
        Self {
            world_id,
            items: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, id: ItemId, fingerprint: Fingerprint) {
        self.items.insert(id, fingerprint);
    }

    pub fn remove(&mut self, id: &ItemId) {
        self.items.remove(id);
    }

    pub fn get(&self, id: &ItemId) -> Option<Fingerprint> {
        self.items.get(id).copied()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The wire form, in deterministic order.
    pub fn to_wire(&self) -> Vec<HeldItem> {
        self.items
            .iter()
            .map(|(&id, &fingerprint)| HeldItem { id, fingerprint })
            .collect()
    }
}

/// Version of the canonical serialization below.
///
/// Participates in every scene fingerprint, so changing the canonical form
/// invalidates all of them at once rather than leaving clients comparing
/// values computed under two different rules. Bump it whenever
/// [`CanonicalSceneState`] changes shape.
pub const CANONICAL_VERSION: u32 = 1;

/// One entity's contribution to a scene's canonical form.
///
/// Floats are stored as fixed-precision integers rather than `f32`, which is
/// the whole point: an `f32` that round-trips through Postgres and back can
/// print differently than it went in, and a fingerprint that changes because
/// of formatting would make every reload look like a modification.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct CanonicalEntity {
    pub id: Uuid,
    /// Position, rotation and scale in milli-units. See
    /// [`CanonicalSceneState::quantize`].
    pub x_milli: i64,
    pub y_milli: i64,
    pub rotation_milli: i64,
    pub scale_milli: i64,
}

/// The stable serialization a scene's fingerprint is taken over.
///
/// Identical logical state must hash identically regardless of the order rows
/// came back from the database or how a float was formatted. Two properties
/// deliver that:
///
/// - entities are sorted by id, so row order cannot leak in;
/// - floats are quantized to fixed precision, so representation cannot.
///
/// Per-viewer state — selection, camera — is excluded. Two users looking at
/// the same scene differently must still agree on its fingerprint, or the
/// delta protocol would report a change every time someone clicked something.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CanonicalSceneState {
    pub version: u32,
    pub scene_id: Uuid,
    entities: Vec<CanonicalEntity>,
}

impl CanonicalSceneState {
    /// Quantize a world-space float to milli-units.
    ///
    /// Round-half-away-from-zero, so a value and its negation quantize
    /// symmetrically — with `as i64` truncation, `-0.0005` and `0.0005` would
    /// land on different sides of zero.
    pub fn quantize(value: f32) -> i64 {
        let scaled = f64::from(value) * 1000.0;
        if scaled >= 0.0 {
            (scaled + 0.5) as i64
        } else {
            (scaled - 0.5) as i64
        }
    }

    /// Build from entities in any order; they are sorted here.
    pub fn new(scene_id: Uuid, mut entities: Vec<CanonicalEntity>) -> Self {
        entities.sort_by_key(|e| e.id);
        Self {
            version: CANONICAL_VERSION,
            scene_id,
            entities,
        }
    }

    pub fn entities(&self) -> &[CanonicalEntity] {
        &self.entities
    }

    /// The bytes this state hashes over.
    ///
    /// Hand-rolled rather than delegating to a serializer, so the format
    /// cannot change underneath us when a dependency updates. A fingerprint
    /// that shifts because `serde_json` changed its spacing would invalidate
    /// every client's cache on a routine `cargo update`.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.version.to_be_bytes());
        out.extend_from_slice(self.scene_id.as_bytes());
        out.extend_from_slice(&(self.entities.len() as u64).to_be_bytes());
        for e in &self.entities {
            out.extend_from_slice(e.id.as_bytes());
            out.extend_from_slice(&e.x_milli.to_be_bytes());
            out.extend_from_slice(&e.y_milli.to_be_bytes());
            out.extend_from_slice(&e.rotation_milli.to_be_bytes());
            out.extend_from_slice(&e.scale_milli.to_be_bytes());
        }
        out
    }

    /// This scene's fingerprint.
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of_bytes(&self.canonical_bytes())
    }
}
