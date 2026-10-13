# Tasks: One Room on the Front Page, Played on the Real Engine

**Input**: Design documents from `specs/091-landing-encounter/`
**Prerequisites**: spec.md, plan.md

**Tests**: Every module in `packages/encounter` has its unit test written
and seen failing before the code. Those modules are:

- the wall rule (T011);
- the map (T013);
- the referee (T015);
- the words (T017);
- the engine scene (T018);
- the engine events (T019).

Every e2e is written against the spec's scenarios, and is seen failing or
finding something before the page change it proves.

**Proof**: The `landing` slice is the proof (T060). Its standalone command
gains `pnpm -F @thunderforge/encounter check`. The demo e2e runs only if
the demo's build changes. The full suite is never used as a gate.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel with the other tasks marked [P] (a
  different file, and no unfinished dependency).
- **[Story]**: one of:
  - US1: the room on the real engine;
  - US2: every goal without the canvas;
  - US3: the tray and the room share one table.

---

## Phase 1: The probe (US3)

- [ ] T001 [US3] `e2e/engine-host.spec.ts`, its first test, the probe.
  Against spec 090's page as it is, boot the tray's frame and throw once.
  Then, in `page.evaluate`, move the canvas into a new 960×540 element
  elsewhere on the page, and check:
  - `engine_stats()`'s frame count still climbs;
  - no `webglcontextlost` fired;
  - a seeded throw sent after the move lands, with `dice_landed()` equal to
    the crate's faces;
  - the canvas still draws at its new size (a screenshot of the new
    element is not blank);
  - a synthetic `keydown`/`keyup` pair on the canvas is counted by
    `movement_state()`'s presses.

  Record the result here, with the evidence.
- [ ] T002 [US3] Record the decision from T001 in plan.md Decisions:
  - **moved canvas**, if every T001 check passed; or
  - **reload fallback** (FR-006, accepted by the owner on 2026-10-13), if
    any failed.

  If the fallback is chosen, T022 builds "Move the table here" as the
  reload, and US3 scenario 1's "no engine file is requested again" is
  checked against the browser cache (`transferSize` 0), not the request
  log. Keep the probe test either way, as the guard on the choice.

## Phase 2: The core, test first

- [ ] T010 `packages/encounter`:
  - `package.json` (`@thunderforge/encounter`, AGPL-3.0-or-later, source
    exports, scripts `test`, `typecheck` and `check` as in dice-throw);
  - `tsconfig.json` copied from dice-throw;
  - an empty `src/index.ts`.

  `scripts/e2e/slices.json` `landing`: add `packages/encounter/**` to the
  paths, and `pnpm -F @thunderforge/encounter check &&` to the front of
  the standalone command. `pnpm install` links it. `pnpm verify` passes.
