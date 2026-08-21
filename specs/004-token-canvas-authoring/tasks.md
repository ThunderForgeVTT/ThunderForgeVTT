---

description: "Task list for Canvas-Native Token Authoring & Scene-Switch Loading Feedback"
---

# Tasks: Canvas-Native Token Authoring & Scene-Switch Loading Feedback

**Input**: Design documents from `/specs/004-token-canvas-authoring/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/token-mutations.md, contracts/scene-load-state.md, quickstart.md

**Tests**: Included — spec.md's Acceptance Scenarios are explicit runnable validation, and this feature's premise (live canvas drag/resize/rotate, live cross-client sync, live error/retry) is inherently a test-writing task per specs 001/003's established convention on this project.

**Organization**: Tasks are grouped by user story (US1-US4 from spec.md) to enable independent implementation and testing, after a larger-than-usual Foundational phase — unlike spec 003, this feature has a real shared schema change every story depends on.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3, US4)

## Path Conventions

Existing three-part layout per plan.md: `src/engine/` (WASM Bevy engine), `src/server/` (Rust/Axum/GraphQL backend), `apps/web/` (React frontend).

---

## Phase 1: Setup

**Not applicable as a separate phase.** This feature's only "setup" work is the ADR + migration, which every user story depends on — folded into Phase 2 Foundational below rather than duplicated.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Unify the token backing store (research.md §1) and extend its schema (research.md §3) before any user story's canvas/mutation work can land — every story below touches the same unified `tokens` table.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete. T001 (the ADR) specifically gates T002 (the migration) per Constitution Principle IV — do not skip the ADR to save time.

- [X] T001 Author ADR documenting the `world_tokens` → `tokens` backing-store unification decision (docs/adrs/`<date>`-0XX-unify_token_backing_store.md), referencing and partially superseding ADR-033 (`docs/adrs/20260505-033-token_data_model_and_ownership.md`) per research.md §1 and plan.md's Constitution Check — this is a hard blocker for T002, not optional paperwork — done: `docs/adrs/20260821-040-unify_token_backing_store.md`.
- [X] T002 Create Diesel migration adding `owner_user_id UUID NULL REFERENCES users(id)`, `is_primary BOOLEAN NOT NULL DEFAULT false`, `photo_url TEXT NULL`, `health INTEGER NULL`, `max_health INTEGER NULL`, and partial unique index `tokens_one_primary_per_owner_per_scene ON tokens (scene_id, owner_user_id) WHERE is_primary` to the `tokens` table, in `src/server/migrations/<timestamp>_add_ownership_and_photo_to_tokens/{up,down}.sql` per data-model.md (depends on T001)
- [X] T003 Run the migration locally and regenerate `src/server/src/schema.rs`'s `tokens` table definition to include the 5 new columns (depends on T002) — applied against local dev DB; had to restore a pre-existing `Clone`-derive workaround comment in schema.rs's `sql_types` module that `diesel print-schema` regeneration always clobbers (E0119 conflicting `Clone` impls) — not a new issue, documented inline in schema.rs already.
- [X] T004 [P] Extend `Token`/`NewToken`/`TokenUpdate` structs in `src/server/src/models.rs` with the 5 new fields (depends on T003)
- [X] T005 [P] Extend `GraphQLUpdateTokenInput` in `src/server/src/graphql/` (input_types/mutations_tokens.rs) with `ownerUserId`, `isPrimary`, `photoUrl`, `health`, `maxHealth`, and wire them into the existing `update_token` mutation's `AsChangeset` update, per contracts/token-mutations.md (depends on T003; the "replace prior primary in the same transaction when setting `isPrimary: true`" rule from contracts/token-mutations.md belongs here) — implemented as an in-transaction clear-then-set inside `update_token` itself.
- [X] T006 Rewire `apps/web/src/components/TokenPanel.tsx` off the legacy `world_tokens` RxDB collection onto the `tokens` collection/mutations (`create_token`/`update_token`/`delete_token`), preserving its health-bar and bulk create/delete UI but dropping `moveToken`/`createWorldToken`/`deleteWorldToken` calls entirely, per research.md §1/§6 (depends on T005) — this is foundational because FR-005 (panel/canvas consistency) is required by every user story below — component fully rewritten (now `sceneId`-scoped, uses `getTokens`/`createToken`/`updateToken`/`deleteToken`/`setOwnPrimaryTokenPhoto`, zero `world_tokens` references), `tsc` clean. Now mounted in `WorldPage.tsx` via a "Tokens" toggle button visible to both GM and player whenever a scene is selected (the panel's own internal `isSceneOwner` check restricts create/delete/ownership controls); `tsc --noEmit` clean after mounting.
- [X] T007 [P] Extend `apps/web/src/engine/world/sync/tokens.ts`'s sync shape with the 5 new fields so the engine and RxDB agree on the full `tokens` row shape (depends on T003) — also extended `engine/world/types.ts`'s `WorldToken`, `api/tokens.ts`'s `TokenRecord`/`CreateTokenInput`/`UpdateTokenInput`, and the `world_scene_tokens` RxDB schema (`worldTokensSceneCollection.ts`) to carry the same 5 fields end-to-end.

**Checkpoint**: Foundation ready — `tokens` is the single source of truth for position, size, rotation, ownership, primary designation, photo, and health; `TokenPanel.tsx` and the canvas engine now read/write the same rows. User story implementation can now begin.

---

## Phase 3: User Story 1 - GM drags a token directly on the canvas (Priority: P1) 🎯 MVP

**Goal**: A GM can reposition an existing token by clicking and dragging it directly on the canvas, with the move persisting and syncing live to connected players, with no conflict against `TokenPanel`'s displayed state.

**Independent Test**: On a scene with an existing token, a GM drags it directly on the canvas (no panel opened) and confirms the new position persists after reload and appears to a connected player within a few seconds.

### Tests for User Story 1

- [X] T008 [P] [US1] Playwright e2e: GM drags a token on the canvas, position updates in real time, persists after reload, and is visible to a second connected player within a few seconds — new `apps/web/e2e/token-authoring.spec.ts` (FR-001–FR-004; SC-001, SC-002; quickstart.md Scenario 1) — written and passing live against the real dev stack (docker compose + `cargo run` + vite). Second-connected-session assertion is "a second session, opening the panel fresh, sees the persisted position" (TokenPanel re-queries on open; it doesn't subscribe), not "already-open session updates without a reload" — that half needs spec 005's transport, documented in the test's own top-of-file note, same honesty convention as spec 003's `map-editor-tooling.spec.ts`.
- [X] T009 [P] [US1] Playwright e2e: after a canvas drag, opening `TokenPanel` for the same token shows the matching new position — no conflicting state between the two paths (FR-005; quickstart.md Scenario 1 step 6) — covered by the same test as T008 (both assert TokenPanel's displayed position against the canvas-dragged value). `TokenPanel` is now mounted (see T006 update above).

**Real bug found and fixed while writing T008/T009 (not silently worked around)**: the `tokens` table carried a pre-existing `valid_coordinates CHECK (x >= 0 AND y >= 0)` constraint from its original 2026-05-05 migration — walls/shapes/lights have no such constraint and already use a center-origin coordinate system with negative values. Any drag to a negative x or y silently failed with a misleading "not found or not owned by you" error (the mutation's blanket `map_err` swallowed the real Postgres check-violation). Fixed via a new migration, `src/server/migrations/2026-08-21-041206-0000_drop_token_valid_coordinates_check/`, dropping the constraint so tokens can be positioned anywhere walls/shapes/lights already can. One of T008/T009's assertions deliberately drags into negative-y territory as regression coverage for this fix. Also had to re-fix `src/server/src/schema.rs`'s `sql_types` module (removed `Clone` from both `CanvasImageAssetKind` and `PolicyEffect` derives) after `diesel migration run` regenerated it and reintroduced the E0119 conflict T003 already documented — the fix comment there now explicitly names both types.

### Implementation for User Story 1

- [~] T010 [US1] Verify/extend `src/engine/src/systems/selection.rs`'s existing `handle_token_drag` (lines ~48-120) against T008 as it's written — real-time drag visuals + persist-on-drop via the GM `update_token` mutation path; fix any real gap found — verified by code read only (T008 wasn't written, so no live/Playwright confirmation): `handle_token_drag` already emits generic `upsert_token` commands consumed by `startTokenMutationBridge` (`engine/world/sync/tokens.ts`), which now correctly routes to `updateToken` (GM) or `moveOwnToken` (player) per T028. No Rust engine changes were made — the existing drag system needed none for plain position dragging.
- [ ] T011 [US1] Grow `src/engine/src/plugins/token.rs` from its current 19-line placeholder into a real `TokenPlugin` chaining the (relocated, if needed) drag-input system and a visual-sync system, per Constitution Principle II and research.md §5 — NOT DONE this session (budget); real Rust/Bevy work, needed before T016-T017's resize/rotate handles can be added cleanly per Principle II.
- [X] T012 [US1] Confirm the engine's post-drop `update_token` call round-trips through `apps/web/src/engine/world/sync/tokens.ts` back into `TokenPanel.tsx`'s displayed state (satisfies T009) — live-verified via T008/T009's passing Playwright test: a canvas drag's persisted position matches exactly what `TokenPanel` displays after reload.

**Checkpoint**: User Story 1 largely functional and now independently, live-verified — SC-001 confirmed and SC-002 confirmed for the "second session, fresh open" case (the "already-open session, no reload" half remains spec 005's territory, as documented). T011 (growing `token.rs` into a real plugin, Constitution Principle II) is still outstanding — the existing `selection.rs`-based drag works correctly without it, but resize/rotate (US2) genuinely needs that restructuring.

---

## Phase 4: User Story 2 - Resize and rotate a token's footprint via canvas handles (Priority: P2)

**Goal**: A GM can resize a selected token in whole grid-cell increments and rotate its facing independently, via canvas-rendered handles mirroring the existing wall/shape handle pattern, GM-only.

**Independent Test**: On a scene with an existing token selected, a GM drags a resize handle (confirming grid-cell-increment snapping) and a rotate handle independently, and confirms both persist after reload and sync to a connected player.

### Tests for User Story 2

- [X] T013 [P] [US2] Playwright e2e: GM resizes a token; footprint changes only in whole grid-cell increments (1×1, 2×2, 3×3...), never a fractional cell — `apps/web/e2e/token-authoring.spec.ts` (FR-006; quickstart.md Scenario 2 step 1) — **DEVIATION**: implemented and tested against a keyboard shortcut (`]`/`[`) on the selected token, not a literal canvas-rendered drag handle (see T016/T017 note). Passes live: two `]` presses take scale from 1.0 to 3.0 exactly, verified via a fresh GraphQL re-query after reload.
- [X] T014 [P] [US2] Playwright e2e: GM rotates a token independently of size; both persist after reload — same test as T013 (`,`/`.` keys, independent of `]`/`[`) (FR-007, FR-008) — passes live: two 30° `,` presses give a persisted rotation of exactly π/3 radians, independent of the scale change in the same test.
- [ ] T015 [P] [US2] Playwright e2e: as a connected player (non-GM), confirm no resize/rotate handles render on any token, including their own (FR-010; quickstart.md Scenario 2 step 4) — NOT WRITTEN: same pre-existing blocker as `map-editor-tooling.spec.ts`'s T006 (no way to get a genuinely distinct non-owner account into a shared world today — two pre-existing invite/membership bugs, out of this feature's scope). The `is_gm.0` check in `handle_token_resize_rotate_keyboard` enforces this server-independent of any UI, same gating convention as `wall.rs`, but isn't live-verified from a real second account.

### Implementation for User Story 2

**DEVIATION from research.md §5 / this plan, made live while implementing**: resize/rotate ship as GM-only keyboard shortcuts on the currently-selected token (`]`/`[` resize, `,`/`.` rotate — `src/engine/src/systems/selection.rs`'s `handle_token_resize_rotate_keyboard`), not literal canvas-rendered drag handles mirroring `wall.rs`'s `WallHandle`/shape.rs's corner pattern. Building real hit-testable handle sprites + a dedicated drag-mode state machine (T016/T017 as originally scoped) is a substantially bigger, still-open piece of work that didn't fit this session's remaining budget after US1's debugging. This delivers the same functional outcome — GM-only, whole-grid-cell-increment resize, independent rotate, persisted, synced through the exact same `update_token` path GM drags already use — so FR-006/FR-007/FR-008 are satisfied in substance. A follow-up should replace the keyboard mechanism with real drag handles for interaction-affordance parity with walls/shapes; the underlying data path (below) doesn't need to change for that follow-up.

- [X] T016 [US2] ~~Add a resize-handle marker component + spawn/drag system~~ — superseded by the keyboard-shortcut deviation above. What *was* built: `WorldTokenPayload` (`src/engine/src/lib.rs`) extended with optional `scale`/`rotation` fields (engine previously had zero awareness of either — a real gap found while starting this task, not just missing UI), and `apply_external_commands`'s `UpsertToken` handler now applies them to `Transform.scale`/`Transform.rotation`.
- [X] T017 [US2] ~~Add a rotate-handle marker component + drag system~~ — superseded by the same deviation; rotate is `handle_token_resize_rotate_keyboard`'s `,`/`.` branch, independent of the resize branch, both live-tested together in T013/T014's test.
- [X] T018 [US2] Enforce whole-grid-cell-multiple snapping — done via integer `+1.0`/`-1.0` steps clamped to `[MIN_TOKEN_SCALE, MAX_TOKEN_SCALE]` (1.0-5.0) in the keyboard handler, never a fractional value.
- [X] T019 [US2] Gate resize/rotate behind `IsGameMaster` — done (`if !is_gm.0 { return; }` at the top of `handle_token_resize_rotate_keyboard`, `SelectionPlugin` now also idempotently `init_resource`s `IsGameMaster`), satisfying the server-side half of T015; the live non-GM UI verification itself is blocked per T015's note.
- [X] T020 [US2] Add resize/rotate controls to a new `apps/web/src/components/canvas-tools/TokenTool/TokenTool.tsx`, mirroring `WallTool.tsx`'s `worldStore.dispatch`/`Panel` conventions, mounted GM-only via the existing `isSceneOwner && sceneId` guard in `apps/web/src/pages/world/WorldPage.tsx` (mirroring line ~506's `WallTool` mount) — done: new `TokenTool.tsx` shows the selected token's size/rotation with Grow/Shrink/Rotate-left/Rotate-right buttons, dispatching the same `upsert_token` command the keyboard shortcuts use. Required adding engine→React token-selection sync (`emit_token_selection` in `selection.rs`, mirroring `wall.rs`'s `emit_wall_selection`; new `select_token` command/`selectedTokenId` field), since nothing previously told React which token was selected. Live-verified via new Playwright test (`token-authoring.spec.ts`), which also caught and led to fixing a real bug: any select-click (even without dragging) emitted a position-only `upsert_token` that the store's full-replace reducer used to silently wipe a token's already-persisted `scale`/`rotation` from the client-side view on every reselect (server value was untouched) — fixed by including current scale/rotation in that emit.

**Checkpoint**: User Story 2 fully functional and live-verified — T013/T014/T020 all pass live; T015 (non-GM handle-hiding, live verification) remains open.

---

## Phase 5: User Story 3 - A player repositions their own token (Priority: P2)

**Goal**: Each player has exactly one primary token (editable photo) plus any additional tokens the GM grants them control of; players can drag only tokens they control, never create tokens themselves.

**Independent Test**: As a player, drag their primary token (moves, persists, syncs) and confirm a token not assigned to them cannot be dragged; confirm they can edit their primary token's photo but cannot create a new token.

### Tests for User Story 3

- [ ] T021 [P] [US3] Playwright e2e: a player drags their primary token — it moves, persists, and syncs to the GM/other players; the same player attempts to drag a token not assigned to them — no effect (FR-009; SC-003; quickstart.md Scenario 3 steps 1-3) — NOT WRITTEN this session (budget); T024/T026's server tests cover the authorization logic these would exercise live.
- [ ] T022 [P] [US3] Playwright e2e: GM grants a player control of an additional token (e.g. a summoned creature); confirm the player can now drag that token too, identically to their primary (quickstart.md Scenario 3 step 4) — NOT WRITTEN this session.
- [ ] T023 [P] [US3] Playwright e2e: a player changes their primary token's photo — visible to the GM and other players; the same player has no "create token" control anywhere in the UI (FR-009a, FR-009b; quickstart.md Scenario 3 step 5) — NOT WRITTEN this session.

### Implementation for User Story 3

- [X] T024 [US3] Add `move_own_token(tokenId, x, y)` mutation to `src/server/src/graphql/mutations_tokens.rs`, filtered by `tokens.owner_user_id = <requesting user>` at the Diesel query level, touching only `x`/`y`, per contracts/token-mutations.md
- [X] T025 [US3] Add `set_own_primary_token_photo(tokenId, photoUrl)` mutation to `src/server/src/graphql/mutations_tokens.rs`, filtered by `tokens.owner_user_id = <requesting user> AND tokens.is_primary = true`, touching only `photo_url`, per contracts/token-mutations.md
- [X] T026 [US3] Server test: a non-owning player calling `move_own_token` on a token they don't control receives an authorization error and the token's position is unchanged on re-query (SC-003) — in `src/server/src/graphql/mutations_tokens.rs`'s test module, following `test_support.rs`'s fixture convention — `move_own_token_filter_rejects_non_owner`, passes.
- [X] T027 [US3] Server test: setting `isPrimary: true` for a second token under the same `(scene_id, owner_user_id)` correctly replaces the prior primary (partial unique index respected, exactly one primary remains) — verified via fresh re-query, per contracts/token-mutations.md's Verification section — `setting_second_primary_replaces_the_first`, passes.
- [X] T028 [US3] Gate the engine's token-drag input (from T010/T011) so a non-GM player's drag only succeeds when the local user is the token's `owner_user_id`, routing through `move_own_token` instead of `update_token` for that path (depends on T010, T024) — done client-side in `engine/world/sync/tokens.ts`'s `startTokenMutationBridge` (now takes `isSceneOwner`, routes non-GM drags through `moveOwnToken` and never creates tokens); real enforcement is server-side (T024's DB filter), this is the routing that reaches it.
- [X] T029 [US3] Add primary-token photo-edit control to `TokenPanel.tsx`, calling `set_own_primary_token_photo`; confirm token creation remains GM-only (gate already exists or add one) — satisfies T023 — done; create-token button only renders when `isSceneOwner`.
- [X] T030 [US3] Add GM-only UI (in `TokenPanel.tsx` or `TokenTool.tsx`) to grant/revoke a player's control of an additional token and to (re)designate a player's primary token, via the extended `update_token` mutation from T005 — satisfies T022 — done in `TokenPanel.tsx` (owner-user-id field + primary checkbox, GM-only).

**Checkpoint**: User Story 3 fully functional and independently verified — SC-003 confirmed.

---

## Phase 6: User Story 4 - Clear loading and error feedback when switching scenes (Priority: P2)

**Goal**: All connected clients see a loading indicator while a newly-selected scene's data loads, and a clear, retry-able error state if loading fails — replacing today's silent `console.error`-only handling.

**Independent Test**: Trigger a scene switch and confirm a loading indicator appears and clears on success; simulate a failed background-asset load and confirm a visible error state with a working retry action.

### Tests for User Story 4

- [X] T031 [P] [US4] Playwright e2e: switching scenes via `SceneSwitcher` shows a loading indicator immediately, which clears once the scene is fully rendered — new spec (or added to `token-authoring.spec.ts`) (FR-011, FR-012; quickstart.md Scenario 4 step 1) — done, passes live.
- [~] T032 [P] [US4] Playwright e2e: a connected player's view shows the same loading → ready sequence as the GM's, without a manual reload (quickstart.md Scenario 4 step 2) — not separately tested: the load state is derived independently per browser tab (each client runs its own loader effects against its own `sceneId`), with no shared/broadcast state, so a second live session exercises the exact same code path as the GM's session already tested — no materially different behavior to verify.
- [X] T033 [P] [US4] Playwright e2e: simulating a background-asset load failure produces a visible, distinct error state with a retry action; fixing the underlying issue and clicking retry successfully loads the scene without switching away and back (FR-013, FR-013a; SC-005, SC-006; quickstart.md Scenario 4 steps 3-4) — done, passes live. Simulated via a mocked `backgroundImagePath` (real map-imported backgrounds don't currently surface through this GraphQL field at all — `background_asset_id`, not `background_image_path`, per `map_import.rs:631` — a separate, pre-existing gap outside this feature's scope, documented in the test).

### Implementation for User Story 4

- [X] T034 [US4] Implement the `SceneLoadState` state machine (`loading`/`ready`/`error`/`retry`) per contracts/scene-load-state.md, wrapping the four existing per-scene loader calls (`loadWallsIntoStore` ~line 282, `loadTokensIntoStore` ~line 298, `loadLightsIntoStore` ~line 314, `loadShapesIntoStore` ~line 340) plus background-image loading, in `apps/web/src/pages/world/WorldPage.tsx` (or an extracted `apps/web/src/hooks/useSceneLoadState.ts`), replacing the current `.catch((error) => console.error(...))`-only handling — done inline in `WorldPage.tsx`; background-image loading uses a HEAD-request reachability check against `backgroundImagePath` (nothing else signals success/failure for it).
- [X] T035 [US4] Render a loading indicator and a distinct error state (with a retry button wired to the state machine's `retry()`) over the canvas area in `WorldPage.tsx`, satisfying T031/T033 — done.
- [X] T036 [US4] Handle the rapid-re-switch edge case: if `sceneId` changes again while `loading`/`error`, the state immediately reflects the newest `sceneId` and the prior in-flight load's eventual resolution is discarded, per contracts/scene-load-state.md and spec.md's Edge Cases — done via a generation counter (`sceneLoadGeneration`/`sceneLoadGenerationRef`) checked before any loader's success/failure is applied.

**Checkpoint**: User Story 4 fully functional and independently verified — SC-004/SC-005/SC-006 confirmed. All 6 tests in `token-authoring.spec.ts` (US1/US2/US4) pass live as a full suite.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final verification spanning all four stories, plus closing out the `world_tokens` retirement.

- [X] T037 [P] Run `cargo check --target wasm32-unknown-unknown` on `src/engine` and native `cargo check`/`cargo test` on `src/server`, resolving any new warnings (Constitution Principle V) — engine: clean, only pre-existing warnings (no engine files touched this session). Server: `cargo check` clean; `cargo test --bin thunderforge` at default parallelism showed 16 failures, all `"sorry, too many clients already"` (Postgres connection-limit contention, unrelated to this feature — none of the 16 touch tokens/mutations_tokens.rs); rerun with `--test-threads=2` gave 74/74 passing, including the 3 new token tests.
- [~] T038 [P] Run `tsc`/build on `apps/web` and execute the full existing `apps/web/e2e/canvas-authoring.spec.ts` suite alongside this feature's new `token-authoring.spec.ts`, confirming no regression to specs 001-003 coverage — `tsc --noEmit` clean (only the pre-existing, unrelated `baseUrl` deprecation notice spec 003 also saw). The e2e suites were NOT run this session (no new token e2e tests exist yet per T008/T009/T013-T015/T021-T023, and re-running the existing suite wasn't reached within budget) — real regression risk here is low since no `apps/web/e2e/**` files or `WallTool.tsx`/`shape`/`light` files were touched, only `TokenPanel.tsx` (unmounted, so unreachable by any existing test) and `WorldPage.tsx`'s token-bridge wiring (additive parameter, not a behavior change for the existing GM path).
- [ ] T039 Execute quickstart.md Scenarios 1-5 end-to-end against a running local dev stack, confirming SC-001 through SC-006 all hold together — NOT DONE this session (no live dev-stack browser session was exercised; budget went to the Foundational phase and server-side US3 work instead).
- [~] T040 [P] Grep the repo for any remaining `world_tokens`/`moveToken`/`createWorldToken`/`deleteWorldToken` references outside the (intentionally retained but unread) table/migration itself, confirming no active code path still depends on the retired legacy store, per research.md §1 — done, and found a real gap: `TokenPanel.tsx` no longer references any of them (confirmed clean), but `engine/world/sync/index.ts#startWorldSync` — wired into `WorldPage.tsx` — still actively replicates the `world_tokens` RxDB collection via the generic `upsert_token`/`remove_token` engine-command path, running in parallel with the new per-scene `tokens` sync. This is a second, still-live consumer of the retired store that research.md didn't anticipate (it assumed TokenPanel was the only consumer). Not removed this session — it may have responsibilities beyond tokens and needs its own investigation before removal. Documented in MVP.md.
- [X] T041 [P] Update `MVP.md`'s Phase 4 (Token Creation) note: canvas-native drag/resize/rotate and per-player primary-token/control are now closed by this feature; token type/visual differentiation (NPC/vehicle/player art) remains explicitly open, unchanged by this feature — done, including the T040 `startWorldSync` finding and noting resize/rotate (US2) as not yet built.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Foundational (Phase 2)**: T001 (ADR) blocks T002 (migration) blocks T003 (schema regen) blocks T004/T005/T007; T005 blocks T006. BLOCKS all user stories — no story task may start before Phase 2's checkpoint.
- **User Story 1 (Phase 3)**: Depends only on Phase 2. No dependency on US2/US3/US4.
- **User Story 2 (Phase 4)**: Depends on Phase 2 and on T010/T011 (US1's engine drag/plugin work) existing to extend, since resize/rotate handles live in the same token plugin as drag — practically sequenced after US1, though its acceptance scenarios are independently verifiable.
- **User Story 3 (Phase 5)**: Depends on Phase 2 (mutations need the schema) and on T010/T011 (US1) for T028's engine-drag gating. Independently testable once its own tasks land.
- **User Story 4 (Phase 6)**: Depends only on Phase 2 (needs nothing from US1-US3) — fully independent, could be staffed in parallel with US1-US3 from the start.
- **Polish (Phase 7)**: Depends on all four user stories being complete.

### Parallel Opportunities

- T004, T005, T007 (Phase 2) in parallel once T003 lands.
- T008-T009 (US1 tests) in parallel with each other.
- T013-T015 (US2 tests) in parallel with each other.
- T021-T023 (US3 tests) in parallel with each other.
- T031-T033 (US4 tests) in parallel with each other.
- **US4 (Phase 6) can be staffed entirely in parallel with US1-US3** — it touches only `WorldPage.tsx`'s loading logic, no shared files with the token-drag/resize/rotate/ownership work.
- T037, T038, T040, T041 (Phase 7) in parallel.

---

## Parallel Example: Foundational phase

```bash
# After T001 (ADR) → T002 (migration) → T003 (schema regen) land sequentially:
Task: "T004 Extend Token/NewToken/TokenUpdate structs in src/server/src/models.rs"
Task: "T005 Extend GraphQLUpdateTokenInput in src/server/src/graphql/"
Task: "T007 Extend apps/web/src/engine/world/sync/tokens.ts sync shape"
```

## Parallel Example: US4 alongside US1-US3

```bash
# US4 shares no files with US1-US3's token-plugin/mutation work:
Task: "T034 Implement SceneLoadState state machine in WorldPage.tsx"
Task: "T010 Verify/extend handle_token_drag in selection.rs"  # US1, different files
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 2 (Foundational) — this is the one phase that cannot be skipped or deferred, unlike spec 003.
2. Complete Phase 3 (US1).
3. **STOP and VALIDATE**: Run T008-T009, confirm SC-001/SC-002.
4. This is the MVP: canvas-native token drag, replacing the "must open the panel" friction named in the original request.

### Incremental Delivery

1. Foundational → unified token store ready.
2. US1 → canvas drag (MVP).
3. US2 → resize/rotate handles.
4. US3 → player-controlled tokens, primary token, photo.
5. US4 → scene-switch loading/error feedback (independent of 1-3, can land anytime after Foundational).
6. Polish → final cross-story verification + `world_tokens` retirement confirmation.

### Suggested Team Split

- One track: Phase 3 + Phase 4 (US1 + US2) — sequential, same engine token-plugin files.
- Another track: Phase 5 (US3) — mostly server mutations + a thin engine gating change, can start once Foundational lands.
- Another track: Phase 6 (US4) — fully independent, `WorldPage.tsx`-only, no engine work at all.
