# Feature Specification: The engine on demand, the dock on its own

**Feature Branch**: `094-engine-on-demand`
**Created**: 2026-10-13
**Status**: Draft (spec, plan and tasks written together; nothing implemented)
**Input**: "The play route mounts the Bevy engine unconditionally, and the dock
is a prop of the canvas layout. A table with no map (theatre of the mind: Fate,
Blades, Roll for Shoes, a 5e session between fights) downloads the whole engine
to look at a grey rectangle, and cannot have the dock (chat, dice, sheets,
initiative) without the board. Load the engine only when the current scene has
a map, or the Game Master asks for the board; let the dock stand alone as the
whole play view when it does not; bring the engine in on a switch to a mapped
scene without a reload; leave the world store and its sync alone, the engine
is one more consumer. System-agnostic. `scene_id` stays NOT NULL."

## Why

Most of a session at many tables has no board. Fate, Blades in the Dark and
Roll for Shoes rarely want one; a 5e table between fights does not need one
either. Today every one of those tables pays the same price as a dungeon
crawl: the play route fetches and starts the whole engine before it will show
anything, and a scene with no map renders as a flat grey plate behind the dock.

The engine's size is measured, not guessed. Spec 087's result
(`specs/087-bevy-0-20/spec.md`, "What exists") records the release
`engine_bg.wasm` on Bevy 0.20 at 31,939,040 bytes raw and 5,186,179 bytes
brotli at quality 11, measured by the method in its `research.md` R18.
Spec 091's T061 (`specs/091-landing-encounter/tasks.md`, 2026-10-13) saw the
same engine served through nginx at 5,192,368 bytes on the wire. `MVP.md`
holds the earlier figure (29.83 MB raw, 4.92 MB brotli, re-measured after
spec 031 on 2026-09-01) and notes that brotli is the number to watch. There
is no engine-size file under `marketing/` on `main`;
`apps/landing/scripts/compress.mjs --marketing` writes one but it is not
committed. Five megabytes compressed, decoded into thirty megabytes of wasm
and compiled, is a wait a dice-only table should never see.

The second half is structural. `WorldLayout` takes the board as its body and
the dock as a prop, so there is no play view without a board. A table that
wants only chat, dice, sheets and initiative has nowhere to stand. Mapless
play already works (see memory `project_theatre_of_the_mind`): the Starting
Scene anchors every actor, dice and chat are world-scoped, and the dock is
usable on an empty plate. What is missing is a play view that admits it has
no board and does not fetch one.

## What exists

Read on 2026-10-13 against `main` at `da60b0fa`.

- `apps/web/src/pages/world/WorldPage.tsx` (879 lines) calls
  `useEngineCanvas({ id, canvasContainerId: "game-canvas-container", playView })`
  unconditionally. `apps/web/src/pages/world/useEngineCanvas.ts` (169 lines)
  wraps `apps/web/src/engine/bevy/useCanvasEngine.ts` (244 lines), whose
  mount effect calls `mountEngine` in `apps/web/src/engine/bevy/mount.ts`
  (108 lines): a WebGL2 check, `getWasmModule()` from
  `apps/web/src/engine/bevy/wasmModule.ts` (which imports
  `@thunderforge/engine/engine` and fetches `engine_bg.wasm` with byte
  progress through `services/downloads`), then `module.start(canvasSelector)`
  once per page. Bevy's winit inserts the `<canvas>` as a child of `<body>`;
  `useEngineCanvas` shows and hides it with a MutationObserver, and
  `startCanvasKeyboardRouting()` routes window keys to it.
- `apps/web/src/pages/world/useEngineBridge.ts` sets `bridgeReady` by calling
  `bindWorldStore(worldStore)` from `apps/web/src/engine/bevy/worldBridge.ts`,
  and `bindWorldStore` begins with `await getWasmModule()`. The world store
  is bound to the engine by fetching the engine. Every scene content hook
  (`useSceneContent`, `useSceneLiveSync`, `useScenePlay`,
  `useSceneTransition`) gates its first dispatch on `bridgeReady`, for the
  reason the file states: a fetch that resolves before the bridge listens
  updates React state but leaves the engine's resources empty. The bridge
  forwards each store command to `apply_world_command` as it is dispatched;
  it does not replay the store's current state when it binds.
