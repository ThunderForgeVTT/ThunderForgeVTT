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
- [x] T006 [P] [US1] `e2e/hero.spec.ts`:
  - a focused party token moves one square on an arrow key, and the live
    note says where;
  - a move across a marker wall is refused, and the note says so;
  - a mouse drag snaps to the grid.
- [x] T007 [P] [US1] `e2e/dice-tray.spec.ts`:
  - every preset prints a result whose total is consistent with its dice
    and the formula's modifier;
  - a typed formula rolls;
  - `nonsense` is refused with the message;
  - history keeps the earlier rolls.
- [x] T008 [P] [US1] `e2e/map-legend.spec.ts`:
  - picking each map changes the image and the overlay;
  - every layer toggle flips `aria-pressed` and hides or shows its layer;
  - the credit and the catalog link are visible.
- [x] T009 [US1] Fix whatever T006–T008 find.

  T006 and T007 found nothing. T008 found one bug: on a phone, Tab along
  the atlas strip left every other tile hidden past its edge (about 7px
  showing), because Chromium does not scroll a focused element that is
  partly visible. Each tile now scrolls itself into view on focus
  (`block`/`inline: "nearest"`). After: the landing suite 63 of 63.
- [x] T010 [US1] `e2e/looks.spec.ts`: a screenshot per section, per
  project, with masks (plan), under light. Under dark, compare against the
  same baselines. Generate the baselines, look at every one, and commit
  them.

  Done (765860e2): 36 baselines, {section}-{phone,tablet,desktop}, each
  looked at. The dice frame (T034) moved every section below it by a
  fraction of a pixel; 15 baselines were regenerated with
  `--update-snapshots=changed`, the dice ones looked at.
- [ ] T011 [US1] Add a `landing` slice to `scripts/e2e/slices.json`, as in
  the plan. Then:
  - `pnpm verify` finds no orphan specs;
  - `pnpm e2e:landing` is green three times.

## Phase 2: Served small (US4)

- [x] T020 [US4] `apps/demo/vite.config.mts`: a plugin writes `engine.json`
  (`js`, `wasm`, `bytes.raw`, `version`). Test: building the demo
  produces it, and its files exist.
- [x] T021 [US4] `apps/landing/scripts/compress.mjs`:
  - write `.br` beside each compressible asset in `dist/assets` and
    `data/demo/assets`;
  - fill `bytes.br` and `bytes.gz` in `data/demo/engine.json`;
  - write `marketing/landing-engine.json`.

  Run it from the Dockerfile's landing stage, after both builds.
- [x] T022 [US4] `nginx.conf.template`:
  - the `$br_suffix` map, `try_files` and the `.br` location;
  - `/demo/engine.json` served with `no-cache`.
- [x] T023 [US4] `apps/landing/scripts/check-serving.mjs`: the nginx header
  proof (plan). Add it to the slice command.

  Done (2026-10-13). Notes for whoever touches this next:
  - `try_files $uri.br` would type the response by `.br`, so a compressible
    asset is rewritten to its twin, and nested `\.<ext>\.br$` locations
    set the original type. `add_header`s are inherited, so they are
    written once on the `.br` location.
  - `marketing/landing-engine.json` is written only with `--marketing` (and
    quality 11): this checkout's dev-profile wasm is 335.7 MB raw and
    18.5 MB brotli at q5, not a number to publish.
  - The check mounts both trees under `/srv` and links the web root to
    them: a bind cannot be created inside a read-only bind.
  - Result: 21 of 21 header and length checks pass. The slice command
    gains it in T011.

## Phase 3: The frame (US2, US3)

- [x] T030 [US2] `packages/dice-throw`: `diceCommand()` moved from the
  sandbox, with unit tests covering:
  - numeric, Fate and d100 dice;
  - kept and dropped dice;
  - success-count rolls.

  The sandbox imports it.

  Done: it takes the resolution, not a formula, so the throw shows the
  very roll the page printed. The tests roll with the real crate. A seed
  of small words lands every die on the same face (xoshiro's first draw
  barely moves), so the explode test spreads its seed words; seeds from
  `crypto` are unaffected.
