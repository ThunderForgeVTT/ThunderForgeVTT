---

description: "Task list for 028-client-world-cache"
---

# Tasks: Client-Side World Cache with Content-Addressed Delta Sync

**Input**: Design documents from `/specs/028-client-world-cache/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: INCLUDED. The spec's success criteria repeatedly say "verified by automated test" (SC-004, SC-012, SC-014, SC-018) and quickstart.md defines the runnable validation. Constitution Principle V makes these non-optional.

**Organization**: Grouped by user story so each is independently implementable and testable.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: US1–US8 per spec.md
- Exact file paths included

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Unblock the Constitution gate and stand up the crates

- [x] T001 Author `docs/adrs/20260826-052-client-cache-offline-and-peer-adjudication.md` recording the amendment to ADR-046's server-authoritative model (offline authoring + peer distribution), the DMCA determination from plan.md, and the peer-transfer default decision — **BLOCKING, no implementation may begin until accepted (Constitution Principle IV)**
- [x] T002 Add index row for ADR-052 to `docs/adrs/README.md`
- [X] T003 Create crate `crates/thunderforge-cache-core/` with `Cargo.toml` (no `web-sys`, no Diesel, no network deps — enforce the purity boundary from contracts/cache-core-api.md), `[lints] workspace = true`, and `license.workspace = true`
- [X] T004 Create crate `crates/thunderforge-cache-browser/` with `Cargo.toml` targeting wasm32, depending on `thunderforge-cache-core`
- [X] T005 Register both crates in the workspace `members` list in `Cargo.toml`
- [X] T006 [P] Add `sha2` to `crates/thunderforge-cache-core/Cargo.toml` and `web-sys` features (OPFS, WebCrypto, IndexedDB, RTCDataChannel) to `crates/thunderforge-cache-browser/Cargo.toml`

**Checkpoint**: ADR accepted, crates exist and compile empty

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The shared policy and server fingerprints every story depends on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Shared policy crate — pure, native-testable

- [X] T007 [P] Implement `Fingerprint` (SHA-256, hex round-trip, strict parsing) in `crates/thunderforge-cache-core/src/fingerprint.rs`
- [X] T008 [P] Implement `verify(bytes, expected)` as the single sanctioned trust choke point in `crates/thunderforge-cache-core/src/fingerprint.rs`
- [X] T009 [P] Implement `ItemId` enum (`SceneState`, `CanvasAsset`) with stable string encoding in `crates/thunderforge-cache-core/src/lib.rs`
- [X] T010 Implement `Manifest` over `BTreeMap` with deterministic wire ordering in `crates/thunderforge-cache-core/src/manifest.rs`
- [X] T011 Implement `CanonicalSceneState` with sorted entities, fixed float precision, per-viewer fields excluded, and an explicit version participating in the hash, in `crates/thunderforge-cache-core/src/manifest.rs`
- [X] T012 [P] Unit tests: fingerprint stability across row orderings and float round-trips in `crates/thunderforge-cache-core/tests/fingerprint.rs`
- [X] T013 [P] Property test: `verify(b, of_bytes(b))` always succeeds; any single-bit mutation fails, in `crates/thunderforge-cache-core/tests/fingerprint.rs`
- [X] T014 [P] Unit test: canonical-version bump invalidates every scene fingerprint in `crates/thunderforge-cache-core/tests/canonical.rs`

### Server fingerprints

- [X] T015 Create Diesel migration `src/server/migrations/NNNN_add_content_fingerprints/{up.sql,down.sql}` adding nullable indexed `content_hash` to `canvas_image_assets` and creating `scene_state_fingerprints`
- [X] T016 Update `src/server/src/schema.rs` for both schema changes
- [X] T017 Compute and persist `content_hash` over post-transcode WebP bytes in the same transaction as the asset row, in `src/server/src/graphql/mutations_assets.rs`
- [X] T018 Recompute `scene_state_fingerprints` when a scene-mutating `world_events` row lands, in `src/server/src/world/`
- [X] T019 [P] Server test: uploaded image yields `content_hash` of stored WebP, not the original upload, in `src/server/src/graphql/mutations_assets.rs`

**Checkpoint**: Shared rules proven by `cargo test -p thunderforge-cache-core`; server publishes fingerprints

---

## Phase 3: User Story 1 - Returning to a world already visited (Priority: P1) 🎯 MVP

**Goal**: Repeat visits read from the local machine and transfer only what changed

**Independent test**: Open a world, close it, reopen — second open transfers ≤5% of the first's bytes and reaches interactive ≥3× faster, showing state identical to the server's

- [X] T020 [P] [US1] Implement `compute_plan(held, authorized_current)` — matched omitted, `None` fingerprint fetched, unknown evicted — in `crates/thunderforge-cache-core/src/delta.rs`
- [X] T021 [P] [US1] Unit tests for all six `compute_plan` branches from contracts/graphql-delta-sync.md in `crates/thunderforge-cache-core/tests/delta.rs`
- [X] T022 [US1] Implement `worldSyncPlan` query per contracts/graphql-delta-sync.md in `src/server/src/graphql/queries/world_sync_plan.rs`, computing the plan **from** authorized items rather than filtering afterwards
- [X] T023 [US1] Register `worldSyncPlan` on the Query root in `src/server/src/graphql.rs`
- [X] T024 [P] [US1] Implement encrypted OPFS blob read/write keyed by fingerprint in `crates/thunderforge-cache-browser/src/opfs.rs`
- [X] T025 [P] [US1] Implement non-extractable AES-GCM session key generation and IndexedDB storage in `crates/thunderforge-cache-browser/src/crypto.rs`
- [X] T026 [US1] Implement the IndexedDB `index` store (fingerprint, size, last read, world) in `crates/thunderforge-cache-browser/src/index.rs`
- [ ] T027 [US1] Create `CachedAssetsPlugin` in `src/engine/src/plugins/cached_assets.rs` routing Bevy asset reads through the cache before the network, registered in `src/engine/src/lib.rs`
- [X] T028 [US1] Build the manifest and parse the `worldSyncPlan` reply in Rust (`crates/thunderforge-cache-browser/src/sync.rs`) — engine-driven, so cache policy has one owner (Principle I). TS triggers and observes; it decides nothing.
- [X] T029 [US1] Verify every fetched item against its promised fingerprint before storing, via `cache_core::fingerprint::verify`, in `crates/thunderforge-cache-browser/src/opfs.rs`
- [ ] T030 [US1] E2E: unchanged reopen transfers ≤5% of first-visit bytes and is ≥3× faster to interactive, **excluding engine bundle bytes** (SC-001, SC-002) in `apps/web/e2e/world-cache.spec.ts`
- [ ] T031 [P] [US1] E2E: single changed background transfers within 10% of that asset's size (SC-003) in `apps/web/e2e/world-cache.spec.ts`
- [ ] T032 [P] [US1] E2E: multiple cached worlds do not read or disturb each other (US1 scenario 4) in `apps/web/e2e/world-cache.spec.ts`

**Checkpoint**: MVP — repeat visits are fast. Independently shippable.

---

## Phase 4: User Story 2 - Losing access to cached content (Priority: P1)

**Goal**: Cached content stops being readable the moment permission is lost

**Independent test**: Cache a world as a member, revoke membership, confirm the content is neither retrievable nor renderable

**⚠️ Must ship in the same release as US1** — a cache outliving a permission grant is a disclosure bug

- [X] T033 [US2] Ensure `worldSyncPlan` re-authorizes from scratch on every call and omits unauthorized items from **both** lists, in `src/server/src/graphql/queries/world_sync_plan.rs`
- [X] T034 [US2] Include revoked and deleted items indistinguishably in `evict`, honouring per-object ADR-050 permissions, in `src/server/src/graphql/queries/world_sync_plan.rs`
- [ ] T035 [US2] Apply `evict` by deleting blobs and index entries in `crates/thunderforge-cache-browser/src/index.rs`
- [ ] T036 [US2] Scope OPFS paths and IndexedDB stores by `user_scope` derived from the session in `crates/thunderforge-cache-browser/src/opfs.rs`
- [ ] T037 [US2] Discard the session key on sign-out, rendering stored bytes inert independently of deletion, in `crates/thunderforge-cache-browser/src/crypto.rs`
- [ ] T038 [US2] Implement lazy background reclamation whose failure never restores readability, in `crates/thunderforge-cache-browser/src/index.rs`
- [ ] T039 [US2] Treat key loss as a cold cache — re-fetch, no error surfaced (FR-016c) — in `crates/thunderforge-cache-browser/src/crypto.rs`
- [X] T040 [P] [US2] Server test: non-member `worldSyncPlan` fails identically to any other non-member access, revealing nothing (contracts/graphql-delta-sync.md) in `src/server/src/graphql/queries/world_sync_plan.rs`
- [X] T041 [P] [US2] Server test: a client claiming an item it may not see never receives it in `fetch`, and its `evict` entry is indistinguishable from a deleted item's, in `src/server/src/graphql/queries/world_sync_plan.rs`
- [ ] T042 [US2] E2E: revoked membership denies access and discards local data (SC-004) in `apps/web/e2e/world-cache-permissions.spec.ts`
- [ ] T043 [US2] E2E: downgraded actor permission evicts only the forbidden part while permitted content still loads from cache (US2 scenario 2) in `apps/web/e2e/world-cache-permissions.spec.ts`
- [ ] T044 [US2] E2E: after sign-out, stored bytes are unreadable — asserted **against OPFS directly, before background cleanup runs**, not through the app (SC-004a) in `apps/web/e2e/world-cache-permissions.spec.ts`

**Checkpoint**: US1 + US2 are the minimum releasable pair

---

## Phase 5: User Story 6 - Knowing the app is loading (Priority: P2)

**Goal**: The unavoidable first-load wait is explained rather than blank

**Independent test**: Load with an empty cache on a throttled connection; progress is continuous and accurate through to interactive, and failure is explained

**Note**: Fully independent of every other story — no dependency on Phase 2. Can be built at any time, including first.

- [X] T045 [P] [US6] Replace `instantiateStreaming` with an explicit streaming fetch exposing `Content-Length` and received bytes in `apps/web/src/engine/bevy/index.ts`
- [X] T046 [US6] Create the loader component with real download progress and a distinct indeterminate "starting" phase in `apps/web/src/components/engine/EngineLoader.tsx`
- [X] T047 [US6] Engage the indeterminate path when no total is knowable (chunked transfer), never a fabricated percentage, in `apps/web/src/components/engine/EngineLoader.tsx`
- [X] T048 [US6] Surface download/startup failure with a plain explanation and a working retry in `apps/web/src/components/engine/EngineLoader.tsx`
- [X] T049 [US6] Suppress or fast-resolve the loader when the browser already holds the bundle (FR-033) in `apps/web/src/engine/bevy/index.ts`
- [X] T050 [P] [US6] E2E: loading state within 1s, progress never regresses or stalls >5s, never reaches max before interactive (SC-009, SC-010) in `apps/web/e2e/engine-loading.spec.ts`
- [X] T051 [P] [US6] E2E: simulated download and startup failures each produce explanation plus working retry (SC-011) in `apps/web/e2e/engine-loading.spec.ts`

**Checkpoint**: First load is honest at any bundle size

---

## Phase 6: User Story 3 - Recovering from a damaged store (Priority: P2)

**Goal**: The client detects and repairs its own store with no user action

**Independent test**: Corrupt or partially delete stored data, reopen, confirm correct render with no error

- [ ] T052 [US3] Validate each blob against its own fingerprint-derived filename on read; discard and re-fetch on mismatch, in `crates/thunderforge-cache-browser/src/opfs.rs`
- [ ] T053 [US3] Reconcile index-vs-OPFS divergence in both directions (index claims a missing blob; orphan blob with no entry) in `crates/thunderforge-cache-browser/src/index.rs`
- [ ] T054 [US3] Fall back to full fetch when the store cannot be opened at all, with no user-visible error, in `crates/thunderforge-cache-browser/src/lib.rs`
- [ ] T055 [US3] Guard concurrent multi-tab writes so a partially-written blob is never readable as complete (FR-021) in `crates/thunderforge-cache-browser/src/opfs.rs`
- [ ] T056 [P] [US3] E2E: corrupted blob and orphaned index entry both self-repair silently (SC-005) in `apps/web/e2e/world-cache-repair.spec.ts`
- [ ] T057 [P] [US3] E2E: two tabs writing the same world concurrently corrupt nothing in `apps/web/e2e/world-cache-repair.spec.ts`

---

## Phase 7: User Story 4 - Staying within available space (Priority: P2)

**Goal**: Bounded, predictable storage that degrades gracefully under pressure

**Independent test**: Exceed the budget across several worlds; LRU worlds are released, the open world is not, nothing breaks

- [X] T058 [P] [US4] Implement `limit_bytes(quota) = min(quota/2, 20GiB)` in `crates/thunderforge-cache-core/src/budget.rs`
- [X] T059 [P] [US4] Implement `plan_eviction` — whole worlds before items, LRU first, never the open world, deterministic tie-breaking — in `crates/thunderforge-cache-core/src/budget.rs`
- [X] T060 [P] [US4] Unit test: `plan_eviction` never selects the open world even when that leaves the budget unsatisfied, in `crates/thunderforge-cache-core/tests/budget.rs`
- [ ] T061 [US4] Read `navigator.storage.estimate()` and recompute the budget on each world open, shrinking the store when quota drops, in `crates/thunderforge-cache-browser/src/index.rs`
- [ ] T062 [US4] Degrade a failed local write to a server fetch, never to a failed load (FR-024), in `crates/thunderforge-cache-browser/src/opfs.rs`
- [ ] T063 [P] [US4] E2E: budget respected across machines whose reported quota differs by an order of magnitude (SC-006) in `apps/web/e2e/world-cache-budget.spec.ts`

---

## Phase 8: User Story 5 - Seeing and reclaiming storage (Priority: P3)

**Goal**: Visibility and manual control over what is stored

**Independent test**: With several worlds cached, reported figures match reality and clearing one world frees exactly that

- [ ] T064 [US5] Expose per-world usage totals from the index in `crates/thunderforge-cache-browser/src/index.rs`
- [ ] T065 [US5] Build the storage view with total and per-world breakdown in `apps/web/src/components/diagnostics/StoragePanel.tsx`
- [ ] T066 [US5] Implement clear-one-world and clear-all, leaving server data untouched, in `apps/web/src/components/diagnostics/StoragePanel.tsx`
- [ ] T067 [P] [US5] E2E: clearing one world zeroes its figure, leaves others intact, and the cleared world still loads (US5 scenario 2) in `apps/web/e2e/world-cache-storage-ui.spec.ts`

---

## Phase 9: User Story 7 - Playing on through a lost connection (Priority: P3)

**Goal**: Disconnected work continues and reconciles on return

**Independent test**: Sever the connection, make changes, restore — changes reach the server and both parties agree on the result

**⚠️ Largest and riskiest story.** Depends on US1 and US3. Build last; may reasonably be its own release.

### Shared conflict policy

- [X] T068 [P] [US7] Implement `resolve(a, b)` — GM beats Player, same role decided by reconnect order, total, never reads a client timestamp — in `crates/thunderforge-cache-core/src/conflict.rs`
- [X] T069 [P] [US7] Unit test: `resolve` is total and antisymmetric across every role/order combination, in `crates/thunderforge-cache-core/tests/conflict.rs`
- [X] T070 [P] [US7] Implement `enqueue`/`replay_order`/`apply_outcomes` returning unresolved changes, in `crates/thunderforge-cache-core/src/queue.rs`
- [X] T071 [P] [US7] Unit test: `apply_outcomes` surfaces every change lacking an outcome (FR-041), in `crates/thunderforge-cache-core/tests/queue.rs`

### Client outbox

- [ ] T072 [US7] Implement the durable IndexedDB outbox storing emitted world-store commands verbatim, persisted before local acknowledgement, in `crates/thunderforge-cache-browser/src/index.rs`
- [ ] T073 [US7] Detect and surface the disconnected state, allowing continued work, in `apps/web/src/engine/world/sync/subscriptionClient.ts`
- [ ] T074 [US7] Restrict offline-editable entities to token position/rotation/scale and refuse create/delete with a clear explanation (FR-035a) in `apps/web/src/engine/world/facets/tokenControl.ts`

### Server reconciliation

- [ ] T075 [US7] Implement `reconcileQueuedChanges` per contracts/graphql-delta-sync.md in `src/server/src/graphql/mutations_reconcile.rs`, replaying through existing mutations so authorization is traversed identically
- [ ] T076 [US7] Re-authorize every queued change against current permissions at reconnect time (FR-042) in `src/server/src/graphql/mutations_reconcile.rs`
- [ ] T077 [US7] Adjudicate conflicts via `cache_core::conflict`, emitting ordinary `world_events` so other clients see results through the existing subscription, in `src/server/src/graphql/mutations_reconcile.rs`
- [ ] T078 [US7] Guarantee exactly one outcome per submitted change in `src/server/src/graphql/mutations_reconcile.rs`

### The supersession edge

- [ ] T079 [US7] Recognise a previously-applied local change being overridden by a later GM reconnect and inform the user (FR-041, the `Applied → Superseded` case) in `apps/web/src/engine/world/sync/index.ts`
- [ ] T080 [US7] Report each reconcile outcome to the user, distinguishing `SUPERSEDED` from a generic failure, in `apps/web/src/components/world/ReconcileReport.tsx`
- [ ] T081 [US7] Preserve outbox entries as a record of lost work when the key is gone, so nothing is silently discarded, in `crates/thunderforge-cache-browser/src/index.rs`
- [ ] T082 [US7] Handle re-disconnection mid-submission without double-applying or dropping the remainder in `apps/web/src/engine/world/sync/index.ts`
- [ ] T083 [US7] E2E: offline change applied on reconnect and outcome reported (SC-015) in `apps/web/e2e/world-cache-offline.spec.ts`
- [ ] T084 [US7] E2E: GM and player edit the same token offline, **player reconnects first**, both converge **and the player is notified of supersession** (SC-016) in `apps/web/e2e/world-cache-offline.spec.ts`
- [ ] T085 [P] [US7] E2E: queued change against server-deleted content is discarded with an explanation, not resurrected, in `apps/web/e2e/world-cache-offline.spec.ts`

---

## Phase 10: Peer-Assisted Distribution (supports US1)

**Goal**: Fetch bytes from session peers instead of the server, safely

**Note**: An optimization layered on US1, not a story of its own. Strictly optional at runtime (FR-048) — every path falls back to the server.

- [ ] T086 [US1] Implement `sendPeerSignal` / `peerSignals` relaying only between current members of the named world, never interpreting payloads, in `src/server/src/peer_signaling.rs`
- [ ] T087 [US1] Report and expose `PlanItem.peerAvailable` as advisory only in `src/server/src/graphql/queries/world_sync_plan.rs`
- [ ] T088 [US1] Implement the WebRTC data channel and the REQUEST/OFFER/CHUNK/DONE/DECLINE protocol in `crates/thunderforge-cache-browser/src/peer.rs`
- [ ] T089 [US1] Request only fingerprints present in the client's own current `SyncPlan.fetch` — the enforcement point for FR-047 — in `crates/thunderforge-cache-browser/src/peer.rs`
- [ ] T090 [US1] Verify peer bytes before storing; on mismatch discard, do not retry that peer, fall back to the server, in `crates/thunderforge-cache-browser/src/peer.rs`
- [ ] T091 [US1] Serve only locally-held verified fingerprints; `DECLINE` otherwise; stop serving on losing membership; rate-limit, in `crates/thunderforge-cache-browser/src/peer.rs`
- [ ] T092 [US1] Add the visible peer-transfer indicator and the persisted enable/disable setting, **defaulting to enabled**, warning that disabling also forfeits server-isolated play (FR-049) in `apps/web/src/components/diagnostics/PeerPanel.tsx`
- [ ] T093 [P] [US1] E2E: peer-supplied mismatched bytes are rejected and the server fallback succeeds (SC-012) in `apps/web/e2e/world-cache-peer.spec.ts`
- [ ] T094 [P] [US1] E2E: content the requester lacks permission for is never requested nor obtained from a peer (SC-014) in `apps/web/e2e/world-cache-peer.spec.ts`
- [ ] T095 [US1] E2E: the whole suite passes with peer transfer disabled, outcomes identical (SC-013) in `apps/web/e2e/world-cache-peer.spec.ts`

---

## Phase 10a: Peer-Adjudicated Play — Server-Isolated State (supports US7)

**Goal**: A client that loses the server but still sees the whole party keeps playing

**Independent test**: Sever one client's server connection while leaving all peer connections intact; play continues, and the resulting state is accepted on reconnection

**⚠️ Largest trust-model change in the feature.** Peers move from distributing bytes to adjudicating state. Depends on Phase 10 (peer channels) and Phase 9 (reconciliation). Blocked on ADR-052 covering it specifically.

- [ ] T096 [US7] Implement three-state connectivity detection — connected / server-isolated / offline — with server-isolated requiring **every** peer reachable **and** the GM among them, in `apps/web/src/engine/world/sync/subscriptionClient.ts`
- [ ] T097 [US7] Surface the current state and reconnection attempts to the user within 5s, in the idiom players expect from online games (FR-063, SC-022), in `apps/web/src/components/world/ConnectionStatus.tsx`
- [ ] T098 [US7] End peer-adjudicated play immediately on losing any peer or the GM, dropping to offline (FR-058, FR-059), in `crates/thunderforge-cache-browser/src/peer.rs`
- [ ] T099 [US7] Implement discrepancy detection: compare client-reported outcomes the server can independently determine (dice first, per ADR-044) against the server's own result, in `src/server/src/graphql/mutations_reconcile.rs`
- [ ] T100 [US7] Implement the PROPOSE / ADJUDICATE / APPLY protocol ordered by session-agreed nonce sequence rather than any clock — **no per-message signatures**, submission rides the GM's authenticated session, in `crates/thunderforge-cache-browser/src/peer.rs`
- [ ] T101 [US7] Restrict peer adjudication to token position/rotation/scale, rejecting anything else outright (FR-060), in `crates/thunderforge-cache-browser/src/peer.rs`
- [ ] T102 [US7] Accept an attributed submission only when the server confirms the submitter holds the GM role, using the existing role check; reject attributed submissions from non-GMs (FR-061, FR-061a), in `src/server/src/graphql/mutations_reconcile.rs`
- [ ] T102a [US7] Render a discrepant result distinctly in the GM's view only, inspectable to show claimed vs determined values — display treatment, **no resolution workflow, no escalation, no dispute process** (FR-065, FR-065a, FR-066, FR-067), in `apps/web/src/components/world/RollResult.tsx`
- [ ] T102c [US7] Report a discrepancy only on a genuine determined-value mismatch — never on timeout, parse failure, version mismatch, or any ambiguity; when in doubt report nothing (FR-067a), in `src/server/src/graphql/mutations_reconcile.rs`
- [ ] T102b [US7] Report no discrepancy where the server has no independent basis for the outcome, e.g. ordinary token movement (FR-068), in `src/server/src/graphql/mutations_reconcile.rs`
- [ ] T103 [US7] Submit peer-adjudicated changes for confirmation and re-authorization on reconnection, reverting locally and informing the originator on rejection (FR-062), in `apps/web/src/engine/world/sync/index.ts`
- [ ] T104 [US7] Handle the server returning mid-adjudication without double-applying or dropping a change, in `apps/web/src/engine/world/sync/index.ts`
- [ ] T105 [P] [US7] E2E: server-isolated client continues play and its state is accepted on reconnection (SC-019) in `apps/web/e2e/world-cache-isolated.spec.ts`
- [ ] T106 [P] [US7] E2E: losing any peer — **including specifically the GM** — stops adjudicated play immediately (SC-020) in `apps/web/e2e/world-cache-isolated.spec.ts`
- [ ] T107 [P] [US7] E2E: a peer partition leaves **both** halves stopped, neither progressing (FR-058) in `apps/web/e2e/world-cache-isolated.spec.ts`
- [ ] T108 [US7] E2E: a non-GM submitting a change attributed to another player is rejected; the same submission from a GM is accepted (SC-021) in `apps/web/e2e/world-cache-isolated.spec.ts`
- [ ] T108a [US7] E2E: a client reporting a dice result the server determined differently is applied, rendered distinctly for the GM with both values inspectable, and never auto-rejected, altered, or shown to other players (SC-021a, SC-021b) in `apps/web/e2e/world-cache-isolated.spec.ts`
- [ ] T108c [US7] E2E: timeout, parse failure, version mismatch and missing determination each produce **no** discrepancy rather than a spurious one (SC-021c) in `apps/web/e2e/world-cache-isolated.spec.ts`
- [ ] T108b [US7] E2E: a GM acting on a player's behalf produces no flag and no notification to anyone (FR-061b) in `apps/web/e2e/world-cache-isolated.spec.ts`

---

## Phase 10b: User Story 8 - Background Prefetch (Priority: P3)

**Goal**: Switching to a never-visited scene in the open world is instant

**Independent test**: Leave a multi-scene world idle briefly, switch to a never-visited scene, confirm no fetch and no loading state

**Note**: A refinement of US1, not a new capability. Needs no Service Worker — the page is open, the WASM is running, and the sync plan already names what is missing.

- [ ] T116 [P] [US8] Implement a low-priority prefetch queue drawing only on the caller's own `SyncPlan`, confined to the open world (FR-069, FR-072, FR-073) in `crates/thunderforge-cache-browser/src/lib.rs`
- [ ] T117 [US8] Yield prefetching to the active scene, user-initiated fetches, and live updates (FR-070) in `crates/thunderforge-cache-browser/src/lib.rs`
- [ ] T118 [US8] Stop prefetching rather than evicting — speculative content must never displace content the user actually has (FR-071) in `crates/thunderforge-cache-core/src/budget.rs`
- [ ] T119 [P] [US8] E2E: switching to a prefetched never-opened scene shows no loading state and issues no fetch (SC-023) in `apps/web/e2e/world-cache-prefetch.spec.ts`
- [ ] T120 [P] [US8] E2E: prefetch adds no more than 5% to active-scene time-to-interactive, measured enabled vs disabled (SC-024) in `apps/web/e2e/world-cache-prefetch.spec.ts`
- [ ] T121 [US8] E2E: no network activity attributable to this feature occurs while the application is closed — no Service Worker, no push subscription, no background-sync registration (SC-025, FR-073) in `apps/web/e2e/world-cache-prefetch.spec.ts`

---

## Phase 11: Polish & Cross-Cutting Concerns

- [ ] T122 [P] Implement the diagnostics panel — hit rate, bytes saved, peer vs server, repairs — in `apps/web/src/components/diagnostics/CachePanel.tsx`
- [ ] T123 [P] E2E: SC-001..003 confirmable from the diagnostics panel during an ordinary session without a test harness (SC-017) in `apps/web/e2e/world-cache-diagnostics.spec.ts`
- [ ] T124 E2E: no outbound request carries cache statistics or usage telemetry (SC-018) in `apps/web/e2e/world-cache-diagnostics.spec.ts`
- [ ] T125 [P] Backfill `content_hash` for existing assets via a background job in `src/server/src/storage/`, relying on NULL-means-fetch so it need not complete before release
- [ ] T126 [P] Run `cargo fmt --all` and `cargo clippy --workspace --all-targets`; fix findings in the new crates
- [ ] T127 [P] Document the two new crates in `docs/` and cross-reference ADR-052
- [ ] T128 Update `MVP.md` post-MVP notes to reflect that engine load feedback shipped and that bundle-size work remains separate and open

---

## Dependencies

```
Phase 1 (Setup, T001 blocking)
        │
        ▼
