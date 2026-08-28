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
- [X] T027 [US1] Create `CachedAssetsPlugin` in `src/engine/src/plugins/cached_assets.rs` routing Bevy asset reads through the cache before the network, registered in `src/engine/src/lib.rs`
- [X] T028 [US1] Build the manifest and parse the `worldSyncPlan` reply in Rust (`crates/thunderforge-cache-browser/src/sync.rs`) — engine-driven, so cache policy has one owner (Principle I). TS triggers and observes; it decides nothing.
- [X] T027a [US1] Wire the cache path end to end — `sync_world_cache(world_id, user_id)` in `src/engine/src/plugins/cached_assets.rs`, called from `apps/web/src/pages/world/WorldPage.tsx` on world open. Was missing: every piece existed but nothing connected them, so the cache had never executed.
- [X] T029 [US1] Verify every fetched item against its promised fingerprint before storing, via `cache_core::fingerprint::verify`, in `crates/thunderforge-cache-browser/src/opfs.rs`
- [X] T030 [US1] E2E: unchanged reopen transfers ≤5% of first-visit bytes and is ≥3× faster to interactive, **excluding engine bundle bytes** (SC-001, SC-002) in `apps/web/e2e/world-cache.spec.ts`
- [X] T031 [P] [US1] E2E: single changed background transfers within 10% of that asset's size (SC-003) in `apps/web/e2e/world-cache.spec.ts`
- [X] T032 [P] [US1] E2E: multiple cached worlds do not read or disturb each other (US1 scenario 4) in `apps/web/e2e/world-cache.spec.ts`

**Checkpoint**: MVP — repeat visits are fast. Independently shippable.

---

## Phase 4: User Story 2 - Losing access to cached content (Priority: P1)

**Goal**: Cached content stops being readable the moment permission is lost

**Independent test**: Cache a world as a member, revoke membership, confirm the content is neither retrievable nor renderable

**⚠️ Must ship in the same release as US1** — a cache outliving a permission grant is a disclosure bug

- [X] T033 [US2] Ensure `worldSyncPlan` re-authorizes from scratch on every call and omits unauthorized items from **both** lists, in `src/server/src/graphql/queries/world_sync_plan.rs`
- [X] T034 [US2] Include revoked and deleted items indistinguishably in `evict`, honouring per-object ADR-050 permissions, in `src/server/src/graphql/queries/world_sync_plan.rs`
- [X] T035 [US2] Apply `evict` by deleting blobs and index entries in `crates/thunderforge-cache-browser/src/index.rs`
- [X] T036 [US2] Scope OPFS paths and IndexedDB stores by `user_scope` derived from the session in `crates/thunderforge-cache-browser/src/opfs.rs`
- [X] T037 [US2] Discard the session key on sign-out, rendering stored bytes inert independently of deletion, in `crates/thunderforge-cache-browser/src/crypto.rs`
- [X] T038 [US2] Implement lazy background reclamation whose failure never restores readability, in `crates/thunderforge-cache-browser/src/index.rs`
- [X] T039 [US2] Treat key loss as a cold cache — re-fetch, no error surfaced (FR-016c) — in `crates/thunderforge-cache-browser/src/crypto.rs`
- [X] T040 [P] [US2] Server test: non-member `worldSyncPlan` fails identically to any other non-member access, revealing nothing (contracts/graphql-delta-sync.md) in `src/server/src/graphql/queries/world_sync_plan.rs`
- [X] T041 [P] [US2] Server test: a client claiming an item it may not see never receives it in `fetch`, and its `evict` entry is indistinguishable from a deleted item's, in `src/server/src/graphql/queries/world_sync_plan.rs`
- [X] T042 [US2] E2E: revoked membership denies access and discards local data (SC-004) in `apps/web/e2e/world-cache-permissions.spec.ts`
- [X] T043 [US2] E2E: downgraded actor permission evicts only the forbidden part while permitted content still loads from cache (US2 scenario 2) in `apps/web/e2e/world-cache-permissions.spec.ts`
- [X] T044 [US2] E2E: after sign-out, stored bytes are unreadable — asserted **against OPFS directly, before background cleanup runs**, not through the app (SC-004a) in `apps/web/e2e/world-cache-permissions.spec.ts`

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

