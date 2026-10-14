# Implementation Plan: The engine on demand, the dock on its own

**Branch**: `094-engine-on-demand` | **Date**: 2026-10-13 | **Spec**: [spec.md](spec.md)

## Summary

Four slices of work, in order, each provable on its own:

1. **The predicate and the probe.** A `boardWanted` value in `WorldPage`,
   a shared `isEngineFile` predicate in `apps/web/e2e/fixtures/`, and a spec
   that opens a mapless world and asserts the request log holds no engine
   file. The spec fails on `main` and is the first thing to turn green.
2. **The gate.** `useEngineCanvas` takes `enabled`; `useEngineBridge` splits
   `storeReady` (immediate) from `bridgeReady` (engine bound) and only calls
   `bindWorldStore` when the board is wanted; the scene content hooks gate on
   `storeReady`; `bindWorldStore` replays the store's state into the engine
   when it binds.
3. **The dock alone.** `WorldLayout` with an optional canvas; `WorldPage`
   renders the dock as the body when the board is not wanted; the Game
   Master's "Show the board" in the settings section.
4. **The switch and the number.** Hiding and parking the engine when the
   board stops being wanted; the switch specs; the measurement spec and
   `marketing/engine-on-demand.json`.

The world store and its sync are not touched. The engine is one more
consumer, and one that may arrive late.

## Technical Context

- **Language/Version**: TypeScript 5 / React 19 / Vite (apps/web); Rust
  (edition 2024) for the engine only if `set_rendering` is not already
  exported to `apps/web` (see Decisions).
- **Primary Dependencies**: the existing `useEngineCanvas` /
  `useCanvasEngine` / `mountEngine` chain; `worldBridge.ts`;
  `WorldLayout`; `WorldDock`; `useDockSections`; Playwright with the
  repo's `fixtures/test` and `fixtures/helpers`.
- **Storage**: none. No schema change. `scene_id` stays NOT NULL.
- **Testing**: Playwright e2e in a new `engine-on-demand` slice; `pnpm
  verify` (tsc, vitest, prettier) and `make lint`.
- **Target Platform**: the web app's play route, Chromium in e2e.
- **Project Type**: web app change in `apps/web` plus one e2e fixture and
  one generated JSON under `marketing/`.
- **Performance Goals**: zero engine bytes on a mapless scene; one engine
  fetch per page across any number of switches.
- **Constraints**: no file over 1000 lines (`WorldPage.tsx` is at 879, so
  new logic goes into hooks, not into the page); nothing under
  `apps/web/src/engine/world/` changes; system-agnostic.
- **Scale/Scope**: eight files touched in `apps/web/src`, one new hook, one
  new fixture, four new specs, one slice entry, one JSON.

## Constitution and AGENTS.md check

- **I. ECS owns simulation, React owns chrome** — Passes. React decides
  whether to host the engine; it does not simulate anything in its place.
  The dock-alone view holds no board state; the store holds the scene, the
  engine renders it when present.
- **II. Plugin-modular engine** — Passes. No engine plugin changes. If
  `set_rendering` is not exported to the web app, it is exposed through the
  existing `render_pause` plugin's surface, not a new one.
- **III. Authorisation at the data boundary** — Passes. "Show the board" is
  a client-side view preference gated on the Game Master flag the page
  already has; it writes nothing. Players lack the control and, if they
  forged it, would see a blank board of a scene they can already read.
- **IV. Specs before divergent implementation** — Passes. This document.
- **V. Verify per target** — Passes. `pnpm verify` for the web app; `cargo
  check --target wasm32-unknown-unknown` only if the engine's export list
  changes (Decision D5).
- **VI. Every feature proven by its own slice** — Passes. Slice
  `engine-on-demand`: own prefix `engine-on-demand-` (four specs: mapless,
  switch, gm-board, measure); neighbours `engine-loading.spec.ts` (the
  loader must still appear on a mapped world), `player-active-scene.spec.ts`
  (a player's view follows the scene), `play-panels-by-role.spec.ts` (the
  dock sections per role), `world-layout-widths.spec.ts` (the layout frame),
  `demo-first-session.spec.ts` (the first world a newcomer opens);
  standalone half is the four own specs; the measurement spec runs in the
  integration half only. Paths: `WorldPage.tsx`, `WorldLayout.tsx`,
  `useEngineCanvas.ts`, `useEngineBridge.ts`, `useBoardWanted.ts`,
  `useCanvasEngine.ts`, `mount.ts`, `worldBridge.ts`, `WorldDock.tsx`,
  `useDockSections.tsx`, `fixtures/engineFiles.ts`. About ten minutes.
