# Implementation Plan: One Room on the Front Page, Played on the Real Engine

**Branch**: `091-landing-encounter` | **Date**: 2026-10-13 | **Spec**: [spec.md](spec.md)

## Summary

The work is five slices, in this order. Each one lands green before the
next starts.

1. **The probe.** One e2e test moves spec 090's booted canvas to another
   element and checks that it still draws, throws and hears keys. Its
   answer picks between the moved canvas and the reload fallback (FR-006,
   accepted by the owner). Everything after it is built on that answer.
2. **The core, test first.** A new package, `packages/encounter`, holds
   everything that can be judged without a browser:
   - the wall rule, ported from canvas-core;
   - the map read into walls with ids;
   - the referee;
   - the words for where the hero stands;
   - the room's engine commands, in order;
   - the engine's events, parsed.

   Each module's tests are written and seen failing first.
3. **One table (US3).** The page gets one engine host and one event
   dispatcher. `EngineFrame` takes the table from the host instead of
   starting its own engine. The dice tray keeps working, and it can lose
   the table and get it back.
4. **The room (US1).** The `#room` section, the hero picker, the goal
   list, the art and poster, the keys, the door, the roll and the ending,
   with the engine e2e and the parity e2e.
5. **Without the canvas (US2), then the proof.** The words path is built
   with the room. It is proven on its own here, with a11y, looks and
   reduced motion over the whole page. Then the slice runs three times and
   the room gets a hand pass.

No engine change is needed (see Decisions).

## Technical Context

- **Landing:**
  - React 19 and Vite 8, static files.
  - Playwright 1.62 against `vite preview` on port 5211, with `/demo/`
    served from `data/demo` by the preview middleware (spec 090 T036).
  - The landing has **no unit-test runner**. Pure logic goes into a
    package, which `node --test` runs, as `packages/dice-throw` and
    `packages/heroes` do.
- **New package:** `@thunderforge/encounter`.
  - TypeScript only, source exports, AGPL-3.0-or-later, no dependencies.
  - `tsconfig` is copied from `packages/dice-throw`: NodeNext,
    `allowImportingTsExtensions` and `erasableSyntaxOnly`, so Node 24 runs
    the tests with no build step.
  - Scripts: `test`, `typecheck` and `check`, as in dice-throw.
- **Engine:** the demo's build at `/demo/`, through spec 090's
  `loadEngine`. Every command and read the room needs already exists (see
  "Engine surface"). A release build is about 5MB brotli; the dev build
  the e2e uses is far larger, and that is expected.
- **No new dependency.** The art and poster scripts use the Playwright
  Chromium that `make-poster.mjs` already uses.
- **Proof:**
  - the `landing` slice, three times;
  - `pnpm -F @thunderforge/encounter check`, which joins the slice's
    standalone command;
  - `make lint` and `pnpm verify`.

## Constitution and AGENTS.md check

- **I. ECS owns simulation, React owns chrome. Justified departure.** The
  referee re-implements one adjudication rule, the movement rule, outside
  the engine and the server.
  - Why: the page plays the server's part, and the DOM-only path must
    work with zero engine bytes, so the rule cannot live in the wasm.
  - Containment: the rule lives in a package, not a component. It is a
    line-for-line port of `movement_blocked_by`, f32 rounding included.
  - Proof: a parity e2e checks it against the engine on every step
    (FR-012). The engine still refuses first; the referee only re-judges,
    as the server does.
- **II. Plugin-modular engine.** Passes. The engine is not changed.
- **III. Authorization at the data boundary.** Not applicable: nothing is
  persisted. The referee stands in for the boundary and says so on the
  page.
- **IV. ADRs and specs first.** Passes. The spec is committed before the
  code. No ADR: one engine per page is a landing detail, and "the page
  plays the server" is a demo-only stance already recorded in spec 090.
- **V. Verify before done.** `tsc` for the landing and the package, `node
  --test` for the package, the slice for the page. No Rust changes.
- **VI. Own slice.** Passes. The `landing` slice gains
  `packages/encounter/**` and the package check. It needs no new spec
  entries: its specs live in `apps/landing/e2e`, which the slice already
  owns by path.
