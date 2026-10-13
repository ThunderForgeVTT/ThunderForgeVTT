# Tasks: The Front Page, Proven, and the Real Dice Thrown on It

**Input**: Design documents from `specs/090-landing-engine-dice/`
**Prerequisites**: spec.md, plan.md

**Tests**: Each check is written and seen failing (or seen to find
something) before the page change it proves. For the pure parts there are
unit tests:

- the throw builder (T030);
- the manifest and compress step (T020).

**Proof**: The `landing` slice is the proof (T050). The demo e2e is also
run, because the demo's build changes. The full suite is never used as a
gate.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel with the other tasks marked [P] (a
  different file, and no unfinished dependency).
- **[Story]**: one of:
  - US1: the front page proven;
  - US2: the throw;
  - US3: the throw never the only way;
  - US4: served small.

---

## Phase 1: The floor (US1)

- [x] T001 [US1] Add `@axe-core/playwright` 4.13.0 to `apps/landing`
  devDependencies.
- [x] T002 [US1] `playwright.config.ts`: the `phone` (390×844, touch),
  `tablet` (768×1024) and `desktop` (1440×900) projects.
  `telemetry.spec.ts` stays on `desktop` only.
- [x] T003 [US1] `e2e/support/page.ts`:
  - `openLanding(page, { scheme })`, with reduced motion;
  - wait for `document.fonts.ready`;
  - scroll through so every lazy section renders;
  - wait for the map image's `decode()`;
  - a console and failed-request guard that fails the test (FR-007).
- [x] T004 [US1] `e2e/a11y.spec.ts`. In each scheme:
  - axe (wcag2a, wcag2aa, wcag21a, wcag21aa, wcag22aa, best-practice) on
    the whole page;
  - a focus walk: the first stop is the skip link, every stop shows a
    focus outline and is fully in the viewport, with no fixed element
    over it;
  - no element is wider than the viewport;
  - no text box clips its text.

  Run it and record what it finds in this ledger.

  First run (2026-10-13): 2 of 18 passed. It found:
  - axe `listitem` (serious), every width: `ul.atlas-list` carried
    `role="group"`, so its `li`s were outside a list;
  - axe `scrollable-region-focusable` (serious), phone and tablet: the
    self-host terminal's `pre` scrolls sideways with no keyboard access;
  - no skip link: the first Tab landed on the brand;
  - the nav GitHub link had no name on a phone when the star count
    failed: its only text is "Star", which a phone hides;
  - tablet: a legend toggle focused by Tab scrolled in flush with the
    bottom edge, its focus ring below the fold;
  - the overflow check flagged the atlas strip's tiles and the terminal's
    code past 390px and 768px. Both live in deliberate sideways scrollers,
    so the check now skips what is inside an `overflow-x: auto|scroll`
    box. The scroller itself must still fit.
- [x] T005 [US1] Fix every T004 finding in the page. Each finding is
  listed here, with its fix. Known in advance:
  - the skip link (FR-001): "Skip to content", first in the page,
    shown on focus, to `<main id="content" tabindex="-1">`;
  - the nav GitHub link name (FR-002): "Star ThunderForge on GitHub",
    with ", N stars" when the count is known;
  - `atlas-list`: `role="group"` dropped; the list keeps its
    `aria-labelledby`;
  - the terminal `pre`: `tabindex="0"`, named "The commands", with an
    inset focus ring (the terminal clips overflow);
  - `html { scroll-padding-block: 3rem }`, so a focused control scrolls in
    with its ring.

  After: `a11y.spec.ts` 18 of 18; the whole landing suite 24 of 24.
- [ ] T006 [P] [US1] `e2e/hero.spec.ts`:
  - a focused party token moves one square on an arrow key, and the live
    note says where;
  - a move across a marker wall is refused, and the note says so;
  - a mouse drag snaps to the grid.
- [ ] T007 [P] [US1] `e2e/dice-tray.spec.ts`:
  - every preset prints a result whose total is consistent with its dice
    and the formula's modifier;
  - a typed formula rolls;
  - `nonsense` is refused with the message;
  - history keeps the earlier rolls.