- **VII. Telemetry on, anonymous, allow-listed** — Passes with one
  addition: a `engine.gate` event (`wanted: boolean`, `reason:
  "map" | "gm" | "none"`) on the allow-list, so the dashboard can say how
  many opens skip the engine. No identifiers. e2e runs `TELEMETRY=false`.
- **AGENTS.md circular flow** — Passes. No state change bypasses the server.
  The predicate reads server-confirmed scene state from the store; the Game
  Master's request is explicitly not state (FR-021) and is held as a view
  preference the way the dock's open section is.

## Decisions (spec open choices, closed here)

- **D1 — Where the predicate lives.** A new `useBoardWanted({ scene,
  isGameMaster })` hook in `apps/web/src/pages/world/` returns
  `{ wanted, reason, askForBoard, dismissBoard }`. The GM request is
  `useState` keyed by `scene.id` and reset in an effect when the id changes
  (FR-021). `WorldPage` calls it once and passes `wanted` down. This keeps
  `WorldPage` under its ceiling and keeps the predicate testable in vitest.
- **D2 — `storeReady` versus `bridgeReady`.** `useEngineBridge` returns
  `{ worldState, storeReady, bridgeReady, allTokens }`. `storeReady` is
  true after the first store subscription effect runs. `bridgeReady` is
  true after `bindWorldStore` resolves, and that call is made only while
  `enabled` (the predicate) is true. The scene content hooks, live sync,
  scene play and the `set_world` dispatch gate on `storeReady`.
  `useSceneTransition` keeps `engineReady` for `beginSceneTransition` and
  gates the rest on `storeReady`. The comment in `useEngineBridge` about
  the fetch racing the bind stays true for the engine and is answered by D3.
- **D3 — The late engine reads the store.** `bindWorldStore` gains a replay:
  after registering the callback and before subscribing, it walks
  `worldStore.getState()` and sends `upsert_token`, `upsert_wall`,
  `upsert_light`, `upsert_shape` (and the ambient and walls-hidden flags the
  store carries) through `apply_world_command` for the current scene. It is
  idempotent: the engine's upserts are keyed by id. This closes the race the
  old gate existed for, for the engine arriving late or first.
- **D4 — Parking, not unmounting.** When `wanted` goes false with the
  engine ready, `useEngineCanvas` hides the canvas (it already owns the
  visibility) and calls `set_rendering(false)`; when `wanted` returns, it
  shows the canvas and calls `set_rendering(true)`. `engineReady` stays true
  while parked so the bridge is not rebound. No `unmountEngine`.
- **D5 — `set_rendering` in the web app.** The landing's host calls
  `set_rendering` on the engine it loads from `/demo/engine.json`; that is
  the same crate. If the export is present in `@thunderforge/engine`'s
  types, use it. If it is not, the fallback is the spec 057
  `canvas-engine-stopped` path (`stopEngine` / `resumeEngine`), which the
  overlays already understand; the task that discovers this records which
  and, if an export is needed, runs `cargo check --target
  wasm32-unknown-unknown` (Constitution V). The engine's dice are still
  thrown on a parked board, but the board is hidden; the dice panel's
  reveal delay reads `engineReady && wanted` so a parked board does not
  delay a total.
- **D6 — The dock alone is `WorldLayout` without a canvas.** `canvas`
  becomes optional. With no canvas, `WorldLayout` renders `{dock}` as a
  full-width body (`w-full h-screen`) and no `absolute inset-0` plate.
  `WorldDock` keeps its absolute-right geometry when a canvas is present;
  when it is the body, it is given a `standalone` prop that widens the
  panel to the page and drops the collapse control (there is nothing to
  collapse to). Same component, same test ids (FR-011).
- **D7 — The GM control is a settings section item.** `useDockSections`
  already builds the settings section with the Game Master's items; it adds
  "Show the board" / "Hide the board" (`world-board-toggle`) when
  `isGameMaster && !sceneHasMap`. No new section.
- **D8 — The engine predicate for tests.** `apps/web/e2e/fixtures/engineFiles.ts`
  exports `isEngineFile(url)`: pathname under `/assets/` matching
  `/engine_bg[.-].*\.wasm$/` or `/engine[.-][^/]*\.js$/`, plus anything
  under `/engine/` for the dev server. Modelled on
  `apps/landing/e2e/support/engine.ts`; not shared across apps because the
  paths differ.
