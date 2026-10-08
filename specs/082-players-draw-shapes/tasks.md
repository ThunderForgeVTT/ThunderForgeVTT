# Tasks: Players Draw Shapes

**Input**: [spec.md](./spec.md), [plan.md](./plan.md), [research.md](./research.md),
[data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: the spec's Proof section asks for server, engine, demo and e2e
tests. Tests come before the code they prove inside each phase.

**File length**: `scripts/check-file-length.sh` fails a Rust file over 1000
lines. `mutations_shapes.rs` (490), `users/mod.rs` (909) and
`systems/shape.rs` (894) are near it, so new code and tests go in new files
as named below.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an open task)
- **[Story]**: US1 a player draws · US2 only their own · US3 clear all ·
  US4 clear a player's · US5 take the tools away · US6 demo

---

## Phase 1: Setup

- [x] T001 Migration `crates/thunderforge-server/migrations/2026-10-07-110000-0000_authoring_tool_revocations/{up,down}.sql`: the `world_authoring_tool_revocations` table, its unique and check constraints, deleting default-tool grant rows and adding `tool NOT IN ('select','shapes')` to `world_authoring_tool_grants` (data-model.md); run it; add the table and `joinable!` to `crates/thunderforge-server/src/schema.rs`
- [x] T002 `NewWorldAuthoringToolRevocation` (no `Queryable`: rows are read as bare `tool` strings, as grants are) in `crates/thunderforge-server/src/models.rs`; `cargo check -p thunderforge-server`

---

## Phase 2: Foundational (blocks every story)

### Tools by default

- [x] T003 Tests in `crates/thunderforge-server/src/auth/authoring_tools.rs` `mod tests`: rewrite `a_player_in_an_untouched_world_may_use_no_tool` as "… may select and draw"; add a revoked player (`shapes` revoked → `["select"]`), a granted player (`walls` granted → `["select","walls","shapes"]`, in `AUTHORING_TOOLS` order), a non-member (empty), a DM (all six); the revocation cascade when the member is removed (`remove_member_impl`)
- [x] T004 `PLAYER_DEFAULT_TOOLS` and a synchronous `effective_tools_on(conn, user_id, is_admin, world_id) -> QueryResult<Vec<String>>` in `crates/thunderforge-server/src/auth/authoring_tools.rs` (R1); `effective_authoring_tools` becomes a `spawn_blocking` wrapper over it; update the module doc (FR-045 superseded)

### One shape authority

- [x] T005 Tests `crates/thunderforge-server/src/auth/shape_authority_tests.rs`: `shape_authority` for a DM (any shape), the creator holding `shapes`, another player's shape, a revoked creator, a non-member, a site admin, creating (`created_by: None`) as a player
- [x] T006 `crates/thunderforge-server/src/auth/shape_authority.rs`: `ShapeAuthority { Dm, Creator, None }` and `shape_authority(conn, user_id, is_admin, scene_id, created_by)` per R3, beside `is_dm_of_scene`; register it and its tests in `src/auth/mod.rs`

### The engine knows shapes' creators and its viewer

- [x] T007 [P] `created_by: Option<String>` (`#[serde(default)]`) on `Shape` in `crates/thunderforge-canvas-core/src/shape.rs`; fill it at every constructor (`cargo check --workspace` names them)
- [x] T008 [P] `WorldShapePayload.created_by` (`createdBy`) in `crates/thunderforge-engine/src/payloads.rs`, copied into `Shape`; `ExternalCommand::SetViewerUser { user_id }`; `set_viewer_user` in `src/sdk.rs`; a `ViewerUserId` resource handled in `src/app.rs` beside `SetIsGameMaster` (contracts/engine-viewer.md)
- [x] T009 [P] Web: `createdBy: string | null` on `WorldShape` in `apps/web/src/engine/world/types.ts`; `shapeRecordToWorldShape` in `sync/shapes.ts` fills it, with a case in `sync/__tests__/shapeEventSync.test.ts`; `setViewerUser` in `apps/web/src/engine/bevy/index.ts` beside `setIsGameMaster`

**Checkpoint**: a player's effective tools are Select and Shapes; one function decides every shape write; both clients carry `createdBy`.

---

## Phase 3: US1 + US2 — a player draws, and edits only their own (P1, MVP)

### Server