- [ ] T008 [P] [US1] `e2e/map-legend.spec.ts`:
  - picking each map changes the image and the overlay;
  - every layer toggle flips `aria-pressed` and hides or shows its layer;
  - the credit and the catalog link are visible.
- [ ] T009 [US1] Fix whatever T006–T008 find.
- [ ] T010 [US1] `e2e/looks.spec.ts`: a screenshot per section, per
  project, with masks (plan), under light. Under dark, compare against the
  same baselines. Generate the baselines, look at every one, and commit
  them.
- [ ] T011 [US1] Add a `landing` slice to `scripts/e2e/slices.json`, as in
  the plan. Then:
  - `pnpm verify` finds no orphan specs;
  - `pnpm e2e:landing` is green three times.

## Phase 2: Served small (US4)

- [ ] T020 [US4] `apps/demo/vite.config.mts`: a plugin writes `engine.json`
  (`js`, `wasm`, `bytes.raw`, `version`). Test: building the demo
  produces it, and its files exist.
- [ ] T021 [US4] `apps/landing/scripts/compress.mjs`:
  - write `.br` beside each compressible asset in `dist/assets` and
    `data/demo/assets`;
  - fill `bytes.br` and `bytes.gz` in `data/demo/engine.json`;
  - write `marketing/landing-engine.json`.

  Run it from the Dockerfile's landing stage, after both builds.
- [ ] T022 [US4] `nginx.conf.template`:
  - the `$br_suffix` map, `try_files` and the `.br` location;
  - `/demo/engine.json` served with `no-cache`.
- [ ] T023 [US4] `apps/landing/scripts/check-serving.mjs`: the nginx header
  proof (plan). Add it to the slice command.

## Phase 3: The frame (US2, US3)

- [ ] T030 [US2] `packages/dice-throw`: `diceCommand()` moved from the
  sandbox, with unit tests covering:
  - numeric, Fate and d100 dice;
  - kept and dropped dice;
  - success-count rolls.

  The sandbox imports it.
- [ ] T031 [US2] Engine: the `set_rendering` command (`payloads.rs`,
  `sdk.rs`, `app.rs`) and a unit test of the parse. Then build the dev
  engine, and check that the frame count stops (in the sandbox, by hand,
  recorded here).
- [ ] T032 [US2] `src/engine/loadEngine.ts`:
  - fetch `/demo/engine.json`;
  - stream the wasm with byte progress;
  - import the JS;
  - init the engine with the bytes;
  - `start("#engine-frame canvas")`.
- [ ] T033 [US2][US3] `src/engine/EngineFrame.tsx`:
  - its states (poster, unsupported, loading, running, paused, failed);
  - the capability check;
  - reduced motion;
  - pause on IntersectionObserver and `visibilitychange`;
  - retry;
  - `aria-hidden` canvas;
  - the readout in DOM text.
- [ ] T034 [US2] `DiceTray.tsx`: mount the frame, and send each roll
  through `diceCommand()` once the frame is running. Queue at most the
  latest roll made during the load.
- [ ] T035 [US2] `scripts/make-poster.mjs`, and the committed
  `public/dice-table.webp`.
- [ ] T036 [US2] `vite.config.mts`: the preview middleware that serves
  `../../data/demo` at `/demo/`.
- [ ] T037 [US2][US3] `e2e/engine-frame.spec.ts`:
  - no engine request before the click, scrolled top to bottom;
  - the click loads from `/demo/assets/` with progress;
  - seeded throws land on the crate's faces, including Fate and d100;
  - quick re-rolls never lose a DOM result;
  - reduced motion is sent;
  - frames stop out of view;
  - capability fallbacks work;
  - a broken manifest leads to the poster, a message and a retry;
  - the a11y checks from T004 pass with the engine running.
- [ ] T038 [US3] Telemetry for frame failures, inside the spec 086
  wording or with the wording updated.

## Phase 4: Proof

- [ ] T050 Run, and record the results here:
  - `pnpm e2e:landing` (three times);
  - `pnpm -F @thunderforge/demo e2e`;
  - the telemetry slice;
  - `make lint`;
  - `pnpm verify`.
- [ ] T051 Owner: approve or replace the readout and unsupported copy (plan
  Decisions).