- [X] T052 [US3] Validate each blob against its own fingerprint-derived filename on read; discard and re-fetch on mismatch. `OpfsStore::read_blob` runs `fingerprint::verify` on the decrypted plaintext and discards on mismatch, which is the only check that can catch a blob that opens cleanly and is not what its name promises — decryption failure is a *different* condition, shared with "the key is gone", and both collapse to `Ok(None)` so key loss stays indistinguishable from a cold cache (FR-016c). The delete is deliberately confined to content that failed to open or failed to verify: a file that was never finished takes neither path, because reclaiming another tab's in-flight write is the bug T055 fixed. The engine then falls through to `fetch_and_deliver`, which re-verifies and re-stores, so recovery completes inside the same page load with no reload and nothing shown to the user. Proved in a browser by the forged-blob leg of T056, in `crates/thunderforge-cache-browser/src/opfs.rs`
- [X] T053 [US3] Reconcile index-vs-OPFS divergence in both directions (index claims a missing blob; orphan blob with no entry). The pure diff — `missing_blobs`, `orphaned_blobs`, `partition_orphans` — lives in `index.rs` and was tested from the start; what was missing until `ebb7fbc` was a **caller**, so both directions of divergence were permanent. `sync::repair_world` is it, run from `run_sync` right after `apply_plan` (the largest batch of both operations the world will see, so the pass repairs its own interruptions as well as a previous session's). The two directions are not symmetrical and the asymmetry is the point: a row naming a missing blob is a *lie* that makes the client claim the item in its manifest, so the server stays silent and the item is never fetched **or** served — a permanent hole that looks like a working cache; an orphan blob is merely unreachable bytes. An unreferenced file that is *unfinished* is indistinguishable from an in-flight write and is never reclaimed. Recorded honestly in the T056 test: because the manifest is built before the repair runs, dropping the row takes one open and refetching the item takes the next, in `crates/thunderforge-cache-browser/src/index.rs` and `crates/thunderforge-cache-browser/src/sync.rs`
- [X] T054 [US3] Fall back to full fetch when the store cannot be opened at all, with no user-visible error. **Landed in the engine plugin rather than in `lib.rs`**, and that is where it belongs: `lib.rs` can only report that OPFS or WebCrypto is absent (`CacheError::Unsupported`), and the decision to carry on without a cache is the caller's. `open_handles` treats a failed key load or a failed `OpfsStore::open` as `Readiness::Unavailable` — one `warn!` line, no handles, no error surfaced — and every asset request then takes the same `fetch_and_deliver` path a cache miss takes. `Unavailable` is terminal for the scope that produced it on purpose: a browser with no OPFS will not grow one mid-session, and retrying per frame would turn a supported degradation into a log flood; only a change of user scope reopens the question. The module header enumerates all six fall-through conditions, in `src/engine/src/plugins/cached_assets.rs`
- [X] T045a [US2] Discard a world's cached content when the server refuses its sync plan. FR-015 does not currently hold for whole-world revocation: eviction is driven only by the `evict` list of a *successful* plan, and a revoked member's request is refused, so `run_sync` takes its transport-error branch, republishes what it holds, and returns degraded — leaving the blobs on disk. Found by T042, which is marked `test.fail()` until this lands, in `src/engine/src/plugins/cached_assets.rs`
- [X] T045b [US2] `public/sw.js` is a second local store that outlives revocation: it caches `/api/canvas-assets/*` cache-first per browser profile and is cleared only on logout, so a revoked member's browser keeps readable plaintext art. Its own docs argue cache-first is safe because assets are content-addressed — true of staleness, silent on revocation. Resolved by stopping: the worker caches nothing and purges every `thunderforge-canvas-assets-*` cache on activate, so existing installs reclaim the bytes they already hold. Participating in eviction was rejected — it would keep unencrypted bytes at rest between eviction events and couple a layer with no session context to one built to have it. Canvas reads are already served by the encrypted OPFS cache; DOM `<img>` consumers fall back to the byte route's `private, max-age=3600`. `world-cache-permissions.spec.ts` now asserts zero cached bytes after revocation instead of reporting them, in `apps/web/public/sw.js`
- [X] T045c [US2] `GET /api/canvas-assets/{id}` authorizes on world membership only, not on `scenes.hidden`, so a member who knows an asset id can fetch art from a scene they cannot see. Separate from the cache but on the same disclosure axis. Fixed by stating the rule once in `src/server/src/auth/scene_visibility.rs` and having both callers ask it: the byte route now checks the asset's scene after membership, and `world_sync_plan` builds its visible-scene set from the same module. A hidden scene's asset answers 404 with the same body as an unknown id, so ids cannot be probed for existence; a world-scoped asset (`scene_id IS NULL`) stays visible to every member. Covered by three route tests and a new byte-route assertion in `world-cache-permissions.spec.ts`, in `src/server/src/canvas_assets_serve.rs`
- [X] T055a [US3] Serialise session-key creation across tabs with the Web Locks API so two tabs starting together cannot each generate one (FR-021a) in `crates/thunderforge-cache-browser/src/crypto.rs`
- [X] T055b [US3] Broadcast sign-out to every tab so an in-memory key is dropped without waiting for a reload (FR-021b) in `apps/web/src/services/worldCache.ts` and `src/engine/src/plugins/cached_assets.rs`
- [X] T055c [US3] Serialise per-world sync/eviction across tabs so one tab cannot evict what another just fetched (FR-021c) in `crates/thunderforge-cache-browser/src/sync.rs`
- [X] T055d [US3] Degrade cleanly where Web Locks or BroadcastChannel are unavailable — an extra fetch or an ineffective cache, never a failed load and never readable content after sign-out (FR-021d) in `crates/thunderforge-cache-browser/src/`
- [X] T055g [US3] Sign every tab out when one signs out — the session cookie is already shared, but other tabs keep presenting a signed-in UI until a request fails, showing content the user believes they closed (FR-021e, FR-021f) in `apps/web/src/hooks/useAuth.ts`
- [X] T055e [P] [US3] E2E: two tabs open the same world concurrently; no corruption, and the cache still works in both (FR-021) in `apps/web/e2e/world-cache-multitab.spec.ts`. Two tabs in one Playwright context (the profile boundary — two contexts would share no key, no OPFS and no lock manager, and every assertion would pass for the wrong reason) open the world under `Promise.all`: both sync, one session key, the asset stored exactly once with no zero-length stub, the bytes sealed, and each tab reloads with zero asset refetches. **Recorded honestly: this does not prove the FR-021a lock serialises anything** — mutating key creation to take no cross-tab lock leaves this test green, because a second engine boot is seconds behind the first and its re-check finds the stored key either way (exactly what `without_web_locks_both_tabs_still_finish_with_a_key` says about the degraded path). The ordering is proved by the interleaving model in `crypto.rs`; this proves the outcome in a real profile.
- [X] T055f [P] [US3] E2E: signing out in one tab makes cached content unreadable in another *without reloading it* (FR-021b) in `apps/web/e2e/world-cache-multitab.spec.ts`. Tab B is proved warm first (zero refetches) so "it stopped serving from cache" is falsifiable, then a `window` sentinel — which survives client-side navigation and not a document load — proves the reaction happened in the same document. Tab B leaves for `/login`, its engine reports dropping the in-memory key, the stored key is gone, surviving blobs stay sealed against a foreign key, and the byte route refuses it. Mutation-tested: silencing `broadcastSignOut()` turns it red.
- [X] T055 [US3] Guard concurrent multi-tab writes so a partially-written blob is never readable as complete (FR-021). The storage layer moved to a new crate, `crates/thunderforge-opfs`, behind a `BlobStore` trait with an in-memory twin — every line of the old module was `#[cfg(target_arch = "wasm32")]`, so the read/write/delete logic had **no native test at all** and the question FR-021 asks could not be asked. Platform facts that decide the design (WHATWG File System Standard): `getFileHandle({create: true})` publishes a **zero-length** entry to every same-origin tab before its promise resolves, while `createWritable()` buffers into a swap file and `close()` replaces the contents wholesale — so *empty is the only intermediate state a reader can observe*, never a prefix. `move()` (write-then-rename) is in no spec, unbound in web-sys 0.3, and file-handles-only in Chrome, so the window cannot be closed portably and is instead made harmless: `BlobShape` classifies an empty file as `Incomplete`, which reads as a miss and — the actual fix — is **never reclaimed**. Previously a reader that found another tab's just-created file concluded "will not decrypt" and deleted it, with no lock held, orphaning the index row that followed. Same bug, second face: `has_blob` now answers `false` for an incomplete file, so the engine's prefetch can no longer skip an asset forever on the strength of a zero-length stub. 7 native concurrency tests in `crates/thunderforge-opfs/tests/concurrent_writes.rs`, mutation-tested (restoring the delete turns `a_reader_does_not_delete_the_file_another_tab_is_writing` red), in `crates/thunderforge-opfs/`
- [X] T056 [P] [US3] E2E: corrupted blob and orphaned index entry both self-repair silently (SC-005). Three legs, all green, all planting the damage by hand because the states they repair come from a crash between two awaits and a browser cannot schedule one. (1) A blob no row refers to is reclaimed while an *unfinished* one is left alone, since an unreferenced file is exactly what another tab's in-flight write looks like. (2) A row naming a blob that is gone is dropped and the asset comes back — and the convergence is recorded honestly: it takes **two opens**, because `run_sync` builds its manifest before calling `repair_world`, so the open that finds the lie has already claimed the item. (3) A blob that decrypts perfectly and does not hash to its own filename is discarded on read and refetched. All three also assert `pageerror` silence, which is the half of SC-005 — *silently* — that was previously assumed.

  **The third leg had to switch scenes, and finding out why was the work.** `try_cached` declines a load when the cache is not ready or no fingerprint is published, and at boot the scene background is both: the sprite load fires when scene state arrives, well before `sync_world_cache` resolves and `publish_fingerprints` runs. Measured ordering — `HEAD 200`, `GET 404` (`.webp.meta`), `GET 200`, *then* `canvas asset cache ready`. So the read path is reachable only from a **mid-session scene switch**, and the other candidate assets cannot stand in at all: a `createCanvasAsset` asset is never displayed, and a pasted one is placed for its own session only (`upsert_canvas_image_asset` is never persisted). Consequence worth keeping: **a scene background is never served from cache on a page load**, only after a switch. The test's closing assertion — reload, wait for the cache, switch, zero GETs — is the first proof in this suite that a cached blob is read back at all rather than merely written.

  Mutation-tested: neutering the `fingerprint::verify` discard in `read_blob` and rebuilding the wasm turns it red on the assertion that names it (0 requests instead of >= 1), and reverting turns it green. `countAssetRequests` counts **GET only** — `WorldPage` HEADs the background on every scene load, and counting that made this assertion pass on no evidence for two runs, in `apps/web/e2e/world-cache-repair.spec.ts`
- [X] T057 [P] [US3] E2E: two tabs writing the same world concurrently corrupt nothing. **Covered by T055e rather than written again here.** `world-cache-multitab.spec.ts` opens one world in two tabs of a single Playwright context under `Promise.all` and asserts exactly this claim: one session key, the asset stored **exactly once**, no zero-length stub, every blob sealed against a foreign key and not a bare image, and both tabs reloading warm with zero refetches. A second test planting the same setup in the repair suite would re-run that and prove nothing further, and the honest limitation is the same one T055e records — a browser cannot schedule two engine boots into the same millisecond, so the *collision* itself is the crate's job (`concurrent_writes.rs`, 7 native tests via the in-memory `BlobStore` twin's `write_interleaved`) and the browser leg proves the outcome, in `apps/web/e2e/world-cache-multitab.spec.ts`

