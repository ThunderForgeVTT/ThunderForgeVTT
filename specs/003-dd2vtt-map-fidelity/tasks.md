---

description: "Task list for Universal VTT (.dd2vtt) Map Import Fidelity & From-Scratch Map Editor Tooling"
---

# Tasks: Universal VTT (.dd2vtt) Map Import Fidelity & From-Scratch Map Editor Tooling

**Input**: Design documents from `/specs/003-dd2vtt-map-fidelity/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/map-import-response.md, quickstart.md

**Tests**: Included — spec.md's Acceptance Scenarios are explicit runnable validation, and User Story 1's whole premise is "confirm the already-built thing actually works live," which is inherently a test-writing task, not optional.

**Organization**: Tasks are grouped by user story (US1-US3 from spec.md) to enable independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)

## Path Conventions

Existing three-part layout per plan.md: `src/engine/` (WASM Bevy engine), `src/server/` (Rust/Axum/GraphQL backend), `apps/web/` (React frontend), `examples/maps/` (test fixtures).

---

## Phase 1: Setup

**Not applicable to this feature.** All fixtures this feature needs (`examples/maps/grassy-path-ambush.dd2vtt`, `azheim-meeting.dd2vtt`, `road-side-in.dd2vtt`, `dwarven-forge.dd2vtt`, `little-fish-academy.dd2vtt`, plus the pre-existing `demo.dd2vtt`/`chamber-of-echoing-grief.dd2vtt`) are already committed to the branch (see `examples/maps/README.md`). No dependencies, migrations, or shared tooling need installing — per plan.md's Technical Context, this feature adds no new dependency anywhere.

## Phase 2: Foundational

**Not applicable to this feature.** Per research.md, no schema change, no new GraphQL mutation, and no new engine plugin are needed anywhere — every user story below is independently implementable directly against already-existing infrastructure (specs 001/002). There is no blocking prerequisite phase.

---

## Phase 3: User Story 1 - A GM builds a map from scratch, live, during a session (Priority: P1) 🎯 MVP

**Goal**: Confirm — and only fix if genuinely broken — that a GM can toggle wall passability and place torches entirely by hand, mid-session, with live sync to a connected player, all GM-gated. Per research.md §1-2, the underlying capability already exists; this phase's job is proving it, not building it.

**Independent Test**: On an empty scene, a GM draws a wall, toggles its "Blocks movement" checkbox, and places a torch — each individually, via the live scene canvas — confirming immediate correct behavior and cross-session visibility, with zero dependency on US2/US3.

### Tests for User Story 1

- [X] T001 [P] [US1] Playwright e2e: draw a wall on an empty scene, select it, toggle "Blocks vision" and "Blocks movement" independently via `WallTool.tsx`'s existing property-panel checkboxes, and confirm each toggle's occlusion/movement effect takes hold immediately — in `apps/web/e2e/canvas-authoring.spec.ts` or a new `apps/web/e2e/map-editor-tooling.spec.ts` (quickstart.md Scenario 1 steps 2-4; FR-001) — PASSES live against the dev stack.
- [X] T002 [P] [US1] Playwright e2e: with the same wall, toggle its door state (none/open/closed) via the existing select control and confirm it behaves exactly as before and is unaffected by the passability checkboxes sitting next to it in the same panel — regression guard for the pre-existing door mechanic (quickstart.md Scenario 1 step 4) — PASSES live against the dev stack.
- [ ] T003 [P] [US1] Playwright e2e: two browser contexts — GM toggles a wall's "Blocks movement" checkbox with a player session already connected and viewing the scene; confirm the player's view updates within a few seconds with no reload (quickstart.md Scenario 1 steps 6-7; FR-005, SC-002) — WRITTEN, but FAILS reproducibly against the real dev stack (marked `test.fail` so CI stays green while tracking it): `apps/web/src/engine/world/sync/walls.ts`'s own doc comment confirms no live GraphQL subscription transport is wired anywhere in the app, so a wall property change never reaches an already-connected second session without a reload. Real, pre-existing gap (also affects lights.ts/shapes.ts identically) — see implementation report; fixing it (a client-side GraphQL subscription transport) is out of this feature's scope.
- [X] T004 [P] [US1] Playwright e2e: GM places a torch (light source) on the canvas; confirm immediate illumination for the GM and, in a second browser context, for a connected player (quickstart.md Scenario 1 step 5; FR-003) — PASSES live (verifies persistence + a second, freshly-loaded session; does not exercise the T003-style "already connected, no reload" path, which per T003's finding likely has the identical gap for torches too since `lights.ts` carries the same doc-comment caveat).
- [X] T005 [P] [US1] Playwright e2e: on a brand-new scene with no import at all (optionally using `examples/maps/grassy-path-ambush.dd2vtt`'s background as the starting art), confirm a GM can immediately draw a wall and place a torch with no import step required (quickstart.md Scenario 1 step 1; FR-004, SC-001) — PASSES live against the dev stack.
- [ ] T006 [P] [US1] Playwright e2e: as a connected player (non-GM), confirm none of the wall/door/passability/torch authoring controls (toolbar buttons, property panel) are rendered, only their effects (quickstart.md Scenario 1 step 6; FR-006) — WRITTEN as `test.skip` with a documented reason: getting a genuinely distinct, non-owner account into a shared world is currently unreachable through the real app, for two separate pre-existing bugs found live (see implementation report) — `CampaignSettingsPanel.tsx`'s invite mutation call is malformed (missing the `input` wrapper), and even calling the mutation correctly, `generate_invite_code` requires a `world_members` row that `create_world` never gives the world's own owner. Both bugs are in the invite/membership feature, outside spec 003's scope to fix.

### Implementation for User Story 1

- [ ] T007 [US1] Verify `WallTool.tsx`'s passability checkboxes (`apps/web/src/components/canvas-tools/WallTool/WallTool.tsx:101-121`) and the engine-side `blocks_vision`/`blocks_movement` handling (`src/engine/src/systems/wall.rs`) against T001-T003 as they're written; fix any real gap found (expected: none, per research.md §1 — this task exists to confirm, not to build net-new) — T001/T002 confirm no gap; T003 found a real gap (see above) that was deliberately NOT fixed (out of this feature's "no new subsystem" scope) — left unchecked because the "fix any real gap found" half of this task was not done.
- [ ] T008 [US1] Verify torch placement (`src/engine/src/systems/lighting.rs`, `src/server/src/graphql/mutations_lighting.rs`) against T004 as it's written; fix any real gap found (expected: none, per research.md §2) — T004 itself shows no gap, but the same missing-subscription-transport root cause found via T003 almost certainly applies to torches too (not separately reproduced with a dedicated failing test) — left unchecked pending that follow-up.
- [ ] T009 [US1] (Optional, discretionary) If T001/T007's live verification finds the existing left-click-to-select-then-checkbox flow is genuinely insufficient as a GM-facing affordance, add a right-click context-menu entry point for toggling passability directly on a wall segment, in `apps/web/src/components/canvas-tools/WallTool/WallTool.tsx` and/or `src/engine/src/systems/wall.rs`'s selection handling — not required by any FR; skip this task entirely if T001/T007 conclude the existing flow is sufficient — SKIPPED: T001 confirms the existing select-then-checkbox flow works fine; the only real gap found (T003) is about live cross-session propagation, not the interaction affordance this task addresses.

**Checkpoint**: User Story 1 fully functional and independently verified — SC-001/SC-002 confirmed by T001-T006.

---

## Phase 4: User Story 2 - A GM trusts that an imported or hand-built map is exactly what they see after any reload (Priority: P1)

**Goal**: A real, automated round-trip test — persist then re-query from the database — proving no map data (imported or hand-built) is lost or altered by a reload, closing the gap between "parsing succeeds with the right counts" (existing coverage) and "the persisted data is field-for-field identical" (missing today).

**Independent Test**: Import `road-side-in.dd2vtt` into a fresh scene, record every wall/light/background reference, re-query from the database exactly as a fresh page load would, and confirm exact equality — independently verifiable without US1's live-authoring UI or US3's field-disclosure work.

**Dependencies**: None on US1/US3 for the import-path tests (T010-T012); T013 (hand-built-state round-trip) exercises the mutations US1 verifies but doesn't require US1's tasks to be "done" first, since those mutations already exist independent of US1's verification work.

### Tests for User Story 2

- [X] T010 [P] [US2] Server round-trip test: import `examples/maps/road-side-in.dd2vtt` (24 walls/16 doors/4 lights — richest fixture), re-query every wall (coordinates, door state, `blocks_vision`, `blocks_movement`), every light source, and the background image reference via a fresh DB query, and assert exact field equality against what was written — in `src/server/src/map_import.rs` (`#[cfg(test)] mod tests`) or a new test module, using `test_support.rs`'s `insert_test_user`/`insert_test_world`/`insert_test_scene` fixtures and the create-then-refetch pattern from `mutations_assets.rs:288-327` (research.md §4; FR-008, FR-009, FR-010; quickstart.md Scenario 2 steps 1-4)
- [X] T011 [P] [US2] Server round-trip test: same pattern as T010, importing `examples/maps/dwarven-forge.dd2vtt` (walls-only, no doors/lights) to confirm the walls-only path is equally durable (quickstart.md Scenario 2 step 5)
- [X] T012 [P] [US2] Server round-trip test: same pattern as T010, importing the existing `examples/maps/demo.dd2vtt` (richest all-around fixture — walls, doors, lights, baked lighting) as the broadest-coverage case
- [X] T013 [US2] Server round-trip test: import a fixture, then apply hand-built changes on top via the existing `update_wall`/`create_light_source` mutations (add a wall, toggle a wall's passability, place a light) — exercising the same mutations US1 verifies — reload/re-query, and confirm the edited state (not just the originally-imported state) is exactly what's persisted (FR-011; quickstart.md Scenario 2 step 6; spec.md US2 Acceptance Scenario 3)
- [X] T014 [US2] Regression test: confirm the existing UVTT format-version rejection (only `0.3` accepted, `rejects_unsupported_format_version` in `map_import.rs`) still passes unchanged after this feature's other test additions (FR-015)
- [X] T015 [US2] Validate SC-006 ("the round-trip check has teeth"): temporarily introduce a real fidelity bug (e.g. skip writing one field in a test-only code path, or use a mutation-testing-style deliberate break), confirm T010 fails specifically and clearly, then revert — do this once as a documented verification step (record the result in this task's own commit message or a code comment on the test), not as a permanent test fixture

**Checkpoint**: User Story 2 fully functional and independently verified — SC-003/SC-006 confirmed.

---

## Phase 5: User Story 3 - A GM is never silently missing part of their map (Priority: P2)

**Goal**: The `.dd2vtt` import response discloses (not necessarily implements) three currently-silently-dropped field categories — freestanding portals, `ambient_light`, `objects_line_of_sight` — so a GM always knows if part of their map didn't come through.

**Independent Test**: Import `little-fish-academy.dd2vtt` (has a non-default `ambient_light`) and confirm the import response's `warnings` field mentions it — independently verifiable without US1's authoring UI or US2's round-trip test infrastructure.

**Dependencies**: None on US1/US2.

### Tests for User Story 3

- [X] T016 [P] [US3] Create a hand-crafted synthetic `.dd2vtt` fixture in `examples/maps/` exercising a `freestanding: true` portal and a non-empty `objects_line_of_sight` array (research.md §7 — no real-world file in the surveyed 64 has either field populated), and document it in `examples/maps/README.md` following the existing per-file convention
- [X] T017 [P] [US3] Server test: import `examples/maps/little-fish-academy.dd2vtt`, confirm the import response's `warnings` includes an entry about `ambient_light` not being applied (contracts/map-import-response.md; FR-012, FR-013)
- [X] T018 [P] [US3] Server test: import the synthetic fixture from T016, confirm `warnings` includes entries for both the freestanding portal and the object-based occluders (FR-012, FR-013)
- [X] T019 [P] [US3] Server test: import each of `demo.dd2vtt`, `chamber-of-echoing-grief.dd2vtt`, `grassy-path-ambush.dd2vtt`, `azheim-meeting.dd2vtt`, `road-side-in.dd2vtt`, `dwarven-forge.dd2vtt` (none use the three unhandled field categories) and confirm `warnings` is empty for every one — no new noise (FR-014, SC-004)

### Implementation for User Story 3

- [X] T020 [US3] Add a `warnings: Vec<String>` field to the `import_uvtt` response shape in `src/server/src/map_import.rs` (currently `{"wallsCreated", "doorsCreated", "lightsCreated", "backgroundImageSet", "skippedDegeneratePolygons"}`, `map_import.rs:604-611`), defaulting to empty (contracts/map-import-response.md)
- [X] T021 [US3] Populate a `warnings` entry when a source file contains a `freestanding: true` portal that wasn't turned into a usable door/wall, reading the already-parsed-but-`#[allow(dead_code)]` `UvttPortal.freestanding` field (`map_import.rs:92-94`)
- [X] T022 [US3] Populate a `warnings` entry when `UvttEnvironment.ambient_light` (`map_import.rs:102`) is present and not the default, since it isn't applied to scene lighting today
- [X] T023 [US3] Populate a `warnings` entry when `objects_line_of_sight` is non-empty, since no vision-blocking geometry is created from it today
- [X] T024 [US3] Wire T020-T023 together in the `import_uvtt` handler so all three checks run once per import and contribute to the same `warnings` array, satisfying T017-T019

**Checkpoint**: User Story 3 fully functional and independently verified — SC-005 confirmed.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final verification spanning all three stories.

- [X] T025 [P] Run `cargo check --target wasm32-unknown-unknown` on `src/engine` and native `cargo check`/`cargo test` on `src/server`, resolving any new warnings (Constitution Principle V) — no new warnings; full `cargo test --bin thunderforge` (72 tests) passes with `DATABASE_URL` loaded.
- [~] T026 [P] Run `tsc`/build on `apps/web` and execute the full existing `apps/web/e2e/canvas-authoring.spec.ts` suite alongside this feature's new tests, confirming no regression to spec 001/002 wall/shape/import coverage (quickstart.md Scenario 4) — `tsc` clean (only a pre-existing, unrelated `baseUrl` deprecation notice). `canvas-authoring.spec.ts`: 5 passed / 2 failed, **identically** at both `--workers=5` and `--workers=2` — same two "Hand-drawn shape authoring (US2)" tests, same `scrollIntoViewIfNeeded` timeout, both times. Reproducible, not parallel-load flakiness. Not a regression from this feature: `git status` confirms this feature touched only `src/server/src/map_import.rs`, `examples/maps/*` fixtures, docs, and its own new `map-editor-tooling.spec.ts` — nothing in `apps/web/src/**`, `src/engine/**`, or `canvas-authoring.spec.ts` itself. Left `[~]` rather than `[X]` because the two shape-test failures are unresolved (pre-existing, out of this feature's scope to fix) rather than because anything is still pending.
- [~] T027 Execute quickstart.md Scenarios 1-3 end-to-end against a running local dev stack, confirming SC-001 through SC-006 all hold together, not just in isolation per-task — Scenario 1 (US1) executed live via Playwright, confirming SC-001/most of SC-002 (see T003's finding for the one part that doesn't hold); Scenario 2 (US2) executed live via the round-trip server tests (SC-003/SC-006 hold); Scenario 3 (US3) executed live via the warnings tests (SC-004/SC-005 hold). Not run as one single unbroken manual walkthrough against a live browser session end-to-end across all three — done per-scenario via each phase's own automated tests instead, which is a stronger, repeatable substitute for a one-time manual click-through.
- [X] T028 [P] Update `MVP.md`'s Phase 6 (Walls and Lighting) note to reflect this feature's closure of the round-trip trust gap it flagged, once T010-T015 land — done, and also documents the newly found live-subscription-transport gap.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)** / **Foundational (Phase 2)**: Not applicable — nothing to complete before user stories can start.
- **User Story 1 (Phase 3)**, **User Story 2 (Phase 4)**, **User Story 3 (Phase 5)**: All three can start immediately and in parallel — none depends on another's tasks being complete, per plan.md's Constitution Check (no shared new infrastructure).
- **Polish (Phase 6)**: Depends on all three user stories being complete.