- [ ] T011 [P] `src/walls.test.ts`. Port the movement and intersection
  cases from `crates/thunderforge-canvas-core/src/wall_tests.rs` and
  `wall_room_and_clear_tests.rs`, plus:
  - touching counts: ending on a wall, and passing through the point where
    two walls meet;
  - an open door never blocks, and a closed one does;
  - a move to the same point is never blocked, even standing on a wall;
  - a path is judged leg by leg;
  - one case where f32 and f64 disagree, with the port giving f32's answer;
  - `doorInReach`: a token whose footprint is one square from the door is
    in reach; one and a half squares is not (`icon_shown`'s token test).

  Seen failing.
- [ ] T012 `src/walls.ts`: `segmentsIntersect`, `movementBlockedBy`,
  `pathBlockedBy`, `distanceToSegment` and `doorInReach`. Every arithmetic
  step goes through `Math.fround`. Each function's comment names the Rust
  function it ports, with its file. T011 green.
- [ ] T013 [P] `src/map.test.ts`, then `src/map.ts`. It reads a
  `demo.json`-shaped map into:
  - walls with stable ids (`wall-00` to `wall-32`, with `door-hall` and
    `door-west` for the two doors);
  - lights;
  - square-to-world and world-to-square conversion.

  The tests use the real `apps/landing/public/maps/demo.json`: 33 walls,
  2 doors, both `closed`, 12 lights, and `door-hall` at x −3.5 to −2.5
  squares on y −2.
- [ ] T014 [P] `src/goals.ts`. Export the four goal ids as a closed union
  (`door-reached`, `door-opened`, `chest-found`, `chest-opened`), their
  order, and `GOAL_EVENT = "thunderforge:encounter-goal"`.
- [ ] T015 `src/referee.test.ts`. One test per rule, against a small
  fixture room and against the real map. It covers:
  - a step into a wall is refused and the hero stays;
  - a step into `door-hall` while it is shut marks `door-reached`, once;
    a refusal at `door-west` or a plain wall marks nothing;
  - "open the door" out of reach is refused with the reason; in reach it
    opens the door in reach and marks `door-opened` (`door-hall` only);
  - opening an open door closes it; no goal is unmarked;
  - the chest region entered marks `chest-found` once; walking away and
    back marks nothing more;
  - a roll before `chest-found` is refused;
  - a total of 11 fails and 12 opens the chest (`chest-opened`); a failed
    roll may be rolled again; the history keeps each try;
  - a move the engine reported and the rule refuses returns the last
    accepted square (the rollback);
  - a path is accepted only if every leg is;
  - every outcome carries one announcement key, and a repeated identical
    action produces no second goal;
  - `reset()` returns the starting state exactly.

  Seen failing.
- [ ] T016 `src/referee.ts`: the state machine. Its state is the accepted
  square, the door states, the goals and the roll history. Its actions
  are:
  - `step(dir)` and `judgeMove(from, to, path?)`;
  - `toggleDoor()` and `doorPressed(wallId)`;
  - `entered(interactiveId)`;
  - `rolled(resolution)`;
  - `reset()`.

  It is pure: no DOM, no engine, no randomness. It returns the new state
  and the outcome. T015 green.
- [ ] T017 [P] `src/describe.test.ts`, then `src/describe.ts`. It puts the
  hero's place in words: the room or hall it is in, the nearest door and
  its state with a distance and a direction, and the chest when it is
  within five squares. For example: "You stand in a dark room. The door to
  the hall is four squares north, shut." The test pins the start square's
  sentence and three others.
- [ ] T018 [P] `src/engineScene.test.ts`, then `src/engineScene.ts`.
  - `roomSetup(script, hero, { reducedMotion })` returns the plan's
    fourteen-step list, and the test pins its order. Two orderings are
    asserted on their own: the hero's `upsert_token` comes before
    `set_token_vision`, and `set_scene_playing` comes after the region.
  - `roomTeardown(script)` removes everything the setup added, and the
    test checks that every id set up is removed.
  - `doorCommand(wall, state)` returns the `upsert_wall` for a door.
  - `rollbackCommand(hero, square)` returns the `upsert_token` that sends
    the hero back.
- [ ] T019 [P] `src/engineEvents.test.ts`, then `src/engineEvents.ts`. It
  parses the engine's JSON strings into typed `upsert_token`,
  `movement_blocked`, `door_icon_pressed`, `interactionTriggered` and
  `sdkError` events. Anything else, and anything malformed, becomes
  `null`. The shapes are checked against `apps/web/src/engine/bevy/engineEvents.ts`'s
  guards. `index.ts` re-exports the package. `pnpm -F
  @thunderforge/encounter check` passes.

## Phase 3: One table (US3)

- [ ] T020 [US3] Read the engine's start state on a fresh boot: the
  camera, the background, the ambient light, the viewer and the grid.
  Record it here. Then give `DiceTray` an explicit `setup` that sends that
  state, so the tray's table is the same after a room visit.
- [ ] T021 [US3] `src/engine/loadEngine.ts`: widen `Engine` to
  `set_event_callback`, `set_viewer_token`, `movement_state`,
  `door_icons`, `sight_probe` and `list_interactives`. `tsc` passes.
- [ ] T022 [US3] `src/engine/host.ts` and `src/engine/dispatch.ts`. The
  host:
  - loads once;
  - creates the canvas imperatively;
  - starts the engine once;
  - tracks the holder;
  - runs `take(frame)`: teardown, move, setup, rendering;
  - holds the reload fallback, if T002 chose it.

  The dispatcher installs the one `set_event_callback`, parses with
  `engineEvents`, and routes to the holder's listener. `sdkError` goes to
  `console.error`.
- [ ] T023 [US3] `EngineFrame.tsx` and `engine-frame.css`.
  - Take the table from the host instead of `loadEngine`.
  - A new `away` state shows the poster, where the table is, and "Bring it
    back".
  - The `interactive` prop gives the stage the pointer, with a
    capture-phase `wheel` stop.
  - The `needs.maxTextureSize` prop holds back with reason `small_texture`.
  - Add `small_texture` to `HeldBack` in `frameEvents.ts`, and to spec
    086's `engine.load_failed` row (`contracts/browser-events.md`), in the
    same commit.

  The file stays under 400 lines; the away state goes in its own small
  component if it does not.
- [ ] T024 [US3] `DiceTray.tsx`: send no throw while the tray is `away`.
  The DOM result prints as before.
- [ ] T025 [US3] `e2e/engine-host.spec.ts`: US3 scenarios 1 to 4. Use the
  tray and a minimal second frame until the room exists (T034), then
  switch to the room. `engine-frame.spec.ts` stays green.

## Phase 4: The room (US1)

- [ ] T030 [P] [US1] `src/room/chest.svg` in the landing's ink style, and
  `scripts/make-room-art.mjs`. Write `public/room/{sir-pip,
  mira-starweave,fenna-swiftbow,brother-oak,chest}.webp` at 256×256. Look
  at each one, and commit them.
- [ ] T031 [US1] `src/room/script.ts`: the room's fixed facts (plan
  Decisions):
  - the hero's start square;
  - the chest's square and region;
  - the hall probe point;
  - the camera box;
  - the torch, `1d20+2` and DC 12.

  Confirm on the engine, recorded here:
  - the y direction;
  - that the hall point is unseen from the start and seen once the door
    is open;
  - that the chest sits in the lamp's dim ring;
  - the units `set_carried_light` takes.