---

## Phase 7: User Story 4 - Staying within available space (Priority: P2)

**Goal**: Bounded, predictable storage that degrades gracefully under pressure

**Independent test**: Exceed the budget across several worlds; LRU worlds are released, the open world is not, nothing breaks

- [X] T058 [P] [US4] Implement `limit_bytes(quota) = min(quota/2, 20GiB)` in `crates/thunderforge-cache-core/src/budget.rs`
- [X] T059 [P] [US4] Implement `plan_eviction` — whole worlds before items, LRU first, never the open world, deterministic tie-breaking — in `crates/thunderforge-cache-core/src/budget.rs`
- [X] T060 [P] [US4] Unit test: `plan_eviction` never selects the open world even when that leaves the budget unsatisfied, in `crates/thunderforge-cache-core/tests/budget.rs`
- [X] T061 [US4] Read `navigator.storage.estimate()` and recompute the budget on each world open, shrinking the store when quota drops. **`budget.rs` had held the whole policy since T058 and had no caller** — nothing read the estimate and nothing acted on a plan, so the store grew without any limit at all. That is the second time this spec has shipped tested policy nobody invokes (FR-019's repair pass was the first), which is why `index::budget_entries` translates rows for the planner in one named, tested place rather than being open-coded at the call site. `sync::enforce_budget` reads the estimate, derives the limit, plans, and evicts; `run_sync` calls it **after the repair and before the prefetch** — the only point where the index is both accurate and not yet about to grow — passing the planned fetch as `incoming`, so the limit governs where the store is going rather than where it has been. Two decisions worth keeping: a refused estimate evicts **nothing** (`unknown_quota`), because reading "no answer" as zero means a limit of zero, which means destroying a working cache over an unavailable diagnostic API; and eviction locks **per victim world**, not the open world, since what is released belongs to other worlds by construction and `apply_plan`'s open-world lock would serialise against the wrong tab and protect nothing. Seven `budget*` fields added to the sync summary. Proved by T063, in `crates/thunderforge-cache-browser/src/index.rs` and `crates/thunderforge-cache-browser/src/sync.rs`
- [X] T062 [US4] Degrade a failed local write to a server fetch, never to a failed load (FR-024). The *failed write* half already held — `fetch_and_deliver` warns and delivers regardless — but the **no room** half had nothing behind it: `plan_eviction` has always been able to answer `insufficient`, and nothing consulted it, so a full store kept writing. `Control::Storable` now carries that verdict to the resource and to a wasm-side flag (duplicated because the write happens in a `spawn_local` task with no access to the Bevy world, the same reason `HANDLES` lives there). No room means fetched bytes are delivered and **not filed**, and the prefetch is skipped entirely — speculative writes into a full store are the worst version of this, spending bandwidth to either fail or evict something equally wanted. `Default` for `CanvasAssetCache` is hand-written for one field: `bool::default()` is `false`, so a derived default would mean "refuse to cache anything" on every browser until a budget pass said otherwise, and permanently on any browser whose quota cannot be estimated — the whole feature switched off by a derive. The flag is a fact about the last measurement, never a latch, and a sign-out restores it so a stranger's full disk cannot disable the next user's cache. Proved by T063's fourth test, in `src/engine/src/plugins/cached_assets.rs`
- [X] T063 [P] [US4] E2E: budget respected across machines whose reported quota differs by an order of magnitude (SC-006). Four tests. The quota is faked through `addInitScript`, and that is not a stub standing in for the system under test — `navigator.storage.estimate()` is the single input the budget derives from, so overriding it *is* the machine difference, expressed exactly. `addInitScript` rather than `evaluate` because the estimate is read on every load and a value patched into one document would vanish at the first reload, leaving the test measuring the real machine while believing otherwise; only `estimate` is replaced, since `getDirectory` is where the bytes live. (1) The limit tracks the quota across two orders of magnitude — 10,000,000 then 100,000 — without a restart. (2) A browser that refuses to estimate reports `quotaUnknown`, evicts nothing, and keeps its cache. (3) A world that no longer fits is released — `limit 250000, inUse 179424, evicted 1, blobsRemoved 1` — while the open world survives. (4) A store with no room reports `insufficient`, files nothing, and still serves the asset (200) with the canvas visible. **The third and fourth tests had to be separated by quota size, and the collision is worth knowing about:** under a limit too small for even the open world, FR-024 stops writes, so the open world ends up uncached — which reads exactly like "the open world was evicted" while being the opposite, nothing taken and nothing added. Sizing test 3's limit to hold one world means an eviction there can only be the LRU rule. Mutation-tested: short-circuiting the eviction application in `enforce_budget` and rebuilding the wasm turns test 3 red (`evicted 0, blobsRemoved 0`), in `apps/web/e2e/world-cache-budget.spec.ts`

---

## Phase 8: User Story 5 - Seeing and reclaiming storage (Priority: P3)

**Goal**: Visibility and manual control over what is stored

**Independent test**: With several worlds cached, reported figures match reality and clearing one world frees exactly that

- [X] T064 [US5] Expose per-world usage totals. **Deliberately in TypeScript, not the Rust index**, and the deviation is the point. Two reasons. A storage screen is reached from settings on a page that has never mounted a canvas, and downloading a multi-megabyte wasm module to print a number is absurd — `worldCache.ts` already set this precedent for sign-out, for exactly this reason. And a Rust `usage_by_world` would have had **no caller**: this spec has now shipped two pieces of well-tested, entirely uninvoked policy (`missing_blobs`/`orphaned_blobs` until FR-019 got its caller, `limit_bytes`/`plan_eviction` until T061), and adding a third of the same shape on the same day the second was found would be choosing the bug. Nothing moved out of Rust that is *policy* — this reads file sizes and deletes directories; what may be stored, what goes when space runs short, and what a fingerprint means all stay where they were. `userScopeName` mirrors `UserScope::for_user` exactly (SHA-256 over the uuid's **raw 16 bytes**, first 32 hex), verified against an independently computed digest rather than against itself — a version that hashed the uuid's *text* would produce a valid-looking directory name that matches nothing and report an empty cache with total confidence. Figures are **ciphertext bytes on disk**, which is what the user's disk actually gives up; the budget accounts in plaintext because that is the unit the server reports. 9 vitest cases, in `apps/web/src/services/worldCacheStorage.ts`
- [X] T065 [US5] Build the storage view with total and per-world breakdown (FR-025). Mounted at **`/settings/storage`** for any authenticated user — deliberately not `/admin/storage`, which is admin-only and concerns the server's object store; this is per-user, per-device, and a player with no admin rights is exactly who needs it. A world cached but no longer in the account's list (left, or deleted server-side) still shows, as its bare id: it is precisely the content a storage screen exists to let someone reclaim, and hiding nameless rows would make the rows stop adding up to the total. World names are a nicety — a failed world-list fetch leaves the breakdown fully usable rather than emptying it. The read lives in an effect keyed on a reload token rather than a `refresh()` called from an effect body, which is both what the lint rule wants and honest about what it is: a subscription to a filesystem that other tabs, the eviction pass and the sync are all writing to, in `apps/web/src/components/diagnostics/StoragePanel.tsx` and `apps/web/src/pages/user/StorageSettingsPage.tsx`
- [X] T066 [US5] Implement clear-one-world and clear-all, leaving server data untouched (FR-026). Server data is untouched by construction: nothing in this path makes a network request. Blobs go **before** index rows, and the order matters — the reverse leaves rows naming files that are gone, which is the *lie* FR-019's repair pass exists to correct, making the client claim content it cannot serve until a repair runs; blobs without rows are merely unreachable bytes, and this deletes them anyway. Clear-all is scoped to the signed-in user's directory rather than wiping OPFS, because another account on the same machine has its own scope and its content is not this user's to delete. The panel states the safety in the UI rather than only in a docstring: "clear" next to a number is a word people hesitate over, and hesitation is what leaves someone stuck with a full disk and a feature they were afraid to use, in `apps/web/src/services/worldCacheStorage.ts`
- [X] T067 [P] [US5] E2E: clearing one world zeroes its figure, leaves others intact, and the cleared world still loads (US5 scenario 2). Two tests, green: `350 KiB in use across 2 worlds`, two rows of `175 KiB` summing to the headline, one row left after clearing, and world A then reopening and re-caching. **Every claim the panel makes is paired with a look at OPFS**, because a storage screen can be wrong in a way nothing else notices — it can report figures that are internally consistent and describe nothing, and a panel claiming a clear it did not perform would satisfy every assertion about its own text. The "still loads" clause is proved by actually reopening the world, not by reasoning that no network call was made: a clear that quietly cost someone a world would be worse than a full disk. The second test proves the account is untouched — the world is still listed, and its asset still serves the same fingerprint, in `apps/web/e2e/world-cache-storage-ui.spec.ts`

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

- [X] T072 [US7] Implement the durable IndexedDB outbox storing emitted world-store commands verbatim, persisted before local acknowledgement. Landed as its own module, `outbox.rs`, rather than inside `index.rs` — the crate is one concern per module (crypto, index, locks, opfs, signal, sync) and queued work is not the content index. The `outbox` store was already declared at upgrade time precisely so this could land without a schema bump. Durability is the whole feature: `append` is `async` and fallible so a caller that reports success without awaiting it has reintroduced the bug the module exists to prevent (FR-037). Commands are stored **verbatim and never parsed here** — Constitution Principle I: replaying the emitted command traverses the same mutation and authorization path an online edit does, which is what makes FR-042 automatic instead of a second mechanism to get right. `enqueued_seq` is derived from the stored rows' high-water mark exactly as `index::high_water` is, so a counter lost in a crash cannot hand out numbers that sort before work already queued and replay someone's edits in an order they never made. `forget_resolved` deletes **only** ids the server spoke about and returns what is left, which makes FR-041 a value the caller must handle rather than an omission it can overlook; a failed delete replays an applied change, which is safe (one outcome per submission), while dropping one is not recoverable. 9 native tests, in `crates/thunderforge-cache-browser/src/outbox.rs`
- [X] T073 [US7] Detect and surface the disconnected state, allowing continued work (FR-036). `LiveSyncState` gains `disconnected`, distinct from `reconnecting` because they are the same situation and a different thing to say about it: a dropped frame recovers in about a second, and announcing "you are offline" for it trains people to ignore the notice. Three failed attempts (~7s of backoff) is the threshold; `navigator.onLine === false` short-circuits it, because there is no interface up and no amount of backoff will help. The converse is deliberately **not** symmetric — `onLine` true means an interface exists, not that the server is reachable, so a café wifi with no route out is online by that measure and disconnected by every useful one. The decision is a pure `connectivityFor` (7 vitest cases) with the socket callbacks reduced to feeding it, since `graphql-ws` callbacks cannot be exercised without a socket. Still non-terminal and needs no manual action: the retry loop is untouched underneath. The banner now says the thing that matters — that work is being kept and play may continue — rather than only announcing the failure, in `apps/web/src/engine/world/sync/subscriptionClient.ts` and `apps/web/src/pages/world/WorldPage.tsx`
- [X] T074 [US7] Restrict offline-editable entities to token position/rotation/scale and refuse create/delete with a clear explanation (FR-035a). `offlineEditVerdict` is one pure, total statement of the rule rather than a condition repeated per call site, which is how two halves of a rule drift apart. **The boundary is about precedence, not storage** — the outbox would queue a deletion happily. `conflict::resolve` decides which of two edits wins, which settles two positions honestly (the loser's value is unused and nothing is lost the user cannot see and redo) and cannot settle a deletion racing an edit, where the only choices are destroying work someone was still doing or resurrecting something someone deliberately removed. Art is refused for that reason plus a second: it points at an asset that may not exist server-side when the change replays. Every refusal explains itself *and* points somewhere — what still works, or to reconnect — because a refusal with no next step invites the user to sit and wait when the table could keep playing; both properties are asserted, in `apps/web/src/engine/world/facets/tokenControl.ts`

### Server reconciliation

- [X] T075 [US7] Implement `reconcileQueuedChanges` per contracts/graphql-delta-sync.md. Applies in submitted order, so a client's own sequential edits do not reorder against each other. Commands arrive verbatim and are parsed **structurally** against FR-035a rather than trusted by their `type` field — the restriction is what keeps conflict resolution honest, so it is enforced where the replay happens and not only where the edit was made. **Deviation, recorded:** the task says "replaying through existing mutations"; the token mutations are `async fn` resolver methods on `TokenMutation` taking a `Context`, not callable from another resolver, so this applies the same authorization *rule* (`move_own_token`'s: GM anything in their world, player only what they own) against the same table rather than routing through the resolver. Extracting shared `*_impl` functions the way `mutations_assets.rs` does would let both call one path and is the better end state; it is a refactor of `mutations_tokens.rs`, not of this file, in `src/server/src/graphql/mutations_reconcile.rs`
- [X] T076 [US7] Re-authorize every queued change against current permissions at reconnect time (FR-042). Membership is read **now**, not as of when the edit was made, so a user removed from a world while offline reconnects to find every queued change refused — the alternative is a window in which revoked access still writes. Non-membership rejects each change individually rather than erroring the call, because the client needs per-change verdicts to know what to tell the user and what to stop replaying. Per-token authorization is asked again too: a player may replay a move of their own token and not of anyone else's, proved with a database-backed test that also asserts the refused replay **wrote nothing**, in `src/server/src/graphql/mutations_reconcile.rs`
- [X] T077 [US7] Adjudicate conflicts via `cache_core::conflict`, emitting ordinary `world_events`. **This is the caller `conflict::resolve` never had** — the third piece of tested-but-uninvoked policy in this spec, and the one whose absence mattered most, since it is the rule the whole offline story rests on. Nothing here reimplements it: the client predicts with the same function so the UI can say what will happen, and the two answers must never differ (FR-040b). `ReconnectSeq` is a server-assigned per-world counter and never a clock — a client timestamp is forgeable and a skewed one would silently overwrite other people's work, which is the exact failure a conflict rule exists to prevent. Applying emits an ordinary `EVENT_CODE_TOKEN_CHANGED`, so other clients learn about a reconciled change through the subscription they already have, with no second delivery path to drift from the first. **Honest limit, stated in the module docs:** the (world, token) marks live in process, so a server restart forgets them and two players reconnecting either side of it both apply, the later winning on last-write. The window is minutes and the cost is a token position — visible and re-doable. Making it durable is a table written in the same transaction as the edit, and is the right call if offline authoring ever widens past FR-035a, at which point what is lost stops being a position and starts being work, in `src/server/src/graphql/mutations_reconcile.rs`
- [X] T078 [US7] Guarantee exactly one outcome per submitted change (FR-041). Enforced by the shape of the code rather than by discipline: outcomes are produced by mapping over the inputs, so an input without an outcome is not a state this function can express, and every early return inside the loop produces a rejection instead of a `continue`. Tested by submitting a valid move, a change against a server-deleted token, and a command outside FR-035a in one call, then asserting the outcome ids equal the submitted ids **in order** — a test that counted outcomes would pass while answering the wrong change, in `src/server/src/graphql/mutations_reconcile.rs`

### The supersession edge

- [X] T079 [US7] Recognise a previously-applied local change being overridden by a later GM reconnect and inform the user (FR-041, `Applied → Superseded`). Landed as `sync/reconcile.ts` re-exported from the barrel — `index.ts` is a barrel and every sibling concern (walls, lights, shapes, tokens) is its own module. The player is long gone from the reconcile call by the time a GM reconnects, so there is no response to put the news in; the only carrier is the ordinary `world_events` subscription they already have. Which means the server event had to grow `reconciled`, `by_user` and `by_role`: without them a replayed change is indistinguishable from someone at the table simply moving a token, and the player is left believing an edit stands that does not. Two conditions gate it and both matter — the event must touch a token this client changed at its own reconnect, **and** come from somebody else, because this client's own reconcile emits these events too and without the check every user would be told their work was overridden by themselves the moment they reconnected. The 20-minute window errs long deliberately: a stale entry costs one notification that is true and merely late, while erring short costs silence about lost work, in `apps/web/src/engine/world/sync/reconcile.ts`
- [X] T080 [US7] Report each reconcile outcome to the user, distinguishing `SUPERSEDED` from a generic failure. Supersession is not an error — it is FR-040 working exactly as designed — and reporting it like a failure teaches the player that reconnecting is unreliable rather than that the GM moved the token. One of those is true and actionable; the other is a bug report against working software. So it says who won, which is the only reason `supersededByRole` exists on the outcome. Three sections rather than one list, because "applied", "refused" and "the server said nothing" are three different things to a user: the third is worded as still-pending, not lost, since those entries stay queued. Not a toast — the report waits to be acknowledged, since an offline session's work vanishing behind a timeout while someone is looking at the map is FR-041's silent loss arriving by another route, in `apps/web/src/components/world/ReconcileReport.tsx`
- [X] T081 [US7] Preserve outbox entries as a record of lost work when the key is gone. It was already true and only by omission — `reclaim` clears the index and the blob scope and never touched the outbox — which is exactly the kind of invariant the next person breaks doing something obviously reasonable, namely adding a store to `ALL_STORES` and a matching `clear`. Now stated in `reclaim` and pinned by a named test. The distinction is the point: everything else the cache holds is a *copy* of content the server still has, worth nothing once the key is gone; a queued change is the only copy of work the server has never seen, so reclaiming it would be the one deletion in that path that destroys rather than frees. Entries also survive key loss on their own terms, being plaintext commands rather than ciphertext — they cannot be *submitted* until someone signs in again, which is a reason to keep and report them, in `crates/thunderforge-cache-browser/src/outbox.rs` and `src/engine/src/plugins/cached_assets.rs`
- [X] T082 [US7] Handle re-disconnection mid-submission without double-applying or dropping the remainder. One rule does it: a change leaves the outbox only once the server has spoken about it, so an interrupted submission leaves the unanswered remainder queued and the next reconnect sends it again. Re-sending an already-applied change is safe **because** of T078 — exactly one outcome per submitted change, so a second submission simply earns a second outcome — while dropping one is not recoverable, and that asymmetry is what decides the behaviour rather than an attempt to detect the interruption precisely. `matchOutcomes` keeps silence separate from refusal for the same reason: a refusal is a decision the user can be told about, and silence is a contract violation whose entries must stay queued rather than be reported as handled, in `apps/web/src/engine/world/sync/reconcile.ts`
- [X] T083 [US7] E2E: offline change applied on reconnect and outcome reported (SC-015) in `apps/web/e2e/world-cache-offline.spec.ts`. **Found a real bug, which is what an end-to-end test is for.** The token mutation bridge seeded its engine-id map once from the scene's tokens at start and only ever extended it from its own `createToken` calls, so a token created afterwards through the token panel — which calls the mutation directly and tells the bridge nothing — was never in the map. Every later drag of it read as a *first sighting*, and FR-035a's "creations are not queued offline" branch swallowed an ordinary move: nothing queued, nothing to replay, and no `reconcileQueuedChanges` request ever made. The bridge now learns from `sync` dispatches, which is how the client hears a token exists at all. Note what nearly hid it: the "the server must not have it yet" assertion passed throughout and was read as proof that queueing worked. It is not — a silently dropped edit leaves the server unchanged too. That assertion separates "queued" from "sent anyway" and says nothing about "dropped". Pinned by `apps/web/src/engine/world/sync/__tests__/tokenBridge.test.ts`, which fails on the old bridge
- [X] T084 [US7] E2E: GM and player edit the same token offline, **player reconnects first**, both converge **and the player is notified of supersession** (SC-016) in `apps/web/e2e/world-cache-offline.spec.ts`. The convergence half was easy; the notification half is the requirement, and it arrives as an ordinary world event because the player is long gone from their own reconcile call by the time the GM makes theirs. **Two client bugs surfaced only because two sessions were driven at once.** A player was never sent the record of the scene they were playing — `scenes(worldId:)` filters hidden scenes for non-GMs (spec 022 FR-008) while the active scene id comes from the world's unfiltered `activeSceneId`, and a new scene is hidden by default — so a player's canvas had no map art and no grid, and with no `SceneGrid` the engine hit-tests a token against a fixed size rather than its grid footprint. Fixed at the source: a member may now read the one scene their world is playing. The test also had to stop aiming clicks at the store's token position: `snap_tokens_to_grid` moves a token to its grid-cell centre inside the engine while the store keeps the unsnapped value, so the two disagree by up to half a cell and a press on the stored position lands on the cell boundary. `dragToken` searches a small lattice around it and confirms the grab from the selection the press produced *before* moving, so a mis-aim can never drag a bystander
- [X] T085 [P] [US7] E2E: queued change against server-deleted content is discarded with an explanation, not resurrected, in `apps/web/e2e/world-cache-offline.spec.ts`

---

## Phase 10: Peer-Assisted Distribution (supports US1)

**Goal**: Fetch bytes from session peers instead of the server, safely

**Note**: An optimization layered on US1, not a story of its own. Strictly optional at runtime (FR-048) — every path falls back to the server.

- [X] T086 [US1] Implement `sendPeerSignal` / `peerSignals` relaying only between current members of the named world, never interpreting payloads, in `src/server/src/peer_signaling.rs`. **Registration is the guard**: `register()` hands back the receiver and a guard whose `Drop` unregisters, and the guard is carried inside the subscription stream's own state — so an entry cannot outlive its connection, which is FR-050 by construction rather than by a cleanup job that can be forgotten. A monotonic token stops a late-dropping old guard evicting a reconnect that reused the same session id. Membership is re-checked per signal at **both** ends, so a revoked player stops being relayed mid-session rather than at the next connect. The contract's input grew `fromSessionId` and its subscription grew `sessionId`; both are recorded in `contracts/peer-protocol.md` with the reasoning. The first is not bookkeeping — it is verified against the registry, because without it a member could forge `PeerSignal.fromSessionId` and impersonate another participant on the channel the recipient is about to trust for SDP
- [X] T087 [US1] Report and expose `PlanItem.peerAvailable` as advisory only in `src/server/src/graphql/queries/world_sync_plan.rs`. Reports **reachability, not holdings**: true iff a live session in that world belongs to someone else. The field's name promised "peers known to hold it", which the server could only answer by tracking what every client caches — a privacy cost nothing in the spec asks for, to sharpen a hint whose whole point is that ignoring it must be safe (FR-048). The doc comment was rewritten to say what it now means, since a comment that lies is worse than the stub it replaced. Read *after* authorization, so a refusal never leaks whether anyone is playing in that world, and the caller's own sessions are excluded because two tabs share an origin and therefore share the cache a transfer would fill
- [X] T088 [US1] Implement the WebRTC data channel and the REQUEST/OFFER/CHUNK/DONE/DECLINE protocol in `crates/thunderforge-cache-browser/src/peer.rs`. One binary frame shape for every message — a peer must not get two parsers to play against each other — and binary rather than JSON because base64 would cost a third more on the one thing this protocol actually moves. `decode` is total and silent: a malformed frame is `None`, never an error. Pure half compiled and tested natively, WebRTC glue behind `cfg(target_arch = "wasm32")`, the split the rest of the crate already uses. No STUN/TURN: host candidates on loopback are what a session of players on the same signaling server needs, and a public reflector would be an unnecessary third party in a privacy-sensitive path
- [X] T089 [US1] Request only fingerprints present in the client's own current `SyncPlan.fetch` — the enforcement point for FR-047 — in `crates/thunderforge-cache-browser/src/peer.rs`. **Unexpressible rather than checked.** `PlanScope::from_plan` is the only constructor of a scope; `PlanScope::request()` is the only constructor of `PeerRequest`, whose fields are private and which is not `Clone`; `PeerDownload::begin` consumes one by value. There is no expression in the program that asks a peer for a fingerprint the server did not list, so FR-047 cannot be regressed by forgetting a check that isn't there
- [X] T090 [US1] Verify peer bytes before storing; on mismatch discard, do not retry that peer, fall back to the server, in `crates/thunderforge-cache-browser/src/peer.rs`. `DownloadStep::Verified` is the only variant carrying bytes and is produced at the single statement where `fingerprint::verify` returns `Ok`. The buffer is private and cleared on every fall-back, so "no partial store" is a property rather than a rule someone must remember. An `OFFER` must also match the **server's** byte count, so a hostile offer is not an allocation primitive. `Fallback::distrusts_peer()` draws the contract's line: dishonesty (mismatch, wrong size, protocol abuse) costs a peer its trust; a decline, a disconnect or a stall does not — those are ordinary
- [X] T091 [US1] Serve only locally-held verified fingerprints; `DECLINE` otherwise; stop serving on losing membership; rate-limit, in `crates/thunderforge-cache-browser/src/peer.rs`. Membership is checked **before** the held-set check, which the contract does not say and which matters: otherwise the *shape* of the refusal — `NOT_HELD` versus `NOT_PERMITTED` — tells a former co-player which assets this client still holds from a world it was removed from. Both answers are identical after membership loss, pinned by a test. `BlobProvider` is injected, so the serving path physically cannot read anything the engine has not handed it
- [ ] T092 [US1] Add the visible peer-transfer indicator and the persisted enable/disable setting, **defaulting to enabled**, warning that disabling also forfeits server-isolated play (FR-049) in `apps/web/src/components/diagnostics/PeerPanel.tsx`
- [X] T093 [P] [US1] E2E: peer-supplied mismatched bytes are rejected and the server fallback succeeds (SC-012) in `apps/web/e2e/world-cache-peer.spec.ts`. The corruption is injected by wrapping `RTCDataChannel.prototype.send` in the *serving* context, outside the application: nothing shipped has a "serve corrupt content" switch, and adding one to make a test possible would put the failure mode into the product. One flipped payload byte, frame otherwise well-formed and correctly sized, so the only thing wrong with it is the hash. **The load-bearing assertion is the last one** — that the player ends up holding the correct bytes — because a test that only checked the rejection would pass on a client that rejected and then gave up, which fails FR-048 just as badly and is the likelier bug. Deliberately not gated on observing `connectedPeers` first: the client drops a lying peer so fast the count rises and falls inside a window a poll can miss, which is the code being right
- [X] T094 [P] [US1] E2E: content the requester lacks permission for is never requested nor obtained from a peer (SC-014) in `apps/web/e2e/world-cache-peer.spec.ts`. Uses art on a hidden scene that is **not** the one being played, which is now the only shape that genuinely denies a player — so it doubles as the guard on the active-scene carve-out from the other side: if that ever widened to "any hidden scene", this fails rather than the rule eroding quietly. Asserts the entitled asset *does* arrive first, so the negative means something rather than passing on a dead channel, and checks the byte route denies it too — plan and bytes agreeing is what `auth::scene_visibility` exists for
- [X] T095 [US1] E2E: the whole suite passes with peer transfer disabled, outcomes identical (SC-013) in `apps/web/e2e/world-cache-peer.spec.ts`. Counts signaling calls **at the wire** rather than reading the client's opinion of itself, because "disabled" has to mean no connection was ever attempted — the IP exposure the setting prevents happens when the connection is made, not when bytes move. The setting is seeded before any application script runs, which is what a user who turned it off in an earlier session actually has

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
- [X] T128 Update `MVP.md` post-MVP notes to reflect that engine load feedback shipped and that bundle-size work remains separate and open. Stated as a distinction rather than a status line, because the two are easy to conflate: feedback is not size, and a first load that reports itself honestly is a different problem from a first load that is large. Only the first is closed

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
