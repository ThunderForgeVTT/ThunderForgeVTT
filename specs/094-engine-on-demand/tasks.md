# Tasks: The engine on demand, the dock on its own

**Input**: [spec.md](spec.md), [plan.md](plan.md)
**Prerequisites**: spec.md, plan.md
**Tests**: Playwright e2e in the new `engine-on-demand` slice; vitest for
the predicate and the replay; `pnpm verify`; `make lint`.
**Proof**: `pnpm e2e:engine-on-demand` three times, `make lint`, `pnpm
verify`, results recorded in the Proof phase. The full suite is not a gate.

## Format: [ID] [P?] [Story] Description

- **[P]**: can run in parallel with its neighbours (different files, no
  dependency on an unfinished task).
- **[Story]**: the user story the task serves (US1 to US4), or **[Setup]**
  and **[Proof]**.

## Phase 1: Probe and audit (Setup, US1)

- [ ] T001 [Setup] Add `apps/web/e2e/fixtures/engineFiles.ts` exporting
      `isEngineFile(url: string): boolean` per plan D8, with a `recordEngineRequests(page)`
      helper that returns the list of matching request URLs seen so far.
  - Modelled on `apps/landing/e2e/support/engine.ts`; paths differ, so not
    shared.
  - A vitest in `apps/web/e2e/fixtures/__tests__/engineFiles.test.ts` with
    one positive case per pattern and a negative case for a React chunk.
- [ ] T002 [Setup] Add the `engine-on-demand` slice to
      `scripts/e2e/slices.json` with own prefix `engine-on-demand-`,
      neighbours `engine-loading.spec.ts`, `player-active-scene.spec.ts`,
      `play-panels-by-role.spec.ts`, `world-layout-widths.spec.ts`,
      `demo-first-session.spec.ts` (each with its seam), and the paths plan
      D-section lists; add `e2e:engine-on-demand` and
      `e2e:engine-on-demand:integration` to root `package.json`.
  - Mark the measurement spec integration-only in the slice entry.
- [ ] T003 [P] [Setup] Audit every spec under `apps/web/e2e/` that waits for
      the canvas, the engine loader, `engine-ready`, or a `data-testid` that
      only the board renders, and list them in this task's sub-bullets with
      whether the world they create has a base map.
  - At least: `engine-loading`, `scene-live-launch`, `canvas-engine-stopped`,
    `telemetry-engine`, `resumable-downloads-engine`, `genie-play-dock-session`,
    `scene-preload`, `player-active-scene`, the `tokens`, `lighting`,
    `canvas`, `interactive` and `engine-limits` slices' specs.
  - Record the list here before Phase 2 lands (plan risk 2).
- [ ] T004 [Setup] Give `registerAndCreateWorld` in
      `apps/web/e2e/fixtures/helpers.ts` an optional `{ baseMap: true }`
      that picks the first catalogue map in the spec 088 picker, and a
      sibling `createMappedScene(page, worldId)` that adds a scene with a map
      and returns its id.
  - No new fixture file; helpers.ts stays under 1000 lines or the two
    helpers go into `fixtures/scenes.ts`.
- [ ] T005 [Setup] Pass `{ baseMap: true }` in every spec T003 listed as
      depending on the engine. Run `pnpm e2e:engine-other` and
      `pnpm e2e:scenes` once to confirm nothing changed for them (the gate
      has not landed yet; this is the baseline).
- [ ] T006 [US1] Write `apps/web/e2e/engine-on-demand-mapless.spec.ts`:
      register, create a world with no base map, open play, assert
      `recordEngineRequests` is empty at the end, `world-dock` visible, each
      `world-dock-tab-<id>` opens its panel, a roll from the dice panel shows
      a total within 2 s, `play-field-status` present (FR-013).
  - Fails on `main` (the engine is fetched). This is SC-001 and SC-002.
  - Add a second test that disables WebGL2 (`--disable-gpu` context or the
    `WEBGL_UNSUPPORTED` hook the engine-loading spec uses) and asserts no
    "unsupported" notice appears (US1 scenario 4).

## Phase 2: The gate (US1)

- [ ] T007 [US1] Add `apps/web/src/pages/world/useBoardWanted.ts` per plan
      D1: `useBoardWanted({ scene, isGameMaster })` returns
      `{ wanted, reason: "map" | "gm" | "none", askForBoard, dismissBoard }`;
      the GM request is state keyed by `scene?.id` and cleared when it
      changes.
  - vitest: mapless scene → `none`; `backgroundUrl` set → `map`; GM asks →
    `gm`; scene id changes → back to `none`; a player calling `askForBoard`
    is a no-op.
  - The hook never imports anything from game systems (FR-003).
