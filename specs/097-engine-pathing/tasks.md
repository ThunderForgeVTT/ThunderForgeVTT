# Tasks: Paths Around Walls

**Input**: [spec.md](./spec.md), [plan.md](./plan.md)

**Tests**: TDD, with `cargo test -p thunderforge-canvas-core pathfinding`.

## Phase 1: Core (US1, US2, US4)

- [ ] T001 [US1] Failing tests:
  - open plane;
  - wall with a gap;
  - corner squeeze;
  - determinism.
- [ ] T002 [US2] Failing tests: shut door is `None`; open door passes.
- [ ] T003 [US4] Failing tests: bounds, blocked cells, budget.
- [ ] T004 Implement `find_path` and make T001–T003 pass.

## Phase 2: Smoothing (US3)

- [ ] T005 [US3] Failing tests:
  - a straight run collapses to two points;
  - the route round a wall keeps its corner;
  - the smoothed path is clear under `path_blocked_by`.
- [ ] T006 [US3] Implement the string-pull.

## Phase 3: Engine export (FR-008)

- [ ] T007 Add the snapshot plugin and the `plan_path` export, with a
  round-trip test.
- [ ] T008 Rebuild the wasm. The landing (098) uses it; its e2e is the proof.

## Follow-ups

- F001 Hex grids.
- F002 Footprints larger than one cell.