- [ ] T032 [US1] `src/sections/Room.tsx`, `src/room/room.css` and
  `App.tsx`. Add a lazy `#room` section after `#map`, with:
  - its heading;
  - the simulation sentence above the frame;
  - the credit (MBRound18, CC BY-SA 4.0, the catalog link) beside the
    frame;
  - the frame, with its poster;
  - the map's weight stated beside the button.
- [ ] T033 [P] [US1] `src/room/HeroPicker.tsx` (four radios, Sir Pip
  default; a change starts the room over) and `src/room/GoalList.tsx` (the
  four goals in order, with done or not done in text, not colour alone).
- [ ] T034 [US1] `src/room/useRoom.ts`. It holds one referee and wires it
  to the host's events and the DOM:
  - the rollback;
  - the parity comparison, with `console.error` on a disagreement;
  - one announcement per outcome;
  - `GOAL_EVENT` dispatched once per goal;
  - `set_viewer_token` after setup.
- [ ] T035 [US1] `src/engine/keys.ts`. Forward arrows and WASD from inside
  the room's controls only, as a synthetic keydown/keyup pair on the
  canvas, and suppress scroll for those keys. The pad's buttons forward the
  same way while the engine runs.
- [ ] T036 [US1] `src/room/roll.ts` and the roll control.
  - The DC and formula are shown before the roll.
  - Roll with the crate and a `crypto` seed.
  - The referee judges the roll, and `diceCommand` throws it when the room
    holds the table.
  - The DOM prints the total no later than the throw lands.
- [ ] T037 [US1] The door. The icon press goes through `doorPressed`; the
  DOM button names the door in reach, or is disabled with the reason. At
  390, 768 and 1440 px, check that:
  - a click on the icon's place opens the door through the CSS `scale`;
  - the wheel over the room scrolls the page.

  Fix the pointer mapping in the frame if it is off. Record here.
