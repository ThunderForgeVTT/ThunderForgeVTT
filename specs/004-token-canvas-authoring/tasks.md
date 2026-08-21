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

- [ ] T001 Author ADR documenting the `world_tokens` → `tokens` backing-store unification decision (docs/adrs/`<date>`-0XX-unify_token_backing_store.md), referencing and partially superseding ADR-033 (`docs/adrs/20260505-033-token_data_model_and_ownership.md`) per research.md §1 and plan.md's Constitution Check — this is a hard blocker for T002, not optional paperwork
- [ ] T002 Create Diesel migration adding `owner_user_id UUID NULL REFERENCES users(id)`, `is_primary BOOLEAN NOT NULL DEFAULT false`, `photo_url TEXT NULL`, `health INTEGER NULL`, `max_health INTEGER NULL`, and partial unique index `tokens_one_primary_per_owner_per_scene ON tokens (scene_id, owner_user_id) WHERE is_primary` to the `tokens` table, in `src/server/migrations/<timestamp>_add_ownership_and_photo_to_tokens/{up,down}.sql` per data-model.md (depends on T001)
- [ ] T003 Run the migration locally and regenerate `src/server/src/schema.rs`'s `tokens` table definition to include the 5 new columns (depends on T002)
- [ ] T004 [P] Extend `Token`/`NewToken`/`TokenUpdate` structs in `src/server/src/models.rs` with the 5 new fields (depends on T003)
- [ ] T005 [P] Extend `GraphQLUpdateTokenInput` in `src/server/src/graphql/` (input_types/mutations_tokens.rs) with `ownerUserId`, `isPrimary`, `photoUrl`, `health`, `maxHealth`, and wire them into the existing `update_token` mutation's `AsChangeset` update, per contracts/token-mutations.md (depends on T003; the "replace prior primary in the same transaction when setting `isPrimary: true`" rule from contracts/token-mutations.md belongs here)
- [ ] T006 Rewire `apps/web/src/components/TokenPanel.tsx` off the legacy `world_tokens` RxDB collection onto the `tokens` collection/mutations (`create_token`/`update_token`/`delete_token`), preserving its health-bar and bulk create/delete UI but dropping `moveToken`/`createWorldToken`/`deleteWorldToken` calls entirely, per research.md §1/§6 (depends on T005) — this is foundational because FR-005 (panel/canvas consistency) is required by every user story below
- [ ] T007 [P] Extend `apps/web/src/engine/world/sync/tokens.ts`'s sync shape with the 5 new fields so the engine and RxDB agree on the full `tokens` row shape (depends on T003)

**Checkpoint**: Foundation ready — `tokens` is the single source of truth for position, size, rotation, ownership, primary designation, photo, and health; `TokenPanel.tsx` and the canvas engine now read/write the same rows. User story implementation can now begin.

---

## Phase 3: User Story 1 - GM drags a token directly on the canvas (Priority: P1) 🎯 MVP

**Goal**: A GM can reposition an existing token by clicking and dragging it directly on the canvas, with the move persisting and syncing live to connected players, with no conflict against `TokenPanel`'s displayed state.

**Independent Test**: On a scene with an existing token, a GM drags it directly on the canvas (no panel opened) and confirms the new position persists after reload and appears to a connected player within a few seconds.

### Tests for User Story 1

- [ ] T008 [P] [US1] Playwright e2e: GM drags a token on the canvas, position updates in real time, persists after reload, and is visible to a second connected player within a few seconds — new `apps/web/e2e/token-authoring.spec.ts` (FR-001–FR-004; SC-001, SC-002; quickstart.md Scenario 1)
- [ ] T009 [P] [US1] Playwright e2e: after a canvas drag, opening `TokenPanel` for the same token shows the matching new position — no conflicting state between the two paths (FR-005; quickstart.md Scenario 1 step 6)

### Implementation for User Story 1

- [ ] T010 [US1] Verify/extend `src/engine/src/systems/selection.rs`'s existing `handle_token_drag` (lines ~48-120) against T008 as it's written — real-time drag visuals + persist-on-drop via the GM `update_token` mutation path; fix any real gap found
- [ ] T011 [US1] Grow `src/engine/src/plugins/token.rs` from its current 19-line placeholder into a real `TokenPlugin` chaining the (relocated, if needed) drag-input system and a visual-sync system, per Constitution Principle II and research.md §5
- [ ] T012 [US1] Confirm the engine's post-drop `update_token` call round-trips through `apps/web/src/engine/world/sync/tokens.ts` back into `TokenPanel.tsx`'s displayed state (satisfies T009)

**Checkpoint**: User Story 1 fully functional and independently verified — SC-001/SC-002 confirmed by T008-T009.

---

## Phase 4: User Story 2 - Resize and rotate a token's footprint via canvas handles (Priority: P2)

