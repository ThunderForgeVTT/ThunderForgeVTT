# Feature Specification: The Front Page, Proven, and the Real Dice Thrown on It

**Feature Branch**: `090-landing-engine-dice`
**Created**: 2026-10-13
**Status**: Draft (spec only; plan.md and tasks.md not yet written)
**Input**: The owner, 2026-10-13:

- "I'm wondering if we can make our public app more gamified to showcase
  more of our engine… on the thunderforge.dev". Of the options surveyed,
  the owner chose a one-room encounter, with the engine dice throw built
  first. This spec is that first slice.
- "ensure the thunderforge.vtt is a11y compliant and looks really good with
  playwright and functions well since this is the front page of people
  getting to know us". This spec makes that the floor the engine is added
  on top of.

## Why

thunderforge.dev (`apps/landing`) is where most people meet ThunderForge.
Today it says the engine is real but never shows it. Everything that moves
on the page is DOM and SVG: the hero's battle mat, the dice tray and the
map legend. "Roll the real dice" runs the server's dice crate as
WebAssembly, but only prints the result, while the board the product runs
on throws those same dice in 3D (spec 083).

The page has also never been checked as a whole:

- **Accessibility.** No Playwright spec runs axe on it.
- **Looks.** Nothing checks how it renders at phone, tablet and desktop
  widths, or in light and dark mode.
- **Function.** The only spec is `e2e/telemetry.spec.ts`. The hero tokens,
  the dice tray and the map legend have no cover at all.

The 2026-10-13 accessibility read found two moderate gaps (no skip link;
the GitHub star link can lose its name under 900px) and nothing worse, but
that read was static. It was never run in a browser.

So this spec does two things in order:

1. It proves the front page as it stands and fixes what the proof finds.
2. On that floor, it lets a visitor who asks for it throw the real dice on
   the real engine, without a first visit paying for it.

## What exists

Read on 2026-10-13 against `main` at `f30e075a`.

### The landing

- React and Vite, built to static files, served by nginx from the
  `landing` stage of the root `Dockerfile` (lines 178-194). The demo's
  build (`data/demo`) is copied into the same image at `/demo/`.
- `src/App.tsx` renders `<Hero />` before `<main>`. It lazy-loads
  `DiceTray`, `MapLegend` and `StarChart`. There is no skip link.
- **Hero** (`src/sections/Hero.tsx`, 524 lines): the battle mat. It has
  draggable party, goblin and ogre tokens, marker walls that refuse a move
  across them, a door, arrow-key movement for a focused token, and an
  `aria-live` note.
- **Dice tray** (`src/sections/DiceTray.tsx`, 184 lines):
  - It loads `@thunderforge/dice` on mount and rolls with
    `roll(formula, "{}", seed)`.
  - It draws each die as an SVG outline with its face, and keeps a history.
  - The result sits in `.tray-felt` with `aria-live="polite"`.
- **Map legend** (`src/sections/MapLegend.tsx`): the seven example maps,
  with walls, doors and lights drawn over them, and the MBRound18 /
  CC BY-SA 4.0 credit.
