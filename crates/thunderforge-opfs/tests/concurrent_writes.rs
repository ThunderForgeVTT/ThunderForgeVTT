//! One tab reading while another writes (T055, FR-021).
//!
//! These are the tests the storage layer could not have before, and they are
//! the reason it is a crate: the window they open — a file created but not
//! yet committed — is real on the platform, reachable by any other tab, and
//! impossible to schedule from a browser test. `write_interleaved` makes it a
//! closure.
//!
//! The platform behaviour being relied on, from the WHATWG File System
//! Standard:
//!
//! - `getFileHandle(name, {create: true})` appends an entry with an empty
//!   byte sequence and resolves. Every same-origin tab sees a zero-length
//!   file at the final name from that moment.
//! - `createWritable()` buffers into a swap file and `close()` replaces the
//!   entry's binary data wholesale, so a reader never observes a prefix.
//!
//! Empty is therefore the *only* incomplete state that exists, which is what
//! makes "an empty file is never finished" a complete rule rather than a
//! heuristic about sizes.

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use thunderforge_cache_core::Fingerprint;
use thunderforge_opfs::memory::MemoryBlobStore;
use thunderforge_opfs::store::{BlobShape, BlobStore};
use uuid::Uuid;

/// Drive a future to completion on this thread.
///
/// No dependency and no executor, because none is needed: every future in
/// this crate's in-memory store completes without ever yielding — they are
/// `async` only to match the shape of the browser implementation, which does
/// await the platform. A future that never pends needs exactly one poll, and
/// this asserts that rather than assuming it, so a future that *did* start
/// yielding would fail loudly here instead of hanging.
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => {
            panic!("the in-memory store is not supposed to yield; this test harness cannot wait")
        }
    }
}

fn world() -> Uuid {
    Uuid::from_u128(0x0281_0000_0000_0000_0000_0000_0000_0055)
}

fn fingerprint_of(bytes: &[u8]) -> Fingerprint {
    Fingerprint::of_bytes(bytes)
}

const CONTENT: &[u8] = b"the sealed bytes of a scene background";

/// The bug this task exists for, as a test.
///
/// Before the guard, the reading tab found a file, read zero bytes, concluded
/// "will not decrypt", and **deleted it** — destroying the write the other
/// tab was in the middle of. The read is supposed to answer "not here yet",
/// and to leave the file entirely alone.
#[test]
fn a_reader_does_not_delete_the_file_another_tab_is_writing() {
    let store = MemoryBlobStore::new();
    let fingerprint = fingerprint_of(CONTENT);

    block_on(async {
        store
            .write_interleaved(world(), &fingerprint, CONTENT, |mid_write| {
                // Everything in here is the second tab, running at the one
                // moment the file exists and is empty.
                let seen = block_on(mid_write.read(world(), &fingerprint)).unwrap();
                assert_eq!(
                    seen, None,
                    "a half-written blob must never read as content, complete or otherwise"
                );

                // The point of the whole task: the reader that could not read
                // it must not have reclaimed it either.
                assert!(
                    mid_write.raw(world(), &fingerprint).is_some(),
                    "the reader deleted the file the writer was still writing"
                );
            })
            .await
            .expect("the write should succeed");

        // And the writer's bytes survived the other tab's visit intact.
        assert_eq!(
            store.read(world(), &fingerprint).await.unwrap().as_deref(),
            Some(CONTENT),
            "the completed write must be readable afterwards"
        );
    });
}

/// A reader must not be able to *skip a fetch* because of a file that has no
/// content in it.
///
/// This is the same bug wearing different clothes, and it is live in the
/// engine's prefetch path: it asks whether a blob exists and `continue`s if
/// so. An existence check that answers "yes" for a zero-length file means the
/// prefetch skips that asset forever — the file is never completed by anyone,
/// because everyone believes it is already there.
#[test]
fn an_empty_file_is_not_a_reason_to_skip_a_fetch() {
    let store = MemoryBlobStore::new();
    let fingerprint = fingerprint_of(CONTENT);

    block_on(async {
        store
            .write_interleaved(world(), &fingerprint, CONTENT, |mid_write| {
                let shape = block_on(mid_write.shape(world(), &fingerprint)).unwrap();
                assert_eq!(shape, BlobShape::Incomplete);
                assert!(
                    !shape.is_readable(),
                    "an incomplete file must not satisfy a caller asking whether to fetch"
                );
            })
            .await
            .unwrap();

        assert_eq!(
            store.shape(world(), &fingerprint).await.unwrap(),
            BlobShape::Complete,
            "once committed, the same name is a reason to skip the fetch"
        );
    });
}

