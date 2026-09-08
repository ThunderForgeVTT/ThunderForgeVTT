---

description: "Task list for 043-worker-blob-store"
---

# Tasks: Blob I/O in a Dedicated Worker

**Input**: Design documents from `/specs/043-worker-blob-store/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/)

**Tests**: Included, and not optional here. FR-017 through FR-020 *are*
requirements about tests — the benchmark, the native concurrency coverage and
the silent-regression guard are deliverables of this feature, not verification
of it.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: US1, US2, US3, US4 per spec.md; SG for the stretch
- Every task names the file it touches

## Path Conventions

Rust storage crates under `crates/`, the web app under `apps/web/`, scripts
under `scripts/`, ADRs under `docs/adrs/`. Per plan.md § Source Code.

## Ordering note: the gate comes first

US1, US2 and US3 are all P1, so priority alone does not order them. **US3 runs
first anyway**, because SC-002 can end this feature and the number should
arrive before the architecture is committed to. Phase 3 ends in an explicit
decision point.

---

## Phase 1: Setup

**Purpose**: The scaffolding both the benchmark and the worker need.

- [ ] T001 Create `scripts/bench-blob-store.mjs` as a runnable stub that parses `--store`, `--sizes` and `--out`, and exits non-zero with usage when given neither store name
- [ ] T002 [P] Add a `bench-blob-store` target to `Makefile` beside `test-mail`, documenting in its comment that it needs a secure context and therefore `pnpm dev` rather than a file:// page
- [ ] T003 [P] Add `specs/043-worker-blob-store/` results directory convention to `.gitignore` for raw benchmark output, keeping only the committed summary

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Decisions on paper, shared types, and the lint wiring the new
artefact needs. **No user story work begins until this phase is complete.**

**⚠️ Principle IV**: the two ADRs land *with* the implementation, not after it.

- [ ] T004 Write ADR-092 (blob I/O crosses a worker boundary) in `docs/adrs/20260908-092-blob_io_worker_boundary.md`: what crosses, why the store and not the crypto, what the protocol guarantees, and why the engine bundle is not duplicated
- [ ] T005 Write ADR-093 (FR-021 under exclusive locking) in `docs/adrs/20260908-093-fr021_under_exclusive_locking.md`: what spec 028's guarantee means when a reader is refused rather than shown an empty file, and why `BlobShape::Incomplete` survives regardless — a write killed mid-flight still leaves a short file
- [ ] T006 [P] Add both ADRs to the index table in `docs/adrs/README.md`, confirming 092 and 093 are still free
- [ ] T007 Add `protocol.rs` to `crates/thunderforge-opfs/src/` with `BlobRequest`, `BlobResponse`, `Outcome` and `PROTOCOL_VERSION` per [data-model.md](./data-model.md), with native tests for serialisation round-trip and for refusing an unknown version
- [ ] T008 [P] Add a native test in `crates/thunderforge-opfs/src/protocol.rs` asserting a `Read` request carrying a payload is rejected as malformed rather than silently ignoring the payload
- [ ] T009 Declare the worker wasm artefact in the build (`scripts/build.mjs`), targeting `thunderforge-opfs` only, and assert in the build that the engine crate is **not** in its dependency graph
- [ ] T010 Add the new wasm artefact to `lint-wasm` in `Makefile` — Principle V's named risk; without this entry nothing lints it
- [ ] T011 [P] Extend `crates/thunderforge-opfs/src/store.rs` docs to name the third implementation and state which of the three is the reference for correctness (the in-memory one)

**Checkpoint**: types, decisions and lint wiring exist; stories can start.

---

## Phase 3: User Story 3 — The speed claim is measured, not asserted (Priority: P1) 🚦 GATE

**Goal**: A number that decides whether the rest of this feature is built.

**Independent Test**: Run the benchmark against the existing store and read
throughput figures for three blob sizes, with the browser, machine and corpus
named. Valuable even if the feature stops here.

- [ ] T012 [US3] Derive the benchmark corpus from a real world's asset distribution — record the sizes chosen and where they came from in `specs/043-worker-blob-store/measurements.md`, rather than inventing a synthetic curve
- [ ] T013 [US3] Implement the write and read loops in `scripts/bench-blob-store.mjs` against the existing async store, reporting bytes per second per size class, read and write separately
- [ ] T014 [P] [US3] Emit browser, machine and corpus alongside every figure in `scripts/bench-blob-store.mjs` (FR-018) — a throughput number without them is not reproducible and does not count
- [ ] T015 [US3] Run it on Chromium and record the results in `specs/043-worker-blob-store/measurements.md`
- [ ] T016 [US3] Run it on Firefox and record the results in `specs/043-worker-blob-store/measurements.md`, **including the case where the existing store does not work there at all** — research R1 predicts this and it is the more interesting outcome (also serves SG-001)
- [ ] T017 [US3] Add a `--store=sync` path to the benchmark that is skipped with a clear message until Phase 4 lands, so the harness is complete before the implementation it measures

**🚦 CHECKPOINT — decision point.** With the async figures in hand and the sync
path implemented (T028), compare. **If writes of 1 MB and above are not at
least twice as fast, SC-002 is not met: stop the feature, keep the benchmark
and the measurements, and record the decision in `measurements.md`.** That is a
successful outcome, not a failed one.

---

## Phase 4: User Story 1 — The table keeps moving while the cache fills (Priority: P1)

**Goal**: Blob bytes are written somewhere other than the thread that draws the
canvas.

**Independent Test**: Load a cold scene above the large-blob threshold while
dragging continuously; compare frame pacing against the caching-disabled
baseline.

### Tests for User Story 1

- [ ] T018 [P] [US1] Add a headless-browser test in `crates/thunderforge-opfs/tests/sync_access.rs` asserting a write is readable back byte-identical through a synchronous handle
- [ ] T019 [P] [US1] Add a test in `crates/thunderforge-opfs/tests/sync_access.rs` asserting a handle is closed on the failure path, by forcing a write error and then opening the same blob again — a leaked handle locks that blob for every tab for the life of the worker, which is the most damaging mistake this design allows
- [ ] T020 [P] [US1] Add a test asserting a write closed **without** `flush()` reads back as absent rather than short (contract rule 4)

### Implementation for User Story 1

- [ ] T021 [US1] Implement `SyncAccessBlobStore` in `crates/thunderforge-opfs/src/sync_access.rs` behind the existing `BlobStore` trait, using the default exclusive `readwrite` mode and **not** passing the `mode` option (research R2)
- [ ] T022 [US1] Guarantee `flush()` then `close()` on every path including early returns, structured so the close cannot be skipped by a `?`
- [ ] T023 [US1] Create the worker host `apps/web/src/workers/blobStore.worker.ts`, loading the storage wasm artefact only
- [ ] T024 [US1] Implement the main-thread client in `crates/thunderforge-cache-browser/src/worker_client.rs`: correlation ids, one response per request, and no inference from arrival order
- [ ] T025 [US1] Serialise writes to one fingerprint inside `apps/web/src/workers/blobStore.worker.ts` (FR-005) — the channel alone does not order them once payloads are transferred
- [ ] T026 [US1] Transfer payloads above the threshold rather than copying, in `crates/thunderforge-cache-browser/src/worker_client.rs` (FR-006), and record the threshold's origin in a comment
- [ ] T027 [US1] Wire `crates/thunderforge-cache-browser/src/opfs.rs` to use the worker client when the probe says so, keeping crypto and fingerprint verification exactly where they are (FR-002)
- [ ] T028 [US1] Enable `--store=sync` in `scripts/bench-blob-store.mjs` and run the Phase 3 checkpoint comparison
- [ ] T029 [US1] Add `apps/web/e2e/world-cache-worker.spec.ts` covering a cold scene load that caches through the worker and reads back identical on reload
- [ ] T030 [US1] Measure frame pacing during a cold load against the caching-disabled baseline and record it in `measurements.md` (SC-001)

**Checkpoint**: bytes are written off the main thread and the scene still loads.

---

## Phase 5: User Story 2 — Nothing gets worse where the fast path is unavailable (Priority: P1)

**Goal**: Every context that works today still works, unchanged.

**Independent Test**: Force the fallback and run the cache suite; results must
be identical to today's.

### Tests for User Story 2

- [ ] T031 [P] [US2] Add a test in `crates/thunderforge-cache-browser/src/worker_client.rs` asserting the probe runs once per session and not per operation (FR-007), by counting probe invocations across many operations
- [ ] T032 [P] [US2] Add a test in `crates/thunderforge-cache-browser/src/worker_client.rs` asserting a worker that dies mid-operation fails the in-flight request rather than reporting success, and that the next operation proceeds on the fallback without a reload (FR-010)
- [ ] T033 [P] [US2] Add a vitest case in `apps/web/src/components/diagnostics/__tests__/storagePanel.test.tsx` asserting no fallback verdict reaches any user-facing surface (FR-009, SC-004)

### Implementation for User Story 2

- [ ] T034 [US2] Implement the capability probe in `crates/thunderforge-cache-browser/src/worker_client.rs` per [contracts/capability-probe.md](./contracts/capability-probe.md) — a real round trip on a real file, not a feature-detect, because a method that exists and throws on use is the case a feature-detect misses
- [ ] T035 [US2] Implement the one-way session transition to the fallback in `crates/thunderforge-cache-browser/src/worker_client.rs`, with the reason retained for reporting (FR-011)
- [ ] T036 [US2] Add the `THUNDERFORGE_BLOB_STORE=async` override that forces the fallback — a test affordance that **must** exist, because on the reference browser nothing else exercises that path
- [ ] T037 [US2] Surface the verdict and its reason in the existing diagnostics panel `apps/web/src/components/diagnostics/StoragePanel.tsx`, and nowhere a player would see it
- [ ] T038 [US2] Extend `apps/web/e2e/world-cache-worker.spec.ts` with the forced-fallback run, asserting the same outcomes as the fast path
- [ ] T039 [US2] Run the full cache suite with the fallback forced (`THUNDERFORGE_BLOB_STORE=async node scripts/e2e-parallel.mjs --only=world-cache`) and confirm results identical to today's (SC-003)

**Checkpoint**: the cache degrades and never fails.

---

## Phase 6: User Story 4 — Two tabs still cannot corrupt each other (Priority: P2)

**Goal**: Spec 028 FR-021 holds under the new store, restated rather than
assumed.

**Independent Test**: The interleaving tests run natively against the in-memory
store's model of exclusive locking; a browser test drives two real tabs into
contention.

### Tests for User Story 4

- [ ] T040 [P] [US4] Extend `crates/thunderforge-opfs/src/memory.rs` to model an exclusive per-file lock, so the concurrency rules stay testable without a browser (FR-019)
- [ ] T041 [P] [US4] Extend `crates/thunderforge-opfs/tests/concurrent_writes.rs` for the exclusive model: a refused lock yields `Absent`, never an error and never a partial read
- [ ] T042 [P] [US4] Add a native test in `crates/thunderforge-opfs/tests/concurrent_writes.rs` asserting a short file left by an interrupted write is treated as absent and is **not** deleted by the reader (FR-013) — a write in flight elsewhere and a write that died are indistinguishable
- [ ] T043 [US4] Add a two-tab e2e in `apps/web/e2e/world-cache-worker.spec.ts` driving both tabs at one uncached blob, repeated enough to mean something (SC-005)
- [ ] T044 [US4] Add the killed-tab case to `apps/web/e2e/world-cache-worker.spec.ts`: close one tab mid-write and assert the other refetches rather than erroring or reclaiming

### Implementation for User Story 4

- [ ] T045 [US4] Map a refused lock to `Absent` in `sync_access.rs` (FR-014), never to a failure the caller must interpret
- [ ] T046 [US4] Assert by construction in `crates/thunderforge-opfs/src/sync_access.rs` that the store takes no cross-tab lock internally (FR-015) — Web Locks are not reentrant and every caller already holds one
- [ ] T047 [US4] Verify in `apps/web/e2e/world-cache-worker.spec.ts` that sign-out still renders cached content unreadable in every tab promptly with a worker holding handles open (FR-016, spec 028 FR-021b)

**Checkpoint**: concurrency guarantees restated and proven under the new store.

---

## Phase 7: Stretch — More browsers than one (SG-001, SG-002)

**Goal**: Find out what else blocks a second engine. **The list is the
deliverable; no support claim is made.**

- [ ] T048 [P] [SG] Run the cache suite on Firefox and record every failure by name in `specs/043-worker-blob-store/browser-support.md`
- [ ] T049 [P] [SG] Audit the cache's WebCrypto usage in `crates/thunderforge-cache-browser/src/crypto.rs` for anything Chromium-specific and record the finding in `specs/043-worker-blob-store/browser-support.md`
- [ ] T050 [P] [SG] Audit the cache's IndexedDB usage in `crates/thunderforge-cache-browser/src/index.rs` the same way, recording it in `specs/043-worker-blob-store/browser-support.md`
- [ ] T051 [SG] Write the summary in `specs/043-worker-blob-store/browser-support.md`: what now works on a second engine, what does not, and what each remaining blocker would cost — explicitly **not** a claim that a second browser is supported, and explicitly not a constitution amendment

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T052 Implement the silent-regression guard in `apps/web/e2e/world-cache-worker.spec.ts` (FR-020): a test that fails when a build stops using the fast path where it is available
- [ ] T053 Demonstrate T052 by making the probe in `crates/thunderforge-cache-browser/src/worker_client.rs` always return the fallback verdict and confirming the guard fails (SC-007) — record it in the commit body
- [ ] T054 Make the four quickstart guards fail on purpose per [quickstart.md](./quickstart.md) § "Making the guards fail on purpose" — leak a handle, skip `flush()`, match responses by arrival order, return `Failed` for a refused lock — and record that each was seen to bite
- [ ] T055 Measure compressed artefact growth before and after in `specs/043-worker-blob-store/measurements.md`, confirm the engine bundle is unchanged, and report the number rather than describing it as small (FR-021, SC-006)
- [ ] T056 Confirm no isolation headers were added to `apps/web/vite.config.mts` or the server's response headers (FR-022); if the fast path turned out to need them, record that as a finding and stop rather than adding them
- [ ] T057 [P] Document the storage path in `docs/` — where blob bytes are written, by which context, and how to tell which path an instance is on
- [ ] T058 [P] Update `crates/thunderforge-opfs/src/lib.rs` module docs: three implementations, which is the reference, and why the fallback exists (contexts, not browsers)
- [ ] T059 Run `cargo test --workspace -j 4`, `make lint` and `pnpm --filter @thunderforge/web test`
- [ ] T060 Run the full suite via `node scripts/e2e-parallel.mjs --shards=2` and record the figures in the commit body
- [ ] T061 Run `pnpm verify` and fix what it reports **in the code this feature added** — wide lint passes get their own commit

---

## Dependencies

```text
Phase 1 Setup
    │
    ▼
