# Feature Specification: Blob I/O in a Dedicated Worker

**Feature Branch**: `043-worker-blob-store`

**Created**: 2026-09-08

**Status**: Draft

**Input**: User description: "Move the client world cache's blob I/O into a dedicated Web Worker so it can use the OPFS synchronous access handle API (`FileSystemFileHandle.createSyncAccessHandle`), which is markedly faster for large blobs and is unavailable on the main thread where the cache currently runs inside the engine wasm."

## Why this is a spec and not a change

The cache already has the seam this needs. `thunderforge-opfs` names storage as
a `BlobStore` trait with two implementations — the real platform and an
in-memory one that exists so the concurrency rules have a native test. A third
implementation is an afternoon.

What is not an afternoon is **where it has to run**. The synchronous access
handle API is available only inside a dedicated Web Worker, and the cache
currently runs inside the engine wasm on the main thread. Reaching the API
therefore means a second execution context, a message protocol between them, a
second wasm artefact to build and ship, and a decision about what the cache's
existing concurrency guarantee means once file locking changes shape. Those are
architecture, and the constitution's Principle IV asks for them on paper first.

There is also a real chance the right answer is **no**. The measurement in
US3 is not a formality: if the gain on realistic blobs is small, this feature
costs a worker, a protocol and a second bundle to buy nothing, and the honest
outcome is to stop. The spec is written so that outcome is a success.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The table keeps moving while the cache fills (Priority: P1)

A Game Master launches a scene with a large map and several hundred tokens.
The client caches the scene's assets as it loads them. Today that writing
happens on the same thread as the canvas, so the moments when the cache is
busiest are the moments the map is least willing to be dragged. The player
sees it as the map "catching" while a scene settles.

After this feature the bytes are written somewhere else entirely, and the
canvas is not asked to wait for them.

**Why this priority**: It is the whole point. Every other story here exists to
make this one safe or to prove it happened.

**Independent Test**: Load a scene whose assets exceed a set size with the
cache cold, drag continuously throughout, and compare frame pacing against the
same run with caching disabled. The gap between them is what this story closes.

**Acceptance Scenarios**:

1. **Given** a cold cache and a scene with assets above the large-blob
   threshold, **When** the scene loads while the user drags the map,
   **Then** frame pacing is materially closer to the no-caching baseline than
   it is today, and the assets are all present in the cache afterwards.
2. **Given** the same scene, **When** it is loaded a second time from cache,
   **Then** every asset reads back byte-identical to what was written.
3. **Given** a scene loading, **When** the user navigates away mid-write,
   **Then** no partially written blob is later read as complete.

---

### User Story 2 - Nothing gets worse where the fast path is unavailable (Priority: P1)

Some contexts cannot offer synchronous access handles: a browser that has not
implemented them, a page not served over a secure context, an environment where
a worker cannot be started. The cache must behave exactly as it does today in
all of them — same guarantees, same degradation, no error the user has to
understand.

**Why this priority**: Equal to US1 because a faster cache that fails anywhere
it did not fail before is not an improvement. `thunderforge-cache-browser`'s
existing posture is degrade, never fail, and this feature must not be the
exception.

**Independent Test**: Run the full cache suite against a build with the fast
path forced off and confirm the results are identical to the current suite.

**Acceptance Scenarios**:

1. **Given** a context where a dedicated worker cannot start, **When** the
   cache is used, **Then** every cache operation still succeeds through the
   existing path and nothing is surfaced to the user.
2. **Given** a context where workers start but synchronous access handles are
   unavailable, **When** the cache is used, **Then** the same is true.
3. **Given** the worker starts and then dies mid-session, **When** the next
   cache operation runs, **Then** it completes through the existing path, and
   the operation that was in flight is reported as failed rather than reported
   as having succeeded.

---

### User Story 3 - The speed claim is measured, not asserted (Priority: P1)

"Markedly faster for large blobs" is the entire justification for this
feature. Nobody has measured it on this codebase, with these blob sizes,
against this browser.

