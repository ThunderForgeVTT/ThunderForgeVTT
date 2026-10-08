# Tasks: Hidden Walls and Box Select

**Input**: Design documents from `specs/085-hidden-walls-and-box-select/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md,
contracts/engine-selection.md, contracts/world-store.md, quickstart.md

**Depends on spec 082.** The tasks marked **[082]** must not start until
these 082 tasks are done:

| 082 task | What it gives 085              |
| -------- | ------------------------------ |
| T008     | `ViewerUserId` and `createdBy` |
| T012     | `may_edit_shape`               |
| T015     | players' Select rail           |
| T018     | shape-bridge rollback          |

Tasks marked **[082 merged]** wait for 082 to be merged, because they
edit `systems/shape.rs`. Nothing in 085 edits 082's files while 082 is in
progress.

**Tests**: Tests come first in every phase and must fail before the code
that passes them.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel: a different file, with no dependency on an
  unfinished task.
- **[Story]**: US1 (the GM hides a wall), US2 (the GM's box), US3 (the
  player's box).

---

## Phase 1: Setup

- [x] T001 Confirm spec 082 tasks T008, T012, T015 and T018 are `[x]` in `specs/082-players-draw-shapes/tasks.md`, and that `ViewerUserId`, `may_edit_shape` and `Shape.created_by` exist (`grep` in `crates/thunderforge-engine/src` and `crates/thunderforge-canvas-core/src/shape.rs`). Until they are, only Phase 2 (T003–T009) and US1 (T010–T016) may start.
- [x] T002 [P] Confirm `apps/web/e2e/canvas-box-select.spec.ts` resolves to the `canvas` slice through the `canvas-` prefix in `scripts/e2e/slices.json` (`pnpm e2e:which apps/web/e2e/canvas-box-select.spec.ts`). Expect no change to `slices.json`.

---

## Phase 2: Foundational (blocks US2 and US3)

**Purpose**: the pure box rule, the group resource, the token's owner in the engine, and the store's group and tally.

- [x] T003 [P] Tests `crates/thunderforge-canvas-core/src/box_select_tests.rs`:
  - `ScreenBox::from_corners` orders the corners in all four drag directions;
  - `wholly_inside`:
    - a point inside, on the edge and outside;
    - a segment with both ends inside, or one end out;
    - a rect inside, crossing the edge, or larger than the box;
  - `apply_box`:
    - without toggle, it returns the hits;
    - with toggle, it removes the held hits and appends the others in order;
    - an empty toggle changes nothing;
    - there are no duplicates.
- [x] T004 `crates/thunderforge-canvas-core/src/box_select.rs` per contracts/engine-selection.md, with `#[cfg(test)] #[path = "box_select_tests.rs"] mod tests;`. Add `pub mod box_select;` to `crates/thunderforge-canvas-core/src/lib.rs`. `cargo test -p thunderforge-canvas-core box_select` is green.
- [x] T005 [P] Tests in a new `crates/thunderforge-engine/src/resources/group_selection.rs` `mod tests`:
  - `set_group` writes `SelectedToken` and each kind's primary;
  - `select_one` resets the group to one item, and `clear` resets it to none;
  - `remove(id)` drops an id from whichever kind holds it;
  - `len()` counts across kinds.
- [x] T006 `GroupSelection` and the helpers T005 names, in `crates/thunderforge-engine/src/resources/group_selection.rs`. Register it in `resources/mod.rs` and `app.rs` (`init_resource`).
- [x] T007 [P] Engine owner:
  - Tests first, in `crates/thunderforge-engine/src/systems/token_owner_tests.rs` (`#[path]` from `systems/token.rs`): a payload without `ownerUserId` keeps the current `TokenOwner`, `null` clears it, and a string sets it.
  - Then `owner_user_id: Option<Option<String>>` on `WorldTokenPayload` (`payloads.rs`, `absent_or_null` deserializer), and a `TokenOwner` component set by the token upsert in `systems/token.rs`.
- [x] T008 [P] Store:
  - Tests first, in `apps/web/src/engine/world/__tests__/store.test.ts`:
    - `select_group` sets the four lists and the three primaries;
    - `select_wall`, `select_light` and `select_shape` set their list to `[id]` or `[]`;
    - `remove_wall`, `remove_light` and `remove_shape` drop the id from the list.
  - Then `SelectGroupCommand`, `DeleteGroupCommand`, `GroupStamp` and an optional `group` on the per-item commands in `apps/web/src/engine/world/types.ts`, and the state fields and cases in `apps/web/src/engine/world/store.ts`.
