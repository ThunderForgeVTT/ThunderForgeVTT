//! Encrypted-blob storage for the client world cache: paths, and a disk.
//!
//! Spec `028-client-world-cache` (T024, T055). Split out of
//! `thunderforge-cache-browser` so that the storage layer has a boundary and
//! a native test.
//!
//! # Why this is its own crate
//!
//! The cache's disk logic lived beside its crypto, its index, its sync and
//! its cross-tab signalling, and all of it was `#[cfg(target_arch =
//! "wasm32")]`. That combination has a specific cost: the crate reported
//! dozens of passing native tests and **not one of them touched a read, a
//! write, or a delete**. They were all about path strings, because path
//! strings were the only part that compiled off the browser.
//!
//! FR-021 asks a question that logic cannot answer — *what does one tab see
//! while another is writing?* — so the storage operations are named as a
//! trait ([`store::BlobStore`]) with two implementations: [`opfs`] against
//! the real platform, and [`memory`] against a map that can be told to
//! interleave. The race is then an ordinary test.
//!
//! # What lives here, and what deliberately does not
//!
//! Here: where a blob goes ([`paths`]), and how bytes get there and back
//! ([`store`], [`opfs`], [`memory`]).
//!
//! Not here: encryption, fingerprint verification, the index, the sync
//! protocol, and cross-tab locking. Encryption in particular stays in
//! `thunderforge-cache-browser`, for two reasons. WebCrypto is exactly as
//! browser-bound as OPFS, so admitting it would put this crate back where it
//! started; and **a store that cannot decrypt what it holds cannot
//! accidentally hand back plaintext.** This crate moves opaque bytes and has
//! no way to know what they mean.
//!
//! Locking stays out for a harder reason than tidiness: Web Locks are not
//! reentrant, so a store that quietly took the world lock inside `write`
//! would deadlock every caller that already holds it — which is all of them.

pub mod memory;
pub mod paths;
pub mod store;

#[cfg(target_arch = "wasm32")]
pub mod opfs;

pub use paths::{
    BlobPath, PathError, UserScope, blob_file_name, fingerprint_from_file_name, world_dir_name,
};
pub use store::{BlobShape, BlobStore, MIN_BLOB_LEN, StoreError};
