//! Browser-side I/O for the client world cache.
//!
//! Spec `028-client-world-cache` (T004, T024–T026), ADR-052.
//!
//! # Why this crate exists
//!
//! [`thunderforge_cache_core`] holds every rule the server and the client
//! must agree on, and holds it without I/O so it runs under plain
//! `cargo test`. This crate is the other half: the parts that can only
//! happen inside a browser — OPFS blobs, a WebCrypto key, IndexedDB records.
//!
//! Keeping them apart is the point. The untestable surface is meant to be
//! small and obvious, so the split is enforced by the manifest rather than by
//! discipline: `wasm-bindgen`, `js-sys` and `web-sys` are declared only under
//! `cfg(target_arch = "wasm32")`. On a native build they are not in the
//! dependency graph at all, so anything that compiles natively provably does
//! no browser I/O. Each module below is organised the same way — pure logic
//! first, compiled everywhere and unit-tested natively; a
//! `#[cfg(target_arch = "wasm32")]` block underneath holding the calls into
//! the platform.
//!
//! # The trust rule
//!
//! Every path in this crate that accepts bytes verifies them through
//! [`thunderforge_cache_core::fingerprint::verify`] before storing or
//! returning them. That function is the single sanctioned trust choke point
//! (FR-010, FR-018, FR-046); nothing here compares digests by hand. Bytes
//! arriving from the server, from a peer, or from this machine's own disk are
//! treated identically, because a local file is not more trustworthy than a
//! remote one — it is merely closer.
//!
//! # What failure looks like
//!
//! There is no degraded state (data-model.md, "State transitions"). Every way
//! out of `Present` ends at `Absent`, and `Absent` is always recoverable by
//! fetching. So a corrupt blob, a decryption failure, and a lost key all
//! produce the same observable outcome as an empty cache: `Ok(None)`, refetch,
//! no error surfaced (FR-016c). Reserving `Err` for genuine bugs is what
//! keeps key loss from needing a whole class of edge-case handling.

pub mod crypto;
pub mod index;
pub mod opfs;

#[cfg(target_arch = "wasm32")]
mod idb;

use std::fmt;

use thunderforge_cache_core::IntegrityError;

pub use crypto::{Envelope, EnvelopeError};
pub use index::{IndexEntry, ReadSeq};
pub use opfs::{BlobPath, PathError, UserScope};

/// Why a cache operation failed outright.
///
/// Deliberately narrow. A miss, a corrupt blob, a wrong key and an absent key
/// are **not** errors — they are `Ok(None)`, indistinguishable from a cold
/// cache by design (FR-016c). What remains here is the set of conditions a
/// caller genuinely cannot proceed through: the platform is missing an API,
/// the platform rejected a call, or a value we ourselves wrote back does not
/// parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheError {
    /// A required browser API is absent. The feature degrades to today's
    /// behaviour rather than failing the session (plan.md, Target Platform).
    Unsupported(&'static str),
    /// A platform call rejected. The `JsValue` is flattened to a string
    /// because `JsValue` is neither `Send` nor comparable, and callers only
    /// ever log this.
    Platform(String),
    /// A path segment was not something we are willing to write to disk.
    Path(PathError),
    /// A stored envelope was not framed the way we frame them.
    Envelope(EnvelopeError),
    /// Bytes did not hash to the fingerprint they were promised under.
    ///
    /// Only surfaced for content arriving from *outside* — a caller handing
    /// us bytes to store. Bytes read back off our own disk that fail this
    /// check are corrupt-therefore-absent, not an error.
    Integrity(IntegrityError),
    /// A record this crate wrote could not be read back. Always a bug in this
    /// crate or a hand-edited database, never a user-facing condition.
    Corrupt(String),
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(what) => write!(f, "browser does not support {what}"),
            Self::Platform(msg) => write!(f, "browser API call failed: {msg}"),
            Self::Path(err) => write!(f, "invalid cache path: {err}"),
            Self::Envelope(err) => write!(f, "malformed encrypted envelope: {err}"),
            Self::Integrity(err) => write!(f, "{err}"),
            Self::Corrupt(msg) => write!(f, "unreadable cache record: {msg}"),
        }
    }
}

impl std::error::Error for CacheError {}

impl From<PathError> for CacheError {
    fn from(err: PathError) -> Self {
        Self::Path(err)
    }
}

impl From<EnvelopeError> for CacheError {
    fn from(err: EnvelopeError) -> Self {
        Self::Envelope(err)
    }
}

impl From<IntegrityError> for CacheError {
    fn from(err: IntegrityError) -> Self {
        Self::Integrity(err)
    }
}

/// The result type every fallible operation in this crate returns.
pub type Result<T> = std::result::Result<T, CacheError>;

/// The IndexedDB database every store in this crate lives in.
///
/// One database, several object stores (data-model.md, "IndexedDB stores"),
/// so that an index write and an outbox write can share a transaction later
/// without a cross-database dance that IndexedDB does not offer.
pub const DB_NAME: &str = "thunderforge-cache";

/// Schema version of [`DB_NAME`].
pub const DB_VERSION: u32 = 1;

/// Object store holding one [`IndexEntry`] per cached item (T026).
pub const STORE_INDEX: &str = "index";

/// Object store holding the non-extractable session key, per user scope (T025).
pub const STORE_KEYS: &str = "keys";

/// Object store holding queued offline changes. Declared at upgrade time so
/// the durable outbox does not require a schema bump to land later; not
/// otherwise used by this crate.
pub const STORE_OUTBOX: &str = "outbox";

/// Object store holding canonical-form version and budget state. Declared for
/// the same reason as [`STORE_OUTBOX`].
pub const STORE_META: &str = "meta";

/// Every store [`DB_VERSION`] creates.
pub const ALL_STORES: [&str; 4] = [STORE_INDEX, STORE_KEYS, STORE_OUTBOX, STORE_META];

#[cfg(target_arch = "wasm32")]
pub(crate) fn js_err(err: wasm_bindgen::JsValue) -> CacheError {
    CacheError::Platform(
        err.as_string()
            .or_else(|| {
                js_sys::Reflect::get(&err, &wasm_bindgen::JsValue::from_str("message"))
                    .ok()
                    .and_then(|m| m.as_string())
            })
            .unwrap_or_else(|| format!("{err:?}")),
    )
}

/// The global scope, whether that is a `Window` or a worker.
///
/// OPFS's fast path (`createSyncAccessHandle`) is worker-only, and the cache
/// is expected to run off the main thread eventually, so nothing in this
/// crate is allowed to assume `window` exists.
#[cfg(target_arch = "wasm32")]
pub(crate) fn global_property(name: &str) -> Result<wasm_bindgen::JsValue> {
    let global = js_sys::global();
    let value =
        js_sys::Reflect::get(&global, &wasm_bindgen::JsValue::from_str(name)).map_err(js_err)?;
    if value.is_undefined() || value.is_null() {
        return Err(CacheError::Unsupported(match name {
            "crypto" => "WebCrypto",
            "indexedDB" => "IndexedDB",
            "navigator" => "navigator",
            _ => "a required global",
        }));
    }
    Ok(value)
}