- `engineReady` (from `useEngineCanvas`) gates: `setIsGameMaster` and
  `setViewerUser` in `useEngineBridge`; exploration persistence in
  `useExploration`; the dice animation in `useWorldEventStreams`, which is
  read through a ref "so a board without an engine never loads one"; the
  reduced-motion flag; `beginSceneTransition` in `useSceneTransition`; and
  the reveal delay in
  `apps/web/src/components/world/DiceRollerPanel/DiceRollerPanel.tsx`, which
  already resolves to zero when `engineReady` is false.
- `apps/web/src/layouts/world-layout/WorldLayout.tsx` (45 lines) takes
  `{ worldId, canvas, toolRail?, dock? }` and renders the canvas as an
  `absolute inset-0` body with the rail and the dock laid over it.
- `apps/web/src/components/world/PlayDock/WorldDock.tsx` (131 lines) renders
  the right rail (`data-testid="world-dock"`, tabs `world-dock-tab-<id>`,
  panels `world-dock-panel-<id>`, `world-dock-collapse`) from
  `sections: DockSection[]` built by
  `apps/web/src/pages/world/useDockSections.tsx` (164 lines): chat, actors,
  combat, system, requests, settings, help. Only the open section mounts.
- A scene's map is `SceneRecord.backgroundUrl: string | null`
  (`apps/web/src/types/scene.ts`), from the server's nullable
  `scenes.background_image_path` and `background_asset_id`. Spec 088 lets the
  Game Master pick a base map for the Starting Scene, or none.
- `world_actors.scene_id` is `UUID NOT NULL REFERENCES scenes ON DELETE
  CASCADE`; every world has a Starting Scene
  (`STARTER_SCENE_NAME` in `crates/thunderforge-server/src/graphql/mutations_worlds.rs`)
  so every actor has an anchor. Dice (`world_roll_records`), chat and combat
  are world-scoped and do not need the engine.
- The landing (specs 090 and 091) already loads its engine on demand:
  `apps/landing/src/engine/loadEngine.ts` streams it with byte progress,
  `host.ts` keeps one engine per page and switches rendering off and on with
  `set_rendering`, and `apps/landing/e2e/support/engine.ts` exports
  `isEngineFile(url)` so a test can assert that no engine file was fetched.
  `apps/web/e2e/engine-loading.spec.ts` (spec 028 US6) pins the byte-level
  progress on the play view.
- The engine package is only ever imported dynamically in `apps/web`
  (`wasmModule.ts`, `tokenStatus.ts`, `stats.ts`, `cacheStats.ts`,
  `sdkFaultProbe.ts`, `interactionProbe.ts`, `services/worldCache.ts`), so
  the gate is where those imports are reached, not in the bundler.
- Slices: `worlds` owns `WorldPage`, `WorldLayout`, `WorldDock`,
  `useDockSections` and the `pages/world` hooks; `engine-other` owns the
  `engine-` spec prefix; `scenes` owns `scene-` and `player-active-scene`.

## Decisions already made

- The engine is one more consumer of the world store. The store, the
  mutation bridges and the event sync in `apps/web/src/engine/world/` run
  exactly as they do today whether or not an engine is present. Nothing in
  `store.ts` or `sync/` changes.
- "The board is wanted" means: the current scene has a map
  (`backgroundUrl !== null`), or the Game Master has asked for the board on
  this scene for this visit. Nothing else wants it. The game system is never
  consulted.
- When the board is not wanted, the play view is the dock, full width, with
  nothing behind it. There is no grey plate and no engine fetch.
- `scene_id` stays NOT NULL. The Starting Scene stays the invisible anchor.
  A mapless table is a table whose current scene has no map, not a table
  with no scene.
- An engine that has already started is never torn down on a switch to a
  mapless scene; its canvas is hidden and its rendering switched off, as spec
  009's research and the landing's host already do. It comes back on the
  next switch to a mapped scene without a second fetch.
- The time-to-interactive comparison is measured by a test and written to a
  generated file; nobody types the number in.

## User Scenarios & Testing

