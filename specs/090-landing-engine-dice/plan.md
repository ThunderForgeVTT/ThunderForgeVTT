# Implementation Plan: The Front Page, Proven, and the Real Dice Thrown on It

**Branch**: `090-landing-engine-dice` | **Date**: 2026-10-13 | **Spec**: [spec.md](spec.md)

## Summary

The work is three slices, in this order. Each one lands green before the
next starts.

1. **The floor (US1).** Playwright proves the landing as it ships. Every
   finding is fixed in the page, never in the check:
   - axe with best-practice rules, at three widths;
   - keyboard focus walk;
   - overflow and clipping checks;
   - hero, tray and legend behaviour;
   - per-section screenshots;
   - a console guard.
2. **Serving (US4).** The build writes brotli files beside every asset. The
   demo's build publishes `engine.json`. Stock nginx picks the `.br` file
   when the client accepts it. A script measures the engine into
   `marketing/landing-engine.json`.
3. **The frame (US2, US3).**
   - An engine command that stops and resumes drawing.
   - A small shared package that builds `trigger_dice_roll` from a dice
     resolution.
   - The `EngineFrame` component: poster, capability check, opt-in boot
     from the demo's manifest, progress, reduced motion, visibility pause,
     failure and retry.
   - The tray, wired to it.
   - A generated poster.
   - e2e with the demo's build served at `/demo/`.

## Technical Context

- **Landing:**
  - React 19 and Vite 8, static files.
  - Playwright 1.62 against `vite preview` on port 5211.
  - Fonts are bundled through `@fontsource`, so text renders the same on
    any host with the same Chromium.
- **New dev dependency:** `@axe-core/playwright` 4.13.0, the version
  `apps/web` pins.
- **Engine:** Bevy on wasm32 (`crates/thunderforge-engine`). Its commands
  are parsed in `sdk.rs` and applied in `app.rs`. The demo's build holds
  the hashed engine in `data/demo/assets/`.
- **nginx:** `nginx:stable-alpine`, with no brotli module.
- **Proof:** the landing slice (below), the demo e2e, `make lint` and
  `pnpm verify`.

## Decisions (spec.md Open items and open choices, closed here)

- **Colour schemes.** The landing has one designed scheme. `styles.css`
  has no `prefers-color-scheme` rule; it is the wet-erase battle mat
  either way. So "light and dark" in the spec means this:
  - axe and the layout checks run under both `colorScheme: light` and
    `colorScheme: dark`;
  - the screenshots taken under `dark` are compared with the **same**
    baselines as `light`.

  A browser default leaking through in dark mode (form controls,
  scrollbars, `canvas` background) then fails as a pixel difference,
  without a second set of baselines.
- **Widths.** Three Playwright projects, one per width:
  - `phone`: 390×844, touch;
  - `tablet`: 768×1024;
  - `desktop`: 1440×900, the existing project.

  The colour scheme is looped inside each spec, so the telemetry spec does
  not run six times.
- **Screenshot baselines.** They are made on Linux with the Playwright
  Chromium that the repo pins, by `pnpm -F @thunderforge/landing e2e
  --update-snapshots`, and committed under `e2e/__screenshots__/`.
  - Comparison uses `maxDiffPixelRatio: 0.01`, with animations disabled
    and reduced motion on.
  - Masked: star counts, the star chart, and the dice results.
  - The hero's tokens are deterministic today: no `Math.random` in the
    sections. The tray's seed comes from `crypto`, so its result area is
    masked.
  - CI does not run e2e (`make ci`), so only one host produces baselines.
