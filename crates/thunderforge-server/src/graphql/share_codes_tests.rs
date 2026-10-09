//! Spec 088 (T010, FR-006): link codes are 26 characters of Crockford
//! base32 drawn from the operating system's random source, and a code read
//! aloud or retyped still finds its link.

use super::*;
use std::collections::HashSet;

/// Crockford's alphabet: digits and capitals without I, L, O and U.
const CROCKFORD: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

#[test]
fn a_code_is_twenty_six_crockford_characters() {
    for _ in 0..256 {
        let code = generate_link_code();
        assert_eq!(code.len(), 26, "code must be 26 characters, got {code}");
        assert!(
            code.chars().all(|c| CROCKFORD.contains(c)),
            "code must use only Crockford base32, got {code}"
        );
    }
}

#[test]
fn a_thousand_codes_never_repeat() {
    let batch: HashSet<String> = (0..1_000).map(|_| generate_link_code()).collect();
    assert_eq!(batch.len(), 1_000, "codes must not repeat");
}

/// The spec 005 regression guard: a clock-derived source shares its leading
/// characters across a burst. A random one spreads them over the alphabet.
#[test]
fn a_burst_of_codes_shows_no_time_ordering() {
    let batch: Vec<String> = (0..5_000).map(|_| generate_link_code()).collect();
    let first: HashSet<char> = batch.iter().filter_map(|c| c.chars().next()).collect();
    assert!(
        first.len() >= 24,
        "the leading character should vary across the alphabet; saw {}",
        first.len()
    );
    let mut sorted = batch.clone();
    sorted.sort();
    assert_ne!(sorted, batch, "generation order must not be sorted order");
}

#[test]
fn the_encoder_writes_known_bytes_as_crockford() {
    assert_eq!(encode_crockford(&[0u8; 16]), "0".repeat(26));
    assert_eq!(
        encode_crockford(&[0xFF; 16]),
        format!("{}W", "Z".repeat(25))
    );
    // 0b00001_00010_00011_... : the first bytes spell 1, 2, 3.
    assert_eq!(&encode_crockford(&[0x08, 0x86, 0x00])[..3], "123");
}

#[test]
fn normalising_reads_a_code_as_it_was_meant() {
    assert_eq!(normalize_link_code(" abcd-efgh "), "ABCDEFGH");
    assert_eq!(normalize_link_code("o0Oi1IlL"), "00011111");
    // An old 20-character hex code is unchanged, so it still finds its link.
    assert_eq!(
        normalize_link_code("0123456789ABCDEF0123"),
        "0123456789ABCDEF0123"
    );
}

#[test]
fn a_generated_code_survives_normalising() {
    for _ in 0..64 {
        let code = generate_link_code();
        assert_eq!(normalize_link_code(&code.to_lowercase()), code);
    }
}
