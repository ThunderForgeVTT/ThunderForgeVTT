# Implementation Plan: Blob I/O in a Dedicated Worker

**Branch**: `043-worker-blob-store` | **Date**: 2026-09-08 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/043-worker-blob-store/spec.md`

> **Outcome: stopped at the Phase 3 gate on 2026-09-08.** SC-002 was not met on
> either browser and the synchronous path was slower than the shipped one at
> 8 MB on Chromium. See [measurements.md](./measurements.md). The plan below is
> kept as written, because what it planned for is exactly what happened.

## Summary

Move the client world cache's blob reads, writes and deletions into a dedicated
Web Worker so they can use OPFS synchronous access handles — which are faster
for large blobs, do not run on the thread that draws the canvas, and, per
[research R1](./research.md), are **more widely supported than the call the
cache uses today**.

The `BlobStore` trait in `crates/thunderforge-opfs` is the seam. A third
implementation sits behind it; the worker hosts it; the main thread talks to
the worker through a small correlated message protocol and falls back to the
existing store when no worker can be started.

The benchmark is built first and can end the feature. That is the plan's
shape, not a caveat on it.

## Technical Context

**Language/Version**: Rust (wasm32-unknown-unknown) for the store and worker
payload; TypeScript for the worker host and lifecycle.

**Primary Dependencies**: `crates/thunderforge-opfs` (the `BlobStore` seam and
its in-memory implementation), `crates/thunderforge-cache-browser` (the caller;
keeps crypto, index, sync and Web Locks), `wasm-bindgen` / `js-sys` /
`web-sys`.

**Storage**: OPFS, via synchronous access handles on the fast path and
`createWritable` on the fallback. No server-side component.

**Testing**: `cargo test` natively for the store's rules through the in-memory
implementation; `wasm-pack test` in a headless browser for the platform calls;
Playwright for the two-tab and fallback-path cases; a purpose-built benchmark
for SC-002.

**Target Platform**: Browser. Chromium is the reference; Firefox and Safari are
a stretch that this work makes plausible rather than promises.

**Project Type**: Client-side library plus a worker host in the web app.

**Performance Goals**: SC-002 — at least 2× write throughput for blobs ≥ 1 MB
against the existing store. SC-001 — frame pacing within 10% of the
caching-disabled baseline while a cold scene loads.

**Constraints**: No duplication of the engine bundle (FR-021 of the spec); no
relaxation of the page's isolation posture (FR-022); Web Locks stay with the
caller and are not taken inside the store (FR-015).

**Scale/Scope**: Three blob operations, one message protocol, one capability
probe, one benchmark. No change to what is cached, when, or what is evicted.

## Constitution Check

*GATE: must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Verdict | Note |
|---|---|---|
| **I — ECS owns simulation, React owns chrome** | ✅ Pass | Touches neither. The cache is beneath both; no canvas state moves and no simulation logic is added to a presentation component. |
| **II — Plugin-modular engine architecture** | ✅ Pass | No engine plugin is added or modified. The worker is a sibling of the engine, not a part of it. |
| **III — Ownership & authorization at the data boundary** | ✅ Pass | No server surface, no new table, no authorization decision. The worker moves opaque bytes it cannot decrypt, which is the existing crate boundary and is preserved deliberately (spec FR-002). |
| **IV — Real ADRs and specs before divergent implementation** | ⚠️ Requires ADRs | This changes an established boundary — where blob I/O executes — and reinterprets spec 028 FR-021. Two ADRs must land **with** the implementation, not after: one for the worker boundary and the protocol, one recording what FR-021 means under exclusive locking. Listed as Phase 2 obligations below. |
| **V — Verify before claiming done** | ✅ Pass, with a named risk | The engine crate compiles only under wasm32 and this feature adds a second wasm target. `make lint` already lints host and wasm separately; the new artefact must join `lint-wasm` or it will be linted by nothing. Called out because it is exactly the gap Principle V exists for. |
| **Browser support: Chromium only** | ✅ Pass, and possibly relaxable | Nothing here narrows support. R1 argues the change may widen it, which is why the stretch exists — but the constitution's constraint stands until each of WebCrypto, IndexedDB and OPFS has been verified on a second engine, which is beyond this feature. |

**Gate result: PASS**, with the two ADRs recorded as mandatory deliverables
rather than as follow-up.

## Project Structure

### Documentation (this feature)

```text
specs/043-worker-blob-store/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   ├── worker-protocol.md
│   └── capability-probe.md
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks — not created here)
```

### Source Code (repository root)

```text
crates/thunderforge-opfs/
├── src/
│   ├── store.rs             # BlobStore trait — unchanged
│   ├── memory.rs            # in-memory implementation — unchanged, still the
│   │                        #   native test bed for the concurrency rules
│   ├── opfs.rs              # existing async store — becomes the fallback
│   ├── sync_access.rs       # NEW: the synchronous-handle store
│   ├── protocol.rs          # NEW: request/response types, versioned
│   ├── quota.rs             # existing
│   └── paths.rs             # existing
└── tests/
    └── concurrent_writes.rs # extended for the exclusive-lock model

