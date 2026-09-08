# Contract: Choosing the path

**Feature**: 043-worker-blob-store

## When

Once per session, before the first blob operation. Never per operation
(spec FR-007).

## What is checked, in order

| Check | Failure verdict |
|---|---|
| A dedicated worker can be constructed and answers a handshake | `WorkerUnavailable` |
| The context is secure | `InsecureContext` |
| `createSyncAccessHandle` exists and a probe file can be opened, written, flushed, read back and closed | `SyncAccessUnavailable` |

The third check is a real round trip on a real file, not a feature-detect. A
method that exists and throws on use is the case a feature-detect misses, and
this whole feature is about a call that is fussier about its context than most.

The probe file is written under a reserved name and removed afterwards, and its
failure to be removed is not itself a failure of the probe.

## What follows

- **All three pass** → `SyncAccessInWorker`.
- **Any fails** → `AsyncOnMainThread`, with the reason retained for reporting.

## Rules

1. **A fallback is never surfaced to the user.** It is a performance
   characteristic, not an error (spec FR-009). It appears in a diagnostic
   panel, and nowhere a player would see it.
2. **The verdict is reported, not just used** (FR-011). "The cache is slow" and
   "the cache is slow *because the worker could not start*" are different
   support conversations.
3. **A worker that dies later moves the session to the fallback** and does not
   re-probe. It may die again for the same reason, and probing per operation is
   the cost this design exists to avoid.
4. **The fallback exists for contexts, not browsers.** Per
   [research R1](../research.md), every browser that lacks synchronous access
   handles also lacks `createWritable`, so there is no browser the fallback
   rescues. Its reason for existing is a worker that cannot start.
5. **Forcing the path is a test affordance and must exist.** The fallback is
   otherwise exercised by nothing on the reference browser, which is how a
   well-tested, entirely uninvoked path gets shipped — a mistake this cache has
   already made twice by its own record.