**Why this priority**: P1, and deliberately not last. The measurement is the
gate: it decides whether the rest of the feature is worth building, so it is
worth building a measurement before committing to the architecture.

**Independent Test**: A benchmark that writes and reads a fixed corpus of
blobs through both stores in the same session and reports both figures. It is
independently valuable even if the feature is abandoned, because it also
measures the store already in use.

**Acceptance Scenarios**:

1. **Given** the benchmark, **When** it runs, **Then** it reports throughput
   for both stores across at least three blob sizes spanning the range this
   cache actually stores.
2. **Given** the results, **When** the fast path is not materially faster on
   realistic sizes, **Then** the recorded outcome is that the feature stops —
   and the benchmark and the measurement remain.
3. **Given** the results, **When** they are recorded, **Then** they name the
   browser, the machine and the corpus, because a throughput figure without
   those is not reproducible.

---

### User Story 4 - Two tabs still cannot corrupt each other (Priority: P2)

Spec 028 FR-021 says concurrent access from more than one tab must not corrupt
the store nor allow a partially written item to be read as complete. That rule
is currently satisfied by a specific mechanism: a file appears at its final
name with zero length before its content arrives, and `BlobShape::Incomplete`
exists to describe exactly that window and to forbid reclaiming it.

Synchronous access handles change the shape of that window — the default mode
takes an exclusive per-file lock, so a second opener is refused outright rather
than shown an empty file. The guarantee must be restated for the new store
rather than assumed to survive.

**Why this priority**: P2 only because it is a property of US1 rather than a
separate journey. It is not optional.

**Independent Test**: The existing interleaving tests run against the new
store's model, and a browser test opens the same world in two tabs and drives
them into the contended case on purpose.

**Acceptance Scenarios**:

1. **Given** two tabs writing the same blob, **When** both proceed,
   **Then** neither observes the other's partial state and the final content
   is a complete, readable blob.
2. **Given** one tab holding a write handle, **When** another tab tries to
   read the same blob, **Then** it either reads the previous complete content
   or reports a miss — never a truncated read, and never an error the caller
   must interpret.
3. **Given** a write interrupted by the tab closing, **When** any tab later
   reads that blob, **Then** it is treated as absent and is repaired by the
   next write of the same content.

---

### Edge Cases

- **The worker cannot start at all** — blocked by policy, or the script fails
  to load. The cache must be exactly as capable as it is today.
- **The worker starts but the probe fails** — workers exist, synchronous
  access does not. Same outcome, and the probe must not be run per operation.
- **The worker dies mid-operation.** An operation in flight must not be
  reported as complete. Restarting it must not require a page reload.
- **A second handle is refused.** Another tab holds the file. This is a
  routine outcome and not an error the user hears about.
- **Changes are written and not flushed.** Persistence is explicit for
  synchronous handles; a blob whose flush did not happen must read as absent
  rather than as short.
- **Sign-out while the worker holds bytes.** Spec 028 FR-021b requires cached
  content to become unreadable in every tab promptly; a worker holding an open
  handle must not delay or defeat that.
- **A blob larger than the remaining quota.** The quota check and the write now
  live in different contexts; a refusal must still leave nothing behind.
- **The bundle grows.** A second wasm artefact must not duplicate the engine.
- **Message ordering.** Two writes to the same blob issued in order must not
  land out of order.
- **A blob big enough to be worth transferring rather than copying.** Copying
  every payload across the boundary would spend on the main thread exactly what
  this feature exists to save.

## Requirements *(mandatory)*

### Functional Requirements

#### The boundary

- **FR-001**: Blob reads, writes and deletions MUST be able to execute in a
  dedicated worker rather than on the thread that runs the canvas.
- **FR-002**: Only opaque bytes MUST cross the boundary. Encryption, key
  handling and fingerprint verification MUST remain outside the storage layer,
  as they are today, so the worker cannot hand back plaintext it was never able
  to read.
- **FR-003**: The message protocol MUST be explicit and versioned, and both
  ends MUST refuse a message they do not understand rather than acting on a
  partial interpretation.
- **FR-004**: Every request MUST be correlated with its response, so that
  concurrent operations cannot be confused for one another.
