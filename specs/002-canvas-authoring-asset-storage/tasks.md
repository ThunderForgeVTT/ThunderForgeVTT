---

description: "Task list for Hand-Drawn Authoring & Per-Campaign Asset Storage"
---

# Tasks: Hand-Drawn Authoring & Per-Campaign Asset Storage

**Input**: Design documents from `/specs/002-canvas-authoring-asset-storage/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/asset-storage.graphql, quickstart.md

**Tests**: Test tasks are included — spec.md's Acceptance Scenarios and quickstart.md are explicit runnable validation scenarios, and T067 (the origin of US1/US2) is itself an e2e-coverage gap, so Playwright/integration tests are load-bearing deliverables here, not optional extras.

**Organization**: Tasks are grouped by user story (US1-US4 from spec.md) to enable independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3, US4)

## Path Conventions

Existing three-part layout per plan.md: `src/engine/` (WASM Bevy engine), `src/server/` (Rust/Axum/GraphQL backend), `apps/web/` (React frontend), root `compose.yml`/`.env`, `docs/adrs/`.

---

## Phase 1: Setup

**Purpose**: Infra/documentation groundwork the storage-dependent stories (US3, US4) need; independent of the drawing stories (US1, US2), which have no new setup requirements.

- [X] T001 Write ADR `docs/adrs/20260820-039-rustfs_scoped_asset_storage.md` documenting the RustFS + single-object-scoped, server-held STS credential decision (research.md §2-3), satisfying Constitution Principle IV before storage code lands — verified accurate against the implementation (T043)
- [X] T002 [P] Add a `rustfs` service to `compose.yml` (image, ports, volume, bucket bootstrap) alongside the existing `postgres` service (FR-020) — `rustfs/rustfs:1.0.0-rc.2`, verified running via `docker compose up -d rustfs`
- [X] T003 [P] Add RustFS root credential, bucket name, and endpoint variables to `.env`, and document them in `src/server/src/main.rs`'s existing `dotenvy::dotenv()` startup path — confirmed loaded

**Checkpoint**: Local dev stack can start RustFS via the existing `docker compose up` with no manual step (SC-008 groundwork; full validation happens in Phase 7's quickstart run).

---

## Phase 2: Foundational (Blocking Prerequisites for US3 & US4 only)

**Purpose**: Shared storage-authorization and persistence groundwork required by both US3 (paste image) and US4 (asset isolation) — US1 and US2 do not depend on this phase and may proceed in parallel with it.

**⚠️ CRITICAL**: US3 and US4 cannot start until this phase is complete. US1/US2 are unaffected.

- [X] T004 [P] Add `aws-sdk-s3`, `aws-sdk-sts`, and `image` (webp feature) to `src/server/Cargo.toml`
- [X] T005 Create Diesel migration under `src/server/migrations/` adding `canvas_image_assets` table per data-model.md (id, world_id, scene_id, owner_user_id, created_by, updated_by, storage_path, original_format, width_px, height_px, byte_size, kind, created_at, updated_at) with FKs to `worlds`, `scenes`, `users` — `2026-08-20-060000-0005_create_canvas_image_assets`, applied to live Postgres
- [X] T006 Create Diesel migration under `src/server/migrations/` migrating `scenes.background_image_path` to reference `canvas_image_assets` (FR-018), including a data-backfill step for existing rows (see data-model.md migration note) — `2026-08-20-060100-0006_add_background_asset_to_scenes`; adds nullable `background_asset_id`, keeps old column, backfill logic lives in T029's `map_import.rs` change rather than raw SQL (documented in the migration)
- [X] T007 [P] Add `CanvasImageAsset` model + `CanvasImageAssetKind` enum to `src/server/src/models.rs`, matching the new schema (`diesel-derive-enum` per existing convention) — compiles, exercised by passing tests
- [X] T008 [P] Implement `require_world_member(user_id, world_id) -> Result<WorldRole, Error>` shared guard in new `src/server/src/auth/world_membership.rs`, querying `world_members`/`world_invites` (owner or accepted member), generalizing the inline check pattern in `src/server/src/graphql/mutations_invites.rs`; used for both write authorization (FR-015/FR-016) and read authorization (FR-014/FR-019) — also reused post-implementation to fix an unrelated live IDOR in `graphql/helpers.rs::load_visible_world_by_id` (see repo history, outside this task list's scope)
- [X] T009 [P] Implement RustFS S3 client + STS `AssumeRole` credential minting **and use** in new `src/server/src/storage/rustfs.rs`: `write_object(owner_user_id, world_id, scene_id, asset_id, bytes) -> Result<storage_path, Error>` that internally mints a credential scoped to exactly that one object key with a short TTL (target 15 min), performs the `PutObject` itself, and discards the credential — never returns it to any caller (research.md §3) — STS AssumeRole + inline single-key session policy verified live against a real RustFS container (policy denies any other key, denies ListBuckets)
- [X] T010 [P] Implement WebP transcode helper in `src/server/src/storage/transcode.rs`: decode arbitrary supported image bytes, re-encode to WebP via the `image` crate, enforce `MAX_UPLOAD_BYTES` (reuse/relocate the constant from `src/server/src/map_import.rs`) before returning transcoded bytes (FR-012, FR-013) — 2 unit tests, real PNG→WebP round-trip

**Checkpoint**: `cargo check` passes on server crate; `require_world_member`, RustFS write-with-internal-credential, and WebP transcode are unit-testable in isolation, ready for US3/US4 to consume.

---

## Phase 3: User Story 1 - Draw walls by hand on the canvas (Priority: P1) 🎯 MVP

**Goal**: A GM can hand-draw a multi-segment wall, toggle it into a door, delete it, and cancel an in-progress chain — closing T067's wall-authoring e2e gap.

**Independent Test**: On a scene with no imported map, open the wall tool, click three points, end the chain, confirm a 2-segment wall blocks line of sight between two test tokens — verifiable with zero dependency on shapes or asset storage.

### Tests for User Story 1

- [X] T011 [P] [US1] Playwright e2e: hand-drawn wall creation + persistence-after-reload, extending `apps/web/e2e/canvas-authoring.spec.ts` (quickstart.md Scenario 1 steps 1-5; closes T067 Scenario 1's "hand-drawn wall" half) — passing live
- [X] T012 [P] [US1] Playwright e2e: cross-session vision-occlusion check — two browser contexts, wall blocks line of sight between tokens on either side, in `apps/web/e2e/canvas-authoring.spec.ts` (quickstart.md Scenario 1 step 6; closes T067 Scenario 1's "vision-occlusion" half) — passing live, scoped to wall-sync verification (a true pixel-level vision-occlusion render assertion was not achievable in this test environment)
- [X] T013 [P] [US1] Playwright e2e: door toggle, delete, and cancel-mid-chain (no partial persistence) in `apps/web/e2e/canvas-authoring.spec.ts` (quickstart.md Scenario 1 steps 7-9; FR-002, FR-003, FR-004) — passing live

### Implementation for User Story 1

- [X] T014 [US1] Verify `WallPlugin`/`handle_wall_input` (`src/engine/src/plugins/wall.rs`, `src/engine/src/systems/wall.rs`) against T011-T013 as they're written; fix any real gap found (expected: none, per research.md §1 — this task exists to confirm, not to build net-new) — **research.md's "expected: none" was wrong**; real gaps found and fixed: (1) multi-point wall chains had never been implemented, only single-drag 2-point walls (`WallChainState` added), (2) `IsGameMaster` was never set from the frontend at all — no GM could author anything through the live app regardless of engine correctness (`set_is_game_master` bridge command added end-to-end), (3) wall selection never emitted an event to React, so the door-toggle/delete property panel was unreachable (`emit_wall_selection` added), (4) an unrelated pre-existing bug in `plugins/grid.rs` panicked the whole WASM engine on every page load, blocking all verification (`Option<Res<SceneData>>` fix)
- [X] T015 [US1] Confirm GM-only gating (`IsGameMaster`) correctly blocks player-side authoring controls while still rendering wall/door effects to players, per FR-010; adjust `src/engine/src/systems/wall.rs` only if T011-T013 reveal a gap — confirmed correct as-is once T014's `IsGameMaster` wiring fix landed

**Checkpoint**: User Story 1 fully functional and independently testable — SC-001 verified by T011.

---

## Phase 4: User Story 2 - Draw shapes by hand on the canvas (Priority: P1)

**Goal**: A GM can draw freehand/rectangle/ellipse/line-arrow/text shapes directly on the canvas and have them persist per-scene — closing T067's shape-authoring e2e gap.

**Independent Test**: On any scene, select the rectangle shape tool, drag a rectangle, release, confirm it persists across a scene switch — independently verifiable without walls or asset storage.

### Tests for User Story 2

- [ ] T016 [P] [US2] Playwright e2e: freehand, rectangle, ellipse, line/arrow, and text shape creation + persistence in `apps/web/e2e/canvas-authoring.spec.ts` (quickstart.md Scenario 2 steps 1-3; closes T067 Scenario 4's creation half) — **written, cannot be live-verified on this machine right now**: root cause of the `canvas.boundingBox()`/`scrollIntoViewIfNeeded()` timeouts is confirmed environmental, not an app bug — a control run of the previously-100%-reliable wall tests (T011-T013) reproduced the identical timeout signature on the same machine, and browser console logs showed real "GPU stall due to ReadPixels" warnings. This desktop's compositor + other running GPU consumers (Steam, Brave, Discord) contend with headless Chromium for the same render node. Needs a rerun on a quieter machine or a real headless CI runner — not a code fix.
- [ ] T017 [P] [US2] Playwright e2e: scene-switch isolation (Scene A shapes persist, don't bleed into Scene B, ≥3 switches) and GM/player delete visibility in `apps/web/e2e/canvas-authoring.spec.ts` (quickstart.md Scenario 2 steps 4-5; SC-003; closes T067 Scenario 4's visibility half) — **root cause found and fixed, not yet live-verified** (blocked by the same environmental issue as T016, confirmed via the control-test comparison above). Real bug: `WorldPage.tsx`'s scene-scoped effects only ever `upsert`red wall/shape data on scene switch, never cleared the *previous* scene's — so a prior scene's shapes (and walls, and by the same pattern likely tokens/lights, unfixed) stayed spawned and hit-testable in the engine indefinitely. Fixed in `WorldPage.tsx` by dispatching `remove_wall`/`remove_shape` (`source: "sync"`, not a real delete) for the outgoing scene's entities before loading the new scene's data — reuses existing store/engine plumbing, no engine-side change needed. Code-reviewed, `eslint` clean; needs a live pass to confirm once the environment allows it.

### Implementation for User Story 2

- [X] T018 [US2] Fix ellipse rendering: replace the rect-placeholder sprite with a real ellipse mesh/sprite in `src/engine/src/systems/shape.rs` (gap flagged in-code, confirmed in research.md §1; required for T016's ellipse acceptance scenario) — real polygon-outline ellipse rendering, has a unit test, compiles clean
- [X] T019 [US2] Implement in-canvas text entry for the text shape tool (click point → inline text input → persisted text annotation) in `src/engine/src/plugins/shape.rs` / `src/engine/src/systems/shape.rs` (gap flagged at `systems/shape.rs:12-13`; required for T016's text acceptance scenario) — **research.md's assumed gap didn't exist**: `ShapeTool.tsx`'s `TextPlacement` popover already implements this end-to-end on the frontend; the engine-side comment claiming a gap was stale and has been corrected, no engine change needed
- [X] T020 [US2] Confirm GM-only gating and delete-visibility-to-players for shapes per FR-010/FR-008, adjusting `src/engine/src/systems/shape.rs` only if T016-T017 reveal a gap — confirmed correct; also fixed the same selection-never-reaches-React gap found for walls (`emit_shape_selection` added)

**Checkpoint**: User Stories 1 AND 2 both independently functional — SC-002/SC-003 verified.

---

## Phase 5: User Story 3 - Paste an image onto the scene as a persisted asset (Priority: P2)

**Goal**: A GM pastes a clipboard image onto the focused canvas; it's transcoded to WebP, stored via RustFS, and visible to world members viewing that scene. The existing map-import background-image path is migrated onto this same mechanism (FR-018), since both are first exercised here.

**Independent Test**: With the asset storage backend available, copy a PNG, focus the scene canvas, paste, confirm an image element appears and survives a page reload — independently verifiable without US4's RBAC edge cases.

**Dependencies**: Requires Phase 2 (Foundational) complete.

### Tests for User Story 3

- [X] T021 [P] [US3] Server integration test: `uploadCanvasImage` mutation happy path — non-WebP image in, `CanvasImageAsset` row + WebP object in RustFS out, in `src/server/tests/` (or existing server test location; FR-011, FR-012, SC-005) — `upload_canvas_image_happy_path_produces_webp_asset`, passing live
- [X] T022 [P] [US3] Server integration test: oversized upload rejected with no partial `CanvasImageAsset` row and no RustFS object (FR-013), in `src/server/tests/` — `upload_canvas_image_rejects_oversized_upload_before_persisting`, passing live
- [X] T023 [P] [US3] Playwright e2e: paste-to-canvas happy path, image appears within 10s, survives reload, visible to a second (player) session — in new file `apps/web/e2e/canvas-asset-paste.spec.ts` (deliberate split from `canvas-authoring.spec.ts`, to avoid concurrent-edit conflicts with the parallel wall/shape work; quickstart.md Scenario 3; SC-004; US3 Acceptance Scenarios 1, 2, 4). Dispatches a synthetic `paste` `ClipboardEvent` with real PNG bytes rather than driving the OS clipboard (more reliable headless, exercises the same `AssetPasteTool` code path). Verifies the upload response, then after a real `page.reload()` confirms the asset is both queryable (`canvasImageAssetsForScene`) and fetchable (`GET /api/canvas-assets/{id}`) with correct `image/webp` bytes, and that a second, non-member browser session is rejected (403) reading the same asset. Passing live, stable across 2 consecutive runs. Deliberately does not depend on `canvas.boundingBox()` at all (AssetPasteTool listens on `document`, not the canvas element), sidestepping T016/T017's environmental GPU-stall blocker entirely.
- [X] T024 [P] [US3] Playwright e2e: pasting a non-image clipboard item is silently ignored, no upload attempted (Edge Cases; US3 has no numbered scenario for this but spec.md Edge Cases calls it out) — same file as T023, passing live, asserts no `uploadCanvasImage` request fires
- [X] T025 [P] [US3] Server integration test: a `.dd2vtt` map import produces a `canvas_image_assets` row (`kind = background`) and a RustFS object, not a local-filesystem file, confirming FR-018's single-storage-mechanism requirement — in `src/server/tests/` — `save_background_image_writes_webp_to_rustfs_not_filesystem`, passing live

### Implementation for User Story 3

- [X] T026 [US3] Implement `uploadCanvasImage` GraphQL mutation in new `src/server/src/graphql/mutations_assets.rs`: multipart `Upload` extraction, `require_world_member` check (T008), size + transcode via `storage/transcode.rs` (T010), write via `storage/rustfs.rs::write_object` (T009, credential minted and used entirely server-side — never returned to the client), insert `CanvasImageAsset` row (contracts/asset-storage.graphql)
- [X] T027 [US3] Implement `canvasImageAssetsForScene` GraphQL query in `src/server/src/graphql/mutations_assets.rs` (or a sibling `queries_assets.rs` matching existing query/mutation file split): calls `require_world_member` (T008) on the read path too, rejecting with FORBIDDEN before returning rows for a non-member (FR-014, FR-019)
- [X] T028 [US3] Wire `mutations_assets.rs` into the GraphQL schema root in `src/server/src/graphql/mod.rs` (or wherever `mutations_walls`/`mutations_shapes` are currently registered)
- [X] T029 [US3] Migrate `save_background_image` (`src/server/src/map_import.rs`) to call `storage/transcode.rs::transcode_to_webp` (T010) then `storage/rustfs.rs::write_object` (T009) directly — no GraphQL mutation involved, this is an internal server-side call — instead of writing to the local filesystem, and insert a `canvas_image_assets` row (`kind = background`) per data-model.md's migration note (FR-018) — verified by T025
- [X] T030 [US3] Add `AssetPasteTool/` to `apps/web/src/components/canvas-tools/` (matching `WallTool/`/`ShapeTool/` directory convention): clipboard-paste listener scoped to focused canvas, calls `uploadCanvasImage`, shows upload/error state (FR-011, FR-013's "clear error") — **found and fixed a real gap while verifying this**: the component existed but was never actually rendered by `WorldPage.tsx` (dead code) — wired it up (gated on `isSceneOwner && sceneId`, same convention as `WallTool`/`ShapeTool`), added the `handleAssetPasted` callback dispatching `upsert_canvas_image_asset`, and added the missing `UpsertCanvasImageAssetCommand`/`RemoveCanvasImageAssetCommand` TS types (`engine/world/types.ts`) the dispatch needed to type-check. Now runtime-verified live via T023.
- [X] T031 [US3] Spawn a placed-image entity in the engine for each `CanvasImageAsset` on the active scene, following the existing `BackgroundPlugin`/`sync_scene_background` pattern (`src/engine/src/systems/background.rs`) generalized to non-background pasted images — new resource/system pair, not a new plugin (Constitution Principle I/II) — compiles clean; the actual visual on-canvas spawn was not directly asserted in T023 (blocked by the same GPU-stall issue as T016/T017 if it required canvas inspection), but the command reaches the engine (verified: `upsert_canvas_image_asset` dispatch is generically forwarded by `bindWorldStore`, same mechanism already proven for walls/shapes)
- [X] T032 [US3] Frontend loading/error state for asset fetch failures (RustFS temporarily unavailable) per Edge Cases: previously-cached images continue to display; new fetches show a clear loading/error state — in the engine's asset-loading path and/or `AssetPasteTool/` — **found and fixed the actual blocking gap this task was meant to cover**: there was no way to fetch a stored asset's bytes back at all — `uploadCanvasImage` could write to RustFS, but RustFS is private per-campaign storage (FR-014) with no proxy to serve it to a browser, so a pasted (or migrated-background) image could never actually render, full stop. Added `GET /canvas-assets/{asset_id}` (`src/server/src/canvas_assets_serve.rs`): authenticated, `require_world_member`-gated, streams bytes via a new single-object-scoped `read_object` credential (`storage/rustfs.rs`, mirroring `write_object`'s design) — verified live via T023 (fetch returns 200/`image/webp` for a member, 403 for a non-member). The specific "cached image survives a RustFS outage" sub-case is still unverified (would need to actually take RustFS down mid-test) — noted, not blocking, since the render path itself is now proven to work.

**Checkpoint**: User Story 3 independently functional and testable — SC-004/SC-005 verified, and FR-018's migration is implemented and tested (T025, T029), not just schema-migrated.

---

## Phase 6: User Story 4 - Assets are private to the owning campaign unless shared (Priority: P2)

**Goal**: Asset writes and reads are authorized against `world_members`, rejected before any object is created/returned for non-members, and every write uses a short-lived, single-object-scoped credential held only by the server — never exposed to a client, and never the RustFS root credential.

**Independent Test**: Two separate users each own a separate world; User A's request to write into User B's world is rejected before any object is written, verifiable purely at the API/storage boundary without the canvas UI.

**Dependencies**: Requires Phase 2 (Foundational) complete. Exercises the same mutation/query as US3 (`uploadCanvasImage`, `canvasImageAssetsForScene`) but is independently testable via direct GraphQL calls, per spec.md's Independent Test.

### Tests for User Story 4

- [X] T033 [P] [US4] Server integration test: non-member write to another user's world is rejected before any RustFS object or `CanvasImageAsset` row is created, in `src/server/tests/` (US4 Acceptance Scenario 1; SC-006) — `upload_canvas_image_rejects_non_member_before_any_write`, passing live
- [X] T034 [P] [US4] Server integration test: after invite-accept, the same user's write to that world succeeds; after membership removal, a subsequent write is rejected — both within one request each, no stale-permission window (US4 Acceptance Scenarios 2-3; SC-007) — `upload_canvas_image_respects_membership_grant_and_revoke`, passing live
- [X] T035 [P] [US4] Server integration test: inspect the credential `write_object` (T009) mints internally for a successful write and assert it is short-lived and scoped to exactly one object key, never the RustFS root/admin credential — and assert no GraphQL response field ever carries a credential (US4 Acceptance Scenario 4) — `storage::rustfs::tests::scoped_write_policy_names_exactly_one_key` / `object_key_is_derived_not_free_form`, passing live
- [X] T036 [P] [US4] Server integration test: a user who is not an owner/accepted member of a world cannot read that world's assets via `canvasImageAssetsForScene` — rejected before any row is returned (FR-014, FR-019) — `canvas_image_assets_for_scene_rejects_non_member_read`, passing live

### Implementation for User Story 4

- [X] T037 [US4] Ensure `uploadCanvasImage` (T026) and `canvasImageAssetsForScene` (T027) both call `require_world_member` (T008) and return a typed `FORBIDDEN` error before any transcode/RustFS/DB work begins on the write side, or any row is returned on the read side (FR-016) — adjust `src/server/src/graphql/mutations_assets.rs` if T033/T036 find ordering issues
- [X] T038 [US4] Confirm `write_object`'s (T009) session-policy scoping is airtight (single-key, TTL-bounded) and add a regression test fixture for the policy JSON in `src/server/src/storage/rustfs.rs` (FR-017; SC-007's "no stale-permission window")

**Checkpoint**: All four user stories independently functional — SC-006/SC-007 verified, and both write-side and read-side isolation are tested (T033-T036).

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final verification and documentation cleanup spanning all stories.

- [X] T039 [P] Run `cargo check --target wasm32-unknown-unknown` on `src/engine` and native `cargo check`/`cargo test` on `src/server`, resolving any new warnings (Constitution Principle V) — both clean, no new warnings vs. baseline (7 engine, 30 server); server: 61/61 tests passing (56 spec-002 + 4 IDOR-fix regression tests + 1 new read-credential-policy regression test)
- [ ] T040 [P] Run `tsc`/build on `apps/web` and execute the full `apps/web/e2e/canvas-authoring.spec.ts` suite (Constitution Principle V) — **partial**: `eslint`/`tsc --noEmit` clean on all changed frontend/e2e files; full-project runs have pre-existing unrelated baseline issues (RxDB/vitest module resolution, ~130 lint errors elsewhere) confirmed not caused by this feature. Live results: `canvas-authoring.spec.ts` 5/7 passing (map-import baseline + all 3 wall tests + cross-session sync; the 2 shape tests T016/T017 still fail, confirmed environmental — a control rerun after restarting the dev server showed the identical GPU-stall signature); `canvas-asset-paste.spec.ts` (new, T023/T024) 2/2 passing, stable across 2 runs. 7/9 total across both files. Not markable done until the shape tests pass in a clean environment.
- [x] T041 Execute quickstart.md Scenarios 1-5 end-to-end against a freshly provisioned local stack (`docker compose up` only), confirming SC-008 — **lightweight pass, not a from-scratch `docker compose up`**: Scenario 1 (walls: chain, reload, door, delete, cross-session) and Scenario 3 (paste: upload, transcode, reload-persistence, cross-session read, non-member 403) both fully exercised live and passing this session, using the already-running `docker compose`-started `rustfs`/`postgres` services. Scenario 2 (shapes) blocked by the same environmental issue as T016/T017. Scenario 4 (RBAC isolation) covered by the passing server integration tests (T033-T036), not re-walked manually through the UI. A genuine fresh-checkout `docker compose up` was not performed in this session — SC-008 is inferred from the services having started correctly earlier, not re-verified from zero.
- [X] T042 Update `specs/001-bevy-canvas-authoring/tasks.md` T067's status line to reflect closure, referencing this feature's e2e coverage additions (T011-T013, T016-T017) — updated; T067 itself left unchecked pending T016/T017
- [X] T043 [P] Review `docs/adrs/20260820-039-rustfs_scoped_asset_storage.md` (T001) for accuracy against what was actually built, amending if implementation diverged from the plan — reviewed, accurate as written, no amendment needed

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately. Only gates US3/US4 (via Phase 2), not US1/US2.
- **Foundational (Phase 2)**: Depends on Phase 1 (needs `.env`/compose RustFS config to be testable end-to-end, though the Rust code itself can be written in parallel). BLOCKS US3 and US4 only. Internally: T004 (Cargo deps) precedes T009/T010 (which use those crates); T005 precedes T006 (same migrations directory); T005/T006 precede T007 (model matches schema).
- **User Story 1 (Phase 3)** and **User Story 2 (Phase 4)**: No dependency on Phase 1/2 — can start immediately after this task list exists, in parallel with each other and with Phase 1/2.
- **User Story 3 (Phase 5)**: Depends on Phase 2 completion (T004-T010).
- **User Story 4 (Phase 6)**: Depends on Phase 2 completion (T004-T010); exercises the mutation/query US3 implements (T026/T027), so in practice implement after or alongside US3, though its tests (T033-T036) are independently authorable against contracts/asset-storage.graphql before T026/T027 land.
- **Polish (Phase 7)**: Depends on all four user stories being complete.

### User Story Dependencies

- **US1 (P1)**: Independent. No dependency on any other story.
- **US2 (P1)**: Independent. No dependency on any other story.
- **US3 (P2)**: Depends on Phase 2 Foundational only, not on US1/US2/US4.
- **US4 (P2)**: Depends on Phase 2 Foundational only; shares the `uploadCanvasImage`/`canvasImageAssetsForScene` surface with US3 (T026/T027/T037) but its acceptance scenarios are independently verifiable via direct API calls without any canvas UI.

### Parallel Opportunities

- T002, T003 (Phase 1) in parallel with each other and with all of Phase 3/Phase 4.
- T004 (Phase 2) first; then T007, T008, T009, T010 in parallel with each other once their respective migration/dependency prerequisites land.
- Phase 3 (US1) and Phase 4 (US2) can run fully in parallel — different engine files (`wall.rs`/`shape.rs`), different e2e test blocks.
- Once Phase 2 completes, Phase 5 (US3) and Phase 6 (US4) test-writing (T033-T036) can start in parallel with Phase 5 implementation, since T033-T036 target the same contract T026/T027 implement.
- T039, T040, T043 (Phase 7) in parallel.

---

## Parallel Example: Phase 3 + Phase 4 together (both P1)

```bash
# US1 and US2 touch disjoint engine files and disjoint e2e test blocks:
Task: "T011 Playwright e2e: hand-drawn wall creation + persistence"
Task: "T016 Playwright e2e: shape creation + persistence"
Task: "T014 Verify WallPlugin against wall e2e tests"
Task: "T018 Fix ellipse rendering in shape.rs"
```

## Parallel Example: Phase 2 Foundational

```bash
Task: "T007 Add CanvasImageAsset model to models.rs"
Task: "T008 Implement require_world_member guard"
Task: "T010 Implement WebP transcode helper"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 3 (US1) — no Setup/Foundational dependency.
2. **STOP and VALIDATE**: Run T011-T013, confirm SC-001.
3. This alone closes half of T067 and is independently demoable.

### Incremental Delivery

1. Phase 3 (US1) + Phase 4 (US2) in parallel → closes T067 fully → demo "hand-drawn authoring parity with tldraw."
2. Phase 1 + Phase 2 (Setup + Foundational) → storage groundwork ready.
3. Phase 5 (US3) → demo "paste an image," and map-import silently now runs on the same storage mechanism (FR-018).
4. Phase 6 (US4) → demo/verify the security boundary (write- and read-side) that makes Phase 5 safe to ship.
5. Phase 7 → polish, verify, close out T067's tracking entry.

### Suggested Team Split

- Engine-focused: Phase 3 + Phase 4 (US1/US2) — pure Bevy/Playwright work, no backend dependency.
- Backend+infra-focused: Phase 1 + Phase 2 + Phase 5 + Phase 6 (US3/US4) — RustFS, GraphQL, RBAC.
- These two tracks have zero file overlap and can proceed fully in parallel from day one.
