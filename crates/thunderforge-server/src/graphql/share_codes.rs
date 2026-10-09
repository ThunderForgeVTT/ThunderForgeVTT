//! Spec 027 (T005, FR-006): the one generator for every user-facing link code.
//!
//! Before this, four copies existed — `generate_share_code()` in each of
//! `mutations_actor_shares.rs`, `mutations_item_shares.rs` and
//! `mutations_ability_shares.rs`, plus an inline variant in
//! `mutations_invites.rs` that took only **8** characters. An invite code
//! grants membership in a world, so ~32 bits of entropy did not meet ADR-049's
//! unguessable-code invariant while content share links already used ~80.
//! Consolidating here raises invites to share-code strength and leaves one
//! place to change if that bar ever moves.
//!
//! # Never derived from a clock
//!
//! Spec 005 US4 found codes taken from the leading characters of a v7 UUID,
//! which front-loads a millisecond timestamp: two links made in the same
//! millisecond collided on `world_invites_invite_code_key`. A code must come
//! from a random source only.
//!
//! # Spec 088: Crockford base32 from the OS
//!
//! A code is now 16 bytes (128 bits) from the operating system's random
//! source, written as 26 characters of Crockford base32 (FR-006). The
//! alphabet leaves out I, L, O and U, so a code read aloud or copied by hand
//! has no look-alike letters, and [`normalize_link_code`] maps the common
//! slips back before a lookup. Old 20-character hex codes still work: hex is
//! a subset of the alphabet and normalising leaves it unchanged.

use rand::TryRng as _;

/// Crockford's base32 alphabet: digits and capitals without I, L, O and U.
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Random bytes in a code: 128 bits, written as 26 characters.
const CODE_BYTES: usize = 16;

/// Generates an unguessable, non-time-derived link code.
pub fn generate_link_code() -> String {
    let mut bytes = [0u8; CODE_BYTES];
    if rand::rngs::SysRng.try_fill_bytes(&mut bytes).is_err() {
        // The thread generator is a CSPRNG seeded from the same OS source;
        // it stands in rather than failing the request.
        rand::fill(&mut bytes);
    }
    encode_crockford(&bytes)
}

/// Writes bytes as Crockford base32, five bits per character, the last
/// character padded with zero bits.
pub(crate) fn encode_crockford(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() * 8).div_ceil(5));
    let mut buffer: u16 = 0;
    let mut bits = 0u32;
    for &byte in bytes {
        buffer = (buffer << 8) | u16::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(CROCKFORD[usize::from((buffer >> bits) & 0x1F)] as char);
        }
    }
    if bits > 0 {
        out.push(CROCKFORD[usize::from((buffer << (5 - bits)) & 0x1F)] as char);
    }
    out
}

/// Reads a code the way it was meant (FR-006): spaces and hyphens dropped,
/// upper case, `O` as `0`, and `I` or `L` as `1`.
pub fn normalize_link_code(code: &str) -> String {
    code.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        })
        .collect()
}

#[cfg(test)]
#[path = "share_codes_tests.rs"]
mod tests;