- **FR-005**: Writes to the same blob issued in order MUST be applied in that
  order.
- **FR-006**: Payloads above a stated size MUST be handed across the boundary
  without copying, where the platform permits it.

#### Availability and fallback

- **FR-007**: The system MUST detect whether the fast path is usable before
  relying on it, and MUST do so once per session rather than per operation.
- **FR-008**: Where the fast path is unusable — no worker, no synchronous
  access, no secure context — every cache operation MUST still succeed through
  the existing path, with the guarantees it has today.
- **FR-009**: Falling back MUST NOT surface anything to the user. It is a
  performance characteristic, not an error.
- **FR-010**: A worker that fails after starting MUST NOT cause an operation
  to be reported as successful. Subsequent operations MUST proceed by the
  existing path without requiring a reload.
- **FR-011**: The system MUST record which path is in use, so that a
  performance report can say which one produced it and a diagnostic screen can
  say why the cache is slow.

#### Concurrency

- **FR-012**: Spec 028 FR-021 MUST hold under the new store: concurrent access
  from more than one tab MUST NOT corrupt the store nor allow a partially
  written item to be read as complete.
- **FR-013**: A blob whose write did not complete MUST be treated as absent by
  every reader, and MUST NOT be deleted on sight by a reader, because a write in
  flight elsewhere and a write that died are indistinguishable from outside.
- **FR-014**: A refused file lock MUST be an ordinary outcome — the caller
  reads what was there before, or reports a miss — and MUST NOT be surfaced as
  a failure.
- **FR-015**: The storage layer MUST NOT acquire the cache's cross-tab locks
  internally. They are not reentrant and every caller already holds one.
- **FR-016**: Sign-out MUST render cached content unreadable in every tab as
  promptly as it does today, regardless of what the worker holds open.

#### Evidence

- **FR-017**: A benchmark MUST measure both stores over the same corpus in the
  same session, across at least three blob sizes spanning what this cache
  stores in practice.
- **FR-018**: The measurement MUST be recorded with the browser, the machine
  and the corpus that produced it.
- **FR-019**: The concurrency rules MUST remain testable without a browser, as
  they are today, so that the interleaving cases stay ordinary tests.
- **FR-020**: A browser-level test MUST exercise the fast path and the fallback
  path, and MUST fail if a build silently stops using the fast path where it is
  available — a silent regression to the slow path is the failure mode this
  feature is most likely to suffer and least likely to notice.

#### Cost

- **FR-021**: The shipped artefacts MUST NOT duplicate the engine bundle, and
  total compressed growth attributable to this feature MUST be reported.
- **FR-022**: The fast path MUST NOT require relaxing the page's isolation
  posture. If it turns out to, that is a finding to record and a reason to
  stop, not a header to add.

### Key Entities

- **Blob request**: One operation to perform — read, write or delete — naming
  the world, the content fingerprint, and the bytes where relevant. Carries a
  correlation identifier.
- **Blob response**: The outcome of one request: the bytes, an absence, or a
  failure. Absence is not a failure.
- **Capability verdict**: The once-per-session answer to "can this context use
  the fast path", and the reason when it cannot.
- **Measurement record**: Throughput per store per blob size, with the browser,
  machine and corpus that produced it.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Loading a cold scene whose assets exceed the large-blob
  threshold, while the user drags the map continuously, produces frame pacing
  within 10% of the same scene loaded with caching disabled.
- **SC-002**: Writing the benchmark corpus is at least twice as fast through
  the fast path as through the existing path, for blobs of 1 MB and above, on
  the reference browser. **If this is not met, the feature stops and the
  measurement is the deliverable.**
- **SC-003**: With the fast path forced off, the cache test suite produces
  results identical to today's — no new failures, no new skips.
- **SC-004**: No user-visible error, notice or diagnostic appears in any
  fallback case.
- **SC-005**: Two tabs driven into contention on the same blob produce no
  truncated read and no corrupted blob across 100 consecutive attempts.
- **SC-006**: Compressed artefact growth attributable to this feature is
  reported, and the engine bundle stays inside its existing budget.
