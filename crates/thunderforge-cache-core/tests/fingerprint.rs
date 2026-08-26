//! Spec 028 T012/T013: fingerprint stability and the verification choke point.

use thunderforge_cache_core::fingerprint::{self, Fingerprint, ParseError};
use thunderforge_cache_core::manifest::{CanonicalEntity, CanonicalSceneState};
use uuid::Uuid;

fn entity(id: u128, x: f32, y: f32) -> CanonicalEntity {
    CanonicalEntity {
        id: Uuid::from_u128(id),
        x_milli: CanonicalSceneState::quantize(x),
        y_milli: CanonicalSceneState::quantize(y),
        rotation_milli: 0,
        scale_milli: 1000,
    }
}

#[test]
fn hex_round_trips() {
    let fp = Fingerprint::of_bytes(b"thunderforge");
    assert_eq!(Fingerprint::from_hex(&fp.to_hex()).unwrap(), fp);
    assert_eq!(fp.to_hex().len(), 64);
}

#[test]
fn parsing_is_strict_never_coercing() {
    // A malformed fingerprint must be an error, not silently treated as a
    // miss — a miss would re-fetch forever instead of surfacing the bug.
    assert_eq!(
        Fingerprint::from_hex("abc"),
        Err(ParseError::WrongLength { found: 3 })
    );
    let uppercase = Fingerprint::of_bytes(b"x").to_hex().to_uppercase();
    assert_eq!(
        Fingerprint::from_hex(&uppercase),
        Err(ParseError::NotLowercaseHex),
        "one piece of content must have exactly one wire representation"
    );
    assert_eq!(
        Fingerprint::from_hex(&"z".repeat(64)),
        Err(ParseError::NotLowercaseHex)
    );
}

#[test]
fn verify_accepts_matching_content() {
    let bytes = b"map background bytes";
    assert!(fingerprint::verify(bytes, &Fingerprint::of_bytes(bytes)).is_ok());
}

#[test]
fn verify_rejects_every_single_bit_mutation() {
    // The property that makes peer transfer safe: a peer can waste bandwidth,
    // it cannot poison the cache.
    let original = b"the quick brown fox".to_vec();
    let expected = Fingerprint::of_bytes(&original);

    for byte_index in 0..original.len() {
        for bit in 0..8 {
            let mut mutated = original.clone();
            mutated[byte_index] ^= 1 << bit;
            assert!(
                fingerprint::verify(&mutated, &expected).is_err(),
                "flipping bit {bit} of byte {byte_index} must fail verification"
            );
        }
    }
}

#[test]
fn scene_fingerprint_is_stable_across_row_order() {
    // Postgres makes no ordering promise without ORDER BY. If row order
    // leaked into the hash, every reload would look like a modification.
    let scene = Uuid::from_u128(7);
    let a = CanonicalSceneState::new(scene, vec![entity(1, 10.0, 20.0), entity(2, 30.0, 40.0)]);
    let b = CanonicalSceneState::new(scene, vec![entity(2, 30.0, 40.0), entity(1, 10.0, 20.0)]);
    assert_eq!(a.fingerprint(), b.fingerprint());
}

#[test]
fn scene_fingerprint_is_stable_across_float_round_trip() {
    // An f32 that survives a database round trip can print differently than
    // it went in. Quantization is what stops that reading as a change.
    let scene = Uuid::from_u128(7);
    // These must be genuinely distinct as f32 — a literal with more
    // precision than f32 carries would round to the same value and the test
    // would compare something to itself.
    assert_ne!(10.0_f32, 10.0004_f32);
    assert_ne!(-62.5_f32, -62.4996_f32);

    let a = CanonicalSceneState::new(scene, vec![entity(1, 10.0, -62.5)]);
    let b = CanonicalSceneState::new(scene, vec![entity(1, 10.0004, -62.4996)]);
    assert_eq!(a.fingerprint(), b.fingerprint());
}

#[test]
fn quantization_is_symmetric_about_zero() {
    // Truncation would put -0.0005 and 0.0005 on different sides of zero.
    assert_eq!(
        CanonicalSceneState::quantize(0.0005),
        -CanonicalSceneState::quantize(-0.0005)
    );
    assert_eq!(CanonicalSceneState::quantize(-62.5), -62_500);
}

#[test]
fn a_real_change_does_change_the_fingerprint() {
    let scene = Uuid::from_u128(7);
    let a = CanonicalSceneState::new(scene, vec![entity(1, 10.0, 20.0)]);
    let b = CanonicalSceneState::new(scene, vec![entity(1, 10.0, 21.0)]);
    assert_ne!(a.fingerprint(), b.fingerprint());
}