- [ ] T038 [US1] The ending (FR-016), with the demo link and the credit
  again, and "Start over" (FR-017).
- [ ] T039 [P] [US1] `scripts/make-room-poster.mjs`, and the committed
  `public/room-table.webp`. Rerun it when the engine's look or the room's
  art changes.
- [ ] T040 [US1] `e2e/support/room.ts` and `e2e/room.spec.ts`: US1
  scenarios 1 to 9 through the engine. It reads the engine through
  `movement_state`, `door_icons`, `sight_probe` (with `expect.poll`),
  `list_interactives` and `dice_landed`. The roll uses a seed that
  succeeds, and one that fails then succeeds. SC-006 and SC-007 are
  recorded here.
- [ ] T041 [US1] `e2e/room-parity.spec.ts` (plan Decisions):
  - the fixed walk;
  - the Node-side verdict from `@thunderforge/encounter`;
  - zero disagreements (SC-004).

  The page's own `console.error` check catches the rest. Record the step
  count here.

## Phase 5: Without the canvas (US2)

- [ ] T050 [US2] `src/room/RoomControls.tsx`. It holds:
  - the pad (four buttons named by direction);
  - the door button;
  - the roll;
  - "Start over";
  - the description from `describe.ts`;
  - one `aria-live="polite"` region.

  All of it works with no engine and drives the same referee. Split off
  the description into its own component if the file passes 250 lines.
- [ ] T051 [US2] `e2e/room-words.spec.ts`:
  - US2 scenario 1: all four goals DOM-only, every outcome announced once,
    no engine request (SC-003);
  - scenario 2: each hold-back reason, including `small_texture` (stub
    `getParameter`), leaves a working DOM room;
  - scenario 3: keys inside the room move the hero, while keys on the
    hero mat or in the dice formula do not, and the page does not scroll.
- [ ] T052 [US2] `a11y.spec.ts`: axe, the focus walk, overflow and
  clipping with the room at rest, booted and completed, at three widths
  and two schemes (SC-005, US2 scenarios 4 and 5). Fix what it finds and
  list each finding and fix here.
- [ ] T053 [US2] Reduced motion (US2 scenario 6):
  - `set_reduced_motion` is the first command;
  - `fit_camera_to` is sent with `immediate`;
  - the dice take the reduced time;
  - the room's CSS adds no motion.
- [ ] T054 [P] [US2] `looks.spec.ts`: the room at rest, per width. Then
  regenerate the baselines below `#map` with
  `--update-snapshots=changed`, look at each one, and commit them.
- [ ] T055 [P] `telemetry.spec.ts`: completing every goal sends no beacon
  beyond those it sends today (FR-019). With `TELEMETRY=false`, nothing is
  sent.
- [ ] T056 Request log (FR-025, SC-001): scrolled top to bottom, no engine
  file, no `public/room/*` art and no second `/maps/demo.webp` is fetched
  before the click.

## Phase 6: Proof

- [ ] T060 Run, and record the results here:
  - `pnpm e2e:landing` (three times, SC-008);
  - `pnpm -F @thunderforge/encounter check`;
  - `make lint` (and no file over 1000 lines);
  - `pnpm verify`;
  - `pnpm -F @thunderforge/demo e2e`, only if the demo's build changed.

  Record the room's click weight too: the engine bytes, plus the map when
  the legend has not been seen.
- [ ] T061 Hand pass, recorded here. On the release engine served by nginx
  (`check-serving.mjs`'s container, or thunderforge-dev after deploy),
  play the room:
  - at 390 px with touch;
  - at 1440 px with a mouse;
  - keyboard only;
  - with a screen reader (Orca and Chromium), DOM-only.

  Note anything that reads wrong, feels slow, or announces twice.
- [ ] T062 Owner: approve or replace the copy (spec Open question 1; plan
  Decisions): the simulation sentence, each goal's announcement, the
  ending, and the away state.