crates/thunderforge-cache-browser/
└── src/
    ├── opfs.rs              # crypto over the store — picks a store, unchanged otherwise
    └── worker_client.rs     # NEW: the main-thread side of the protocol

apps/web/
├── src/
│   └── workers/
│       └── blobStore.worker.ts   # NEW: worker host; loads the storage wasm only
└── e2e/
    └── world-cache-worker.spec.ts # NEW: fast path, fallback path, two tabs

scripts/
└── bench-blob-store.mjs     # NEW: SC-002's measurement
```

## Phases

### Phase 0 — Research ✅ complete

[research.md](./research.md). Both spec open questions resolved; one finding
(R1) reversed the feature's framing from "an optimisation with a fallback" to
"the portable path, with a safety net".

### Phase 1 — Design & contracts ✅ complete

- [data-model.md](./data-model.md) — the entities crossing the boundary.
- [contracts/worker-protocol.md](./contracts/worker-protocol.md) — the message
  contract, its versioning, and what each end does with a message it does not
  understand.
- [contracts/capability-probe.md](./contracts/capability-probe.md) — how the
  fast path is chosen once per session, and every reason it may not be.
- [quickstart.md](./quickstart.md) — the scenarios that validate this by hand.

### Phase 2 — Implementation obligations (for `/speckit-tasks`)

Ordered so that the gate comes first.

1. **The benchmark, against the existing store, on two browsers.** It is the
   deliverable that can end the feature (SC-002), and it measures the shipped
   store either way.
2. **ADR: the worker boundary.** What crosses it, why the store and not the
   crypto, and what the protocol guarantees. Principle IV.
3. **ADR: FR-021 under exclusive locking.** What the guarantee means when a
   reader is refused rather than shown an empty file, and why
   `BlobShape::Incomplete` survives regardless. Principle IV.
4. `sync_access.rs` behind the existing trait, with the in-memory
   implementation still carrying the concurrency tests.
5. The protocol and the worker host, including the `lint-wasm` entry the new
   artefact needs (Principle V).
6. The capability probe and the fallback, with the forced-off test that keeps
   the fallback honest.
7. The regression guard: a test that fails when a build stops using the fast
   path where it is available (spec FR-020 / SC-007), demonstrated by making it
   fail on purpose.
8. Bundle measurement and the report FR-021 asks for.
9. **Stretch**: run the suite on Firefox, and record what else blocks it. The
   deliverable is the list, not the support claim.

## Risks

| Risk | Handling |
|---|---|
| The speed gain is not there | The benchmark runs first. SC-002 states the threshold that stops the feature, and the spec is written so stopping is a success. |
| A silent regression to the slow path | The likeliest failure and the hardest to see. FR-020 requires a test that fails on it, demonstrated deliberately. |
| The second wasm artefact bloats the bundle | The worker loads the storage crate only. Measured and reported, not assumed. |
| FR-021 is weakened without anyone noticing | Its own ADR, and the in-memory implementation keeps the interleaving tests native and cheap to run. |
| The new artefact is linted by nothing | Principle V's named gap. `lint-wasm` entry is an implementation task, not a cleanup. |
| The worker cannot start in the field | The fallback exists for exactly this and for nothing else (R3), and its hit rate is worth reporting once shipped. |