**Goal**: A GM can resize a selected token in whole grid-cell increments and rotate its facing independently, via canvas-rendered handles mirroring the existing wall/shape handle pattern, GM-only.

**Independent Test**: On a scene with an existing token selected, a GM drags a resize handle (confirming grid-cell-increment snapping) and a rotate handle independently, and confirms both persist after reload and sync to a connected player.

### Tests for User Story 2

- [ ] T013 [P] [US2] Playwright e2e: GM drags a token's resize handle; footprint changes only in whole grid-cell increments (1×1, 2×2, 3×3...), never a fractional cell — `apps/web/e2e/token-authoring.spec.ts` (FR-006; quickstart.md Scenario 2 step 1)
- [ ] T014 [P] [US2] Playwright e2e: GM drags a token's rotate handle independently of size; both resize and rotate persist after reload and sync to a connected player within a few seconds (FR-007, FR-008; quickstart.md Scenario 2 steps 2-3)
- [ ] T015 [P] [US2] Playwright e2e: as a connected player (non-GM), confirm no resize/rotate handles render on any token, including their own (FR-010; quickstart.md Scenario 2 step 4)

### Implementation for User Story 2

- [ ] T016 [US2] Add a resize-handle marker component + spawn/drag system to the token plugin (`src/engine/src/plugins/token.rs` + a new `src/engine/src/systems/token.rs`), mirroring `shape.rs`'s corner-resize pattern (`handle_shape_input` ~line 261, `sync_shape_visuals` ~line 617)
- [ ] T017 [US2] Add a rotate-handle marker component + drag system, independent of the resize handle, mirroring `wall.rs`'s `WallHandle`/`WallDragMode::MovingEndpoint` pattern (~lines 47, 63)
- [ ] T018 [US2] Enforce whole-grid-cell-multiple snapping in the resize-handle drag math (engine-side) before the resulting `scale` value is sent via `update_token`, per research.md §2 and the resize clarification in spec.md
- [ ] T019 [US2] Gate resize/rotate handle rendering behind the existing `IsGameMaster` resource, mirroring `wall.rs`'s GM-only endpoint-handle spawn (~lines 627-657) — satisfies T015
- [ ] T020 [US2] Add resize/rotate controls to a new `apps/web/src/components/canvas-tools/TokenTool/TokenTool.tsx`, mirroring `WallTool.tsx`'s `worldStore.dispatch`/`Panel` conventions, mounted GM-only via the existing `isSceneOwner && sceneId` guard in `apps/web/src/pages/world/WorldPage.tsx` (mirroring line ~506's `WallTool` mount)

**Checkpoint**: User Story 2 fully functional and independently verified, alongside User Story 1.

---

## Phase 5: User Story 3 - A player repositions their own token (Priority: P2)

**Goal**: Each player has exactly one primary token (editable photo) plus any additional tokens the GM grants them control of; players can drag only tokens they control, never create tokens themselves.

**Independent Test**: As a player, drag their primary token (moves, persists, syncs) and confirm a token not assigned to them cannot be dragged; confirm they can edit their primary token's photo but cannot create a new token.

### Tests for User Story 3

- [ ] T021 [P] [US3] Playwright e2e: a player drags their primary token — it moves, persists, and syncs to the GM/other players; the same player attempts to drag a token not assigned to them — no effect (FR-009; SC-003; quickstart.md Scenario 3 steps 1-3)
- [ ] T022 [P] [US3] Playwright e2e: GM grants a player control of an additional token (e.g. a summoned creature); confirm the player can now drag that token too, identically to their primary (quickstart.md Scenario 3 step 4)
- [ ] T023 [P] [US3] Playwright e2e: a player changes their primary token's photo — visible to the GM and other players; the same player has no "create token" control anywhere in the UI (FR-009a, FR-009b; quickstart.md Scenario 3 step 5)

### Implementation for User Story 3