- **VII. Telemetry.** Passes. No new telemetry event. The goal event
  (FR-019) is dispatched on `window`, and `telemetry.ts` does not listen to
  it. A telemetry e2e check proves nothing is sent when a goal is reached.
- **AGENTS.md circular flow.**
  - The engine moves the token optimistically.
  - The referee adjudicates, as the server would.
  - A refusal is rolled back by sending the last accepted position.
  - The DOM shows only what the referee accepted.
  - No component holds network state; there is no network.

## Decisions (spec open choices, closed here)

- **One engine, moved canvas (T002).** The T001 probe passed every check:
  the canvas moved into another element keeps drawing, throwing and
  taking keys, with no lost context. The host moves the canvas between
  frames; the reload (FR-006) stays as the fallback only for a failure.

- **No engine change.** Everything the room needs already exists:
  - `set_scene_playing` is an SDK command (`sdk.rs:154`), so no wasm export
    is needed to turn the region trigger on;
  - `set_viewer_token` is a direct export with `""` for none
    (`lighting_vision.rs:31`);
  - `set_controlled_token` takes `null` for none (`sdk.rs:258`).

  If the probe or the room finds a gap, it is named in the task's ledger
  before any engine edit, and gets a parse test in `sdk.rs` like spec 090
  T031.
- **The package boundary.** `packages/encounter` is the agnostic core plus
  one engine adapter. It knows nothing about React or the DOM.
  - The core: `walls`, `map`, `referee`, `describe`, `goals`.
  - The adapter: `engineScene` (commands out) and `engineEvents` (events
    in), written against the engine's JSON contract.

  The landing is the thin layer: React, the host, keys and pointer.
- **The wall rule is ported in f32.** `segments_intersect` compares cross
  products with `0.0` in f32. The map's coordinates are centred, with
  half-integer square edges on x. A cross product of two half-integer
  spans of up to 4095 world units can pass 2^24, where f32 rounds and f64
  does not.
  - So every arithmetic step in the port goes through `Math.fround`.
  - The tests include a case that differs between the two.
