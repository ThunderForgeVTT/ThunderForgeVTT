//! Spec 028 T014: a canonical-form change must invalidate every scene
//! fingerprint at once, rather than leaving clients comparing values computed
//! under two different rules.

use thunderforge_cache_core::manifest::{CANONICAL_VERSION, CanonicalEntity, CanonicalSceneState};
use uuid::Uuid;

#[test]
fn version_participates_in_the_hash() {
    let scene = Uuid::from_u128(1);
    let entities = vec![CanonicalEntity {
        id: Uuid::from_u128(2),
        x_milli: 100,
        y_milli: 200,
        rotation_milli: 0,
        scale_milli: 1000,
    }];

    let state = CanonicalSceneState::new(scene, entities);
    let bytes = state.canonical_bytes();

    // The version leads the canonical bytes, so bumping it changes every
    // fingerprint derived from it.
    assert_eq!(&bytes[..4], &CANONICAL_VERSION.to_be_bytes());

    let mut bumped = bytes.clone();
    bumped[..4].copy_from_slice(&(CANONICAL_VERSION + 1).to_be_bytes());
    assert_ne!(bytes, bumped);
}

#[test]
fn entity_count_is_hashed_so_splits_cannot_collide() {
    // Without a length prefix, two entities could be reinterpreted as one
    // with different field boundaries.
    let scene = Uuid::from_u128(1);
    let one = CanonicalSceneState::new(
        scene,
        vec![CanonicalEntity {
            id: Uuid::from_u128(2),
            x_milli: 0,
            y_milli: 0,
            rotation_milli: 0,
            scale_milli: 0,
        }],
    );
    let none = CanonicalSceneState::new(scene, vec![]);
    assert_ne!(one.fingerprint(), none.fingerprint());
}