- **SC-007**: A build that stops using the fast path where it is available
  fails a test, demonstrated by making it happen on purpose.
- **SC-008** *(stretch)*: The measurement in SC-002 exists for at least one
  non-Chromium browser, and the remaining blockers to supporting it are written
  down by name rather than estimated.

## Assumptions

- **Blob I/O only.** The worker owns reads, writes and deletions of opaque
  blobs. The index, the keys, the crypto, the sync protocol and cross-tab
  locking stay where they are. This follows the existing crate boundary, which
  was drawn for the same reason.
- **The fast path is an optimisation, never a requirement.** The existing store
  remains the reference implementation and the fallback, and its tests remain
  the definition of correct.
- **Chromium is the reference browser**, and remains the only supported one
  when this feature ships. The fallback exists for contexts rather than for a
  second browser — per research R1 there is no browser it rescues. The stretch
  goal asks what else would be needed for a second engine; it does not deliver
  one.
- **"Large" means at least 1 MB** for the purposes of the threshold in US1 and
  SC-002, subject to what the benchmark finds.
- **No change to what is cached or when.** Eviction, budget, prefetch and
  admission policy are untouched; this feature changes only where bytes are
  written and by what mechanism.
- **The benchmark is a developer tool**, not a product surface, and does not
  ship enabled to operators.

## Dependencies

- Spec 028 (client world cache) — this feature modifies its storage layer and
  must preserve FR-021, FR-021b and the degrade-never-fail posture.
- `crates/thunderforge-opfs` — the `BlobStore` seam this depends on already
  exists, along with the in-memory implementation that keeps FR-019 possible.

## Out of Scope

- Moving the index, the crypto or the sync protocol into the worker.
- Supporting a second browser engine.
- Changing what is cached, when it is cached, or what is evicted.
- Any change to the server.

## Resolved Questions

Both were answered in [research.md](./research.md), and one finding reversed
this spec's framing.

- **Q1 — locking posture (FR-012, FR-014)**: **Exclusive `readwrite`, the
  default, with the `mode` option not passed at all.** The option is a later
  addition and is not portable, and choosing a single-engine option to enable a
  goal about portability would be incoherent. See research R2 for what FR-021
  becomes as a result: the guarantee is unchanged and the mechanism is simpler,
  because a reader is refused rather than shown an empty file — but
  `BlobShape::Incomplete` survives, since a write killed mid-flight still
  leaves a short file behind.
- **Q2 — fallback lifetime (FR-008, SC-003)**: **Retained, with its purpose
  narrowed to contexts rather than browsers.** It exists for "a worker could
  not start", and for nothing else. It is explicitly *not* a browser-support
  story, because there is no browser it rescues — see below.

## The framing this feature was written on was wrong

`createSyncAccessHandle` has been Baseline **widely available since March
2023**. `createWritable` — the call this cache uses today — became Baseline
**newly available in September 2025**, two and a half years later.

So the path being introduced here is not an optimisation layered on a
well-supported base. It is the **more portable** of the two, and the current
store is the narrower one. That does not change any requirement below, but it
changes what this feature is for, and it is why the stretch goal exists.

## Stretch Goal — More browsers than one

The constitution states Chromium-only support, and the world cache is the
reason: it depends on OPFS, WebCrypto and IndexedDB, and Chromium is where all
three were verified.

This feature replaces the storage layer's single largest portability blocker
with an API that has been available across engines for years. That does not
make Firefox or Safari supported — WebCrypto and IndexedDB usage would each
need their own verification, and that is a separate piece of work — but it
makes the question worth asking for the first time.

- **SG-001**: The benchmark (US3) MUST be run on at least one non-Chromium
  browser, against the existing store as well as the new one. What it finds is
  the deliverable, including if the finding is that the current store does not
  work there at all.
- **SG-002**: The cache suite SHOULD be run on that browser and the remaining
  blockers recorded by name. **The list is the deliverable, not a support
  claim.** Nothing here promises a second supported browser, and no
  constitution constraint is relaxed by this feature.