- **Wall ids.** `map.ts` gives each wall in `demo.json` a stable id from
  its index: `wall-00` to `wall-32`. The two doors get names: `door-hall`
  (x −3.5 to −2.5, y −2, the room's door) and `door-west`. The ids are
  used by the referee, the engine and the e2e alike.
- **Both doors work. Only the hall door is a goal.**
  - The referee opens or closes any door within reach, by the icon or by
    the DOM button.
  - The button names the door in reach ("Open the door to the hall") and
    is disabled with a reason when none is.
  - Goals 1 and 2 count `door-hall` only.
- **Coordinates.** Engine world units are `demo.json`'s, centred on the
  map. A square is 117. T031 confirms the engine's y direction, and the
  room script fixes "north" from it.

  Proposed facts, which T031 confirms on the engine:
  - hero start: the centre of column x −3 (the door's column), four squares
    south of the door's wall;
  - chest: in the hall, about three and a half squares from the lamp at
    (−2.56, −6.09), inside its dim ring, outside its bright one;
  - hall probe point: two squares north of the door's middle. It is lit by
    the lamp, and the door wall hides it from the hero while the door is
    shut;
  - camera: fitted to the middle room and the hall's near half.
- **Engine host and dispatcher.**
  - `src/engine/host.ts` owns the one engine, the one canvas and the
    holder. It creates the canvas with `document.createElement`, outside
    React, and appends it to the holder frame's stage. React never renders
    it, so React never removes it.
  - `src/engine/dispatch.ts` installs the single `set_event_callback` once.
    It parses each JSON string once, through `engineEvents`, and hands the
    typed event to the current holder's listener. Any other listener gets
    nothing.
  - An `sdkError` is also sent to `console.error`, so the console guard
    fails a test on a malformed command.
- **Taking the table.**
  1. The old holder's teardown removes everything its scene sent.
  2. The canvas moves.
  3. The new holder's `setup` runs from scratch.
  4. `set_rendering` follows the new holder's visibility.

  The dice tray gains an explicit `setup`: the camera, background, ambient
  light and viewer its throw expects. Today it relies on the engine's
  start state, which the room changes. T020 reads those defaults off a
  fresh engine and writes them down.
- **The reload fallback (FR-006, accepted).** If T001 fails, "Move the
  table here" reloads the page with `?table=<frameId>#<section>`. The host
  reads that on boot and starts in that frame without a second click. The
  wasm comes from the browser cache. The tray's history and the room's
  progress are lost, which the owner accepted.
- **Setup order.** `engineScene.roomSetup(script, hero)` returns this
  list, and a unit test pins the order:
  1. `set_reduced_motion`, so nothing moves before it is known;
  2. `set_scene_grid` (117, map size, origin);
  3. `set_scene_background` `/maps/demo.webp`, 4095×2340;
  4. `upsert_wall` for each of the 33 walls, with ids; the doors are
     `closed`;
  5. `upsert_light` for each of the 12 lights;
  6. `set_ambient_light` `dark`;
  7. `upsert_token` for the chest (`tokenType: "object"`, its art);
  8. `upsert_interactive`: the chest's region, `enter`, `once`;
  9. `set_carried_light` for the hero (a torch);
  10. `upsert_token` for the hero (`tokenType: "character"`, its art);
  11. `set_token_vision` for the hero, which must come after the token
      (`app.rs:803`);
  12. `set_controlled_token` for the hero;
  13. `set_scene_playing` `true`, which arms the region trigger
      (`interaction.rs:299`);
  14. `fit_camera_to` the room, `immediate` under reduced motion.

  `set_viewer_token(hero)` is a direct export, not a command. The host
  calls it after step 12. `roomTeardown` reverses the list:
  - remove the tokens, interactive, walls and lights;
  - `set_scene_playing` false;
  - `set_controlled_token` null;
  - `set_viewer_token("")`;
  - the background null.
- **Movement and parity.**
  - The room forwards keys to the canvas only from its own controls
    (`src/engine/keys.ts`). It sends a synthetic `keydown` and `keyup`
    pair, as `apps/web/src/engine/canvasKeyboard.ts` does, and suppresses
    the page's scroll for arrows and Space.
  - Before forwarding a step, the room asks the referee for its verdict
    on the same step.
  - When the engine answers with `upsert_token` (moved) or
    `movement_blocked` (refused), the two are compared. A disagreement is
    sent to `console.error` with both verdicts, so every e2e fails on it
    through the console guard.
  - A move the engine allowed and the referee refuses is rolled back by
    `upsert_token` to the last accepted square.
  - A drag carries no intended step, so it is only re-judged, not
    compared.
  - A Shift route carries a `path`, which is judged leg by leg
    (`pathBlockedBy`).
- **The parity e2e** (`room-parity.spec.ts`).
  - Node imports `@thunderforge/encounter` directly (Playwright compiles
    TS) and reads `public/maps/demo.json`.
  - It drives a fixed walk: into every wall of the middle room, into both
    door jambs, into the shut door, diagonally at the door frame's corner,
    through the open door, and back.
  - Before each step it computes the rule's verdict in Node. It reads the
    engine's verdict from the room's `data-square` and the announcement.
  - Any disagreement fails, and so does the room's own `console.error`.
- **The door icon needs the pointer.**
  - Spec 090's stage is `pointer-events: none` (`engine-frame.css:32`), so
    the page keeps the wheel. The room's frame sets an `interactive` prop:
    its stage takes the pointer, and a capture-phase `wheel` listener on
    the stage stops propagation. Winit never sees the wheel, and the page
    still scrolls.
  - T037 proves a click on the icon at 390, 768 and 1440 px through the
    CSS `scale`. If the scaled stage maps the pointer wrongly, it is fixed
    in the frame (the scale is applied to the canvas's box, and the
    canvas's CSS size is set so winit reads true sizes). The DOM button
    works either way.
- **Capability.** `EngineFrame` gains `needs?: { maxTextureSize: number }`.
  The room passes 4096. `holdBack` reads `MAX_TEXTURE_SIZE` from the same
  WebGL2 probe and reports the reason `small_texture`.
  - This is a new reason for spec 086's `engine.load_failed` contract row.
    Its stage is `probe`. The 086 row is updated in the same commit as the
    check.
- **The roll.** `room/roll.ts` rolls `1d20+2` with `@thunderforge/dice`
  (the chunk the tray loads) and a `crypto` seed. It hands the resolution
  to the referee, which judges it against DC 12, and to `diceCommand` when
  the room holds the table. The DOM prints the total before or as the
  throw lands.
- **Art.** `scripts/make-room-art.mjs` opens Chromium on a blank page.
  - It renders each `PARTY` hero's `createHero(spec).token()` SVG, and
    `src/room/chest.svg`, to 256×256 WebP.
  - It writes them to `public/room/<id>.webp`, and the files are
    committed.
  - The script is run by hand when `packages/heroes` changes the token's
    look. The landing slice does not run it, so `packages/heroes` stays in
    the hero-builder slice.
- **Poster.** `scripts/make-room-poster.mjs` builds on `make-poster.mjs`'s
  pattern.
  - It boots the landing's preview with `/demo/` and presses "Play the
    room" with Sir Pip.
  - It waits for `sight_probe` to report `looking`, and shoots the canvas
    at 1200×675, DPR 1, to `public/room-table.webp`, which is committed.
- **The goal event.** `thunderforge:encounter-goal`, with `detail: { goal }`
  and goal one of `door-reached`, `door-opened`, `chest-found` or
  `chest-opened`. The ids are exported from `goals.ts` as a closed union.
- **Placement.** `App.tsx` gains a lazy `#room` section after `#map`, as
  `#dice` and `#map` are.
- **Copy.** Placeholder wording ships until the owner approves it (T061):
  - the simulation sentence: "This room runs in your browser. The page
    plays the server's part: it checks every move and opens every door,
    as a ThunderForge server would.";
  - goal 1's announcement: "The door is shut. Walls and closed doors
    refuse a move, as the server would.";
  - the ending: "That was the engine your table runs on. The rest of the
    Proving Ground is in the demo.";
  - the away state: "The table is in the room below. Bring it back."

## Engine surface the room uses

All of these exist today; none is added.

- **Commands:** the ones in the setup order above, plus `upsert_token` (the
  rollback), `upsert_wall` (doors), `trigger_dice_roll`,
  `set_reduced_motion` and `set_rendering`.
- **Direct exports, added to `loadEngine`'s `Engine` type:**
  - `set_event_callback`
  - `set_viewer_token`
  - `movement_state`
  - `door_icons`
  - `sight_probe`
  - `list_interactives`
- **Events, parsed in `engineEvents`:** `upsert_token`,
  `movement_blocked`, `door_icon_pressed`, `interactionTriggered` and
  `sdkError`. Every other event is ignored.

## Project Structure

Every file stays well under 1000 lines. The budgets below are the plan's
estimates.

```text
packages/encounter/                    # new; node --test
├── package.json, tsconfig.json
└── src/
    ├── index.ts                       # re-exports                       ~20
    ├── walls.ts                       # f32 port: segmentsIntersect,
    │                                  # movementBlockedBy, pathBlockedBy,
    │                                  # distanceToSegment, doorInReach  ~140
    ├── walls.test.ts                  # ported wall_tests cases + f32    ~220
    ├── map.ts                         # demo.json → walls with ids,
    │                                  # lights, square ↔ world           ~90
    ├── map.test.ts                                                       ~80
    ├── goals.ts                       # the four ids, order, closed set  ~40
    ├── referee.ts                     # the state machine                ~280
    ├── referee.test.ts                                                   ~320
    ├── describe.ts                    # position and nearby, in words    ~120
    ├── describe.test.ts                                                  ~100
    ├── engineScene.ts                 # roomSetup / roomTeardown         ~160
    ├── engineScene.test.ts            # order pinned                     ~120
    ├── engineEvents.ts                # JSON → typed events, guards      ~110
    └── engineEvents.test.ts                                              ~90
apps/landing/
├── e2e/
│   ├── engine-host.spec.ts            # T001 probe, then US3             ~200
│   ├── room.spec.ts                   # US1 through the engine           ~300
│   ├── room-parity.spec.ts            # FR-012, SC-004                   ~180
│   ├── room-words.spec.ts             # US2, no engine                   ~250
│   ├── support/room.ts                # boot the room, read the engine   ~120
│   ├── a11y.spec.ts, looks.spec.ts    # gain the room's states
│   └── telemetry.spec.ts              # a goal sends nothing
├── scripts/
│   ├── make-room-art.mjs              # hero + chest WebPs               ~120
│   └── make-room-poster.mjs           # the room's poster                ~110
├── public/room/{sir-pip,mira-starweave,fenna-swiftbow,brother-oak,chest}.webp
├── public/room-table.webp
└── src/
    ├── App.tsx                        # lazy #room after #map
    ├── engine/
    │   ├── loadEngine.ts              # Engine type widened
    │   ├── host.ts                    # one engine, one canvas, holder,
    │   │                              # reload fallback                  ~200
    │   ├── dispatch.ts                # the one set_event_callback       ~70
    │   ├── keys.ts                    # scoped key forwarding            ~90
    │   ├── EngineFrame.tsx            # takes the table; away state;
    │   │                              # interactive; needs              (264 → ~330)
    │   └── engine-frame.css           # interactive stage, away state
    ├── sections/
    │   ├── DiceTray.tsx               # explicit setup; no throw away
    │   └── Room.tsx                   # the section: heading, sentence,
    │                                  # picker, frame, goals, credit     ~180
    └── room/
        ├── useRoom.ts                 # referee ↔ host ↔ DOM             ~220
        ├── HeroPicker.tsx             # four radios                      ~70
        ├── GoalList.tsx               # the four goals                   ~60
        ├── RoomControls.tsx           # pad, door, roll, start over,
        │                              # description, live region         ~200
        ├── roll.ts                    # 1d20+2, crate + diceCommand      ~60
        ├── script.ts                  # the room's fixed facts           ~60
        ├── chest.svg
        └── room.css                                                      ~150
scripts/e2e/slices.json                # landing: + packages/encounter/**,
                                       # + encounter check in standalone
specs/086-*/                           # engine.load_failed: + small_texture
```

## Risks

- **The canvas may not survive a move.** A WebGL context survives a DOM
  move in Chromium, and winit's listeners sit on the element. However,
  winit's `ResizeObserver` and its device-pixel-ratio watch are unproven
  across parents. T001 decides; the reload fallback is accepted.
- **Synthetic keys and `just_pressed`.** A `keydown` and `keyup` pair in
  one task may fall in one Bevy frame. `ButtonInput` keeps `just_pressed`
  in that case, and `apps/web` relies on it in production. T001 counts the
  presses through `movement_state()` to be sure.
- **f32 against f64.** Covered by the f32 port and its test. Where the
  port and the engine still disagree, the parity e2e finds it.
- **`sight_probe` lags.** It reads the last sight sheet the GPU wrote, so
  it trails a door change by a frame or more. The e2e polls it with
  `expect.poll`, never reads it once.
- **The engine's defaults are implicit.** The tray has relied on the
  engine's start state. T020 writes the tray's scene out explicitly, or a
  room visit leaves the dice table dark.
- **Two doors, one button.** Without the reach rule, the button could open
  `door-west` from the wrong room. The referee uses the icon's own reach
  rule (`icon_shown`'s token test), ported and tested.
- **Wheel and pointer.** Giving the room's stage the pointer risks taking
  the page's wheel, which the capture listener prevents. T037 checks that
  scrolling over the room still scrolls the page.
- **Page weight.** A visitor who skips the legend pays 1.95MB for the map
  on the click. The readout states it (spec Edge Cases). It is measured in
  T060, and a cropped map stays the later lever.
- **Baselines.** A new section moves every section below it. The looks
  baselines below `#map` are regenerated with `--update-snapshots=changed`
  and each is looked at, as in spec 090 T010.
