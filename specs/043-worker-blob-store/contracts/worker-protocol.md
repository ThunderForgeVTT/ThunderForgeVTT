# Contract: The worker message protocol

**Feature**: 043-worker-blob-store

One request, one response, correlated. Everything else in this document is a
consequence of that sentence.

## Shape

```text
main thread                                  dedicated worker
     │  BlobRequest { version, id, op, … }         │
     ├────────────────────────────────────────────►│
     │                                             │  open handle
     │                                             │  read / write / truncate
     │                                             │  flush            ← required
     │                                             │  close            ← always
     │  BlobResponse { version, id, outcome, … }   │
     │◄────────────────────────────────────────────┤
```

## Rules

1. **Every request carries a version; a mismatch is refused, not interpreted.**
   Both ends refuse a message whose version they do not know rather than acting
   on the fields they happen to recognise. A worker script and a page can be
   different builds — a cached worker, a deploy mid-session — and a protocol
   that guesses in that situation corrupts a cache instead of failing one
   operation.

2. **Every response carries the correlation id of its request.** Nothing is
   inferred from arrival order. Reads and writes overlap by design, and
   matching by position is the bug that only appears under load.

3. **The handle is closed on every path, including failure.** `readwrite` mode
   takes an exclusive lock, so a handle leaked by an early return locks that
   blob for the life of the worker — for every tab, not just this one. This is
   the single most damaging mistake available in this design.

4. **`flush()` is called before `close()` on every write.** Synchronous handles
   do not persist implicitly. A write that is closed without flushing may leave
   a short file, which readers correctly treat as absent — so the symptom is a
   cache that silently never works rather than an error.

5. **`Absent` is the answer for every "not usable" case**: no file, a file too
   short to be complete, and a lock held by another tab. Callers must not have
   to distinguish them, because they cannot act differently on them.

6. **A refused lock is never retried inside the worker.** The caller owns
   retry, and it already holds a Web Lock; spinning here would hold that lock
   while waiting on another tab that may be waiting on it.

7. **Writes to one fingerprint are serialised in the worker.** Ordering is
   promised by FR-005 and the message channel alone does not provide it once
   payloads are transferred rather than copied.

8. **Nothing that can decrypt crosses the boundary.** No key, no fingerprint
   verification, no plaintext. The worker moves sealed bytes and could not read
   them if it tried, which is what makes "the worker was compromised" a
   smaller sentence than it would otherwise be.

## What is deliberately absent

- **No streaming.** Blobs are bounded by the cache's own admission policy and a
  chunked protocol would add ordering and partial-failure states to buy nothing
  at these sizes.
- **No batch request.** One operation per message keeps failure attributable.
  If the round trip proves to be the cost, that is a measurement worth having
  first.
- **No progress events.** Nothing in the product can act on a half-written
  blob, and reporting progress would imply otherwise.