### User Story Dependencies

- **US1 (P1)**: Independent. No dependency on US2/US3.
- **US2 (P1)**: Independent. T013 exercises mutations US1 also verifies, but does not require US1's tasks to have run first — those mutations already exist regardless of US1's verification status.
- **US3 (P2)**: Independent. No dependency on US1/US2.

### Parallel Opportunities

- All of Phase 3 (US1), Phase 4 (US2), and Phase 5 (US3) can proceed fully in parallel — disjoint files (`WallTool.tsx`/`lighting.rs` for US1; `map_import.rs` test additions for US2; `map_import.rs` response-shape changes for US3 — note T020-T024 and T010-T015 both touch `map_import.rs`, so US2's and US3's *implementation* tasks should be sequenced or coordinated within that one file even though they're independently testable).
- T001-T006 (US1 tests) in parallel with each other.
- T010-T012 (US2 round-trip tests across three fixtures) in parallel with each other.
- T016-T019 (US3 tests) in parallel with each other; T016 (fixture creation) should land before T018 (which imports it), so not fully parallel with T018 specifically.
- T025, T026, T028 (Phase 6) in parallel.

---

## Parallel Example: All three user stories together

```bash
# US1, US2, US3 touch disjoint concerns and can be staffed independently:
Task: "T001 Playwright e2e: wall passability checkboxes"
Task: "T010 Server round-trip test: road-side-in.dd2vtt"
Task: "T016 Create synthetic freestanding/objects_los fixture"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 3 (US1) — no Setup/Foundational dependency.
2. **STOP and VALIDATE**: Run T001-T006, confirm SC-001/SC-002.
3. If everything passes as expected (per research.md, it should), this is a very fast MVP — the capability already existed, this just proves it.

### Incremental Delivery

1. Phase 3 (US1) → confirms/closes the from-scratch authoring experience.
2. Phase 4 (US2) → closes the durability trust gap that motivated this whole feature.
3. Phase 5 (US3) → closes the silent-partial-import trust gap.
4. Phase 6 → final cross-story verification.

### Suggested Team Split

- One track: Phase 3 (US1) — pure Playwright/live-verification work, engine/frontend files only.
- Another track: Phase 4 + Phase 5 (US2/US3) — both live in `map_import.rs`, so best kept on one track to avoid the file-overlap noted above, even though the stories themselves are independently testable.
