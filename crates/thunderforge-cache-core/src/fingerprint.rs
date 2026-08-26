//! Content fingerprints, and the single sanctioned way to trust bytes.
//!
//! Spec 028 FR-005/FR-010/FR-046, contracts/cache-core-api.md.
//!
//! A fingerprint is SHA-256 over the bytes **as stored** — post-transcode,
//! not as uploaded. `transcode_to_webp` means what a client receives is
//! never what was uploaded, so hashing the original would produce a value
//! no client could ever verify against what it actually holds.

use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A SHA-256 digest of some content, compared for equality and nothing else.
///
/// `Ord` exists solely so collections of fingerprints serialize
/// deterministically; it carries no meaning beyond byte order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Fingerprint([u8; 32]);

/// Why a hex string was not a fingerprint.
///
/// Parsing never coerces: a malformed fingerprint is an error, not a miss.
/// Treating one as a miss would silently re-fetch forever instead of
/// surfacing the bug producing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// Not exactly 64 characters.
    WrongLength { found: usize },
    /// Contained something outside `0-9a-f`. Uppercase is rejected too, so
    /// that one piece of content has exactly one wire representation.
    NotLowercaseHex,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength { found } => {
                write!(f, "expected 64 hex characters, found {found}")
            }
            Self::NotLowercaseHex => f.write_str("expected lowercase hex"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Received content did not match what was promised.
///
/// Always a discard-and-refetch, never a warning: content that fails this
/// check has no claim to being what the server said it was, whether it came
/// from a peer, the server, or this machine's own disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegrityError {
    pub expected: Fingerprint,
    pub actual: Fingerprint,
}

impl fmt::Display for IntegrityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "content fingerprint mismatch: expected {}, got {}",
            self.expected, self.actual
        )
    }
}

impl std::error::Error for IntegrityError {}

impl Fingerprint {
    /// Hash the given bytes.
    pub fn of_bytes(bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Self(hasher.finalize().into())
    }

    /// Lowercase hex, the only wire representation.
    pub fn to_hex(&self) -> String {
        let mut out = String::with_capacity(64);
        for byte in self.0 {
            out.push_str(&format!("{byte:02x}"));
        }
        out
    }

    /// Parse lowercase hex. Strict by design — see [`ParseError`].
    pub fn from_hex(s: &str) -> Result<Self, ParseError> {
        if s.len() != 64 {
            return Err(ParseError::WrongLength { found: s.len() });
        }
        let bytes = s.as_bytes();
        let mut out = [0u8; 32];
        for (i, slot) in out.iter_mut().enumerate() {
            let hi = hex_nibble(bytes[i * 2])?;
            let lo = hex_nibble(bytes[i * 2 + 1])?;
            *slot = (hi << 4) | lo;
        }
        Ok(Self(out))
    }

    /// The raw digest, for callers that must hand it to a byte-oriented API.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

fn hex_nibble(c: u8) -> Result<u8, ParseError> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        _ => Err(ParseError::NotLowercaseHex),
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint({})", self.to_hex())
    }
}

/// Verify content against the fingerprint it was promised under.
///
/// This is the **only** sanctioned way to decide that bytes are what they
/// claim to be, and every path that accepts bytes goes through it: server
/// responses (FR-010), peer transfers (FR-046), and blobs read back off
/// local disk (FR-018). Concentrating it here is what makes "we never
/// trusted unverified content" a checkable claim rather than a hope.
pub fn verify(bytes: &[u8], expected: &Fingerprint) -> Result<(), IntegrityError> {
    let actual = Fingerprint::of_bytes(bytes);
    if actual == *expected {
        Ok(())
    } else {
        Err(IntegrityError {
            expected: *expected,
            actual,
        })
    }
}