- [ ] T008 [US1] Add `enabled: boolean` to `useEngineCanvas` and
      `useCanvasEngine`. While false the mount effect returns early,
      `engineReady` is false, `engineError` null, `loadProgress` absent,
      and the keyboard routing is not started (FR-004).
  - `useCanvasEngine`'s effect deps gain `enabled`; a flip to true runs the
    mount once; a flip back to false does not unmount (D4 handles parking in
    Phase 4).
- [ ] T009 [US1] Split `useEngineBridge` per plan D2: return `storeReady`
      (true after the first store subscription effect) and keep
      `bridgeReady` for the bind; call `bindWorldStore` only in an effect
      gated on an `enabled` argument (FR-005).
  - The cache sync effect stays on identity, untouched.
- [ ] T010 [P] [US1] Gate `useSceneContent`, `useSceneLiveSync`,
      `useScenePlay` and the `set_world` dispatch in `WorldPage` on
      `storeReady` instead of `bridgeReady` (FR-007). `useSceneTransition`
      gates its load grant on `storeReady` and keeps `engineReady` for
      `beginSceneTransition`.
  - Each hook's gating comment is updated to say why the store, not the
    engine, is the gate now, and points at D3 for the engine's side.
- [ ] T011 [US1] Wire `WorldPage`: call `useBoardWanted` with the selected
      scene record and `isGameMaster`; pass `enabled: wanted` to
      `useEngineCanvas` and `useEngineBridge`.
  - `WorldPage.tsx` stays under 1000 lines; note its line count in this
    task when done.
- [ ] T012 [US1] Add the `engine.gate` telemetry event (`wanted`, `reason`)
      to the allow-list beside `engine.load`, emitted once per predicate
      change. Run `pnpm e2e:telemetry` once (the `telemetry-engine` spec is
      the neighbour for this task).
- [ ] T013 [US1] Run `pnpm e2e:engine-on-demand` (standalone). T006 passes.
      Run `pnpm e2e:engine-other` and `pnpm e2e:scenes` to confirm T005's
      specs still see their loader and canvas on mapped worlds (SC-009).

## Phase 3: The dock alone (US1, US3)

- [ ] T014 [US1] Make `canvas` optional on `WorldLayout`
      (`apps/web/src/layouts/world-layout/WorldLayout.tsx`): with no canvas
      the dock is the body, full width, no `absolute inset-0` plate;
      `data-world-id` and the `h-screen w-screen` frame stay (FR-014).
- [ ] T015 [US1] Add a `standalone` prop to `WorldDock`: full-page panel
      width, no collapse control, same section tabs and test ids (FR-011,
      plan D6).
  - `world-layout-widths.spec.ts` is the neighbour; run `pnpm e2e:worlds`
    once after this task.
- [ ] T016 [US1] In `WorldPage`, when `!wanted` render `<WorldLayout
      worldId dock={<WorldDock standalone .../>} />` with no canvas, no tool
      rail, no board overlays, no tokens button, no level tabs, no selection
      bar, no map credit; keep the play-field notice, connectivity status
      and pause notice (FR-010, FR-012, FR-013).
  - If `WorldPage.tsx` would pass 950 lines, move the board branch's JSX
    into `apps/web/src/pages/world/WorldBoardView.tsx` first, as its own
    commit.
- [ ] T017 [US3] Add "Show the board" / "Hide the board"
      (`data-testid="world-board-toggle"`) to the settings section in
      `useDockSections` when `isGameMaster && !sceneHasMap`, calling
      `askForBoard` / `dismissBoard` (FR-020 to FR-022, plan D7).
  - Players never get the item; the item is not rendered when the scene has
    a map.
- [ ] T018 [US3] Write `apps/web/e2e/engine-on-demand-gm-board.spec.ts`:
      GM on a mapless world presses the toggle, sees the loader then the
      canvas and the tool rail; reloads and sees the dock alone; a second
      browser context as a player sees no toggle in settings (SC-005).
- [ ] T019 [US1] Extend T006 with the dock-alone assertions: no
      `game-canvas-container`, no `world-dock-collapse`, panel width equals
      the viewport width minus the tab rail; `play-panels-by-role.spec.ts`
      is the neighbour, run `pnpm e2e:combat` once.

## Phase 4: The switch (US2)

