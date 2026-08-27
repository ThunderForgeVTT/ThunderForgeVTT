//! Encrypted blobs on disk: this crate's crypto over `thunderforge-opfs`'s
//! storage.
//!
//! Spec `028-client-world-cache` (T024, T055).
//!
//! # What moved, and why
//!
//! The paths and the filesystem calls now live in `thunderforge-opfs`, behind
//! a [`BlobStore`] trait with an in-memory implementation beside the real
//! one. They left because every line of them was `#[cfg(target_arch =
//! "wasm32")]`, which meant the read, write and delete logic had no native
//! test at all — this crate's many passing tests were about path strings, and
//! the question FR-021 actually asks ("what does one tab see while another is
//! writing?") had no way to be asked.
//!
//! What stayed is what only this crate can do: seal the bytes, open them
//! again, and decide what a file that will not open deserves. Encryption
//! stayed on purpose rather than by omission — WebCrypto is exactly as
//! browser-bound as OPFS, so moving it would have put the storage crate back
//! where it started, and a store that cannot decrypt what it holds cannot
//! hand back plaintext by accident.
//!
//! # The rule this file adds on top of the store
//!
//! The store reports an incomplete file as absent and refuses to delete it
//! (see [`thunderforge_opfs::store::BlobShape`]). This file decides the other
//! case: a *complete* file that will not open, or that opens to bytes which
//! do not hash to their own filename, is garbage and is deleted. That
//! distinction is the whole of T055 — "will not open" used to mean both
//! things, so a reader could and did reclaim a file another tab was still
//! writing.

// The path and naming policy is re-exported so that this module remains the
// one place the rest of the crate asks about cache paths — moving the code
// out was not meant to move the vocabulary.
pub use thunderforge_opfs::paths::{
    BlobPath, PathError, UserScope, blob_file_name, fingerprint_from_file_name, world_dir_name,
};
pub use thunderforge_opfs::store::{BlobShape, BlobStore};