- [x] T009 [P] Tally:
  - Tests first, in `apps/web/src/engine/world/sync/__tests__/groupMoves.test.ts`:
    - all answers OK gives no toast;
    - 2 refused of 5 gives one toast, "2 of 5 could not be moved.";
    - an answer without a stamp is ignored;
    - a stamp unanswered for 30 s is dropped silently (fake timers).
  - Then `settleGroup` in `apps/web/src/engine/world/sync/groupMoves.ts`.

**Checkpoint**: The canvas-core, engine and web units are green. `make lint` passes for both targets.

---

## Phase 3: User Story 1 — The GM hides a wall (P1) 🎯 MVP

**Goal**: Any wall can be hidden from the table and still blocks. Independent of US2/US3 except T014's group button (in T025).

**Independent Test**: The GM hides a plain wall. The player's board does not draw it, and the player's token cannot cross it.

- [x] T010 [P] [US1] Door click, tests first, in `crates/thunderforge-engine/src/systems/wall_door_tests.rs`: a click that makes a hidden plain wall a door keeps `secret: true`, and its changes are exactly `{doorState:"closed", locked:false}`.
- [x] T011 [US1] Extract the door click's wall update into `door_click_update(&Wall) -> (Wall, serde_json::Value)` in a new `crates/thunderforge-engine/src/systems/wall_door.rs`. Call it from `systems/wall.rs` (≈278–292), dropping `updated.secret = false` and `secret: false`, so `wall.rs` shrinks.
- [x] T012 [P] [US1] Wall bridge, tests first, in a new `apps/web/src/engine/world/sync/__tests__/wallBridge.test.ts`, with `setDoorSecret` and `updateWall` mocked:
  - `set_walls_hidden` for 3 walls sends 3 `setDoorSecret` calls, each in its wall's turn;
  - one refusal restores that wall's prior record with source `"sync"` and leaves the other two;
  - one toast says "1 of 3 could not be hidden.";
  - a refused `update_wall` restores the prior record.
- [x] T013 [US1] Wire up hiding:
  - `SetWallsHiddenCommand` in `apps/web/src/engine/world/types.ts`;
  - the optimistic `secret` in `apps/web/src/engine/world/store.ts`, with a store test in `store.test.ts`;
  - the bridge case, rollback and `settleGroup` in `apps/web/src/engine/world/sync/walls.ts`, importing `setDoorSecret` from `apps/web/src/api/interactives.ts`.
- [x] T014 [US1] **Hidden from the table** checkbox in `apps/web/src/components/canvas-tools/WallTool/WallTool.tsx`:
  - it shows for the GM with a wall selected, whether door or not;
  - it is checked from `walls[id].secret`;
  - it dispatches `set_walls_hidden` for `[id]`;
  - its test id is `wall-hidden-toggle`.
