---

description: "Task list for feature implementation"
---

# Tasks: Scene Management Overhaul

**Input**: Design documents from `/specs/022-scene-management-overhaul/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/scene-management.graphql.md, quickstart.md

**Tests**: Included — this project's established convention (every prior spec) pairs backend `cargo test` resolver coverage with a Playwright e2e spec per user story; quickstart.md's "Automated coverage expectations" section already calls these out explicitly.

**Organization**: Tasks are grouped by user story (P1-P4 from spec.md) so each can be implemented, tested, and shipped independently.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1-US4)

## Path Conventions

Existing monorepo layout: `src/server/` (Rust/GraphQL backend + Diesel migrations), `src/engine/` (Bevy/WASM), `apps/web/` (React frontend + Playwright e2e under `apps/web/e2e/`).

---

## Phase 1: Setup

**Purpose**: Satisfy Constitution Principle IV before the architecturally-significant part of this feature (server-authoritative active scene) begins.

- [X] T001 Author an ADR at `docs/adrs/<next-number>-server-authoritative-active-scene.md` documenting the decision to introduce `worlds.active_scene_id` and extend the existing `world_events`/WebSocket subscription transport to broadcast live scene switches (per plan.md's Constitution Check and research.md §6) — record the alternatives considered (polling, a dedicated channel) and why the existing transport was reused.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Schema, generated types, and the Scenes section's navigation/route shell that every user story's UI hangs off.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T002 Create Diesel migration `src/server/migrations/<ts>_add_scene_summary_and_hidden/{up,down}.sql` adding `scenes.summary_markdown TEXT`, `scenes.summary_rendered_html TEXT`, `scenes.hidden BOOLEAN NOT NULL DEFAULT true`, `scenes.preview_asset_id UUID NULL` (per data-model.md's Scene table)
- [X] T003 [P] Create Diesel migration `src/server/migrations/<ts>_add_world_scene_settings/{up,down}.sql` adding `worlds.default_scene_grid_type TEXT NOT NULL DEFAULT 'square' CHECK (default_scene_grid_type IN ('square','hex','gridless'))` (reusing the exact value set `scenes.grid_type` already enforces, see the corrected research.md §4) and `worlds.active_scene_id UUID NULL` (FK to `scenes(scene_id)`) (per data-model.md's World table)
- [X] T004 Run `diesel migration run` and update generated `src/server/src/schema.rs` for both `scenes` and `worlds` tables
- [X] T005 Update `Scene`/`NewScene`/`SceneUpdate` structs in `src/server/src/models.rs` with the four new scene fields; update `World` struct with `default_scene_grid_type` and `active_scene_id`
- [X] T006 [P] Add `summaryMarkdown`, `summaryRenderedHtml`, `hidden`, `previewUrl` (computed, `format!("/scene-assets/{}/thumb", preview_asset_id)` or null) to `GraphQLScene` and its `From<Scene>` in `src/server/src/graphql.rs` (or `graphql/types.rs`, matching where `GraphQLScene` already lives)
- [X] T007 [P] Add `defaultSceneGridType`, `activeSceneId` to `GraphQLWorld` and its `From<World>` in the same GraphQL types module
- [X] T008 [P] Add the four new fields to `SceneRecord` in `apps/web/src/types/scene.ts`, and `defaultSceneGridType`/`activeSceneId` to `WorldRecord` in `apps/web/src/types/world.ts`
- [X] T009 [P] Add `summaryMarkdown`, `summaryRenderedHtml`, `hidden`, `previewUrl` to `SCENE_FIELDS` in `apps/web/src/api/scenes.ts`, and `defaultSceneGridType`, `activeSceneId` to `WORLD_FIELDS` in `apps/web/src/api/world.ts`
- [X] T010 Add a "Scenes" category to `WorldSidebarNav`'s category list in `apps/web/src/layouts/world-layout/WorldSidebarNav.tsx`, positioned between Session Setup and NPCs, linking to `/world/:id/scenes`
- [X] T011 Add two routes in `apps/web/src/routes/AppRoutes.tsx`: `/world/:id/scenes` (list) → new `apps/web/src/pages/world/scenes/ScenesRoutePage.tsx`, and `/world/:id/scenes/:sceneId` (detail) → new `apps/web/src/pages/world/scenes/SceneDetailRoutePage.tsx` (both fetch `world`+`isGm` like `WorldCompendiumRoutePage.tsx`, wrap content in `WorldSectionShell`). Per the mid-implementation spec update (FR-001a): every scene already has its own persistent id (`sceneId`, pre-existing) — this detail route is what gives that id a real, addressable "gateway" page, mirroring the existing actor/lore/item detail-route convention, so GM edit actions and the player read-only view both operate on one linkable scene record instead of only an inline list/modal.
- [X] T012 Create `apps/web/src/pages/world/scenes/ScenesPage.tsx` skeleton (list view, branches on `isGm`) and `apps/web/src/pages/world/scenes/SceneDetailPage.tsx` skeleton (detail view for one `sceneId`, also branches on `isGm`) — both fleshed out in US1/US2 below

**Checkpoint**: Schema exists, types are wired end-to-end, and the Scenes section is reachable (even if empty) — user story phases can now proceed independently.

---

## Phase 3: User Story 1 - GM manages scenes from a dedicated Scenes section (Priority: P1) 🎯 MVP

**Goal**: GM can create a scene, import a dd2vtt map, write/save a Markdown summary, toggle hidden, and Launch — all from the new Scenes section; Session Setup no longer has scene controls.

**Independent Test**: Per spec.md — GM navigates to Scenes, creates a scene, imports a dd2vtt file, writes+saves a summary, toggles hidden/visible, all without touching Session Setup.

- [X] T013 [US1] Add `updateSceneHidden(sceneId, hidden)` mutation in `src/server/src/graphql.rs` (`SceneMutation`) — GM/Owner-only (reuse the existing owner-check pattern from `updateScene`), per contracts/scene-management.graphql.md
- [X] T014 [US1] Extend `updateScene`'s input/impl to accept `summaryMarkdown`, rendering `summaryRenderedHtml` via the existing lore Markdown pipeline (`src/server/src/markdown/`) on write, in `src/server/src/graphql.rs` + `input_types.rs`
- [X] T015 [US1] Add `launchScene(worldId, sceneId)` mutation: validates the scene belongs to the world, sets `worlds.active_scene_id`, and calls `record_world_event` with a new event code (next free value in `src/server/src/world_events.rs`'s existing 1-5/10-15 catalog) carrying `{ worldId, sceneId }` — GM/Owner-only
- [X] T016 [P] [US1] `cargo test` coverage in `src/server/src/graphql.rs` (or its test module) for: `updateSceneHidden` authorization + effect, `launchScene` authorization + event emission + cross-world scene rejection, `updateScene` summary render-on-write
- [X] T017 [P] [US1] Add `updateScene`, `updateSceneHidden`, `launchScene` wrapper functions to `apps/web/src/api/scenes.ts`
- [X] T018 [US1] Build `apps/web/src/pages/world/scenes/ScenesGmView.tsx`: lists all scenes (incl. hidden) with name and a hidden badge; each row links to `/world/:id/scenes/:sceneId` (the detail gateway, T011); a "New scene" form calling `createScene` navigates to the new scene's detail route on success
- [X] T019 [US1] Build the GM branch of `SceneDetailPage.tsx`: hidden-toggle switch (`updateSceneHidden`) and a "Launch" button (`launchScene`) for this one scene, plus its rendered summary
- [X] T020 [US1] Build `apps/web/src/pages/world/scenes/SceneSummaryEditor.tsx` — CodeMirror Markdown editor reusing the same `markdown()`/theme/basicSetup configuration as `apps/web/src/pages/world/lore/LoreMarkdownEditor.tsx`, but without its lore-specific `[[link]]` autocomplete and paste-image-upload extensions (scenes don't need those); saves via `updateScene({ summaryMarkdown })`; mounted in `SceneDetailPage.tsx`'s GM branch
- [X] T021 [US1] Wire dd2vtt import into `SceneDetailPage.tsx`'s GM branch by mounting/adapting the existing `apps/web/src/components/canvas-tools/MapImportTool/MapImportTool.tsx` against this route's `sceneId` (reuses the existing `POST /api/scenes/{scene_id}/import/uvtt` endpoint — no backend change)
- [X] T022 [US1] Remove `SceneSwitcher` and all scene creation/management UI from `apps/web/src/layouts/world-layout/WorldStagingPage.tsx` (FR-002); remove now-unused `scenes`/`sceneId`/`onSceneChange`/`onSceneCreated` props from `WorldStagingPage`, `WorldStagingRoutePage.tsx`, and `getScenes` call there if nothing else on that page needs it
- [X] T023 [US1] In `apps/web/src/pages/world/WorldPage.tsx`, change the initial scene selection to read `world.activeSceneId` (falling back to `null`, not "the world's first scene") so Play shows the empty/unloaded state until a GM has launched something (FR-002d)
- [X] T024 [US1] Add a new `applySceneWorldEvent`-style handler alongside the existing wall/token/light/shape handlers in `apps/web/src/engine/world/sync/` and wire it into `WorldPage.tsx`'s existing `subscribeToWorldEvents(id)` loop: on the new scene-launch event, call `setSelectedSceneId(sceneId)` (research.md §6) — this is what makes Launch live-unload/load for everyone already in Play (FR-002b)
- [X] T025 [P] [US1] Playwright e2e `apps/web/e2e/scene-management.spec.ts`: GM creates a scene, imports a dd2vtt file, writes+saves a summary, toggles hidden, and Launches it — asserting Session Setup shows no scene controls (quickstart.md §2)
- [X] T026 [P] [US1] Playwright e2e (same file or a second `apps/web/e2e/scene-live-launch.spec.ts`) using two `browser.newContext()` pages (pattern from `system-settings.spec.ts`/`world-compendium.spec.ts`): both open `/world/:id/play` with nothing launched (assert empty canvas state, not an error), GM Launches scene A then scene B, assert both tabs reflect each switch without manual rejoin (quickstart.md §4, SC-006)

**Checkpoint**: User Story 1 is independently functional — GM has a full scene-management + launch workflow.

---

## Phase 4: User Story 2 - Players browse and preview visible scenes (Priority: P2)

**Goal**: Non-GM members see only non-hidden scenes in a table, and can open a detail view with rendered summary + preview thumbnail.

**Independent Test**: Per spec.md — non-GM member opens Scenes, confirms hidden scenes are absent, clicks a visible row to see summary + thumbnail.

- [X] T027 [US2] Add GM-vs-player branching to the `scenes(worldId)` query resolver in `src/server/src/graphql/queries/scene.rs`: players receive only `hidden = false` rows, GM/Owner receive all (mirror the existing `shapes.visible_to_players` branching pattern)
- [X] T028 [US2] Reuse `src/server/src/storage/transcode.rs`'s resize+WebP-encode helpers to generate a scene preview (max-dimension capped, per research.md §5) whenever a scene's background image is set: wired into the dd2vtt import path (`src/server/src/map_import/mod.rs`, alongside `save_background_image`, via new `save_scene_preview_image`), storing the result in a new `scene_preview_images` table and setting `scenes.preview_asset_id`. **Scope cut**: the plain canvas-image-upload path (`graphql/mutations_assets.rs::upload_canvas_image_impl`) does NOT set `scenes.background_asset_id` at all today (verified — it only inserts a `canvas_image_assets` row), so it isn't actually the mechanism that sets "the scene's background" the way dd2vtt import is; left unwired pending a decision on whether/how that path should update the scene's background at all, which is a pre-existing gap outside this feature's scope.
- [X] T029 [US2] Add a scene-preview serve route (mirroring `src/server/src/lore_assets_serve.rs`) at e.g. `/scene-assets/{asset_id}/thumb`, mounted in `src/server/src/main.rs`
- [X] T030 [P] [US2] `cargo test` coverage for: `scenes` query hidden-filtering (GM sees all, player sees only non-hidden), and preview generation being triggered on background-image-set (unit-test the transcode call site if practical, or assert `preview_asset_id` is populated post-import)
- [X] T031 [US2] Implementation note: delivered as the non-GM branch of `ScenesPage.tsx` (T018) directly — a table of the (already server-filtered) visible scenes, rather than a separate `ScenesPlayerTable.tsx`, since the GM/player table markup differs only by one extra column (visibility badge). Clicking a row navigates to `/world/:id/scenes/:sceneId` (the same detail gateway T011/T019 use, not a separate player-only route)
- [X] T032 [US2] Build the non-GM branch of `SceneDetailPage.tsx`: read-only `LoreMarkdownRenderer`-rendered `summaryRenderedHtml` and the `previewUrl` image, with a graceful placeholder (per FR-013) when either is null; wire `ScenesPlayerTable` into `ScenesPage.tsx`'s non-GM branch (replacing the Phase 2 placeholder)
- [X] T033 [P] [US2] Playwright e2e `apps/web/e2e/scene-player-browsing.spec.ts`: two-context test (GM + player) — GM creates/unhides one scene and leaves another hidden; player sees only the visible one in the table, opens its detail view, sees summary + thumbnail; GM re-hides it and the player's refreshed table no longer shows it (quickstart.md §3)

**Checkpoint**: User Stories 1 AND 2 both work independently and together.

---

## Phase 5: User Story 3 - New scenes default to the world's configured grid type (Priority: P3)

**Goal**: A world-level default grid type (None/Squares/Hexagons) seeds every newly created scene; "None" is a real gridless mode (no snap, pixel measurement).

**Independent Test**: Per spec.md — set the world's default to Hexagons, create a scene, confirm it starts with a Hexagons grid without manual selection.

- [X] T034 [US3] Add `updateWorldDefaultSceneGridType(worldId, gridType)` mutation in `src/server/src/graphql.rs`, mirroring `update_world_genie_resource_carryover_impl`'s shape exactly (GM/Owner-gated via `is_dm_of_world`, validates `gridType ∈ {"gridless","square","hex"}`)
- [X] T035 [US3] Update `createScene`'s impl in `src/server/src/graphql.rs` to read `worlds.default_scene_grid_type` when `input.gridType` is omitted, instead of always defaulting to `"square"` — the existing `scenes_grid_type_check` CHECK constraint already rejects any value outside `('square','hex','gridless')` when a caller passes `gridType` explicitly, so no new validation code is needed there (research.md §4, corrected)
- [X] T036 [P] [US3] `cargo test` coverage for: `updateWorldDefaultSceneGridType` authorization + persistence, `createScene` inheriting the world default when `gridType` is omitted, and rejection of an invalid `gridType` value on both mutations
- [X] T037 [P] [US3] Add `updateWorldDefaultSceneGridType` wrapper to `apps/web/src/api/world.ts`
- [X] T038 [US3] Add a "Default Scene Grid Type" `Select` (None/Squares/Hexagons) to `apps/web/src/pages/world/settings/WorldSystemSettingsPage.tsx`, calling `updateWorldDefaultSceneGridType` on change. Implemented together with T041/T042 (same card, one pass).
- [X] T039 [US3] **Verification only, no new engine code**: confirm `GridType::Gridless` (which `"gridless"` already deserializes to) already disables grid rendering (`src/engine/src/plugins/grid.rs`) and token snap-to-grid (`src/engine/src/movement.rs::apply_grid_snapping`) end-to-end for a scene created via the new "None" option — exercise it manually in a running dev instance rather than writing new engine logic (research.md §4, corrected: this behavior already shipped in spec 018). No measurement/ruler tool exists yet in this codebase, so the "raw pixel distances" half of the Clarification has nothing to implement today — note this in the task's completion, don't build a measurement tool as part of this feature.
- [X] T040 [P] [US3] Playwright e2e `apps/web/e2e/scene-default-grid-type.spec.ts`: GM sets world default to Hexagons, creates a scene, confirms its grid type is Hexagons without explicit selection; sets default to None, creates another scene, confirms no grid/snap and pixel-based measurement in Play (quickstart.md §1 tail, §5)

**Checkpoint**: User Stories 1-3 all work independently and together.

---

## Phase 6: User Story 4 - System Settings page is relabeled and gains the default grid type control (Priority: P4)

**Goal**: The System Settings card heading reads "System Settings," the game-system picker is labeled "Change System," and the Default Scene Grid Type control (built in US3) sits alongside it with final copy.

**Independent Test**: Per spec.md — open System Settings, confirm heading text, picker label, and grid-type control presence.

- [X] T041 [US4] In `apps/web/src/pages/world/settings/WorldSystemSettingsPage.tsx`, rename the card heading from `{activeManifest ? "Change system" : "Assign a system"}` to a static "System Settings", and add an explicit "Change System" label to the existing game-system `Select` (e.g. via `Field`/`Label`, matching how `CreateWorldPage.tsx`'s "Game system" field is labeled)
- [X] T042 [US4] Confirm/adjust the Default Scene Grid Type control's (T038) final label and placement under the renamed "System Settings" heading per spec.md User Story 4's acceptance scenarios
- [X] T043 [P] [US4] Update `apps/web/e2e/system-settings.spec.ts` assertions that check the old heading text (`"Currently using"`/card structure) if any now-stale text is asserted, and add an assertion for the new "System Settings" heading, "Change System" label, and Default Scene Grid Type control's presence
- [X] T044 [P] [US4] Playwright e2e coverage (can extend `system-settings.spec.ts` or `scene-default-grid-type.spec.ts`) asserting the exact heading/label text from spec.md's User Story 4 acceptance scenarios

**Checkpoint**: All four user stories complete and independently verified.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Fix the ripple effects of removing scene selection from Session Setup, and final verification.

- [X] T045 Audit and update every existing Playwright spec that currently selects/creates a scene via `WorldStagingPage`'s old `SceneSwitcher` before clicking Play — at minimum check `apps/web/e2e/world-staging-route.spec.ts`, `gm-staging-page.spec.ts`, `dice-roll.spec.ts`, `canvas-authoring.spec.ts`, `token-authoring.spec.ts`, `map-editor-tooling.spec.ts`, `canvas-asset-paste.spec.ts`, and the `genie-*.spec.ts` files that reach Play — each needs to switch to the new Scenes-section Launch flow (or a shared `launchScene`-calling test helper) as its path into Play
- [X] T046 [P] Add a shared `launchScene` Playwright helper (e.g. in `apps/web/e2e/fixtures/helpers.ts`, alongside the existing `playButton`-style helpers) so the specs updated in T045 don't each hand-roll the Scenes-section navigation
- [X] T047 Run `cargo check` and `cargo test` for `src/server`, `cargo check --target wasm32-unknown-unknown` for `src/engine` if T039 touched it, and `tsc`/build for `apps/web`, resolving any new warnings introduced by this feature (Constitution Principle V) — pre-existing warnings/failures unrelated to this feature are not blocking
- [X] T048 Full quickstart.md walkthrough (all 5 sections) against a running dev instance before calling the feature done

---

## Dependencies & Execution Order

- **Setup (Phase 1)** → **Foundational (Phase 2)**: strictly sequential; Foundational blocks every user story.
- **User Story 1 (Phase 3)**: depends only on Foundational. This is the MVP — deliverable and demoable alone.
- **User Story 2 (Phase 4)**: depends on Foundational (schema) and benefits from US1 existing (scenes to browse) but its own tasks (query filtering, thumbnails, player UI) don't modify US1's files — can be built in parallel by a second engineer once Foundational lands, though realistically sequenced after US1 for a believable demo.
- **User Story 3 (Phase 5)**: depends on Foundational; independent of US1/US2's files (touches `createScene`'s default-application + System Settings + engine grid systems). Can run in parallel with US2.
- **User Story 4 (Phase 6)**: depends on US3 (T038's control must exist before T041/T042 can finalize its placement) — the one real cross-story dependency in this feature, called out in spec.md itself.
- **Polish (Phase 7)**: depends on all of Phase 3 (T022's removal is what breaks the other specs) — T045 specifically cannot start meaningfully before US1 is done.

## Parallel Execution Examples

- Within Foundational: T006, T007, T008, T009 are all `[P]` — independent files once T004/T005 land.
- Within US1: T016 (server tests) and T017 (frontend API wrappers) are `[P]` once T013-T015 land; T025/T026 (e2e) are `[P]` with each other but depend on the whole US1 implementation set.
- Across stories: once Foundational is done, US3's Phase 5 (T034-T040) can proceed in parallel with US2's Phase 4 (T027-T033) — different files, no shared mutations.

## Implementation Strategy

**MVP = User Story 1 alone** (Phase 1 → 2 → 3): a GM gets a real scene-management home with create/import/summary/hidden/Launch, and Play becomes scene-selection-driven and live-syncable. This is independently demoable and delivers the bulk of the user-facing value described in the original request.

Recommended incremental delivery: Setup → Foundational → US1 (MVP checkpoint) → US2 (player-facing payoff) → US3 (grid-type convenience + the engine work) → US4 (final polish, small) → Polish (fix the e2e ripple + final verification pass).
