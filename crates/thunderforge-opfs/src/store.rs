//! The storage seam: what the cache needs a disk to do, stated once.
//!
//! Spec `028-client-world-cache`, FR-021 (T055).
//!
//! # Why this trait exists
//!
//! Every byte of the real implementation is `#[cfg(target_arch = "wasm32")]`,
//! because OPFS only exists in a browser. That is not a problem for the code
//! — it is the right place for it — but it meant the read, write and delete
//! logic had **no native test at all**. The crate reported dozens of passing
//! tests, and all of them were about path strings; nothing exercised what
//! happens when a read meets a half-written file, which is precisely the
//! question FR-021 asks.
//!
//! So the operations are named here as a trait, implemented twice: by
//! [`crate::opfs::OpfsBlobStore`] against the real platform, and by
//! [`crate::memory::MemoryBlobStore`] against a `BTreeMap` that can be told
//! to interleave. The policy the cache applies on top — when a file is
//! garbage worth deleting versus another tab's work in progress — is then an
//! ordinary function over an ordinary store, and an ordinary test can pin it.
//!
//! # Sealed bytes, not plaintext
//!
//! The trait deals in the bytes that go on disk. Encryption and fingerprint
//! verification live above it, in
//! `thunderforge-cache-browser`, for two reasons: WebCrypto is as
//! browser-bound as OPFS is, so keeping it out is what leaves this crate
//! testable natively; and a store that cannot decrypt what it holds is a
//! store that cannot accidentally hand back plaintext.
//!
//! # No locking here either
//!
//! Cross-tab exclusion is the caller's business (`locks.rs` in
//! `thunderforge-cache-browser`), and deliberately so: Web Locks are **not
//! reentrant**, so a store that took the world lock internally would deadlock
//! the moment it was called from `apply_plan`, which already holds it. What
//! this crate owes the caller is that a single operation is not observable
//! half-done — see [`BlobStore::write`].

use core::fmt;

use thunderforge_cache_core::Fingerprint;
use uuid::Uuid;

use crate::paths::PathError;

/// Why a store operation could not be completed.
///
/// Deliberately small. A cache is an optimisation: almost every caller's
/// response to any of these is "then fetch it from the network", so a rich
/// taxonomy would be information nobody acts on. The one distinction worth
/// keeping is *absent* from *broken*, and absence is not an error here — it
/// is `Ok(None)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// The platform has no such capability. Degrade, never fail (FR-021d).
    Unsupported(&'static str),
    /// A platform call rejected, flattened to a string because the browser's
    /// error values are neither `Send` nor comparable and callers only log.
    Backend(String),
    /// A name we were asked to use is not one we are willing to write.
    Path(PathError),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(what) => write!(f, "storage does not support {what}"),
            Self::Backend(msg) => write!(f, "storage call failed: {msg}"),
            Self::Path(err) => write!(f, "invalid storage path: {err}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<PathError> for StoreError {
    fn from(err: PathError) -> Self {
        Self::Path(err)
    }
}

/// A store's answer for one blob.
pub type Result<T> = core::result::Result<T, StoreError>;

/// The smallest file this cache will ever treat as a blob.
///
/// One byte. The number is not the interesting part — the rule is: **an empty
/// file is never something we finished writing.**
///
/// It is load-bearing for FR-021 rather than defensive. Creating a file and
/// filling it are two operations in every filesystem this runs on, OPFS
/// included: `getFileHandle({create: true})` publishes a zero-length entry at
/// the final name immediately, and the content only appears when the writable
/// stream is closed. So a reader in another tab can, and at some point will,
/// open a file that exists and holds nothing.
///
/// Before this rule, that reader concluded "will not decrypt", and
/// `read_blob`'s repair path **deleted the file the other tab was still
/// writing**. The write then completed into a removed entry and its index row
/// pointed at nothing. Treating an empty file as absent — and, crucially, as
/// *not ours to reclaim* — is what stops one tab's read from destroying
/// another tab's write.
pub const MIN_BLOB_LEN: usize = 1;

/// What a stored file's size alone says about it, before any key is involved.
///
/// Separated from the read path so the decision can be tested without a
/// browser, a key, or a filesystem — it is the whole of FR-021's read-side
/// rule and it is three lines long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlobShape {
    /// Nothing at that name.
    Absent,
    /// A file exists but is too small to be anything we finished writing.
    ///
    /// The two ways to reach this state are a write in flight *right now* in
    /// this or another tab, and a write that died before committing. They are
    /// indistinguishable from the outside, which is exactly why neither may
    /// be deleted on sight: the first belongs to someone else, and the second
    /// is repaired for free by the next write of the same fingerprint, which
    /// targets the same name.
    Incomplete,
    /// Big enough to be a blob. Whether it *is* one is for the key to say.
    Complete,
}