/// No reader ever sees a prefix of the bytes.
///
/// Guaranteed by the platform rather than by us — `close()` replaces the
/// entry's binary data in one step — but asserted here because the whole
/// design of the guard rests on it. If this ever became false, "empty is the
/// only incomplete state" would stop being true and a length check would not
/// be enough.
#[test]
fn a_reader_never_observes_a_partial_prefix() {
    let store = MemoryBlobStore::new();
    let fingerprint = fingerprint_of(CONTENT);

    block_on(async {
        store
            .write_interleaved(world(), &fingerprint, CONTENT, |mid_write| {
                let raw = mid_write
                    .raw(world(), &fingerprint)
                    .expect("the entry exists");
                assert!(
                    raw.is_empty(),
                    "the only intermediate state is empty; saw {} bytes",
                    raw.len()
                );
            })
            .await
            .unwrap();
    });
}

/// An abandoned write repairs itself on the next attempt, which is why the
/// reader is allowed to leave it alone.
///
/// Not deleting an incomplete file would be a leak if nothing ever cleaned it
/// up. Nothing has to: the name is the fingerprint of the content, so the
/// next write of that content targets the same name and replaces it.
#[test]
fn an_abandoned_write_is_repaired_by_the_next_write_of_the_same_content() {
    let store = MemoryBlobStore::new();
    let fingerprint = fingerprint_of(CONTENT);

    block_on(async {
        // A write that created its file and died — the tab was closed.
        store
            .write_interleaved(world(), &fingerprint, CONTENT, |_| {})
            .await
            .unwrap();
        store.remove(world(), &fingerprint).await.unwrap();
        store
            .write_interleaved(world(), &fingerprint, b"", |_| {})
            .await
            .unwrap();
        assert_eq!(
            store.shape(world(), &fingerprint).await.unwrap(),
            BlobShape::Incomplete,
            "precondition: an abandoned, empty file is sitting at the name"
        );

        // The next fetch of that asset writes it again, to the same name.
        store.write(world(), &fingerprint, CONTENT).await.unwrap();

        assert_eq!(
            store.read(world(), &fingerprint).await.unwrap().as_deref(),
            Some(CONTENT),
            "the abandoned file must not block the name it occupies"
        );
    });
}

/// Two tabs writing the same fingerprint is not a conflict worth preventing.
///
/// `createWritable` takes a *shared* lock, so both writes succeed and the last
/// `close` wins — and in Firefox and Safari there is no exclusion available at
/// all. That is safe here for a reason specific to this cache: the filename is
/// the hash of the content, so two tabs writing the same name are writing
/// identical bytes. Last-write-wins between two identical writes is not a lost
/// update.
#[test]
fn two_writers_of_one_fingerprint_are_writing_the_same_bytes() {
    let store = MemoryBlobStore::new();
    let fingerprint = fingerprint_of(CONTENT);

    block_on(async {
        store
            .write_interleaved(world(), &fingerprint, CONTENT, |mid_write| {
                // The second tab completes its own write of the same content
                // while the first is still in flight.
                block_on(mid_write.write(world(), &fingerprint, CONTENT)).unwrap();
            })
            .await
            .unwrap();

        assert_eq!(
            store.read(world(), &fingerprint).await.unwrap().as_deref(),
            Some(CONTENT),
        );
        assert_eq!(store.len(), 1, "two writers must not leave two files");
    });
}

/// Removal is by name, and one world's eviction cannot reach another's.
#[test]
fn worlds_are_evicted_independently() {
    let store = MemoryBlobStore::new();
    let other = Uuid::from_u128(0x0281_0000_0000_0000_0000_0000_0000_0099);
    let fingerprint = fingerprint_of(CONTENT);

    block_on(async {
        store.write(world(), &fingerprint, CONTENT).await.unwrap();
        store.write(other, &fingerprint, CONTENT).await.unwrap();

        store.remove_world(world()).await.unwrap();

        assert_eq!(store.read(world(), &fingerprint).await.unwrap(), None);
        assert_eq!(
            store.read(other, &fingerprint).await.unwrap().as_deref(),
            Some(CONTENT),
            "evicting one world must not touch another's copy of the same content"
        );
    });
}

/// `list` reports incomplete files too, because a repair pass that could not
/// see them could never reclaim an abandoned one.
#[test]
fn list_reports_incomplete_files_so_repair_can_see_them() {
    let store = MemoryBlobStore::new();
    let fingerprint = fingerprint_of(CONTENT);

    block_on(async {
        store
            .write_interleaved(world(), &fingerprint, CONTENT, |mid_write| {
                let listed = block_on(mid_write.list(world())).unwrap();
                assert_eq!(
                    listed,
                    vec![fingerprint],
                    "an in-flight file is physically present and must be listed"
                );
            })
            .await
            .unwrap();
    });
}