- [x] T031 [US2] Engine: the `set_rendering` command (`payloads.rs`,
  `sdk.rs`, `app.rs`) and a unit test of the parse. Then build the dev
  engine, and check that the frame count stops (in the sandbox, by hand,
  recorded here).

  Done (d8b015a0): `plugins/render_pause.rs`; off is a reactive loop that
  waits only for a user wake-up, and every host command wakes it once so
  the queue drains. Parse test passes; engine tests 369 of 369. The frame
  count stopping is still owed: T037 proves it in the browser.

  Proved by T037 ("frames stop out of view and in a hidden tab, and
  resume"): the count holds still for a second out of view and in a hidden
  tab, then climbs again.
- [x] T032 [US2] `src/engine/loadEngine.ts`:
  - fetch `/demo/engine.json`;
  - stream the wasm with byte progress;
  - import the JS;
  - init the engine with the bytes;
  - `start("#engine-frame canvas")`.
- [x] T033 [US2][US3] `src/engine/EngineFrame.tsx`:
  - its states (poster, unsupported, loading, running, paused, failed);
  - the capability check;
  - reduced motion;
  - pause on IntersectionObserver and `visibilitychange`;
  - retry;
  - `aria-hidden` canvas;
  - the readout in DOM text.
- [x] T034 [US2] `DiceTray.tsx`: mount the frame, and send each roll
  through `diceCommand()` once the frame is running. Queue at most the
  latest roll made during the load.

  Done. The frame sits under the tray. The engine draws a 960x540 board
  scaled to the frame (`--engine-scale`): its dice keep one size in screen
  pixels, so a frame-sized canvas cut the d20 in half and a Fireball's row
  off the sides. The stage takes no pointer, so the page keeps the wheel.
  The panel sits at the top, clear of where the dice land. Winit sets
  `tabindex=0` on the canvas; the frame holds it at -1 (axe:
  aria-hidden-focus).
- [x] T035 [US2] `scripts/make-poster.mjs`, and the committed
  `public/dice-table.webp`.

  Done: 1d20+5 = 17, seeded so the crate rolls it. Xoshiro's first draw
  reads only the second seed word, so that word is the spread one. The
  shot is taken at once, at the poster's size and DPR 1: a landed throw
  holds 2.5 s and a headless shot of a live canvas takes about 1 s.
- [x] T036 [US2] `vite.config.mts`: the preview middleware that serves
  `../../data/demo` at `/demo/`.
- [x] T037 [US2][US3] `e2e/engine-frame.spec.ts`:
  - no engine request before the click, scrolled top to bottom;
  - the click loads from `/demo/assets/` with progress;
  - seeded throws land on the crate's faces, including Fate and d100;
  - quick re-rolls never lose a DOM result;
  - reduced motion is sent;
  - frames stop out of view;
  - capability fallbacks work;
  - a broken manifest leads to the poster, a message and a retry;
  - the a11y checks from T004 pass with the engine running.

  Done: 19 passed at every width, the 7 engine tests at desktop only (14
  skips by design); the landing suite is 88 passed, 14 skipped.
- [x] T038 [US3] Telemetry for frame failures, inside the spec 086
  wording or with the wording updated.

  Done: the frame raises `thunderforge:engine-frame-failed` on the window
  and `telemetry.ts` sends it as `engine.load_failed` `{stage, reason}`,
  the event spec 086 already allow-lists (the plan's `landing_error` is
  not allow-listed, so it was dropped). The 086 contract row lists the
  landing's stages (`probe`, `manifest`, `wasm`, `js`, `init`, `start`,
  `engine`) and reasons (`no_webgl2`, `save_data`, `low_memory`, `error`).
  "Try again" is offered only before `init`: a trapped engine cannot
  start twice in one page, so later failures offer "Reload the page".

## Phase 4: Proof

- [ ] T050 Run, and record the results here:
  - `pnpm e2e:landing` (three times);
  - `pnpm -F @thunderforge/demo e2e`;
  - the telemetry slice;
  - `make lint`;
  - `pnpm verify`.
- [ ] T051 Owner: approve or replace the readout and unsupported copy (plan
  Decisions).
