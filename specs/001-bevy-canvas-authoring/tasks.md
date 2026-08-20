---
description: "Task list for Native Canvas — Full tldraw Replacement (Walls, Map Import, Lighting, Shapes)"
---

# Tasks: Native Canvas — Full tldraw Replacement (Walls, Map Import, Lighting, Shapes)

**Input**: Design documents from `specs/001-bevy-canvas-authoring/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/graphql.md, quickstart.md

**Tests**: Included — the constitution's Verify Before Claiming Done
principle and quickstart.md's "Automated coverage" section both call for
them; kept scoped to ownership/occlusion/plugin-independence/parser
correctness, not full TDD.

**Organization**: Tasks are grouped by user story (US1 = walls, US2 = map
import, US3 = lighting, US4 = shapes + tldraw removal) so each ships and
is independently verifiable per quickstart.md's scenarios.

**Revision notes**:
- v1→v2: scope expanded from authoring-tools-only to full tldraw
  replacement + map import; US2 (import) inserted, old US2/US3
  renumbered to US3/US4.
- v2→v3 (this revision, per `/speckit-analyze`): added wall/light undo
  tasks (T014, T039 — research.md §4 requires per-plugin undo stacks for
  wall/light/shape, but v2 only implemented it for shapes); added an
  SC-007 import-timing check (T070); fixed T034/T050's GraphQL
  registration guidance, which incorrectly pointed at `schema.rs` (the
  Diesel table-macro file) instead of the `QueryRoot`/`MutationRoot`
  `MergedObject` tuples actually defined in `graphql.rs`; standardized
  US2's light-backend dependency citation to T030-T031 everywhere (the
  model struct + Diesel table are all `map_import.rs` actually needs —
  it writes via Diesel directly, not through the GraphQL mutation).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an
  incomplete task)
- Paths are relative to the repository root

---

## Phase 1: Setup

- [X] T001 [P] Add `door_state` column migration (`ALTER TABLE walls ADD COLUMN door_state TEXT NOT NULL DEFAULT 'none' CHECK (door_state IN ('none','open','closed'))`) in `src/server/migrations/<timestamp>_add_door_fields_to_walls/{up,down}.sql`
- [X] T002 [P] Add `background_image_path` column migration (`ALTER TABLE scenes ADD COLUMN background_image_path TEXT`) in `src/server/migrations/<timestamp>_add_background_image_to_scenes/{up,down}.sql`
- [X] T003 [P] Add `light_sources` migration in `src/server/migrations/<timestamp>_create_light_sources_table/{up,down}.sql` per data-model.md (columns incl. `casts_shadows`, CHECK constraints, FK to `scenes`/`tokens`, `idx_light_sources_scene_id` index)
- [X] T004 [P] Add `shapes` migration in `src/server/migrations/<timestamp>_create_shapes_table/{up,down}.sql` per data-model.md (columns, `kind` CHECK constraint incl. `rect`/`ellipse`/`line`, FK to `scenes`, `idx_shapes_scene_id` index)
- [X] T005 Run `diesel migration run` against the dev database and confirm all four `down.sql` scripts cleanly reverse via `diesel migration redo` for each

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: Close the real-time propagation gap (all stories need it for
FR-003/SC-001) and establish the shared layer-ordering resource every
rendering system in this feature consumes (FR-016).

- [X] T006 Add NOTIFY emission to `create_wall`/`update_wall`/`delete_wall` in `src/server/src/graphql/mutations_walls.rs`, following the existing LISTEN/NOTIFY channel/payload convention used elsewhere in `src/server/src/graphql.rs`
- [ ] T007 Extend `apps/web/src/engine/world/sync` to subscribe to and dispatch wall change events into the world store, mirroring the existing token event dispatch path
- [ ] T008 Add `Wall` variant handling to the engine-side world-command dispatch in `src/engine/src/lib.rs` (`ExternalCommand`/`apply_external_commands`) so wall events reach the engine, following the existing `UpsertToken`/`RemoveToken` pattern
- [ ] T009 [P] Create `CanvasLayers` resource (ordered layer list: background, grid, walls, lighting, shapes, tokens, fog + GM/player visibility rule per layer) in `src/engine/src/resources/canvas_layer.rs`, re-exported via `resources/mod.rs`
- [ ] T010 [P] Create `CanvasLayerPlugin` in `src/engine/src/plugins/canvas_layer.rs` registering `CanvasLayers`; register it in `src/engine/src/lib.rs` before the wall/lighting/shape plugins added in later phases (data-model.md's Canvas Layer section)

**Checkpoint**: Wall create/move/delete now round-trips server → all
connected clients within a few seconds, with no UI yet, and a shared
layer-ordering resource exists for every later plugin to consume. This
unblocks US1.

---

## Phase 3: User Story 1 — GM authors walls (and doors) that block vision and movement (Priority: P1) 🎯 MVP

**Goal**: A GM can draw, move, reconfigure, delete, and door-toggle wall
segments on the canvas; vision/movement respects them; players never see
the tool.

**Independent Test**: quickstart.md Scenario 1.

### Engine (Rust/Bevy, wasm32 target)

- [ ] T011 [P] [US1] Create `WallSet` resource (segment list + simple spatial index + door state + a bounded per-session undo stack, research.md §4) in `src/engine/src/resources/wall.rs`, re-exported via `src/engine/src/resources/mod.rs`
- [ ] T012 [P] [US1] Create wall input systems (click-drag create, endpoint drag to move, select+delete, blocks_vision/blocks_movement toggle, door-state toggle) in `src/engine/src/systems/wall.rs`, re-exported via `src/engine/src/systems/mod.rs`
- [ ] T013 [US1] Implement 2D shadow-casting vision occlusion against `WallSet` (research.md §3), treating an `'open'` door as non-blocking regardless of its stored flags (data-model.md door semantics), as a system in `src/engine/src/systems/wall.rs`, triggered on `WallSet` change and on token movement, feeding the existing fog rendering slot reserved by ADR-032
- [ ] T014 [US1] Implement wall undo (FR-012): re-issue the inverse mutation (e.g. undo of "move wall" re-issues `updateWall` with prior coordinates; undo of a door toggle re-issues the prior `doorState`) through the same GraphQL path as a normal edit, wired to `WallSet`'s undo stack from T011 — same approach as research.md §4, applied to walls
- [ ] T015 [US1] Create `WallPlugin` in `src/engine/src/plugins/wall.rs` wiring T011-T014's systems/resources, rendering into the walls layer from `CanvasLayers` (T009); register it in `src/engine/src/lib.rs` alongside the existing `SelectionPlugin`/`TokenPlugin` registrations, independently addable per Constitution Principle II
- [ ] T016 [US1] Reject/ignore zero-length wall creation (edge case from spec.md) in the create system from T012

### Backend (Rust/GraphQL)

- [X] T017 [US1] Add `door_state` to `Wall`/`WallUpdate` in `src/server/src/models.rs` and `GraphQLWall`/`GraphQLUpdateWallInput` in `src/server/src/graphql.rs`/`input_types.rs`, implementing the door semantics from data-model.md
- [X] T018 [US1] Add a scene-ownership assertion test for `create_wall`/`update_wall`/`delete_wall` (including door-state updates) in `src/server/src/graphql/mutations_walls.rs` if not already covered

### Frontend (React/RxDB)

- [ ] T019 [P] [US1] Create `apps/web/src/db/collections/worldWallsCollection.ts` mirroring `worldTokensCollection.ts` (schema incl. `doorState`, replication pull/push)
- [ ] T020 [US1] Wire wall create/update/delete GraphQL calls into `apps/web/src/engine/world/sync`, dispatching into the RxDB collection from T019
- [ ] T021 [US1] Create `apps/web/src/components/canvas-tools/WallTool/` (toolbar button + blocks_vision/blocks_movement/door-toggle property panel), GM-only rendered (hidden entirely for non-owners per FR-009)
- [ ] T022 [US1] Mount `WallTool` from `apps/web/src/pages/world/WorldPage.tsx`, gated on the existing GM/scene-owner check already used elsewhere on that page

**Checkpoint**: quickstart.md Scenario 1 and Scenario 5 (wall portion)
pass. Ship-able MVP increment.

---

## Phase 4: User Story 2 — GM imports a Universal VTT (`.dd2vtt`) map (Priority: P2)

**Goal**: A GM can upload a `.dd2vtt` file and have its background art,
walls, doors, and lights appear on the scene in one action, fully
editable afterward with the normal tools.

**Independent Test**: quickstart.md Scenario 2 (requires US1's wall
rendering/occlusion to verify visually; requires US3's `LightSource`
model struct and Diesel table — T030-T031 — for the light-import step,
see Dependencies).

### Backend (Rust)

- [ ] T023 [P] [US2] Write the UVTT (format 0.3) JSON parser (deserialize `resolution`/`line_of_sight`/`objects_line_of_sight`/`portals`/`environment`/`lights`/`image`; reject unsupported `format` values; skip/report degenerate `line_of_sight` polygons with <2 points) as a standalone module `src/server/src/map_import.rs`, unit-tested directly against both `examples/maps/*.dd2vtt` fixtures
- [ ] T024 [US2] Implement coordinate scaling (research.md §8: `scene_px = grid_units * target_scene.grid_size`) and the polygon→wall-segment / portal→door-wall / light→LightSource conversion functions in `src/server/src/map_import.rs` (light conversion depends on T030-T031's `NewLightSource` struct/table existing)
- [ ] T025 [US2] Implement background-image decode + save under `state.directories.asset_directory` (research.md §9), writing the relative path, in `src/server/src/map_import.rs`
- [ ] T026 [US2] Add the `POST /api/scenes/{scene_id}/import/uvtt` Axum multipart handler in `src/server/src/map_import.rs` per contracts/graphql.md: scene-ownership check, request size cap, parse-and-validate-before-write, single DB transaction wrapping T024/T025's writes plus bulk `Wall`/`LightSource` inserts, response counts; register the route in `src/server/src/serve/mod.rs`
- [ ] T027 [US2] Add NOTIFY emission for the import transaction (one event for the whole batch, per contracts/graphql.md), reusing T006's pattern

### Frontend (React)

- [ ] T028 [P] [US2] Create `apps/web/src/components/canvas-tools/MapImportTool/` (file picker, upload progress, error display for 400/403/413 responses), GM-only rendered
- [ ] T029 [US2] Mount `MapImportTool` from `apps/web/src/pages/world/WorldPage.tsx`; on success, trigger a re-fetch of `walls`/`lightSources`/scene queries so the imported content appears without a manual reload

**Checkpoint**: quickstart.md Scenario 2 passes on top of Scenario 1 and
US3's `LightSource` model/table (T030-T031). SC-007's <30s target is
verified in Phase 7 (T070), not here.

---

## Phase 5: User Story 3 — GM places and manages lighting sources (Priority: P3)

**Goal**: A GM can place, move, resize, and delete light sources; light is
occluded by vision-blocking walls (including closed doors); lights can
attach to a token.

**Independent Test**: quickstart.md Scenario 3 (requires US1's walls for
the occlusion check).

### Backend (Rust/GraphQL)

- [ ] T030 [P] [US3] Add `LightSource`/`NewLightSource`/`LightSourceUpdate` structs (incl. `casts_shadows`) to `src/server/src/models.rs` per data-model.md
- [ ] T031 [P] [US3] Add `light_sources` table to `src/server/src/schema.rs`
- [ ] T032 [US3] Add `GraphQLLightSource` type + `CreateLightSourceInput`/`UpdateLightSourceInput` to `src/server/src/graphql/input_types.rs` and `src/server/src/graphql.rs` per contracts/graphql.md
- [ ] T033 [US3] Create `LightSourceMutation` (create/update/delete, scene-ownership enforced, CHECK-constraint errors surfaced) in `src/server/src/graphql/mutations_lighting.rs`, using `mutations_walls.rs` as the direct template
- [ ] T034 [US3] Add `lightSources(sceneId: ID!)` query to `src/server/src/graphql/queries/scene.rs`; register the query type into the `QueryRoot` `MergedObject` tuple and `LightSourceMutation` into the `MutationRoot` `MergedObject` tuple, both in `src/server/src/graphql.rs` (~L1633-1644) — **not** `src/server/src/schema.rs`, which is the unrelated Diesel table-macro file
- [ ] T035 [US3] Add NOTIFY emission to the light mutations from T033, same pattern as T006

### Engine (Rust/Bevy, wasm32 target)

- [ ] T036 [P] [US3] Create `LightSet` resource (incl. a bounded per-session undo stack, research.md §4) in `src/engine/src/resources/lighting.rs`, re-exported via `resources/mod.rs`
- [ ] T037 [P] [US3] Create light input systems (place, drag-reposition, resize radius/intensity, select+delete, attach-to-token) in `src/engine/src/systems/lighting.rs`, re-exported via `systems/mod.rs`
- [ ] T038 [US3] Implement occlusion-aware illumination sampling: for each `LightSet` entry with `casts_shadows = true`, apply the shadow-casting result from T013 against `WallSet` (treating open doors as non-blocking) to determine the lit region; resolve token-attached light position from the attached entity's live `Transform` each frame; lights with `casts_shadows = false` skip occlusion entirely
- [ ] T039 [US3] Implement light undo (FR-012): re-issue the inverse mutation (e.g. undo of "resize light" re-issues `updateLightSource` with the prior radius/intensity) through the same GraphQL path as a normal edit, wired to `LightSet`'s undo stack from T036 — same approach as T014 for walls
- [ ] T040 [US3] Create `LightingPlugin` in `src/engine/src/plugins/lighting.rs` wiring T036-T039, rendering into the lighting layer from `CanvasLayers` (T009); register independently in `src/engine/src/lib.rs` per Constitution Principle II
- [ ] T041 [US3] Reject/ignore zero-radius light creation (edge case from spec.md) in the create system from T037

### Frontend (React/RxDB)

- [ ] T042 [P] [US3] Create `apps/web/src/db/collections/worldLightsCollection.ts` mirroring `worldTokensCollection.ts`
- [ ] T043 [US3] Wire light create/update/delete GraphQL calls into `apps/web/src/engine/world/sync`
- [ ] T044 [US3] Create `apps/web/src/components/canvas-tools/LightingTool/` (toolbar button + radius/intensity/color/attach-to-token property panel), GM-only rendered
- [ ] T045 [US3] Mount `LightingTool` from `apps/web/src/pages/world/WorldPage.tsx` alongside `WallTool`/`MapImportTool`

**Checkpoint**: quickstart.md Scenario 3 passes on top of Scenario 1.
T030-T031 (model struct + Diesel table only) are the actual dependency
of US2's T024 (light-import conversion) — see Dependencies below.

---

## Phase 6: User Story 4 — GM draws/manages shapes, full tldraw parity, tldraw removed (Priority: P4)

**Goal**: A GM can draw/edit/delete freehand strokes, rectangles,
ellipses, lines/arrows, and text labels natively on the canvas with a
GM-only/player-visible flag — full parity with tldraw's tool set; tldraw
and `WorldWhiteboard.tsx` are removed once this reaches that parity.

**Independent Test**: quickstart.md Scenario 4.

### Backend (Rust/GraphQL)

- [X] T046 [P] [US4] Add `Shape`/`NewShape`/`ShapeUpdate` structs to `src/server/src/models.rs` per data-model.md
- [X] T047 [P] [US4] Add `shapes` table to `src/server/src/schema.rs`
- [X] T048 [US4] Add `GraphQLShape`/`ShapeKind` type + `CreateShapeInput`/`UpdateShapeInput` to `src/server/src/graphql/input_types.rs` and `src/server/src/graphql.rs` per contracts/graphql.md
- [X] T049 [US4] Create `ShapeMutation` (create/update/delete, scene-ownership enforced) in `src/server/src/graphql/mutations_shapes.rs`
- [X] T050 [US4] Add `shapes(sceneId: ID!)` query to `src/server/src/graphql/queries/scene.rs`, filtering to `visible_to_players = true` for non-owner callers (FR-009); register the query type into the `QueryRoot` `MergedObject` tuple and `ShapeMutation` into the `MutationRoot` `MergedObject` tuple, both in `src/server/src/graphql.rs` (~L1633-1644) — same correction as T034, not `schema.rs`
- [X] T051 [US4] Add NOTIFY emission to the shape mutations from T049, same pattern as T006

### Engine (Rust/Bevy, wasm32 target)

- [ ] T052 [P] [US4] Create `ShapeSet` resource + bounded per-session undo stack (research.md §4) in `src/engine/src/resources/shape.rs`, re-exported via `resources/mod.rs`
- [ ] T053 [P] [US4] Create draw/edit input systems for all five shape kinds (freehand stroke, rectangle, ellipse, line/arrow, text) — create, move, resize, restyle, select+delete — in `src/engine/src/systems/shape.rs`, re-exported via `systems/mod.rs`
- [ ] T054 [US4] Implement shape undo: re-issue the inverse mutation through the same GraphQL path as a normal edit (research.md §4), wired to the undo stack from T052 — same approach as T014/T039
- [ ] T055 [US4] Implement visibility filtering so GM-only shapes never render for non-GM sessions (defense in depth on top of T050's server-side filter)
- [ ] T056 [US4] Create `ShapePlugin` in `src/engine/src/plugins/shape.rs` wiring T052-T055, rendering into the shapes layer from `CanvasLayers` (T009); register independently in `src/engine/src/lib.rs` per Constitution Principle II

### Frontend (React/RxDB)

- [ ] T057 [P] [US4] Create `apps/web/src/db/collections/worldShapesCollection.ts` mirroring `worldTokensCollection.ts`
- [ ] T058 [US4] Wire shape create/update/delete GraphQL calls into `apps/web/src/engine/world/sync`
- [ ] T059 [US4] Create `apps/web/src/components/canvas-tools/ShapeTool/` (five sub-tools + GM-only/player-visible toggle + style panel), GM-only rendered
- [ ] T060 [US4] Mount `ShapeTool` from `apps/web/src/pages/world/WorldPage.tsx` alongside `WallTool`/`MapImportTool`/`LightingTool`

### tldraw removal (only after T046-T060 pass quickstart Scenario 4)

- [ ] T061 [US4] Remove `<WorldWhiteboard />` usage and the component itself: delete `apps/web/src/engine/tldraw/WorldWhiteboard.tsx` and its usages in `apps/web/src/pages/world/WorldPage.tsx`
- [ ] T062 [US4] Remove the `tldraw` package dependency from `apps/web/package.json` and run the package manager's lockfile update
- [ ] T063 [US4] Update ADR-004's status note and ADR-037's Consequences section to record removal completion date

**Checkpoint**: quickstart.md Scenario 4 passes; zero remaining tldraw
references anywhere in the codebase (SC-006).

---

## Phase 7: Polish & Cross-Cutting Concerns

- [ ] T064 [P] Add plugin-independence integration tests to `src/engine/src/integration_tests.rs`: each of `WallPlugin`/`LightingPlugin`/`ShapePlugin` must compile/run correctly with the others absent from the `App` builder (Constitution Principle II); `CanvasLayerPlugin` must be present for any of them to render but is otherwise a fixed dependency, not itself optional
- [ ] T065 [P] Add an occlusion unit test (light + wall/door geometry → expected lit/unlit regions, including the open-door-does-not-occlude case) near the systems added in T013/T038
- [ ] T066 [P] Add scene-ownership rejection tests for all mutations (T033, T049) and the import endpoint (T026) mirroring existing `mutations_walls.rs` test coverage expectations
- [ ] T067 [P] Add Playwright e2e coverage for quickstart.md Scenario 1 (walls), Scenario 2 (import), and Scenario 4 (shapes) in the existing e2e harness
- [ ] T068 Run `cargo check -p thunderforge_engine --target wasm32-unknown-unknown` and `cargo check -p dnd5e-server` and resolve any new warnings introduced by this feature (Constitution Principle V)
- [ ] T069 Execute quickstart.md Scenario 5 (authorization boundary) and Scenario 6 (empty-scene regression) manually before marking the feature done
- [ ] T070 Verify SC-007: time a full import of `examples/maps/demo.dd2vtt` (quickstart.md Scenario 2, steps 2-6) end-to-end and confirm it completes in under 30 seconds; record the measurement in the PR/commit description

---

## Dependencies & Execution Order

- **Setup (Phase 1)** blocks **Foundational (Phase 2)**.
- **Foundational (Phase 2)** blocks all user stories (T006-T008 are the
  NOTIFY/dispatch plumbing every story's real-time propagation relies on;
  T009-T010 are the shared layer resource every rendering plugin uses).
- **User Story 1 (Phase 3)** has no dependency on US2/US3/US4 and is the
  MVP (walls already have a backend; this only adds authoring UI, door
  semantics, undo, and occlusion).
- **User Story 2 (Phase 4)** depends on US1's `WallSet`/occlusion (T013)
  to verify walls visually, and on US3's `LightSource` model struct +
  Diesel table (T030-T031 only — the import path writes via Diesel
  directly, it never calls the GraphQL mutation) to have something to
  convert imported lights into. US2's own backend tasks (T023-T025, the
  parser and conversion functions) can be developed and unit-tested
  against the `examples/maps/` fixtures in parallel with US1/US3, since
  parsing doesn't require either to exist; only T024's light-conversion
  call site needs T030-T031 merged first.
- **User Story 3 (Phase 5)** depends on US1's `WallSet`/shadow-casting
  system (T013) for occlusion (T038); its backend (T030-T035) has no
  dependency on US1/US2 and can be built in parallel with either.
- **User Story 4 (Phase 6)** is independent of US1/US2/US3 at the
  data/engine level (shapes don't interact with walls, lights, or import)
  but its final sub-phase (T061-T063, tldraw removal) MUST wait until
  T046-T060 pass quickstart Scenario 4, per research.md §6.
- **Polish (Phase 7)** runs after all targeted user stories are complete;
  T070 specifically depends on Phase 4 (US2) being complete.

## Parallel Execution Examples

- Within Phase 3 (US1): T011, T012 (different files) can run in parallel;
  T019 (frontend RxDB collection) can run in parallel with T011-T016
  (engine) since neither depends on the other's output, only on T006-T008.
- Within Phase 4 (US2): T023 (parser) can start immediately after Phase 2,
  fully unit-testable against `examples/maps/` fixtures without any other
  story's code; T028 (frontend tool) can run in parallel with T023-T027.
- Within Phase 5 (US3): T030 and T031 (different files) in parallel; T036
  and T037 (different files) in parallel; T042 in parallel with the engine
  tasks.
- Within Phase 6 (US4): T046 and T047 (different files) in parallel; T052
  and T053 (different files) in parallel; T057 in parallel with the engine
  tasks.
- Across stories: once Phase 2 is done, US4's backend tasks (T046-T051)
  can start in parallel with US1/US2/US3 entirely, since shapes share no
  files or data with walls/lights/import.

## Implementation Strategy

**MVP = Phase 1 + Phase 2 + Phase 3 (User Story 1)**. This alone makes the
already-shipped Phase 6 walls backend usable for the first time and
delivers the highest-priority spec outcome (SC-001/SC-002 groundwork,
FR-001-003/FR-012/FR-017) without touching tldraw or map import at all.

Ship incrementally: US1 → US2 (import) → US3 (lighting) → US4 (shapes,
with tldraw removal as the final sub-step, not before). US2's parser work
(T023) can start as soon as Phase 2 is done, in parallel with the rest of
US1, since it only needs the `examples/maps/` fixtures — not live wall/
light data — to be developed and tested; it just can't be *wired into a
scene end-to-end* until US1 (for walls) and US3 (for lights, or at least
T030-T031) land.

Each checkpoint above is independently demoable per quickstart.md.