- [ ] T020 [US2] Replay on bind in `apps/web/src/engine/bevy/worldBridge.ts`
      per plan D3: after the callback is registered and before the store
      subscription, send the store's tokens, walls, lights, shapes, ambient
      and walls-hidden for the current scene through `apply_world_command`
      as upserts (FR-008).
  - vitest with a fake module: a store holding two tokens and a wall yields
    three upserts before any new dispatch; binding twice does not duplicate.
- [ ] T021 [US2] Park and unpark in `useEngineCanvas` per plan D4: when
      `enabled` goes false with `engineReady` true, hide the canvas and call
      `set_rendering(false)`; when it returns, show and `set_rendering(true)`.
      `engineReady` stays true while parked (FR-016, FR-017).
  - Plan D5: check `@thunderforge/engine`'s exports for `set_rendering`
    first. If absent, use the spec 057 `stopEngine` / `resumeEngine` path and
    record which here; if an export must be added, add it to the
    `render_pause` plugin and run `cargo check --target
    wasm32-unknown-unknown` (Constitution V).
- [ ] T022 [US2] `DiceRollerPanel`'s reveal delay reads `engineReady &&
      wanted` so a parked board does not delay a total (plan D5); pass
      `boardShown` from `WorldPage`.
- [ ] T023 [US2] Confirm the dock's `openSectionId` survives both directions
      of the switch (FR-018): it is `WorldPage` state, not layout state; if
      the standalone and board branches remount `WorldDock`, hoist it so
      they do not.
- [ ] T024 [US2] Write `apps/web/e2e/engine-on-demand-switch.spec.ts`:
      mapless world → `createMappedScene` → make it active → loader then
      canvas, `location.href` unchanged, navigation count unchanged; place
      a token through the store (dispatch via the actors panel or the
      existing token placement helper) while mapless, then after the switch
      assert it is on the board (`engine_stats` or the token status probe);
      switch back to the Starting Scene → canvas hidden, dock alone; switch
      forward → canvas back, `recordEngineRequests` holds exactly one wasm
      URL (SC-003, SC-004).
  - A second test with two contexts: the GM switches, the player's page
    brings the engine in without reload (US2 scenario 5, FR-019);
    `player-active-scene.spec.ts` is the neighbour.
- [ ] T025 [US2] Edge cases from the spec as tests in the switch spec: a
      scene event that sets `backgroundUrl` on the current scene loads the
      engine in place; one that clears it parks the engine; a GM "Show the
      board" followed by a switch to a mapped scene leaves the toggle absent.

## Phase 5: The number (US4)

- [ ] T026 [US4] Write `apps/web/e2e/engine-on-demand-measure.spec.ts`
      per plan D9: two worlds (no base map; first catalogue base map), each
      opened in a fresh context; record dock-interactive time, engine-ready
      time where applicable, total bytes and engine bytes via
      `isEngineFile`; write `marketing/engine-on-demand.json` with
      `{ commit, date, measuredBy, engineBytes, mapless, mapped }`; assert
      `mapless.engineBytes === 0`, `mapped.engineBytes > 0`,
      `mapless.dockInteractiveMs < mapped.dockInteractiveMs` (SC-006).
  - Integration-only in the slice; skipped unless `ENGINE_PROFILE=release`
    or the harness reports a release engine, because a dev wasm (335 MB
    raw per spec 087) is not the number anyone wants recorded.
- [ ] T027 [US4] Commit the generated `marketing/engine-on-demand.json`
      with the numbers the run produced, and add one line to `MVP.md`'s
      engine size section pointing at the file and this spec. Numbers are
      pasted from the file, never typed.
- [ ] T028 [P] [US4] Add a short note to `docs/guides/` only if a guide
      already describes the play view; otherwise none. Update
      `docs/CONTRIBUTING.md`'s slice list with `engine-on-demand`.

## Phase 6: Proof

- [ ] T029 [Proof] Run `pnpm e2e:engine-on-demand` three times
      (standalone), then `pnpm e2e:engine-on-demand:integration` once with
      a release engine; `make lint`; `pnpm verify`; `git diff --stat main
      -- apps/web/src/engine/world` is empty (SC-008); `scripts/check-file-length.sh`
      reports no file over 1000 lines (FR-025). Record each result here
      with the date and the commit.
  - Run 1:
  - Run 2:
  - Run 3:
  - Integration:
  - `make lint`:
  - `pnpm verify`:
  - `WorldPage.tsx` lines:
  - `marketing/engine-on-demand.json` mapless/mapped dockInteractiveMs and
    engineBytes, pasted from the file:
