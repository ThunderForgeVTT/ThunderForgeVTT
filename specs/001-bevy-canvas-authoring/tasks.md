---
description: "Task list for Native Canvas Authoring (Walls, Lighting, Annotations)"
---

# Tasks: Native Canvas Authoring (Walls, Lighting, Annotations)

**Input**: Design documents from `specs/001-bevy-canvas-authoring/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/graphql.md, quickstart.md

**Tests**: Included — the constitution's Verify Before Claiming Done
principle and quickstart.md's "Automated coverage" section both call for
them; kept scoped to ownership/occlusion/plugin-independence, not full TDD.

**Organization**: Tasks are grouped by user story (US1 = walls, US2 =
lighting, US3 = annotations + tldraw removal) so each ships and is
independently verifiable per quickstart.md's scenarios.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an
  incomplete task)
- Paths are relative to the repository root

---

## Phase 1: Setup

- [ ] T001 Add `light_sources` migration in `src/server/migrations/<timestamp>_create_light_sources_table/{up,down}.sql` per data-model.md (columns, CHECK constraints, FK to `scenes`/`tokens`, `idx_light_sources_scene_id` index)
- [ ] T002 Add `annotations` migration in `src/server/migrations/<timestamp>_create_annotations_table/{up,down}.sql` per data-model.md (columns, `kind` CHECK constraint, FK to `scenes`, `idx_annotations_scene_id` index)
- [ ] T003 Run `diesel migration run` against the dev database and confirm both `down.sql` scripts cleanly reverse via `diesel migration redo` for each

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: Close the real-time propagation gap identified in
research.md §Real-time propagation / contracts/graphql.md, which all
three user stories depend on for FR-003/SC-001.

- [ ] T004 Add NOTIFY emission to `create_wall`/`update_wall`/`delete_wall` in `src/server/src/graphql/mutations_walls.rs`, following the existing LISTEN/NOTIFY channel/payload convention used elsewhere in `src/server/src/graphql.rs`
- [ ] T005 Extend `apps/web/src/engine/world/sync` to subscribe to and dispatch wall change events into the world store, mirroring the existing token event dispatch path
- [ ] T006 Add `Wall` variant handling to the engine-side world-command dispatch in `src/engine/src/lib.rs` (`ExternalCommand`/`apply_external_commands`) so wall events reach the engine, following the existing `UpsertToken`/`RemoveToken` pattern

**Checkpoint**: Wall create/move/delete now round-trips server → all
connected clients within a few seconds, with no UI yet. This unblocks US1.

---

## Phase 3: User Story 1 — GM authors walls that block vision and movement (Priority: P1) 🎯 MVP

**Goal**: A GM can draw, move, reconfigure, and delete wall segments on
the canvas; vision/movement respects them; players never see the tool.

**Independent Test**: quickstart.md Scenario 1.

### Engine (Rust/Bevy, wasm32 target)

- [ ] T007 [P] [US1] Create `WallSet` resource (segment list + simple spatial index) in `src/engine/src/resources/wall.rs`, re-exported via `src/engine/src/resources/mod.rs`
- [ ] T008 [P] [US1] Create wall input systems (click-drag create, endpoint drag to move, select+delete, blocks_vision/blocks_movement toggle) in `src/engine/src/systems/wall.rs`, re-exported via `src/engine/src/systems/mod.rs`
- [ ] T009 [US1] Implement 2D shadow-casting vision occlusion against `WallSet` (research.md §3) as a system in `src/engine/src/systems/wall.rs`, triggered on `WallSet` change and on token movement, feeding the existing fog rendering slot reserved by ADR-032
- [ ] T010 [US1] Create `WallPlugin` in `src/engine/src/plugins/wall.rs` wiring T007-T009's systems/resources; register it in `src/engine/src/lib.rs` alongside the existing `SelectionPlugin`/`TokenPlugin` registrations, independently addable per Constitution Principle II
- [ ] T011 [US1] Reject/ignore zero-length wall creation (edge case from spec.md) in the create system from T008

### Backend (Rust/GraphQL)

- [ ] T012 [US1] GM-only visibility check for the wall authoring tool: confirm `queries/scene.rs::walls` already returns data safely to non-owners (read-only) and add a scene-ownership assertion test for `create_wall`/`update_wall`/`delete_wall` in `src/server/src/graphql/mutations_walls.rs` if not already covered

### Frontend (React/RxDB)

- [ ] T013 [P] [US1] Create `apps/web/src/db/collections/worldWallsCollection.ts` mirroring `worldTokensCollection.ts` (schema, replication pull/push)
- [ ] T014 [US1] Wire wall create/update/delete GraphQL calls into `apps/web/src/engine/world/sync`, dispatching into the RxDB collection from T013
- [ ] T015 [US1] Create `apps/web/src/components/canvas-tools/WallTool/` (toolbar button + blocks_vision/blocks_movement property panel), GM-only rendered (hidden entirely for non-owners per FR-009)
- [ ] T016 [US1] Mount `WallTool` from `apps/web/src/pages/world/WorldPage.tsx`, gated on the existing GM/scene-owner check already used elsewhere on that page

**Checkpoint**: quickstart.md Scenario 1 and Scenario 4 (wall portion)
pass. Ship-able MVP increment.

---

## Phase 4: User Story 2 — GM places and manages lighting sources (Priority: P2)

**Goal**: A GM can place, move, resize, and delete light sources; light is
occluded by vision-blocking walls; lights can attach to a token.

**Independent Test**: quickstart.md Scenario 2 (requires US1's walls for
the occlusion check).

### Backend (Rust/GraphQL)

- [ ] T017 [P] [US2] Add `LightSource`/`NewLightSource`/`LightSourceUpdate` structs to `src/server/src/models.rs` per data-model.md
- [ ] T018 [P] [US2] Add `light_sources` table to `src/server/src/schema.rs`
- [ ] T019 [US2] Add `GraphQLLightSource` type + `CreateLightSourceInput`/`UpdateLightSourceInput` to `src/server/src/graphql/input_types.rs` and `src/server/src/graphql.rs` per contracts/graphql.md
- [ ] T020 [US2] Create `LightSourceMutation` (create/update/delete, scene-ownership enforced, CHECK-constraint errors surfaced) in `src/server/src/graphql/mutations_lighting.rs`, using `mutations_walls.rs` as the direct template
- [ ] T021 [US2] Add `lightSources(sceneId: ID!)` query to `src/server/src/graphql/queries/scene.rs`, register `LightSourceMutation` in the root schema in `src/server/src/graphql.rs`/`src/server/src/schema.rs`
- [ ] T022 [US2] Add NOTIFY emission to the light mutations from T020, same pattern as T004

### Engine (Rust/Bevy, wasm32 target)

- [ ] T023 [P] [US2] Create `LightSet` resource in `src/engine/src/resources/lighting.rs`, re-exported via `resources/mod.rs`
- [ ] T024 [P] [US2] Create light input systems (place, drag-reposition, resize radius/intensity, select+delete, attach-to-token) in `src/engine/src/systems/lighting.rs`, re-exported via `systems/mod.rs`
- [ ] T025 [US2] Implement occlusion-aware illumination sampling: for each `LightSet` entry, apply the shadow-casting result from T009 against `WallSet` to determine the lit region; resolve token-attached light position from the attached entity's live `Transform` each frame
- [ ] T026 [US2] Create `LightingPlugin` in `src/engine/src/plugins/lighting.rs` wiring T023-T025; register independently in `src/engine/src/lib.rs` per Constitution Principle II
- [ ] T027 [US2] Reject/ignore zero-radius light creation (edge case from spec.md) in the create system from T024

### Frontend (React/RxDB)

- [ ] T028 [P] [US2] Create `apps/web/src/db/collections/worldLightsCollection.ts` mirroring `worldTokensCollection.ts`
- [ ] T029 [US2] Wire light create/update/delete GraphQL calls into `apps/web/src/engine/world/sync`
- [ ] T030 [US2] Create `apps/web/src/components/canvas-tools/LightingTool/` (toolbar button + radius/intensity/color/attach-to-token property panel), GM-only rendered
- [ ] T031 [US2] Mount `LightingTool` from `apps/web/src/pages/world/WorldPage.tsx` alongside `WallTool`

**Checkpoint**: quickstart.md Scenario 2 passes on top of Scenario 1.

---

## Phase 5: User Story 3 — GM draws freeform annotations, tldraw removed (Priority: P3)

**Goal**: A GM can draw/erase freeform strokes, shapes, and text labels
natively on the canvas with a GM-only/player-visible flag; tldraw and
`WorldWhiteboard.tsx` are removed once this reaches parity.

**Independent Test**: quickstart.md Scenario 3.

### Backend (Rust/GraphQL)

- [ ] T032 [P] [US3] Add `Annotation`/`NewAnnotation`/`AnnotationUpdate` structs to `src/server/src/models.rs` per data-model.md
- [ ] T033 [P] [US3] Add `annotations` table to `src/server/src/schema.rs`
- [ ] T034 [US3] Add `GraphQLAnnotation`/`AnnotationKind` type + `CreateAnnotationInput`/`UpdateAnnotationInput` to `src/server/src/graphql/input_types.rs` and `src/server/src/graphql.rs` per contracts/graphql.md
- [ ] T035 [US3] Create `AnnotationMutation` (create/update/delete, scene-ownership enforced) in `src/server/src/graphql/mutations_annotations.rs`
- [ ] T036 [US3] Add `annotations(sceneId: ID!)` query to `src/server/src/graphql/queries/scene.rs`, filtering to `visible_to_players = true` for non-owner callers (FR-009); register `AnnotationMutation` in the root schema
- [ ] T037 [US3] Add NOTIFY emission to the annotation mutations from T035, same pattern as T004

### Engine (Rust/Bevy, wasm32 target)

- [ ] T038 [P] [US3] Create `AnnotationSet` resource + bounded per-session undo stack (research.md §4) in `src/engine/src/resources/annotation.rs`, re-exported via `resources/mod.rs`
- [ ] T039 [P] [US3] Create stroke-capture and shape/text-placement input systems in `src/engine/src/systems/annotation.rs`, re-exported via `systems/mod.rs`
- [ ] T040 [US3] Implement undo: re-issue the inverse mutation through the same GraphQL path as a normal edit (research.md §4), wired to the undo stack from T038
- [ ] T041 [US3] Implement visibility filtering so GM-only annotations never render for non-GM sessions (defense in depth on top of T036's server-side filter)
- [ ] T042 [US3] Create `AnnotationPlugin` in `src/engine/src/plugins/annotation.rs` wiring T038-T041; register independently in `src/engine/src/lib.rs` per Constitution Principle II

### Frontend (React/RxDB)

- [ ] T043 [P] [US3] Create `apps/web/src/db/collections/worldAnnotationsCollection.ts` mirroring `worldTokensCollection.ts`
- [ ] T044 [US3] Wire annotation create/update/delete GraphQL calls into `apps/web/src/engine/world/sync`
- [ ] T045 [US3] Create `apps/web/src/components/canvas-tools/AnnotationTool/` (draw/erase/text tool + GM-only/player-visible toggle), GM-only rendered
- [ ] T046 [US3] Mount `AnnotationTool` from `apps/web/src/pages/world/WorldPage.tsx` alongside `WallTool`/`LightingTool`

### tldraw removal (only after T032-T046 pass quickstart Scenario 3)

- [ ] T047 [US3] Remove `<WorldWhiteboard />` usage and the component itself: delete `apps/web/src/engine/tldraw/WorldWhiteboard.tsx` and its usages in `apps/web/src/pages/world/WorldPage.tsx`
- [ ] T048 [US3] Remove the `tldraw` package dependency from `apps/web/package.json` and run the package manager's lockfile update
- [ ] T049 [US3] Update ADR-004's status note and this feature's ADR-037 consequences section to record removal completion date

**Checkpoint**: quickstart.md Scenario 3 passes; zero remaining tldraw
references in `apps/web` (SC-006).

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T050 [P] Add plugin-independence integration tests to `src/engine/src/integration_tests.rs`: each of `WallPlugin`/`LightingPlugin`/`AnnotationPlugin` must compile/run correctly with the other two absent from the `App` builder (Constitution Principle II)
- [ ] T051 [P] Add an occlusion unit test (light + wall geometry → expected lit/unlit regions) near the systems added in T009/T025
- [ ] T052 [P] Add scene-ownership rejection tests for all six new mutations (T020, T035) mirroring existing `mutations_walls.rs` test coverage expectations
- [ ] T053 [P] Add Playwright e2e coverage for quickstart.md Scenario 1 and Scenario 3 in the existing e2e harness
- [ ] T054 Run `cargo check -p thunderforge_engine --target wasm32-unknown-unknown` and `cargo check -p dnd5e-server` and resolve any new warnings introduced by this feature (Constitution Principle V)
- [ ] T055 Execute quickstart.md Scenario 4 (authorization boundary) and Scenario 5 (empty-scene regression) manually before marking the feature done

---

## Dependencies & Execution Order

- **Setup (Phase 1)** blocks **Foundational (Phase 2)**.
- **Foundational (Phase 2)** blocks all user stories (T004-T006 are the
  NOTIFY/dispatch plumbing every story's real-time propagation relies on).
- **User Story 1 (Phase 3)** has no dependency on US2/US3 and is the MVP
  (walls already have a backend; this only adds authoring UI + occlusion).
- **User Story 2 (Phase 4)** depends on US1's `WallSet` and shadow-casting
  system (T009) for occlusion (T025); cannot be tested independently of
  US1 per quickstart.md Scenario 2, but its backend (T017-T022) can be
  built in parallel with US1's frontend work.
- **User Story 3 (Phase 5)** is independent of US1/US2 at the data/engine
  level (annotations don't interact with walls or lights) but its final
  sub-phase (T047-T049, tldraw removal) MUST wait until T032-T046 pass
  quickstart Scenario 3, per research.md §6.
- **Polish (Phase 6)** runs after all targeted user stories are complete.

## Parallel Execution Examples

- Within Phase 3 (US1): T007, T008 (different files) can run in parallel;
  T013 (frontend RxDB collection) can run in parallel with T007-T011
  (engine) since neither depends on the other's output, only on T004-T006.
- Within Phase 4 (US2): T017 and T018 (different files) in parallel; T023
  and T024 (different files) in parallel; T028 in parallel with the engine
  tasks.
- Across stories: once Phase 2 is done, US3's backend tasks (T032-T037)
  can start in parallel with US1/US2 entirely, since annotations share no
  files or data with walls/lights.

## Implementation Strategy

**MVP = Phase 1 + Phase 2 + Phase 3 (User Story 1)**. This alone makes the
already-shipped Phase 6 walls backend usable for the first time and
delivers the highest-priority spec outcome (SC-001/SC-002 groundwork,
FR-001-003) without touching tldraw at all.

Ship incrementally: US1 → US2 → US3 (with tldraw removal as the final
sub-step of US3, not before). Each checkpoint above is independently
demoable per quickstart.md.