impl BlobShape {
    /// Classify by size.
    pub fn of(len: Option<usize>) -> Self {
        match len {
            None => Self::Absent,
            Some(len) if len < MIN_BLOB_LEN => Self::Incomplete,
            Some(_) => Self::Complete,
        }
    }

    /// Whether a reader should treat this as content it can attempt to open.
    pub fn is_readable(self) -> bool {
        matches!(self, Self::Complete)
    }

    /// Whether the cache may delete this file to reclaim space.
    ///
    /// `Incomplete` answers **false**, and that is the FR-021 guarantee in
    /// one method: a file that might be another tab's work in progress is
    /// never reclaimed by a reader that could not open it.
    pub fn is_reclaimable(self) -> bool {
        matches!(self, Self::Complete)
    }
}

/// The disk operations the world cache needs.
///
/// Implemented by [`crate::opfs::OpfsBlobStore`] in a browser and
/// [`crate::memory::MemoryBlobStore`] in a test. Every method is scoped to
/// one user — the scope is fixed when the store is opened — so no caller can
/// reach another user's bytes by passing the wrong argument.
#[allow(async_fn_in_trait)]
pub trait BlobStore {
    /// Store `sealed` under this world and fingerprint.
    ///
    /// **Must not be observable half-done at the final name.** An
    /// implementation may create the entry before it has the content — that
    /// is unavoidable on OPFS — but the bytes that appear there must go from
    /// "none" to "all of them" in one step, never through a prefix. The
    /// zero-length window that creation opens is what [`BlobShape`] exists to
    /// make safe.
    async fn write(&self, world_id: Uuid, fingerprint: &Fingerprint, sealed: &[u8]) -> Result<()>;

    /// The sealed bytes, or `None` when there is nothing complete there.
    ///
    /// An [`BlobShape::Incomplete`] file reads as `None` and is left alone.
    async fn read(&self, world_id: Uuid, fingerprint: &Fingerprint) -> Result<Option<Vec<u8>>>;

    /// What is at this name, without reading it.
    ///
    /// Callers deciding whether to skip a fetch must use this rather than a
    /// bare existence check: an `Incomplete` file exists and is not a reason
    /// to skip anything.
    async fn shape(&self, world_id: Uuid, fingerprint: &Fingerprint) -> Result<BlobShape>;

    /// Delete one blob. Absent is success — the postcondition is "not there".
    async fn remove(&self, world_id: Uuid, fingerprint: &Fingerprint) -> Result<()>;

    /// Every fingerprint physically present for a world, complete or not.
    ///
    /// The ground truth a repair pass diffs the index against. Incomplete
    /// files are included: a repair that could not see them could never
    /// reclaim one that was abandoned.
    async fn list(&self, world_id: Uuid) -> Result<Vec<Fingerprint>>;

    /// Drop a world's bytes wholesale.
    async fn remove_world(&self, world_id: Uuid) -> Result<()>;

    /// Drop this user's entire cache.
    async fn remove_scope(&self) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_file_is_never_something_we_finished_writing() {
        assert_eq!(BlobShape::of(Some(0)), BlobShape::Incomplete);
        assert_eq!(BlobShape::of(None), BlobShape::Absent);
        assert_eq!(BlobShape::of(Some(1)), BlobShape::Complete);
    }

    #[test]
    fn an_incomplete_file_is_neither_readable_nor_reclaimable() {
        // The second half is the FR-021 guarantee. A reader that cannot open
        // a file must not conclude it is garbage, because the file may be
        // another tab's write in progress — and deleting it is how one tab
        // destroys another's work.
        assert!(!BlobShape::Incomplete.is_readable());
        assert!(!BlobShape::Incomplete.is_reclaimable());
    }

    #[test]
    fn absent_is_not_reclaimable_either_because_there_is_nothing_to_reclaim() {
        assert!(!BlobShape::Absent.is_readable());
        assert!(!BlobShape::Absent.is_reclaimable());
    }

    #[test]
    fn a_complete_file_may_be_read_and_may_be_reclaimed() {
        // Reclaimable is not "will be reclaimed": a complete file that opens
        // correctly is kept. This says only that the *shape* does not forbid
        // it, which is what lets a genuinely corrupt blob be cleaned up.
        assert!(BlobShape::Complete.is_readable());
        assert!(BlobShape::Complete.is_reclaimable());
    }
}