- **D9 — The measurement.** `engine-on-demand-measure.spec.ts` creates two
  worlds in one run (one with no base map, one with a spec 088 base map),
  opens each in a fresh context, and uses `page.on("response")` with
  `isEngineFile` to sum bytes, `performance.timeOrigin` and a
  `world-dock-tab-chat` enabled-wait for dock-interactive time, and the
  `engine-ready` test id for engine-ready time. It writes
  `marketing/engine-on-demand.json` with `{ commit, date, measuredBy,
  mapless, mapped }` and the engine bytes measured. The file is committed
  (spec open item 2) and regenerated by the proof task whenever the engine
  changes materially; the measuredBy field names the spec so a reader can
  rerun it.
- **D10 — `engine-loading.spec.ts` creates a mapped world.** It must, or
  there is no loader to pin. `registerAndCreateWorld` gains an optional
  `baseMap` argument that picks the first catalogue map in the spec 088
  picker. Other specs that need the engine on the play view
  (`scene-live-launch`, `canvas-engine-stopped`, `telemetry-engine`,
  `resumable-downloads-engine`, `genie-play-dock-session` if it rolls on
  the board) are audited in Phase 1 and given a base map where they depend
  on the engine. That audit is a task, not an assumption.

## Project Structure

```text
specs/094-engine-on-demand/
├── spec.md
├── plan.md
└── tasks.md

apps/web/src/pages/world/
├── WorldPage.tsx            # calls useBoardWanted; passes enabled; dock-alone branch
├── useBoardWanted.ts        # new: the predicate and the GM's per-visit request
├── useEngineCanvas.ts       # enabled flag; park/unpark on wanted
├── useEngineBridge.ts       # storeReady vs bridgeReady; bind only when enabled
├── useSceneContent.ts       # gate on storeReady
├── useSceneLiveSync.ts      # gate on storeReady
├── useScenePlay.ts          # gate on storeReady
├── useSceneTransition.ts    # gate on storeReady; engineReady for the transition
└── useDockSections.tsx      # Show/Hide the board in settings (GM, mapless)

apps/web/src/engine/bevy/
├── useCanvasEngine.ts       # enabled gate on the mount effect
└── worldBridge.ts           # replay on bind (D3)

apps/web/src/layouts/world-layout/
└── WorldLayout.tsx          # canvas optional; dock as body

apps/web/src/components/world/PlayDock/
└── WorldDock.tsx            # standalone prop

apps/web/e2e/
├── fixtures/engineFiles.ts            # isEngineFile
├── fixtures/helpers.ts                # registerAndCreateWorld({ baseMap })
├── engine-on-demand-mapless.spec.ts
├── engine-on-demand-switch.spec.ts
├── engine-on-demand-gm-board.spec.ts
├── engine-on-demand-measure.spec.ts
└── engine-loading.spec.ts             # mapped world

scripts/e2e/slices.json      # engine-on-demand slice
package.json                 # e2e:engine-on-demand scripts
marketing/engine-on-demand.json
apps/web/src/telemetry/allowlist (wherever engine.load lives)  # engine.gate
```

## Risks

- **The bind race returns in a new shape.** The old `bridgeReady` gate
  existed because a fetch could dispatch before the engine listened. D3's
  replay answers it, but the replay must cover every kind the engine
  renders. The switch spec's SC-004 assertion ("every token the store holds
  is on the board") is the check; the task list names walls, lights and
  shapes too.
- **Specs that assume the engine.** Any play-view spec that creates a world
  without a base map and waits for the canvas will now wait forever. D10's
  audit is first in Phase 1 so the breakage is found before the gate lands,
  not after.
- **`WorldPage.tsx` at 879 lines.** The dock-alone branch and the predicate
  call add lines. The predicate is a hook (D1) and the branch is a short
  early return; if the page crosses 950 the overlays or the dock branch move
  into a sibling file first.
- **`set_rendering` not exported to the web app.** D5 names the fallback.
- **The measurement is noisy.** SC-006 asserts inequalities and shape, not a
  figure; the JSON carries the numbers for a reader to judge. A run on a
  loaded machine still satisfies the inequalities because the mapless path
  does strictly less.
- **Telemetry allow-list.** Adding `engine.gate` touches the `telemetry`
  slice's paths; the `telemetry-engine` spec is a neighbour for the one
  task that adds it.