- [x] T015 [US1] Probe `drawnWalls()`: `wall_visuals()` in `crates/thunderforge-engine/src/sdk.rs` (wall ids with a visual entity, from `sync_wall_visuals`'s marker), and the reader in `apps/web/src/engine/bevy/index.ts` `__engineProbe`. *Landed:* `drawn_wall_ids()` (spec 030) already publishes exactly that list from `sync_wall_visuals`, for any wall, so no new engine export was added; `__engineProbe.drawnWalls()` reads it.
- [x] T016 [US1] E2E `apps/web/e2e/canvas-box-select.spec.ts`, first test, with a GM and a player in two contexts:
  - the GM hides a plain wall through `wall-hidden-toggle`;
  - the player's `drawnWalls()` lacks it and the GM's still has it;
  - the player's drag across it ends where it began;
  - `sight` at a point behind it is unseen;
  - the GM converts it to a door and it stays hidden;
  - a reload keeps all of it (SC-001).

**Checkpoint**: `pnpm e2e:canvas` passes the US1 test. US1 ships alone.

---

## Phase 4: User Story 2 — The GM boxes several things (P1)

**Goal**: In Select, the GM boxes, moves, toggles and deletes a group, with the filter honoured.

**Independent Test**: Box three tokens and a wall, drag, reload. All four moved.

- [x] T017 [P] [US2] Tests in `crates/thunderforge-engine/src/systems/box_select.rs` `mod tests`: `box_candidate` for a GM with each `SelectionFilter` field on and off, one row per kind.
- [x] T018 [US2] [082] `crates/thunderforge-engine/src/systems/box_select.rs`:
  - the `BoxDrag` resource;
  - press on empty board, a drag past the click threshold, a gizmo rectangle, and the release;
  - footprints built from the ECS (the token footprint as `tokens_at` measures it, the wall segment, the light centre, the shape's bounds);
  - `box_candidate`, then `apply_box` with shift;
  - write `GroupSelection` and emit `select_group`.

  Register it in `app.rs` under `AuthoringMode::Select`, `.before(handle_token_drag)`.

  *Landed:* registered in `plugins/group_select.rs` (`GroupSelectPlugin`) rather than `app.rs`, which is near the 1000-line limit. The rule already carries the player rows; T029 adds their tests.

- [x] T019 [US2] Two guard clauses at the top of `handle_token_drag`'s press branch in `crates/thunderforge-engine/src/systems/token.rs`:
  - with shift held, an empty-board press does not deselect;
  - a press on a token in a group of two or more is left to `group_move`.

  Test both in a new `crates/thunderforge-engine/src/systems/token_press_tests.rs` (`#[path]` from `token.rs`). Shift-click on a token toggles it in the group (`apply_box` with one hit).

- [x] T020 [US2] [082 merged] Move `translate_geometry` and its two tests from `crates/thunderforge-engine/src/systems/shape.rs` to a new `crates/thunderforge-canvas-core/src/shape_geometry.rs` as `translate`. Add `shape_bounds(kind, &geometry)` with tests for rect, ellipse, line, stroke and text. `systems/shape.rs` calls `shape_geometry::translate`.
- [x] T021 [P] [US2] Tests in `crates/thunderforge-engine/src/systems/group_move.rs` `mod tests` for a pure `release_events(members, offset, stamp) -> Vec<Value>`:
  - a token gets `upsert_token` with its fields;
  - a wall gets `update_wall` with all four ends moved by the offset;
  - a light gets `update_light` with `x` and `y`;
  - a shape gets `update_shape` with translated geometry;
  - every event carries the same `group`, whose `size` equals the count;
  - the offset is snapped by the pressed item's snapping.
- [x] T022 [US2] [082 merged] `crates/thunderforge-engine/src/systems/group_move.rs`:
  - the `GroupDrag` resource, started by a press on a member of a group of two or more;
  - live preview of every member;
  - `release_events` on release;
  - Delete and Backspace emit `delete_*` per member, stamped;
  - `ExternalCommand::DeleteSelection` (`payloads.rs`) and `delete_selection()` (`sdk.rs`) do the same.

  Register it in `app.rs` under Select.

  *Landed:* registered in `GroupSelectPlugin` (`plugins/group_select.rs`), as T018 was. `delete_selection()` and the `delete_group` command both raise one request the system consumes, the pattern `set_selection_filter` uses. A GM's group Delete also removes tokens, as `remove_token` (FR-012: every item the viewer may delete).

- [x] T023 [P] [US2] Light bridge, tests first, in `apps/web/src/engine/world/sync/__tests__/lightBridge.test.ts`:
  - a refused `update_light` or `delete_light` restores the cached record with source `"sync"`;
  - every answer settles its `group`.

  Then the code in `apps/web/src/engine/world/sync/lights.ts`.

- [x] T024 [P] [US2] [082] Token and shape bridges, tests first, in `tokenBridge.test.ts` and `shapeEventSync.test.ts`:
  - each answer settles its `group`;
  - a refused grouped `upsert_token` still re-reads and puts the token back, but shows no toast of its own.

  Then the code in `apps/web/src/engine/world/sync/tokens.ts` (`applyMoveRefusal` takes the stamp) and `sync/shapes.ts`.

- [x] T025 [US2] Select bar, test first in `apps/web/src/components/canvas-tools/SelectionBar/__tests__/SelectionBar.test.tsx`:
  - it renders only with two or more selected, and shows counts by kind;
  - **Hidden from the table** appears only for a GM with walls, and dispatches `set_walls_hidden` with the group's wall ids;
  - **Delete** dispatches `delete_group`.

  Then `SelectionBar.tsx`. Render it from `apps/web/src/pages/world/WorldPage.tsx` in Select mode. Forward `delete_group` to `deleteSelection()` in `apps/web/src/engine/bevy/index.ts`.

- [x] T026 [US2] Probe `selection()`: `selection_state()` in `crates/thunderforge-engine/src/sdk.rs`, and the reader in `apps/web/src/engine/bevy/index.ts`.
- [x] T027 [US2] E2E in `canvas-box-select.spec.ts`, as the GM:
  - box three tokens and a wall (mouse down, move, up on the canvas); `selection()` holds all four;
  - drag one token by two cells; a reload shows all four moved by the same offset (SC-002);
  - shift-box one token out and shift-click it back;
  - untick walls in `SelectionFilterMenu`, box again, and the wall is not taken;
  - hide the group's walls from the Select bar;
  - Delete removes the group, on both the GM's board and the server's answer.
- [x] T028 [US2] `make lint` passes for host and wasm32, with the two new systems.

**Checkpoint**: US1 and US2 green in `pnpm e2e:canvas`.

---

## Phase 5: User Story 3 — A player boxes what is theirs (P2)

**Goal**: The player's box takes only their visible tokens and their own shapes. Group moves are judged per token.

**Independent Test**: A player with two tokens boxes them plus an NPC. Only their two are selected and moved.

- [x] T029 [P] [US3] [082] Tests in `systems/box_select.rs` `mod tests` for a player's `box_candidate`:
  - their visible token is taken;
  - their token hidden by sight (`seen: false`) is not;
  - another's token is not;
  - walls and lights are never taken, with any filter;
  - their own shape is taken and the GM's is not;
  - with no viewer id, nothing is taken.
- [x] T030 [US3] [082] Player rows of `box_candidate`:
  - `ViewerUserId`, `TokenOwner`, `Visibility` and `may_edit_shape`, in `crates/thunderforge-engine/src/systems/box_select.rs`;
  - in `systems/token.rs`'s upsert, a token whose new owner is not the viewer leaves `GroupSelection` and `SelectedToken` for a non-GM, with a test in `token_owner_tests.rs`.
- [x] T031 [US3] Tests in `group_move.rs` `mod tests`, then the code:
  - for a non-GM, a token whose own path crosses a blocking wall (`token_move::refuse_at_wall`) is not sent and goes back;
  - `size` counts only what was sent;
  - Delete sends only shapes the viewer may edit.
- [x] T032 [US3] E2E in `canvas-box-select.spec.ts`, with a GM and a player who owns two tokens:
  - the player's box over their two tokens, an NPC, a wall, a light, their shape and the GM's shape selects exactly their two tokens and their shape (SC-003);
  - a token of theirs hidden by sight inside the box is not taken;
  - a group drag where one path crosses a wall leaves that token back and the other moved, on both boards;
  - one toast says "1 of 2 could not be moved." (SC-004).
- [ ] T033 [US3] `pnpm e2e:canvas` green.

**Checkpoint**: All three stories green.

---

## Phase 6: Polish & Proof

- [x] T034 [P] `docs/guides/doors-and-walls.md`: hiding any wall from the table, and that a hidden wall still blocks and casts a shadow. `docs/guides/lights-and-drawings.md`: selecting several things with a box, shift, the Select bar, and what a player's box takes.
- [ ] T035 [P] Demo check: in `pnpm -F @thunderforge/demo dev`, hide a wall and box a group as the GM and as a player (quickstart.md). It should need no `apps/demo` change. If one is needed, add it with a test in `apps/demo/src/backend/` and record why here.
- [ ] T036 Run the following, each green:
  - `make lint`;
  - `cargo test -p thunderforge-canvas-core`;
  - `cargo test -p thunderforge-engine`;
  - `pnpm -F @thunderforge/web test`;
  - `pnpm -F @thunderforge/web typecheck`.
- [ ] T037 **Proof**: `pnpm e2e:canvas`, which runs `apps/web/e2e/canvas-box-select.spec.ts`. Then `pnpm e2e:which --diff`, and run each slice it names. No cross-cutting path changes, so the full suite is not required.

---

## Dependencies & Execution Order

- **Setup (T001–T002)**: start now.
- **Foundational**:
  - T003→T004, T005→T006, T007, T008 and T009 can start now. They touch no 082 file.
  - US2 and US3 need all of them.
- **US1 (T010–T016)**: needs T008 (store) and T009 (tally) only. It can be built and shipped before 082 lands.
- **US2**:
  - needs the Foundational phase;
  - T018, T024 and T030 need 082 T008, T012 and T018;
  - T020 and T022 need 082 merged (`systems/shape.rs`);
  - T019 follows T018 (same gesture), and T027 follows T018–T026.
- **US3**: needs US2's T018 and T022, and 082 T015 (the player's Select).
- **Polish**: after the stories it documents.

## Parallel Opportunities

- T003, T005, T007, T008 and T009 are all different files.
- T010 and T012, in US1.
- T017, T021, T023 and T024, in US2.
- T029 runs alongside T031's tests.
- T034 and T035.

## Implementation Strategy

1. **MVP**: Setup, then Foundational T008 and T009, then US1. Hidden walls
   ship on their own, and they need nothing from 082.
2. **Then the rest of Foundational and US2**, once 082 has merged.
3. **Then US3**, then Polish, with `pnpm e2e:canvas` as the gate.