- [ ] T024 [US3] Add `move_own_token(tokenId, x, y)` mutation to `src/server/src/graphql/mutations_tokens.rs`, filtered by `tokens.owner_user_id = <requesting user>` at the Diesel query level, touching only `x`/`y`, per contracts/token-mutations.md
- [ ] T025 [US3] Add `set_own_primary_token_photo(tokenId, photoUrl)` mutation to `src/server/src/graphql/mutations_tokens.rs`, filtered by `tokens.owner_user_id = <requesting user> AND tokens.is_primary = true`, touching only `photo_url`, per contracts/token-mutations.md
- [ ] T026 [US3] Server test: a non-owning player calling `move_own_token` on a token they don't control receives an authorization error and the token's position is unchanged on re-query (SC-003) — in `src/server/src/graphql/mutations_tokens.rs`'s test module, following `test_support.rs`'s fixture convention
- [ ] T027 [US3] Server test: setting `isPrimary: true` for a second token under the same `(scene_id, owner_user_id)` correctly replaces the prior primary (partial unique index respected, exactly one primary remains) — verified via fresh re-query, per contracts/token-mutations.md's Verification section
- [ ] T028 [US3] Gate the engine's token-drag input (from T010/T011) so a non-GM player's drag only succeeds when the local user is the token's `owner_user_id`, routing through `move_own_token` instead of `update_token` for that path (depends on T010, T024)
- [ ] T029 [US3] Add primary-token photo-edit control to `TokenPanel.tsx`, calling `set_own_primary_token_photo`; confirm token creation remains GM-only (gate already exists or add one) — satisfies T023
- [ ] T030 [US3] Add GM-only UI (in `TokenPanel.tsx` or `TokenTool.tsx`) to grant/revoke a player's control of an additional token and to (re)designate a player's primary token, via the extended `update_token` mutation from T005 — satisfies T022

**Checkpoint**: User Story 3 fully functional and independently verified — SC-003 confirmed.

---

## Phase 6: User Story 4 - Clear loading and error feedback when switching scenes (Priority: P2)

**Goal**: All connected clients see a loading indicator while a newly-selected scene's data loads, and a clear, retry-able error state if loading fails — replacing today's silent `console.error`-only handling.

**Independent Test**: Trigger a scene switch and confirm a loading indicator appears and clears on success; simulate a failed background-asset load and confirm a visible error state with a working retry action.

### Tests for User Story 4

- [ ] T031 [P] [US4] Playwright e2e: switching scenes via `SceneSwitcher` shows a loading indicator immediately, which clears once the scene is fully rendered — new spec (or added to `token-authoring.spec.ts`) (FR-011, FR-012; quickstart.md Scenario 4 step 1)
- [ ] T032 [P] [US4] Playwright e2e: a connected player's view shows the same loading → ready sequence as the GM's, without a manual reload (quickstart.md Scenario 4 step 2)
- [ ] T033 [P] [US4] Playwright e2e: simulating a background-asset load failure produces a visible, distinct error state with a retry action; fixing the underlying issue and clicking retry successfully loads the scene without switching away and back (FR-013, FR-013a; SC-005, SC-006; quickstart.md Scenario 4 steps 3-4)

### Implementation for User Story 4

- [ ] T034 [US4] Implement the `SceneLoadState` state machine (`loading`/`ready`/`error`/`retry`) per contracts/scene-load-state.md, wrapping the four existing per-scene loader calls (`loadWallsIntoStore` ~line 282, `loadTokensIntoStore` ~line 298, `loadLightsIntoStore` ~line 314, `loadShapesIntoStore` ~line 340) plus background-image loading, in `apps/web/src/pages/world/WorldPage.tsx` (or an extracted `apps/web/src/hooks/useSceneLoadState.ts`), replacing the current `.catch((error) => console.error(...))`-only handling
- [ ] T035 [US4] Render a loading indicator and a distinct error state (with a retry button wired to the state machine's `retry()`) over the canvas area in `WorldPage.tsx`, satisfying T031/T033
- [ ] T036 [US4] Handle the rapid-re-switch edge case: if `sceneId` changes again while `loading`/`error`, the state immediately reflects the newest `sceneId` and the prior in-flight load's eventual resolution is discarded, per contracts/scene-load-state.md and spec.md's Edge Cases

**Checkpoint**: User Story 4 fully functional and independently verified — SC-004/SC-005/SC-006 confirmed.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final verification spanning all four stories, plus closing out the `world_tokens` retirement.

- [ ] T037 [P] Run `cargo check --target wasm32-unknown-unknown` on `src/engine` and native `cargo check`/`cargo test` on `src/server`, resolving any new warnings (Constitution Principle V)
- [ ] T038 [P] Run `tsc`/build on `apps/web` and execute the full existing `apps/web/e2e/canvas-authoring.spec.ts` suite alongside this feature's new `token-authoring.spec.ts`, confirming no regression to specs 001-003 coverage
- [ ] T039 Execute quickstart.md Scenarios 1-5 end-to-end against a running local dev stack, confirming SC-001 through SC-006 all hold together
- [ ] T040 [P] Grep the repo for any remaining `world_tokens`/`moveToken`/`createWorldToken`/`deleteWorldToken` references outside the (intentionally retained but unread) table/migration itself, confirming no active code path still depends on the retired legacy store, per research.md §1
- [ ] T041 [P] Update `MVP.md`'s Phase 4 (Token Creation) note: canvas-native drag/resize/rotate and per-player primary-token/control are now closed by this feature; token type/visual differentiation (NPC/vehicle/player art) remains explicitly open, unchanged by this feature

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