- **Copy rules.** `PRODUCT.md` holds the voice and principles ("one
  authority", "honest numbers or none"). `DESIGN.md` holds the visual
  system (the "wet-erase battle mat").
- **e2e.** `playwright.config.ts` builds the page and serves it with
  `vite preview` on port 5211. That is one Desktop Chrome project at
  1440×900, which does not serve `/demo/`. `e2e/telemetry.spec.ts` is the
  only spec. The `telemetry` slice in `scripts/e2e/slices.json` owns
  `apps/landing/e2e/**` and runs `pnpm -F @thunderforge/landing e2e`.
- **No axe.** `@axe-core/playwright` 4.13.0 is a dependency of `apps/web`
  only.

### nginx (`apps/landing/nginx.conf.template`)

- `gzip on` covers `application/wasm`. Nothing is served precompressed and
  there is no brotli.
- `/assets/` and `/demo/assets/` are cached `immutable` for a year.
- The CSP on `/` allows `script-src 'self' 'wasm-unsafe-eval'` and
  `connect-src 'self' <telemetry>`. A same-origin engine loads under it
  unchanged.

### The engine as the demo ships it

- The demo's build writes the engine with hashed names:
  `data/demo/assets/engine-<hash>.js`, `engine_bg-<hash>.js` and
  `engine_bg-<hash>.wasm`.
- The app loads it through `apps/web/src/engine/bevy/wasmModule.ts`
  (`import("@thunderforge/engine/engine")` and `engine_bg.wasm?url`).
- There is no manifest naming those files. The landing, built separately,
  cannot know the hashes.

### What the engine already offers a host page

- **Boot.** `start(canvasSelector)` runs once per page (`startup.rs:42`)
  and has no stop.
- **Commands** go through `apply_world_command(json)`.
  - `trigger_dice_roll` takes a whole roll in the shape `apps/web` sends.
    `apps/engine-sandbox/src/dice.ts` `diceCommand()` builds it from the
    crate's resolution, with no totalling of its own.
  - `set_reduced_motion` (spec 083) shortens throws to `reducedMs`.
  - `set_scene_background` and `set_camera` also exist.
- **Reads.** `dice_landed()` lists the throws that finished or were
  skipped, at most 50, oldest first, each with its faces.
  `dice_entity_count()` reaches 0 once every throw has faded.
- **No render pause.** `set_scene_playing` is spec 030's play/prepare
  switch, not a pause. The engine sets no `UpdateMode`, so it draws every
  frame for as long as the page is open.

### Size

- The engine's release build is **4.92MB brotli (-q11) / 29.8MB raw**,
  measured in `MVP.md`. Over today's gzip it is about 7.9MB.
- The owner's concern line is 100MB brotli.

## Decisions already made

The owner made these on 2026-10-13. They are recorded in memory
(`project_gamified_landing`).

- **Order.** The engine dice throw comes first, as the plumbing slice. The
  one-room encounter (walls, doors, a chest, sight) is a later spec, built
  on this frame. It waits for the playtest defects in walls, doors and
  keyboard movement to close.
- **Cost.** About 5MB brotli, downloaded only on the visitor's click, is
  acceptable. A first visit downloads no engine bytes. The poster comes
  first.
- **Source.** The landing loads the engine the demo already ships under
  `/demo/assets/`, so the browser cache is shared between them. The two
  builds are coupled on purpose.
- **Badges.** Achievement badges are wanted and may be counted in
  telemetry, which means updating the spec 086 disclosure. They are **not**
  in this spec. They belong with the encounter, where most of them are
  earned.
- **Brotli.** Precompressed brotli is served from nginx itself. No new
  service is added.
- **Proof before engine.** The front page is accessible, renders well at
  three widths in both colour schemes, and has its interactive sections
  proven, before the engine frame lands on it (owner, 2026-10-13).

## User Scenarios & Testing

### User Story 1 - The front page as it stands is accessible, looks right and works (Priority: P1)

A first-time visitor, on a phone, a tablet or a desktop, in light or dark
mode, with a mouse, a keyboard or a screen reader:

- reads the page;
- moves a hero token and is refused by a wall;
- rolls dice in the tray;
- flips through the example maps.

Every one of these works, and nothing is unlabelled, hidden behind
something else, clipped or overflowing.

**Why this priority**: The owner named it as the condition for everything
else, and it costs no new bytes. Today none of it is checked.

**Independent Test**: The landing e2e runs on the built page, with no
engine involved. Every assertion below passes at 390, 768 and 1440 px
widths, in light and dark.

**Acceptance Scenarios**:

1. **Given** the built landing, **When** axe runs on the full page with the
   WCAG 2.2 A and AA rules *and* the best-practice rules (landmarks, one
   `h1`, region, skip link), **Then** it reports zero violations, at each
   width and in each scheme, after every lazy section has rendered.
2. **Given** a keyboard-only visitor, **When** they press Tab from the top
   of the page, **Then**:
   - the first stop is a visible "Skip to content" link that moves focus
     into `<main>`;
   - every later stop shows a visible focus indicator;
   - every stop is fully on screen and not covered by anything fixed (WCAG
     2.4.11).
3. **Given** a narrow screen (390px) and a GitHub star count that failed to
   load, **When** a screen reader reaches the GitHub link in the nav,
   **Then** it is named "ThunderForge on GitHub" (finding M8).
4. **Given** the hero:
   - **When** a visitor focuses a party token and presses an arrow key,
     **Then** the token moves one square and the live note says where.
   - **When** that move would cross a marker wall, **Then** the token
     stays put and the note says the wall refused it.
   - **When** the visitor drags a token with the mouse, **Then** it snaps
     to the grid.
5. **Given** the dice tray, **When** a visitor rolls each preset, and a
   typed formula, **Then**:
   - the printed total equals the crate's total for that roll;
   - the history keeps the earlier rolls;
   - an invalid formula is refused with a readable message, not a blank
     tray.
6. **Given** the map legend, **When** the visitor changes map and toggles
   each layer, **Then** the image and its overlay change together, and the
   MBRound18 / CC BY-SA 4.0 credit and the catalog link stay visible.
7. **Given** each width and scheme, **When** the page is screenshotted,
   section by section, **Then**:
   - it matches the committed baseline within a small tolerance;
   - no element runs past the viewport's right edge;
   - no text is clipped (a check that compares each text box's
     scroll size with its client size).
8. **Given** a visitor with `prefers-reduced-motion: reduce`, **When** the
   page renders and is used, **Then** nothing animates beyond what the
   existing reduced-motion rules allow.
9. **Given** the browser's console, **When** any of the above runs, **Then**
   it logs no error and no failed request.

---

### User Story 2 - A visitor throws the real dice on the real engine, when they ask to (Priority: P1)

A visitor in the dice tray rolls `1d20+5` and gets 17 + 5 = 22. Next to the
result is "Throw it on the table". They press it. A poster of the table is
replaced by the engine, with a readout of how much is downloading ("4.9 MB,
once; the demo reuses it"). The d20 tumbles onto the felt and lands showing
17. Every later roll in the tray is thrown on the table as well.

**Why this priority**: This is the slice the owner chose. It puts the
engine on the front page, and proves the frame the encounter will reuse.

**Independent Test**: In the landing e2e, with the demo's build served at
`/demo/`:

- record every request before the click;
- press the button;
- roll seeded formulas;
- compare `dice_landed()` against the crate's resolution for the same
  seed.

**Acceptance Scenarios**:

1. **Given** a first visit, **When** the page loads and is scrolled top to
   bottom, **Then** no request is made for any engine file
   (`/demo/assets/engine*`). The tray shows a static poster of the table
   and the button.
2. **Given** the visitor presses "Throw it on the table", **When** the
   engine loads, **Then**:
   - it is fetched from the demo's own hashed files under `/demo/assets/`,
     not from a second copy in the landing's `/assets/`;
   - a progress readout counts the bytes and states the total;
   - the poster stays until the engine draws its first frame.
3. **Given** the engine is running, **When** the visitor rolls any preset
   or formula, **Then**:
   - the tray rolls with the crate exactly as today;
   - the same resolution is sent as one `trigger_dice_roll`;
   - the faces `dice_landed()` reports for that throw equal the crate's
     `rolls` for every die, kept and dropped alike;
   - the DOM total appears no later than the throw's landing.
4. **Given** a Fate (`4dF`) or a d100 roll, **When** it is thrown, **Then**
   it lands as spec 083's contract says (`"F"` faces; a d100 reported once
   as `100` sides).
5. **Given** a visitor rolls again before the last throw has landed,
   **When** the engine's queue drops a throw, **Then** the DOM still shows
   every roll's result, and a skipped throw is not an error.
6. **Given** a visitor who prefers reduced motion, **When** the engine
   boots, **Then** it gets `set_reduced_motion` before the first throw.
   Throws then take the contract's reduced time, and later changes of the
   media query are passed on.
7. **Given** the frame is scrolled out of view, or the tab is hidden,
   **When** at least one second has passed, **Then** the engine stops
   drawing frames. It starts again when the frame is visible.

---

### User Story 3 - The throw is never the only way to know the result (Priority: P1)

A screen-reader user, or a visitor whose browser cannot run the engine,
gets the same roll, in words, from the same tray.

**Why this priority**: The front page must stay accessible after the
engine arrives. US1's floor holds with the engine on.

**Independent Test**: Re-run US1's axe and keyboard checks with the engine
booted. Then force each fallback condition and check that the tray still
rolls.

**Acceptance Scenarios**:

1. **Given** the engine is running, **When** axe runs, **Then** it reports
   zero violations:
   - the canvas is decoration (`aria-hidden`), outside the tab order;
   - the button, readout and fallback text are ordinary labelled DOM;
   - the existing `aria-live` result announces each roll exactly once.
2. **Given** a browser without WebGL2, or `navigator.connection.saveData`,
   or `navigator.deviceMemory` below 4, **When** the tray renders, **Then**
   the button is replaced by a sentence saying why the table is not
   offered here, with a link to the demo. The tray rolls as today.
3. **Given** the engine fails to load (a network error, a missing or
   mismatched demo build, or an engine panic), **When** the visitor has
   pressed the button, **Then**:
   - the poster comes back with a plain message and a retry;
   - the failure is reported through the existing landing telemetry, within
     the spec 086 disclosure's current wording;
   - the tray keeps rolling.
4. **Given** keyboard only, **When** the visitor tabs to the button,
   presses Enter, and then rolls with the presets, **Then** every step
   works without a pointer. Focus is never lost into, or trapped by, the
   canvas.

---

### User Story 4 - The engine is served small (Priority: P2)

The wasm and JS that nginx serves for the landing and the demo go over the
wire as precompressed brotli to browsers that accept it, and as gzip to
those that do not.

**Why this priority**: Visitors who opt in pay about 4.9MB instead of about
7.9MB, and the demo gains the same saving for free. It is not needed for
the throw to work.

**Independent Test**: Build the landing image, run it, and request the
engine wasm with and without `Accept-Encoding: br`.

**Acceptance Scenarios**:

1. **Given** the landing image, **When** `/demo/assets/engine_bg-<hash>.wasm`
   is requested with `Accept-Encoding: br`, **Then**:
   - the response is `Content-Encoding: br`;
   - it carries `Vary: Accept-Encoding`;
   - its length is the precompressed file's length.
2. **Given** a client without brotli, **When** the same file is requested,
   **Then** it is served gzip.
3. **Given** the build, **When** the image is assembled, **Then** the
   brotli files are produced by the build, not committed.
4. **Given** the build, **When** it finishes, **Then** the engine's brotli
   and gzip sizes are written to `marketing/landing-engine.json` by script.
   That figure is the one the readout quotes, never a hand-typed number.

### Edge Cases

- **Throw before boot.** The visitor rolls before the engine has finished
  loading. That roll is thrown once the engine is ready, or not thrown at
  all, but it is never thrown twice. The DOM result never waits on it.
- **Second press.** The visitor presses the button again during the load.
  Nothing new starts. `start` is once per page.
- **Leaving and coming back.** A visitor scrolls away and back, or hides
  and reshows the tab. The same engine resumes. Nothing reboots, and
  nothing leaks a second canvas.
- **Demo and landing out of step.** The demo build at `/demo/` is missing
  or from another release than the landing. US3 scenario 3 applies, and
  the engine is never half-loaded.
- **Very large rolls** (`100d6`). The engine already caps throws (spec 083
  queue and contract). The DOM shows the full result whatever the engine
  draws.
- **Small screens.** At 390px the frame keeps the tray usable: the formula
  field and the presets stay reachable above the canvas, and the canvas
  never forces a horizontal scroll.
- **Background throws.** A tab in the background throws nothing while
  hidden. Results still print.
- **Telemetry off.** With `TELEMETRY=false`, the frame sends nothing, as
  the rest of the page does.
- **CSP.** The engine loads under the existing `/` CSP. If the engine needs
  a `blob:` worker or more, that is found in the e2e against nginx's real
  headers, and the CSP change is part of this spec.

## Requirements

### Functional Requirements

**The front page, proven (US1)**

- **FR-001**: The landing MUST have a "Skip to content" link as the first
  focusable element. It MUST be visible on focus and move focus to
  `<main>`.
- **FR-002**: The nav's GitHub link MUST carry its accessible name on the
  `<a>` itself, whether or not the star count loaded.
- **FR-003**: The landing e2e MUST run axe (WCAG 2.2 A/AA plus the
  best-practice rule set) on the whole page, once every lazy section has
  rendered. It MUST run at 390, 768 and 1440 px, in light and dark, with
  zero violations allowed. Any rule exemption MUST be listed in the spec
  file with its reason.
- **FR-004**: The landing e2e MUST check, at each width:
  - that focus is visible at every tab stop;
  - that no tab stop is covered or off-screen;
  - that no element overflows the viewport horizontally;
  - that no text box clips its text.
- **FR-005**: The landing e2e MUST cover:
  - the hero: keyboard move, wall refusal, drag-and-snap, live
    announcements;
  - the dice tray: every preset, a typed formula, an invalid formula,
    history;
  - the map legend: map change, each layer toggle, credit and link
    present.
- **FR-006**: The landing e2e MUST compare per-section screenshots against
  committed baselines, for each width and scheme. The comparison MUST:
  - mask anything that changes between runs (star counts, random tokens,
    rolled dice);
  - run with reduced motion, so the screenshots are deterministic.
- **FR-007**: The landing e2e MUST fail on any console error or failed
  same-origin request.
- **FR-008**: Every finding the FR-003 to FR-007 checks turn up on today's
  page MUST be fixed in this spec. A finding cannot be passed by loosening
  its check.

**The engine frame (US2, US3)**

- **FR-009**: The landing MUST NOT request any engine file until the
  visitor presses "Throw it on the table". It MUST NOT prefetch or preload
  one, including on scroll or visibility.
- **FR-010**: The demo's build MUST publish a small manifest at a fixed,
  uncached path (for example `/demo/engine.json`). The manifest names its
  hashed engine JS and wasm, and their sizes. The landing MUST read it on
  the click to find the files. Nothing in the landing's build may depend
  on the demo's hashes.
- **FR-011**: Before the click, the frame MUST show a static poster of the
  table and the button. The poster MUST be generated from the engine by
  script, not painted by hand, and MUST be committed or built, not
  downloaded from elsewhere.
- **FR-012**: The frame MUST hold back the button, and say why, when the
  browser has no WebGL2, asks to save data, or reports `deviceMemory`
  below 4. It MUST then link to the demo.
- **FR-013**: During the load, the frame MUST show downloaded and total
  bytes, from the manifest's sizes, in text a screen reader can reach.
- **FR-014**: Once the engine is running, every roll the tray makes MUST
  be sent as one `trigger_dice_roll`, built from the crate's resolution.
  The command builder MUST be shared with
  `apps/engine-sandbox/src/dice.ts`, not copied, so the shape is defined
  once.
- **FR-015**: The tray's DOM result MUST stay the source of truth and the
  only live announcement. The canvas MUST be `aria-hidden` and out of the
  tab order.
- **FR-016**: The frame MUST send `set_reduced_motion` from
  `prefers-reduced-motion` before the first throw, and again on every
  change.
- **FR-017**: The engine MUST stop drawing frames while the frame is out of
  the viewport or the tab is hidden, and MUST resume when it is visible
  again. If no SDK command can do this today, this spec adds one (for
  example a reactive `UpdateMode` switch). It MUST NOT be faked by hiding
  the canvas.
- **FR-018**: A load or engine failure MUST:
  - restore the poster, with a message and a retry;
  - keep the tray working;
  - report through the landing's existing telemetry within the current
    spec 086 wording, or update that wording and its test in the same
    change.
- **FR-019**: The frame MUST be one reusable component (`EngineFrame`).
  It owns the poster, the capability check, the opt-in boot, the progress,
  reduced motion, the visibility pause and the failure handling. The
  encounter spec MUST be able to mount it with a different scene.
- **FR-020**: Every US1 check MUST also pass with the engine booted and
  after a throw.

**Serving (US4)**

- **FR-021**: The landing image MUST serve precompressed brotli for `.wasm`,
  `.js` and `.css` under `/assets/` and `/demo/assets/`, with
  `Vary: Accept-Encoding`, and fall back to gzip. The brotli files MUST be
  made at build time. No runtime compression module that needs a
  different nginx build may be required, unless the plan shows the stock
  image cannot serve precompressed files.
- **FR-022**: The build MUST write the engine's measured brotli and gzip
  sizes to `marketing/landing-engine.json`. The readout's total MUST come
  from the manifest, whose sizes come from the same build.

**Proof**

- **FR-023**: The landing's Playwright suite MUST be able to serve the
  demo's build at `/demo/` beside the landing, so US2 and US3 run without
  a server.
- **FR-024**: A slice MUST own the landing's specs, the frame, the
  manifest and the nginx template. That is either a new `landing` slice in
  `scripts/e2e/slices.json`, or the `telemetry` slice widened and renamed.
  `pnpm verify` MUST still find no orphan specs.
- **FR-025**: The US4 header checks MUST run against the real nginx
  template, either in the built image or in a stock nginx container with
  the template, not against `vite preview`.

### Key Entities

- **Engine manifest**: what the demo publishes about its engine build.
  It holds the hashed JS URL, the wasm URL, the raw, brotli and gzip
  sizes, and the build's version. It is read on the click and never
  cached.
- **EngineFrame**: the reusable opt-in frame. Its states are *poster*,
  *unsupported*, *loading (bytes)*, *running*, *paused (out of view)* and
  *failed (retry)*.
- **Throw**: one tray roll, sent as `trigger_dice_roll`. The tray's
  resolution decides it; the engine only stages it.

## Success Criteria

### Measurable Outcomes

- **SC-001**: Axe finds zero violations on the landing, best-practice rules
  included. That holds at 390, 768 and 1440 px, in light and dark, with
  and without the engine booted.
- **SC-002**: A first visit, scrolled top to bottom, transfers **0 bytes**
  of engine files. That is checked by request log in the e2e.
- **SC-003**: Opting in costs the brotli figure in
  `marketing/landing-engine.json`, about 4.9MB at today's build, and no
  more than the demo would. A visitor who then opens `/demo/` downloads
  no engine bytes again (checked by request log against a warm cache).
- **SC-004**: For every throw in the e2e, the faces in `dice_landed()`
  equal the crate's rolls for the same seed, die for die.
- **SC-005**: Every check above, including the screenshot baselines, runs
  in the landing slice without network access beyond the preview server.
  The slice passes three times in a row.
- **SC-006**: With the frame out of view for 2 seconds, the engine's frame
  counter (`engine_stats()`) does not advance.

### Proof

The changes are proven with the landing slice (FR-024), plus:

- the `telemetry` slice, if telemetry wording or the landing's telemetry
  code changes;
- `pnpm -F @thunderforge/demo e2e`, because the demo's build gains a
  manifest;
- `make lint` and `pnpm verify`.

The full e2e suite is not a gate.

## Assumptions

- `@axe-core/playwright` is added to `apps/landing`'s dev dependencies at
  the version `apps/web` already pins (4.13.0). No other new dependency is
  expected.
- The landing's e2e moves from one Desktop Chrome project to the three
  widths and two schemes (or loops over them inside one project, as the
  plan finds simpler). Firefox and WebKit are not in scope.
- The poster is generated with the engine sandbox and Playwright, in the
  same build step that already prepares the landing's static maps
  (`apps/landing/scripts/prepare-static.mjs`).
- The dice table needs no map. It uses `set_scene_background` with a felt
  texture and a fixed camera.
- The engine's GraphQL client and websocket stay idle without
  `set_world`, as they do in the sandbox. Any stray connection would be
  blocked by the CSP and would fail the console check (FR-007).

## Out of scope

- The one-room encounter (walls, doors, chest, sight). That is the next
  spec, gated on the playtest defects.
- Achievement badges and their telemetry disclosure. They go with the
  encounter.
- Accessibility fixes in `apps/web`, `apps/demo` and `apps/hero-builder`
  from the 2026-10-13 read, including the keyboard token-selection
  blocker. They need their own spec.
- Changing the landing's design system. Visual fixes follow `DESIGN.md`.

## Open items

- **Download readout wording.** The owner should approve the exact copy
  for the readout ("4.9 MB, once; the demo reuses it") and for the
  unsupported sentence.
- **Screenshot baselines are machine-sensitive.** The plan must pick where
  baselines are produced (the e2e container, or this machine with pinned
  fonts) so they do not flap between hosts.
