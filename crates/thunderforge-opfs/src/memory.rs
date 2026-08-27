//! A [`BlobStore`] in a `BTreeMap`, for tests that need to schedule a race.
//!
//! Spec `028-client-world-cache`, FR-021 (T055).
//!
//! # What this is for, and what it is not
//!
//! It is not a mock standing in for OPFS so that unrelated tests can run
//! without a browser. It exists for one question: *what does a reader see
//! while another tab is writing?* — which on the real platform is a window a
//! test cannot open on purpose, and which
//! [`MemoryBlobStore::write_interleaved`] opens deterministically.
//!
//! # It models the platform, not a filesystem
//!
//! The behaviour reproduced here is the WHATWG File System Standard's, and
//! the two facts it encodes are the ones that make FR-021 non-trivial:
//!
//! 1. **Creation is visible before content is.**
//!    `getFileHandle(name, {create: true})` sets the entry's binary data to
//!    an empty byte sequence and appends it to the directory's children
//!    before its promise resolves. Every same-origin tab can see that
//!    zero-length file immediately. There is no spec mechanism that hides a
//!    newly created file until its first commit.
//!
//! 2. **Content appears all at once.**
//!    `createWritable()` buffers into a swap file; `close()` sets the entry's
//!    binary data to the whole buffer. A reader therefore never observes a
//!    *prefix* of the bytes — the only intermediate state that exists is the
//!    empty one from (1).
//!
//! Together those are why the rule in [`BlobShape`] is "an empty file is
//! never finished" rather than a guess at a minimum size: empty is the only
//! incomplete state the platform can produce.
//!
//! # What it does not model
//!
//! Cross-tab locking. `createWritable` takes a *shared* lock, so two tabs
//! writing one file both succeed and the last `close` wins — in Firefox and
//! Safari there is no exclusion at all. That is deliberately not simulated
//! here, because it is not this layer's problem: the callers serialise with
//! `navigator.locks`, and two tabs writing the same fingerprint are writing
//! the same bytes anyway, since the name *is* the hash of the content.

use std::cell::RefCell;
use std::collections::BTreeMap;

use thunderforge_cache_core::Fingerprint;
use uuid::Uuid;

use crate::store::{BlobShape, BlobStore, Result};

/// Where a blob lives: one world, one fingerprint.
type Key = (Uuid, String);

/// An in-memory [`BlobStore`].
///
/// Single-threaded and `RefCell`-based on purpose: the code under test is
/// wasm, which has one thread, and a `Mutex` here would model a concurrency
/// the real thing does not have.
#[derive(Debug, Default)]
pub struct MemoryBlobStore {
    files: RefCell<BTreeMap<Key, Vec<u8>>>,
}

impl MemoryBlobStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn key(world_id: Uuid, fingerprint: &Fingerprint) -> Key {
        (world_id, crate::paths::blob_file_name(fingerprint))
    }

    /// How many files exist, complete or not. Test convenience.
    pub fn len(&self) -> usize {
        self.files.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The raw bytes at a name, bypassing every rule this crate applies.
    ///
    /// For assertions about what is *physically* there — notably "the file
    /// another tab was writing is still present", which the ordinary
    /// [`BlobStore::read`] cannot answer, because its whole job is to report
    /// an incomplete file as absent.
    pub fn raw(&self, world_id: Uuid, fingerprint: &Fingerprint) -> Option<Vec<u8>> {
        self.files
            .borrow()
            .get(&Self::key(world_id, fingerprint))
            .cloned()
    }

    /// Perform a write, running `during` at the exact moment the file has
    /// been created and its content has not yet been committed.
    ///
    /// This is the whole reason the crate has an in-memory store. On the real
    /// platform that window is real, reachable by any other tab, and
    /// impossible to schedule from a test; here it is a closure.
    ///
    /// `during` receives `&self`, so it can do anything a second tab could —
    /// read the half-written name, ask its shape, try to delete it.
    pub async fn write_interleaved<F>(
        &self,
        world_id: Uuid,
        fingerprint: &Fingerprint,
        sealed: &[u8],
        during: F,
    ) -> Result<()>
    where
        F: FnOnce(&Self),
    {
        // Step 1: `getFileHandle({create: true})`. The entry exists now, with
        // an empty byte sequence, and every other tab can see it.
        self.files
            .borrow_mut()
            .insert(Self::key(world_id, fingerprint), Vec::new());

        // The other tab gets its turn here.
        during(self);

        // Step 2: `close()`. The buffer replaces the entry's binary data in
        // one step — no prefix is ever observable.
        self.files
            .borrow_mut()
            .insert(Self::key(world_id, fingerprint), sealed.to_vec());
        Ok(())
    }
}

impl BlobStore for MemoryBlobStore {
    async fn write(&self, world_id: Uuid, fingerprint: &Fingerprint, sealed: &[u8]) -> Result<()> {
        self.write_interleaved(world_id, fingerprint, sealed, |_| {})
            .await
    }

    async fn read(&self, world_id: Uuid, fingerprint: &Fingerprint) -> Result<Option<Vec<u8>>> {
        let bytes = self.raw(world_id, fingerprint);
        if !BlobShape::of(bytes.as_ref().map(Vec::len)).is_readable() {
            return Ok(None);
        }
        Ok(bytes)
    }

    async fn shape(&self, world_id: Uuid, fingerprint: &Fingerprint) -> Result<BlobShape> {
        Ok(BlobShape::of(
            self.files
                .borrow()
                .get(&Self::key(world_id, fingerprint))
                .map(Vec::len),
        ))
    }

    async fn remove(&self, world_id: Uuid, fingerprint: &Fingerprint) -> Result<()> {
        self.files
            .borrow_mut()
            .remove(&Self::key(world_id, fingerprint));
        Ok(())
    }

    async fn list(&self, world_id: Uuid) -> Result<Vec<Fingerprint>> {
        Ok(self
            .files
            .borrow()
            .keys()
            .filter(|(world, _)| *world == world_id)
            .filter_map(|(_, name)| crate::paths::fingerprint_from_file_name(name).ok())
            .collect())
    }

    async fn remove_world(&self, world_id: Uuid) -> Result<()> {
        self.files
            .borrow_mut()
            .retain(|(world, _), _| *world != world_id);
        Ok(())
    }

    async fn remove_scope(&self) -> Result<()> {
        self.files.borrow_mut().clear();
        Ok(())
    }
}