#[cfg(target_arch = "wasm32")]
pub use wasm::OpfsStore;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use thunderforge_cache_core::{Fingerprint, fingerprint};
    use thunderforge_opfs::opfs::OpfsBlobStore;
    use thunderforge_opfs::paths::UserScope;
    use thunderforge_opfs::store::{BlobShape, BlobStore};
    use uuid::Uuid;

    use crate::Result;
    use crate::crypto::SessionKey;

    /// The encrypted blob store rooted at this user's OPFS scope.
    pub struct OpfsStore {
        inner: OpfsBlobStore,
    }

    impl OpfsStore {
        /// Open (creating if absent) the scope directory for this user.
        pub async fn open(scope: UserScope) -> Result<Self> {
            Ok(Self {
                inner: OpfsBlobStore::open(scope).await?,
            })
        }

        /// The scope this store is confined to.
        pub fn scope(&self) -> &UserScope {
            self.inner.scope()
        }

        /// Store bytes, encrypted, at the path their fingerprint dictates.
        ///
        /// The `expected` fingerprint is what the *server* promised for this
        /// content. Verification happens before anything is encrypted or
        /// written, so bytes that are not what they claim never reach disk —
        /// and because the verified fingerprint is also the filename, a
        /// caller cannot file good bytes under the wrong name even by
        /// mistake (FR-010, FR-046).
        pub async fn write_blob(
            &self,
            world_id: Uuid,
            expected: &Fingerprint,
            plaintext: &[u8],
            key: &SessionKey,
        ) -> Result<()> {
            fingerprint::verify(plaintext, expected)?;
            let sealed = key.seal(plaintext).await?;
            self.inner.write(world_id, expected, &sealed).await?;
            Ok(())
        }

        /// Read and decrypt the blob for a fingerprint.
        ///
        /// Returns `Ok(None)` for every recoverable condition — absent file,
        /// a file no one has finished writing, a key we no longer hold,
        /// ciphertext that will not open, plaintext that does not hash to its
        /// own filename. All of them mean the same thing to a caller (fetch
        /// it again), and collapsing them is what makes key loss
        /// indistinguishable from a cold cache (FR-016c).
        ///
        /// # What gets deleted, and what emphatically does not
        ///
        /// Content that fails to *open* is deleted. Leaving it would mean
        /// re-reading and re-failing on it forever, and it can never become
        /// readable again: the filename is a claim about the plaintext, so
        /// plaintext that disagrees is not a different version of anything,
        /// it is garbage occupying budget.
        ///
        /// Content that was never finished is **not** deleted, and that is
        /// T055. The store answers `None` for a file that is too small to be
        /// anything we completed, and this path never reaches the delete for
        /// it. The two cases used to be one: a reader that found the
        /// zero-length file another tab had just created concluded "will not
        /// decrypt" and reclaimed it — with no lock held — destroying a write
        /// in progress and orphaning the index row that followed it.
        pub async fn read_blob(
            &self,
            world_id: Uuid,
            expected: &Fingerprint,
            key: &SessionKey,
        ) -> Result<Option<Vec<u8>>> {
            // `None` here is absent *or* incomplete. Neither is ours to
            // delete, and the store is what keeps those two straight.
            let Some(sealed) = self.inner.read(world_id, expected).await? else {
                return Ok(None);
            };

            let Some(plaintext) = key.open(&sealed).await? else {
                self.discard(world_id, expected).await;
                return Ok(None);
            };
            if fingerprint::verify(&plaintext, expected).is_err() {
                self.discard(world_id, expected).await;
                return Ok(None);
            }
            Ok(Some(plaintext))
        }

        /// Whether a *complete* blob exists, without decrypting it.
        ///
        /// Presence is not proof of readability — only [`Self::read_blob`]
        /// can establish that — so this is for repair and accounting.
        ///
        /// It answers `false` for a file that exists but was never finished.
        /// That matters because callers use this to decide a fetch can be
        /// skipped: an existence check that said `true` for a zero-length
        /// file would make the prefetch skip that asset forever, since
        /// nobody would ever complete the file everyone believed was there.
        pub async fn has_blob(&self, world_id: Uuid, fingerprint: &Fingerprint) -> Result<bool> {
            Ok(self.inner.shape(world_id, fingerprint).await?.is_readable())
        }

        /// What is at this name — absent, unfinished, or complete.
        ///
        /// Exposed for a repair pass, which is the one caller that needs to
        /// tell an abandoned file from an absent one.
        pub async fn blob_shape(
            &self,
            world_id: Uuid,
            fingerprint: &Fingerprint,
        ) -> Result<BlobShape> {
            Ok(self.inner.shape(world_id, fingerprint).await?)
        }

        /// Delete one blob. Absent is success — the postcondition is "not
        /// there", and it already holds.
        pub async fn remove_blob(&self, world_id: Uuid, fingerprint: &Fingerprint) -> Result<()> {
            self.inner.remove(world_id, fingerprint).await?;
            Ok(())
        }

        /// Every fingerprint physically present for a world.
        ///
        /// The ground truth the FR-019 repair pass diffs the index against.
        /// Files whose names are not ours are skipped rather than reported.
        pub async fn list_fingerprints(&self, world_id: Uuid) -> Result<Vec<Fingerprint>> {
            Ok(self.inner.list(world_id).await?)
        }

        /// Drop a world's bytes wholesale — the coarse eviction step
        /// (data-model.md, `BudgetPlan`: whole worlds before individual
        /// items).
        pub async fn remove_world(&self, world_id: Uuid) -> Result<()> {
            self.inner.remove_world(world_id).await?;
            Ok(())
        }

        /// Drop this user's entire cache directory.
        ///
        /// Sign-out (FR-016a) discards the *key* first and synchronously;
        /// this is the FR-016b reclamation that follows and may be slow. The
        /// ordering matters: a large store cannot be wiped instantly, and
        /// deletion alone would leave a readable window. Encryption is what
        /// closes that window, so the key must go first and this must not be
        /// treated as the thing that makes the cache unreadable.
        pub async fn remove_scope(&self) -> Result<()> {
            self.inner.remove_scope().await?;
            Ok(())
        }

        /// Best-effort removal of content that failed to open. The read
        /// already decided the answer is `None`; whether the delete lands is
        /// a matter of reclaiming space, not of correctness.
        async fn discard(&self, world_id: Uuid, fingerprint: &Fingerprint) {
            let _ = self.inner.remove(world_id, fingerprint).await;
        }
    }
}