Phase 2 (Foundational) ────────────────────────┐
        │                                       │
        ▼                                       │
Phase 3 US1 (P1, MVP) ──┬──▶ Phase 10 Peer ──┐   │
        │                │                      │
        ▼                │                      │
Phase 4 US2 (P1) ◀───────┘                      │
        │                                       │
        ├──▶ Phase 6 US3 (P2) ──▶ Phase 9 US7 ──▶ Phase 10a
        │                                       │   (peer adjudication)
        ├──▶ Phase 7 US4 (P2)                   │
        └──▶ Phase 8 US5 (P3)                   │
                                                │
Phase 5 US6 (P2) ◀──────── independent ─────────┘
        (needs only Phase 1; can be built first)
```

**Hard dependencies**

- **T001 (ADR-052) blocks everything.** Constitution Principle IV. It must
  now cover peer-adjudicated play (FR-055 to FR-063) as well, which is the
  largest departure from ADR-046 in the feature.
- **Phase 10a depends on both Phase 9 (reconciliation) and Phase 10 (peer
  channels).** It is the last thing built.
- US2 must ship with US1 — not after.
- US7 depends on US1 (a populated store) and US3 (a trustworthy one).
- Phase 10 depends on US1's plan and verification path.
- **US6 depends on nothing but Phase 1** and is the cheapest early win.

## Parallel Opportunities

- **Phase 2**: T007–T009 and T012–T014 are independent files; T015–T019 (server) run alongside the whole shared crate.
- **Phase 3**: T020/T021 (core) ∥ T024/T025 (browser I/O) ∥ T031/T032 (E2E).
- **Phase 5**: entirely parallel with Phases 2–4 — different files, no shared dependencies.
- **Phase 9**: T068–T071 (pure policy) all parallel before any wiring.
- **Phase 11**: T096, T097, T099, T100, T101 all independent.

## Implementation Strategy

**MVP = Phase 1 + Phase 2 + Phase 3 (US1) + Phase 4 (US2).**

US2 is in the MVP not because it adds user value but because omitting it ships a disclosure bug. US1 alone is not releasable.

**Suggested order**

1. **T001 (ADR-052)** — everything else is blocked on it
2. **Phase 5 (US6)** — independent, cheap, immediately visible, and useful regardless of what happens to the rest
3. **Phase 2** — the shared crate is where the leverage is; get it under `cargo test` early
4. **Phases 3 + 4** — the releasable MVP
5. **Phases 6, 7** — safety rails; a cache that can serve stale data or eat the disk should not sit in production long
6. **Phase 10** — peer transfer, once US1 is proven
7. **Phase 8** — storage UI
8. **Phase 9 (US7)** — offline, plausibly its own release
9. **Phase 10a** — peer-adjudicated play, genuinely last. It needs peer
   channels proven, reconciliation proven, and an ADR that specifically
   covers moving peers from distributing bytes to adjudicating state.

**Where the risk is**

- **T084** — GM-over-player means a player's change can be applied and *then* superseded when the GM reconnects later. A test asserting only convergence will pass while the player is never told their work was overridden, which is the requirement that actually matters (FR-041).
- **T102 / T108** — the server accepts changes attributed to user A arriving over the GM's connection. That is intended: the GM is trusted by design (FR-061b). The test that matters is the *negative* one — a **non-GM** must never submit on someone else's behalf (T108).
- **T108a / T108c** — discrepancy display is the one place this feature touches a friendship. It shows the GM something and stops: no workflow, no escalation, no visibility to other players. And because a false positive puts an innocent player under suspicion in front of the one person who can act on it, **T108c matters more than T108a** — every ambiguous case must produce silence, not a flag. Get the detection right and the rest is the table's problem; get it wrong and we have manufactured an accusation.
- **T107** — a peer partition must leave *both* halves stopped. The tempting test is "one half keeps playing"; that test passing would mean split-brain shipped.