- **Brotli without a module.** The build compresses every `.js`, `.css`,
  `.wasm`, `.svg` and `.json` under `dist/assets` and `data/demo/assets`
  to `<file>.br` (Node's `zlib.brotliCompressSync`, quality 11). nginx
  then does this:
  - a `map $http_accept_encoding $br_suffix` yields `.br` when `br` is
    accepted;
  - the assets location tries `$uri$br_suffix` then `$uri`;
  - a nested location for `\.br$` sets `Content-Encoding: br`, the
    original type, and `Vary: Accept-Encoding`, and turns gzip off for
    that response.

  Stock nginx can do all of this, so no image change is needed.
- **Engine manifest.** A Vite plugin in `apps/demo/vite.config.mts` (the
  `generateBundle` hook) writes `engine.json` to the demo's root. It holds
  `{ js, wasm, bytes: { raw, br, gz }, version }`, with URLs relative to
  `/demo/`. The brotli and gzip sizes are filled in by the compress step,
  which rewrites the file after compressing. nginx serves `/demo/engine.json`
  with `Cache-Control: no-cache`.
- **The shared throw builder.** `diceCommand()` moves from
  `apps/engine-sandbox/src/dice.ts` to a new tiny package,
  `packages/dice-throw`. The landing and the sandbox both import it.
  - It is TypeScript only, with `@thunderforge/dice` as a peer, and no
    build step (source exports, as `packages/heroes` does).
  - Apps may not import apps (`pnpm verify` layout), so it cannot stay in
    the sandbox.
- **Render pause.** A new SDK command,
  `{ "type": "set_rendering", "on": bool }`, switches Bevy's
  `WinitSettings` between `UpdateMode::Continuous` and
  `UpdateMode::reactive_low_power(Duration::MAX)`. Off means no new
  frames, unless a window event arrives. `engine_stats()`'s frame count is
  the proof (SC-006).
  - The command is generic engine plumbing; it says nothing about the
    landing.
  - If `WinitSettings` turns out not to stop the wasm
    `requestAnimationFrame` loop, the fallback is to gate the render
    schedule's `run_if` on a resource. Either way, the frame count must
    stop.
- **Serving the demo in e2e.** The landing's `vite.config.mts` gains a
  `configurePreviewServer` middleware. It serves `../../data/demo` at
  `/demo/` when that directory exists. Only the preview server has it;
  the production build is unaffected.
  - The frame specs `test.skip` with a message naming
    `pnpm -F @thunderforge/demo run build` when `data/demo/engine.json` is
    missing.
  - The slice command builds the demo first, so the slice itself never
    skips.
- **nginx headers proof (FR-025).** `apps/landing/scripts/check-serving.mjs`
  starts `nginx:stable-alpine` with the template, `dist/` and `data/demo/`
  mounted read-only, on a free port. It requests the engine wasm with and
  without `br`, checks the headers and lengths, and removes the container.
  It is part of the slice's `standalone` command, and needs Docker, which
  every e2e host already has.
- **Slice.** A new `landing` slice owns:
  - `apps/landing/**` (except the telemetry files, which stay in
    `telemetry`);
  - `packages/dice-throw/**`;
  - `apps/demo/vite.config.mts` (manifest);
  - `apps/landing/nginx.conf.template`, now shared by both slices.

  Its `standalone` command:
  `pnpm -F @thunderforge/demo run build && pnpm -F @thunderforge/landing e2e && node apps/landing/scripts/check-serving.mjs`.
- **Telemetry for frame failures.** The landing's telemetry already sends
  errors under the disclosure's "errors the page hits" wording. An engine
  load failure is sent as `landing_error` with `{ where: "engine_frame",
  stage }`, inside that wording. If the WhatWeMeasure test says otherwise,
  the wording and its appendix change in the same commit.
- **Poster.** `apps/landing/scripts/make-poster.mjs` boots the engine
  sandbox's built page in Playwright. It sets the felt background and the
  camera the frame uses, throws a fixed seed (`1d20+5`, landing on 17),
  waits for `dice_landed()`, and saves a 1200×675 webp to
  `apps/landing/public/dice-table.webp`.
  - The webp is **committed**, so a landing build needs no GPU.
  - The script is rerun by hand when the engine's look changes, and the
    tasks say when.
- **Copy (spec Open item).** Placeholder wording ships behind the
  owner's approval:
  - readout: "Loading the table: 2.1 of 4.9 MB. Once; the demo reuses it.";
  - unsupported: "Your browser can't run the table here (no WebGL2). The
    dice above are the same code. Try the demo on another device."

  The owner may change both before release. This plan does not decide
  the final copy.

## Project Structure

```text
apps/landing/
├── e2e/
│   ├── support/page.ts          # widths, schemes, console guard, lazy-section wait
│   ├── a11y.spec.ts             # axe + focus walk + overflow/clip (US1, US3.1)
│   ├── hero.spec.ts             # keyboard, wall, drag/snap, live note
│   ├── dice-tray.spec.ts        # presets, formula, invalid, history
│   ├── map-legend.spec.ts       # map change, layers, credit
│   ├── looks.spec.ts            # per-section screenshots
│   ├── engine-frame.spec.ts     # US2, US3 (needs data/demo)
│   └── telemetry.spec.ts        # unchanged
├── scripts/
│   ├── compress.mjs             # .br beside assets, sizes into engine.json + marketing
│   ├── check-serving.mjs        # nginx headers proof
│   └── make-poster.mjs          # generated poster
├── public/dice-table.webp
└── src/
    ├── SkipLink.tsx (or inline in App)
    ├── engine/EngineFrame.tsx   # the reusable frame
    ├── engine/loadEngine.ts     # manifest → dynamic import with byte progress
    └── sections/DiceTray.tsx    # gains the frame
packages/dice-throw/             # diceCommand(), shared with the sandbox
crates/thunderforge-engine/src/  # set_rendering command (sdk.rs, payloads.rs, app.rs)
apps/demo/vite.config.mts        # engine.json
marketing/landing-engine.json
```

## Risks

- **Hash-less dynamic import.** The engine's JS glue imports its wasm by
  URL. Importing `/demo/assets/engine-<hash>.js` from the landing origin
  works because the origin is the same. In the preview server, the
  middleware has to serve the JS with the right MIME type, or the
  browser refuses the module.
- **Byte progress.** A dynamic `import()` cannot report progress. The
  loader therefore fetches the wasm itself, through a streaming reader
  that counts bytes. It hands the bytes to the glue's init
  (`init({ module_or_path: bytes })`), and imports the small JS without
  progress.
- **Flaky screenshots.** These come from lazy sections and webfonts.
  `support/page.ts` waits for `document.fonts.ready`, for every lazy
  section's content, and for the map image's `decode()` before any
  screenshot.
- **Unknown violations.** axe best-practice rules may fail on things the
  static read missed, for example `region` for the hero outside `<main>`.
  Each is fixed in markup. This could grow US1, which is the point of
  doing it first.
