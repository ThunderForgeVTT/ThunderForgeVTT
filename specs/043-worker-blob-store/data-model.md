# Phase 1 — Data Model: Blob I/O in a Dedicated Worker

**Feature**: 043-worker-blob-store · **Date**: 2026-09-08

No database, no server entity, no persisted schema. What follows is what
crosses the worker boundary and what the main thread remembers about it.

---

## BlobRequest

One operation to perform. Sent main thread → worker.

| Field | Meaning | Notes |
|---|---|---|
| `protocol_version` | Which contract this message speaks | A mismatch is refused, never guessed at (spec FR-003) |
| `correlation_id` | Ties a response to its request | Required; concurrent operations are otherwise indistinguishable (FR-004) |
| `scope` | The user scope the blob belongs to | Fixed when the store opens, so a request cannot reach another user's bytes by naming one |
| `world_id` | Which world | Directories are per world so eviction can stay coarse |
| `fingerprint` | Content address of the blob | Also its filename |
| `op` | `Read` · `Write` · `Delete` | Closed set |
| `bytes` | Sealed payload, `Write` only | Opaque. The worker cannot decrypt it (FR-002) |

**Rules**

- `bytes` is present for `Write` and absent otherwise; a `Read` carrying a
  payload is a malformed message, not a request to ignore the payload.
- Requests for the same `fingerprint` are applied in the order sent (FR-005).
- Payloads above the transfer threshold are handed over without copying
  (FR-006); below it, copying is cheaper than the ceremony.

## BlobResponse

The outcome of exactly one request. Sent worker → main thread.

| Field | Meaning |
|---|---|
| `protocol_version` | As above |
| `correlation_id` | The request this answers |
| `outcome` | `Bytes` · `Absent` · `Failed` |
| `bytes` | Present only for `Bytes` |
| `failure` | Present only for `Failed`: a reason, flattened to a string |

**Rules**

- **`Absent` is not a failure.** It is the ordinary answer for a cache miss,
  for a blob whose write never completed, and for a read refused because
  another tab holds the write lock (FR-014). Callers respond to all three the
  same way: fetch it from the network.
- A `Failed` response never carries partial bytes. A truncated read is
  reported as `Absent`, because a short blob is indistinguishable from a write
  in flight and must not be treated as data (FR-013).
- No response is emitted for a request the worker could not parse; the version
  mismatch is reported once, on the channel, not per message.

## CapabilityVerdict

Computed once per session, on the main thread, before any request is sent.

| Field | Meaning |
|---|---|
| `path` | `SyncAccessInWorker` · `AsyncOnMainThread` |
| `reason` | Why, when the answer is the fallback |

`reason` is one of a closed set: `WorkerUnavailable`, `SyncAccessUnavailable`,
`InsecureContext`, `WorkerDied`. It exists to be reported (FR-011), so a
diagnostic screen can say why the cache is slow instead of leaving somebody to
guess.

**Rule**: computed once, never per operation (FR-007). A probe on the hot path
would spend on every write what it exists to save.

## MeasurementRecord

The benchmark's output. Not a product surface — a developer artefact that must
be reproducible.

| Field | Meaning |
|---|---|
| `store` | Which implementation produced the figure |
| `blob_size` | The size class measured |
| `throughput` | Bytes per second, read and write reported separately |
| `browser`, `machine`, `corpus` | Without these the number is not reproducible (FR-018) |

---

## State: the worker session

```text
                 probe
   (no worker) ─────────► AsyncOnMainThread ──── terminal for the session
        │
        └── worker starts, sync access present
                 │
                 ▼
          SyncAccessInWorker ──── worker dies ───► AsyncOnMainThread
                 │                                      │
                 │                                      └── in-flight request
                 │                                          resolves Failed,
                 │                                          never "succeeded"
                 └── ordinary operation
```

**The one transition that matters**: a worker dying mid-operation moves the
session to the fallback *and* fails the request that was in flight. It must not
be retried silently into the fallback, because the write may have partly
happened; the caller decides, and the next write of the same fingerprint
repairs it (spec FR-010, FR-013).

The transition is one-way within a session. A worker that died once may die
again, and re-probing per operation is the cost this design exists to avoid.