### User Story 1 - A table with no map gets the dock and nothing else (Priority: P1)

A Game Master opens a world whose current scene has no map. The play view is
the dock: chat, actors, combat, the system panel, requests, settings and
help, laid out as the whole page. No engine file is requested. Dice rolled
from the dock resolve and show their total at once. A player joining the same
world sees the same.

**Why this priority**: it is the whole point. Every mapless table today pays
five megabytes and a wasm compile for nothing.

**Independent Test**: register, create a world with no base map, open play,
record every request; assert none is an engine file, the dock is visible and
each tab opens; roll from the dice panel and see the total.

**Acceptance Scenarios**:

1. **Given** a world whose current scene has `backgroundUrl === null`,
   **When** the play route opens, **Then** no request matches the engine
   predicate and `world-dock` is visible with every section's tab.
2. **Given** that view, **When** the user opens each dock tab in turn,
   **Then** each panel mounts and is usable (chat sends, the actors list
   loads, combat opens, the system panel renders the world's system content).
3. **Given** that view, **When** a d20 is rolled from the dice panel,
   **Then** the result shows without waiting for a dice animation.
4. **Given** that view on a browser without WebGL2, **When** the play route
   opens, **Then** nothing differs: the engine was never asked for, so its
   WebGL2 check never runs and no "unsupported" notice shows.

---

### User Story 2 - Switching to a mapped scene brings the engine in (Priority: P1)

The same table switches to a scene with a map. The engine loads with the
existing byte progress, the board appears with its tokens, walls, lights and
shapes, and the dock stays where it was, with its open section still open.
Nothing reloads. Switching back to a mapless scene hides the board and keeps
the engine; switching forward again does not fetch it twice.

**Why this priority**: without it, US1 would be a trap: a mapless table that
could never reach its battle map without a page reload.

**Independent Test**: open a mapless world, add a mapped scene, make it the
active scene, see the loader then the canvas, with the tokens the store
holds; count engine fetches across a mapless-mapped-mapless-mapped switch.

**Acceptance Scenarios**:

1. **Given** a mapless play view, **When** the active scene changes to one
   with a map, **Then** the engine loader appears (the `engine-loading`
   progress spec 028 pins), then the canvas, and `document.location` has not
   changed and no navigation occurred.
2. **Given** the engine has just started on that switch, **When** it binds
   to the store, **Then** every token, wall, light and shape the store
   already holds for the scene is on the board, not only those dispatched
   after the bind.
3. **Given** a mapped scene with the engine running, **When** the active
   scene changes to a mapless one, **Then** the canvas is hidden, the dock
   stands alone, and no engine teardown occurs.
4. **Given** that, **When** the active scene changes back to a mapped one,
   **Then** the board returns and the engine file was fetched exactly once
   for the page.
5. **Given** a player at the table, **When** the Game Master switches the
   active scene from a mapless to a mapped one, **Then** the player's view
   brings the engine in through the same scene event, with no reload.

---

### User Story 3 - The Game Master asks for the board on a mapless scene (Priority: P2)

A Game Master on a mapless scene wants the grid anyway: to draw a quick
sketch, to place tokens on a blank field, or to show the players distances.
A control in the dock's settings section, "Show the board", brings the
engine in for this scene for this visit. It is the Game Master's, not a
player's; it is not saved; it never changes the scene.

**Why this priority**: it keeps the gate from being a wall. A table that
sometimes wants a blank grid must not have to upload a map to get one.

**Independent Test**: open a mapless world as the Game Master, press "Show
the board", see the loader then the canvas; reload and see the dock alone
again; as a player, confirm the control is absent.

**Acceptance Scenarios**:

1. **Given** a Game Master on a mapless scene, **When** they press "Show the
   board", **Then** the engine loads and the blank board renders behind the
   dock with the tool rail.
2. **Given** that, **When** the page reloads, **Then** the play view is the
   dock alone: the request was for the visit, not the scene.
3. **Given** a player on the same mapless scene, **When** they open the
   settings section, **Then** there is no "Show the board" control.
4. **Given** the Game Master has shown the board, **When** they press "Hide
   the board", **Then** the canvas is hidden and the dock stands alone; the
   engine is kept.

---

### User Story 4 - The comparison is measured, not assumed (Priority: P2)

A test opens the same world twice, once with the current scene mapless and
once mapped, and records time to a usable dock and bytes on the wire for
each. The numbers go into a generated file so the claim "a mapless table
waits for nothing" is something a reader can check against the build that
made it.

**Why this priority**: the project records performance as measured numbers
in generated files (`marketing/*.json`), and this feature's whole case is a
number.

**Independent Test**: run the measurement spec; the file exists, carries both
timings and both byte counts, the mapless byte count holds no engine bytes,
and the mapless dock-interactive time is lower than the mapped one.

**Acceptance Scenarios**:

1. **Given** a fresh browser context, **When** a mapless world's play route
   opens, **Then** the test records the time from navigation to the first
   dock tab being clickable and the sum of response bytes.
2. **Given** the same, **When** a mapped world's play route opens, **Then**
   the test records the time to the same tab and to `engineReady`, and the
   sum of response bytes including the engine.
3. **Given** both runs, **When** the spec finishes, **Then**
   `marketing/engine-on-demand.json` holds both records, the engine bytes
   measured, the commit and the date, and the mapless record's engine bytes
   are zero.

---

### Edge Cases

- A map is added to the current scene while the table is open (a scene
  event with a new `backgroundUrl`). The board becomes wanted; the engine
  loads in place, as on a switch.
- A map is removed from the current scene while the engine is running. The
  board stops being wanted; the canvas hides; the engine is kept.
- The engine fails to load on a switch (no WebGL2, a fetch error). The
  existing `WorldBoardOverlays` error and retry show in the board area; the
  dock stays usable throughout. A mapless table never sees this.
- A scene with levels (spec 022) whose current level has no map but another
  level does: the predicate reads the scene's map, not a level's, so the
  board is wanted. Levels are out of scope for the predicate.
- The play-field companion (spec 036) and the staging route are unchanged:
  staging has always loaded its own engine for the Game Master's layout work
  and is not the play route.
- A player whose actor is on the Starting Scene while the Game Master is on
  another: the active scene is the world's, per spec `player-active-scene`;
  the predicate reads that scene.
- `prefers-reduced-motion`, `saveData` and low `deviceMemory` do not change
  the predicate here. The landing holds its engine back on those; the play
  route follows the scene, because the board is the game when there is one.
- The Game Master's "Show the board" on a mapless scene and then a switch to
  a mapped scene: the switch wins; the per-visit request is cleared when
  the scene changes.

## Requirements

### Functional Requirements

**The gate**

- **FR-001**: The play route MUST NOT request any engine file (the wasm, the
  engine's JS chunk, or the engine's assets) while the board is not wanted.
- **FR-002**: The board MUST be wanted when, and only when, the current
  scene's `backgroundUrl` is non-null, or the Game Master has asked for the
  board on the current scene during this visit.
- **FR-003**: The predicate MUST NOT read the world's game system, the
  system's content, or any per-system setting.
- **FR-004**: `useEngineCanvas` MUST accept an `enabled` flag; while false,
  its mount effect returns without calling `mountEngine`, `engineReady`
  stays false, `engineError` stays null and `loadProgress` is absent.
- **FR-005**: `bindWorldStore` MUST NOT be called while the board is not
  wanted, because it fetches the engine. The store MUST be usable by every
  dock section and every sync hook before, and without, the bind.

**The store without an engine**

- **FR-006**: The world store, the mutation bridges and the event sync
  (`apps/web/src/engine/world/store.ts`, `sync/`) MUST run unchanged whether
  or not the engine is present. No file under `apps/web/src/engine/world/`
  changes for this spec.
- **FR-007**: The scene content hooks (`useSceneContent`, `useSceneLiveSync`,
  `useScenePlay`, `useSceneTransition`) MUST dispatch into the store on a
  mapless scene so the dock's actors, combat and chat see the scene's state.
  They MUST gate on a store-ready signal that does not depend on the engine.
- **FR-008**: When the engine binds to a store that already holds state, the
  bind MUST replay the store's tokens, walls, lights, shapes and ambient
  into the engine before forwarding new commands, so a late engine shows
  what the store shows.
- **FR-009**: Every `engineReady` consumer MUST behave correctly with
  `engineReady === false` for the life of the page: dice resolve with no
  reveal delay, exploration persistence waits, reduced motion is not sent,
  the Game Master flag is sent on the first ready.

**The dock alone**

- **FR-010**: When the board is not wanted, the play view MUST render the
  dock as the whole page: full width, every section available, no canvas
  container, no tool rail, no grey plate, no engine overlays.
- **FR-011**: The dock-alone view MUST use the same `WorldDock` component,
  the same `DockSection` list from `useDockSections`, the same test ids
  (`world-dock`, `world-dock-tab-<id>`, `world-dock-panel-<id>`,
  `world-dock-collapse`) and the same open-section state as the board view.
- **FR-012**: The dock-alone view MUST reach the dice panel, the chat, the
  actors (with sheets), combat, the system panel, requests, settings and help
  with no difference in behaviour from the board view, other than the
  absence of board-only controls (tokens button, token panel, tool rail,
  level tabs, selection bar, map credit).
- **FR-013**: The dock-alone view MUST keep the play-field claim notice
  (`play-field-status`, spec 036), the connectivity status and the pause
  notice (spec 051), because they are about the table, not the board.
- **FR-014**: `WorldLayout` MUST make the canvas optional; with no canvas it
  renders the dock as the body. It MUST keep `data-world-id` and the
  `h-screen w-screen` frame.

**Switching**

- **FR-015**: When the board becomes wanted on an open page (a scene switch,
  a scene event adding a map, or the Game Master's request), the engine MUST
  load in place, without a navigation or reload, with the byte progress spec
  028 pins.
- **FR-016**: When the board stops being wanted on an open page, the canvas
  MUST be hidden and rendering switched off (`set_rendering(false)` or the
  equivalent the engine exposes); the engine MUST NOT be unmounted.
- **FR-017**: Across any number of switches on one page, the engine file
  MUST be fetched at most once.
- **FR-018**: The dock's open section MUST survive a switch in either
  direction.
- **FR-019**: A player's view MUST follow the active scene's map through the
  existing scene events; no new event type or subscription is added.

**The Game Master's request**

- **FR-020**: The settings section MUST offer a Game Master a "Show the
  board" control on a mapless scene, and a "Hide the board" control once
  shown. Players MUST NOT see either.
- **FR-021**: The request MUST be per visit and per scene: held in page
  state, cleared on a scene change, never persisted, never sent to the
  server, never written into the scene.
- **FR-022**: The request MUST NOT change `backgroundUrl` or any scene
  field; it only sets the predicate's second term.

**Measurement**

- **FR-023**: A measurement spec MUST record, for a mapless and a mapped
  open of comparable worlds: time from navigation to the first dock tab
  being clickable, time to `engineReady` where applicable, total response
  bytes, and engine response bytes, and write them with the commit and date
  to `marketing/engine-on-demand.json`.
- **FR-024**: The measurement MUST use a predicate for engine files modelled
  on the landing's `isEngineFile`, written for the web app's asset paths
  (`engine_bg*.wasm` and the `@thunderforge/engine` JS chunk under
  `/assets/`), exported from `apps/web/e2e/fixtures/` so every spec in the
  slice shares it.
- **FR-025**: Every file touched or added MUST stay under 1000 lines.

### Key Entities

- **Board wanted**: a boolean derived in `WorldPage` from the current
  scene's `backgroundUrl` and the Game Master's per-visit request. It is the
  only input to the engine gate.
- **Store ready**: the signal the scene content hooks gate on, true as soon
  as the world store exists for this page. It replaces `bridgeReady` as
  their gate; `bridgeReady` keeps meaning "the engine is bound".
- **Engine host state**: `off` (never asked for), `loading` (asked for,
  progress visible), `on` (ready, canvas shown), `parked` (ready, canvas
  hidden, rendering off), `failed` (error and retry shown). Derived from
  `useEngineCanvas` plus the predicate; not a new state machine.
- **The dock-alone view**: `WorldLayout` with no canvas; the dock is the
  body.
- **The measurement record**: `{ commit, date, mapless: { dockInteractiveMs,
  bytes, engineBytes: 0 }, mapped: { dockInteractiveMs, engineReadyMs, bytes,
  engineBytes } }` in `marketing/engine-on-demand.json`.

## Success Criteria

### Measurable Outcomes

- **SC-001**: On a mapless scene, zero requests match the engine predicate
  from navigation until the test ends, measured by the request log in the
  `engine-on-demand-mapless` spec.
- **SC-002**: On a mapless scene, every dock tab opens and its panel is
  visible, and a roll from the dice panel shows a total within 2 s of the
  click, in the same spec.
- **SC-003**: On a switch from a mapless to a mapped scene, `engineReady`
  becomes true, the canvas is visible, and the page's navigation count is
  unchanged, within the `engine-loading` budget spec 028 already pins.
- **SC-004**: Across mapless-mapped-mapless-mapped on one page, the engine
  wasm is requested exactly once, and after the second return every token
  the store holds is on the board, measured in the `engine-on-demand-switch`
  spec.
- **SC-005**: The Game Master's "Show the board" brings `engineReady` true
  on a mapless scene; a reload returns the dock alone; a player's settings
  section has no such control. Measured in the `engine-on-demand-gm-board`
  spec.
- **SC-006**: `marketing/engine-on-demand.json` is written by the
  `engine-on-demand-measure` spec with `mapless.engineBytes === 0`,
  `mapped.engineBytes > 0`, and `mapless.dockInteractiveMs <
  mapped.dockInteractiveMs`. The actual numbers are whatever the run
  measures; the spec asserts the shape and the inequalities, not a figure.
- **SC-007**: `useEngineBridge`, `useSceneContent`, `useSceneLiveSync`,
  `useScenePlay` and `useSceneTransition` never call `getWasmModule` or
  `bindWorldStore` on a mapless scene, proven by SC-001's request log (the
  wasm would be fetched if they did).
- **SC-008**: No file under `apps/web/src/engine/world/` differs from `main`
  at the end of the work (`git diff --stat main -- apps/web/src/engine/world`
  is empty).
- **SC-009**: Every existing spec in the `worlds`, `scenes` and
  `engine-other` slices still passes; `engine-loading.spec.ts` in
  particular, which opens a world that must now be created with a base map
  to see the loader.

### Proof

The feature is proven with the `engine-on-demand` slice (new; see plan.md),
plus `make lint` and `pnpm verify`. The full e2e suite is not a gate.

## Assumptions

- `backgroundUrl` is the only thing that makes a scene "have a map". A
  scene whose map is a solid colour or a grid with no image is mapless and
  gets the dock alone until the Game Master asks for the board.
- `set_rendering` or an equivalent is exposed by the engine's wasm API to
  `apps/web` as it is to the landing's host; if it is not, hiding the canvas
  and pausing through the existing `canvas-engine-stopped` path (spec 057)
  is the fallback, and the plan says which.
- The `engine-loading` spec's world creation can be given a base map through
  the spec 088 picker in `registerAndCreateWorld` or a sibling helper,
  without a new fixture.
- `useDockSections` needs the Game Master flag it already has to add the
  "Show the board" control to the settings section.

## Out of scope

- Per-collection or per-plugin lazy loading inside the engine (wasm has no
  code splitting; `MVP.md` tracks it as open).
- Changing what a mapless scene *is* on the server. No schema change, no
  nullable `scene_id`, no "theatre of the mind" scene kind.
- A different dock for mapless tables. The dock is the dock.
- The staging route, the landing, the companion and the play-pause page.
- Holding the engine back on `saveData` or low memory on the play route.
- Any change under `apps/web/src/engine/world/`.

## Open items

- Whether the replay on bind (FR-008) belongs in `bindWorldStore` or in a
  `replayWorldState` the page calls right after. The plan picks the bridge
  (the engine is a consumer; a consumer that arrives late reads the store).
- Whether `marketing/engine-on-demand.json` is committed or generated into
  `marketing/` and gitignored like `landing-engine.json`. The plan commits
  it, because the number is the case for the feature, and names the task
  that regenerates it.