- [x] T010 [US1] [US2] Tests `crates/thunderforge-server/src/graphql/mutations_shapes_tests.rs` (move the file's `mod tests` there, `#[path]`): a player creates a shape (`created_by` = player, `visible_to_players` true even when asked false); a player updates and deletes their own; a player's update on the GM's and on player B's shape errors and its delete answers `false`, rows and events unchanged; a player's update cannot set `visible_to_players` false; a revoked player's create is refused; a non-member is refused; the GM's paths unchanged; a paused scene refuses a player's create
- [x] T011 [US1] [US2] `createShape`, `updateShape`, `deleteShape` in `crates/thunderforge-server/src/graphql/mutations_shapes.rs` call `shape_authority` in place of `is_dm_of_scene`; `Creator` forces `visible_to_players` (R4); refusals keep today's answers

### Engine

- [x] T011a [US2] Tests in `crates/thunderforge-engine/src/systems/shape_authority.rs` `mod tests`: `may_edit_shape` for a GM, the creator, another viewer, no viewer, a shape with no creator; a viewer whose allow list lacks `shapes` cannot enter the Shapes mode (`tool_is_allowed`)
- [x] T012 [US2] `crates/thunderforge-engine/src/systems/shape_authority.rs`: `may_edit_shape` (R7); replace the `IsGameMaster` gates in `systems/shape.rs` (`handle_shape_tool_selection`, `handle_shape_input`, `handle_shape_keyboard_toggles`, `handle_shape_undo`) and `plugins/context_menu.rs` as the table in contracts/engine-viewer.md says; hit-testing skips shapes the viewer may not edit; `sync_shape_visuals` keeps its GM tint. Also (contracts' "shape context menu" row): the web's `shapeMenuActions` offers a player Remove on their own drawing
- [x] T013 [US2] `make lint` (host and wasm32) passes with the engine change

### Web

- [x] T014 [P] [US1] Tests in `apps/web/src/lib/__tests__/authoringTools.test.ts`: `railTools(tools, allowed, isOwner)`: an owner with `null` sees all; a non-owner with `null` sees none; a non-owner with `["select","shapes"]` sees those two
- [x] T015 [US1] `railTools` in `apps/web/src/lib/authoringTools.ts`; `WorldPage.tsx` renders `GmToolRail` when `sceneId` and `railTools(…)` is non-empty (around line 3064); Escape-to-Select for anyone with a rail (around line 993); `setViewerUser(user?.id ?? null)` in the effect at around line 1176; `AssetPasteTool` stays owner-only
- [x] T016 [US1] `ShapeTool.tsx` takes `isGm`; hides the visible-to-players toggle for a player (FR-015); `submitText` dispatches `create_shape` instead of calling `createShape` (FR-014)
- [x] T017 [US2] Tests in `apps/web/src/engine/world/sync/__tests__/shapeEventSync.test.ts`: a refused `update_shape` or `delete_shape` restores the shape as the store held it before the command
- [x] T018 [US2] Rollback in `startShapeMutationBridge` (`apps/web/src/engine/world/sync/shapes.ts`): cache the shape before sending, restore it with source `"sync"` on a refusal (an error, or `deleteShape` answering `false`)

### E2E

- [x] T019 [US1] [US2] `apps/web/e2e/canvas-shapes-by-players.spec.ts`, first tests: a GM and two players in three contexts, no grant rows; the player's rail shows exactly Select and Shapes; player A draws and both other boards show it, `shapes` answers `createdBy` = A; player A's direct `updateShape`/`deleteShape` on the GM's and B's shapes are refused and the rows are unchanged (SC-002); player A moves their own shape. Check whether `apps/web/e2e/map-editor-tooling.spec.ts` asserts a player has no rail and update it
- [x] T020 [US1] [US2] `pnpm e2e:canvas` green — 2026-10-07: 22 passed, 1 skipped (`canvas-engine-stopped` panic test, release engine exports no `debug_panic`)

**Checkpoint**: MVP. Players draw, and touch only their own.

---

## Phase 4: US3 — the GM clears the board (P1)

- [x] T021 [US3] Tests in `crates/thunderforge-server/src/graphql/mutations_clear_shapes_tests.rs` (its own file, beside the mutation): `clearShapes` with no filter (every level), another scene untouched, one `deleted` event per shape in the same transaction, the count; as a player → `NotFound`, nothing deleted
- [x] T022 [US3] `clearShapes` in `crates/thunderforge-server/src/graphql/mutations_clear_shapes.rs` (R5), DM only via `shape_authority`, `refuse_scene_if_paused`; register it in the mutation root
- [x] T023 [US3] Add `clearShapes` to GATED in `crates/thunderforge-server/src/graphql/play_pause_surface_tables.rs`; regenerate `apps/thunderforge/schema.graphql` (`node scripts/check-graphql-contract.mjs --schema --fix`)
- [x] T024 [P] [US3] Tests in `shapeEventSync.test.ts`: `clear_shapes` sends `clearShapes(sceneId, createdBy)` and removes nothing locally; the `deleted` events remove the shapes
- [x] T025 [US3] `ClearShapesCommand` in `apps/web/src/engine/world/types.ts`; `clearShapes` in `apps/web/src/api/shapes.ts`; the bridge case in `sync/shapes.ts`
- [x] T026 [US3] "Clear all shapes" in `ShapeTool.tsx` for a DM, with a confirmation naming the scene (`apps/web/src/components/canvas-tools/ShapeTool/ClearShapesDialog.tsx`, the `ui/dialog` wrapper); dispatches `clear_shapes`
- [x] T027 [US3] E2E in `canvas-shapes-by-players.spec.ts`: GM and players draw; cancel leaves them; confirm empties all three boards and the server's answer; the player's panel has no clear action and their direct `clearShapes` is refused — 2026-10-07: green in the canvas slice

---

## Phase 5: US4 — the GM clears one player's shapes (P2)

- [x] T028 [US4] Tests: `clearShapes` with `createdBy` (only those creators, the GM's and B's untouched, byte-for-byte for SC-004) and with `[]` (nothing, no events); `shapeCreators` lists players with counts, excludes DMs, includes a removed member (`isMember: false`), refuses a player
- [x] T029 [US4] `shapeCreators` in `crates/thunderforge-server/src/graphql/queries/shape_creators.rs` (R6), registered in `queries/mod.rs`; regenerate the schema
- [x] T030 [US4] `getShapeCreators` in `apps/web/src/api/shapes.ts`; "Clear a player's shapes…" in `ClearShapesDialog.tsx`: a checkbox per creator with count, disabled with a reason when there are none, dispatches `clear_shapes` with `createdBy`
- [x] T031 [US4] E2E: the GM clears player A only; the GM's and B's shapes remain — 2026-10-07: green in the canvas slice, after a server fix: the world's creator has no `world_members` row, so `shapeCreators` listed the GM as "(no longer in this world)"; it now leaves out `worlds.created_by` (95da29c0)

---

## Phase 6: US5 — the GM takes the tools away (P2)

- [x] T032 [US5] (done with Phase 2: the defaults broke these tests, so they moved with it) Tests in `crates/thunderforge-server/src/graphql/mutations_authoring_tools.rs` `mod tests`: `granted: false` for `shapes` writes a revocation and the answer drops it; `granted: true` removes it; a non-default tool behaves as today; `authoringToolGrants` lists every non-DM member with effective tools (a member with no rows shows Select and Shapes); update the existing asserts around lines 260, 313 and 458
- [x] T033 [US5] `set_authoring_tool_grant_impl` writes revocations for a default tool (data-model.md table) and answers effective tools; `authoringToolGrants` in `queries/authoring_tools.rs` answers effective tools per member
- [x] T034 [US5] `AuthoringToolGrantsCard.tsx` reads the effective lists; intro text says players select and draw by default; update the doc comment in `apps/web/src/api/authoringTools.ts`. Also (R12, found while implementing): `setAuthoringToolGrant` records event 38 and `useAuthoringTools` re-asks on it, which SC-005 needs and no task carried
- [x] T035 [US5] E2E: the GM unticks Shapes; the player's rail drops it without a reload and their `createShape` is refused; ticking it again restores both, and the shapes they drew earlier are untouched — 2026-10-07: green in the canvas slice; the test clicks the grants checkbox and waits for the server's answer, because `AuthoringToolGrantsCard` is not optimistic and Playwright's `check()` gave up first (95da29c0)

---

## Phase 7: US6 — the demo (P3)

- [x] T036 [P] [US6] Tests `apps/demo/src/backend/handlers/shapes.test.ts`: the cases of T010, T021 and T028 for the tab's viewer; `authoringTools` per viewer
- [x] T037 [US6] `apps/demo/src/backend/handlers/shapes.ts`: move the shape handlers out of `handlers.ts`; stamp `createdBy`/`updatedBy` with `viewerUser(state).id`; ownership and forced visibility; `clearShapes`, `shapeCreators`; a stored shape with no `createdBy` reads as `DEMO_USER.id`; `authoringTools` and `authoringToolGrants` per R10
- [x] T038 [US6] `apps/demo/e2e/shapes-by-players.spec.ts`: as a player, draw and fail to select the GM's shape; as the GM, clear the player's shape — 2026-10-07: green. The player's write to the GM's shape is proved refused through the demo's backend rather than by a click. It found that a guest tab got every refusal as `{}` (a structured clone of a `GraphQLError` loses `toJSON`); `tabs.ts` now posts the answer as JSON

---

## Phase 8: Account deletion

- [x] T039 Tests in `crates/thunderforge-server/src/users/shape_cleanup_tests.rs`: a player who drew in another's world deletes their account: it succeeds, their shapes are gone with `deleted` events by the world's owner, a shape they only edited stays with `updated_by = created_by`; `shapes_deleted` counts
- [x] T040 `crates/thunderforge-server/src/users/shape_cleanup.rs` called from `delete_user_data_on` before the user row goes (R9); `UserDataDeleteSummary.shapes_deleted`

---

## Phase 9: Polish and proof

- [x] T041 [P] `docs/guides/lights-and-drawings.md`: players draw by default and own their shapes; the GM's two clear actions; taking the tools away. CONTRIBUTING: `shape_authority` is the one rule for shape writes
- [x] T042 SC-003: a server test or e2e step clearing 200 shapes, timing it; record the number here — 2026-10-07: `clearing_two_hundred_shapes_is_quick`, three runs: 131 ms, 122 ms, 118 ms for the server (200 rows, 200 `deleted` events); asserts under 2 s
- [x] T043 `make lint`; `make test-rust ARGS="-p thunderforge-server"`; `cargo test -p thunderforge-engine`; `pnpm -F @thunderforge/web test`; `pnpm -F @thunderforge/demo test` and the demo e2e; `pnpm e2e:canvas` — 2026-10-07: lint clean; server 1933 passed, 6 ignored; engine 324 passed; the wasm in `dist/engine` was stale since 6595369f and was rebuilt with `build.mjs --only-wasm` before the e2e; web 841; demo unit 113; demo e2e 32 of 34; canvas 40 passed, 1 skipped. Canvas also needed `canvas-authoring.spec.ts` to pick the Shapes tool up again after Escape, which since 2026-10-06 returns to Select (95da29c0)
  - The two demo e2e failures, `demo.spec.ts:540` and `fight.spec.ts:310`, failed at HEAD without 082's demo files too, so they are not 082's; left for a separate fix. fd11e947 (UVTT import: a map import is judged by the tab that asked for it) landed alongside.
    both fixed by e433d31f
- [x] T044 `pnpm e2e:which --diff`, and run each slice it names — 2026-10-07, all exit 0, no flakes: canvas 40 passed, 1 skipped; accounts 67; engine-limits 4; engine-other 9; play-pause 13; resumable-downloads 8; rolls 18; worlds 20 — 179 passed, 1 skipped in all
- [x] T045 ~~The full suite, `node ./scripts/e2e-parallel.mjs`~~ — skipped by owner decision 2026-10-07: slices are the gate

---

## Dependencies

- Phase 1 → Phase 2 → stories. Every story needs T004 and T006; the
  engine and web work needs T007–T009.
- US3 needs US1's bridge work (T018). US4 needs US3's mutation and dialog
  (T022, T026). US5 needs only Phase 2. US6 copies settled rules, so it
  follows T011, T022 and T029. Phase 8 needs only Phase 1.
- Order of delivery: US1+US2 (MVP) → US3 → US5 → US4 → Phase 8 → US6 →
  Phase 9.

## Parallel opportunities

- T007, T008 and T009 are separate crates and apps.
- The engine (T011a–T013) beside the server (T010–T011) and web (T014–T018).
- US5 and Phase 8 beside US3/US4.

## Implementation strategy

The MVP is US1 with US2: a player must never ship drawing without the
ownership rule. US3 follows at once, since default drawing fills boards.
Phase 8 must land before the defaults reach a real instance, or players who
drew could not delete their accounts.