Phase 2 Foundational ── ADRs, protocol types, lint wiring
    │
    ▼
Phase 3 US3 (benchmark) ── 🚦 GATE: may end the feature
    │
    ▼
Phase 4 US1 (worker fast path) ── the architecture
    │
    ├──────────────► Phase 5 US2 (fallback + probe)
    │                     │
    └──────────────► Phase 6 US4 (concurrency)
                          │
                          ▼
                    Phase 7 SG (stretch)
                          │
                          ▼
                    Phase 8 Polish
```

**Story independence**: US2 and US4 are independent of each other and can run
in parallel once US1 lands. US3 depends on nothing but Setup, which is why it
goes first. US4's *native* half (T040–T042) needs only Phase 2 and can start
early — the in-memory model is where its rules live.

## Parallel execution examples

**Phase 2**: T006, T008 and T011 touch different files with no ordering between them.

**Phase 3**: T014 runs alongside T013.

**Phase 4 tests**: T018, T019 and T020 are three separate cases in one new file — write them together, then implement T021.

**Phase 6**: T040, T041 and T042 are all native and all in the storage crate; they need no browser and no worker.

**Phase 7**: T048, T049 and T050 are three independent audits.

## Implementation Strategy

**MVP is Phase 3 alone.** The benchmark against the existing store, on two
browsers, is a complete and useful increment: it measures a store nobody has
measured, it answers whether this feature is worth building, and it produces
the first evidence about a second browser. If SC-002 is not met, that is where
this stops and nothing after Phase 3 is wasted.

**Then Phase 4** for the architecture, **then 5 and 6 in parallel** for the
guarantees around it. Phase 7 is genuinely optional and Phase 8 closes.

The ADRs in Phase 2 are deliberately before the code rather than after it, per
Principle IV — in particular ADR-093, because "what does FR-021 mean now" is a
question that gets answered implicitly by an implementation if nobody answers
it explicitly first.
