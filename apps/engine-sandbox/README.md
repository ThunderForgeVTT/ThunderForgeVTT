# Engine Sandbox

A standalone harness for the Bevy engine. **No server, no auth, no GraphQL,
no React** — just the wasm engine, a canvas, and example maps as ordinary
static files.

## Why

The engine's entire integration surface is three wasm exports:

```
start(canvasSelector)
apply_world_command(json)
set_event_callback(fn)
```

Nothing about driving it requires the app. But debugging it *through* the app
means: rebuild wasm → restart the dev stack → log in → navigate into a world →
wait for a scene to load, for every single change. Here it is: rebuild wasm →
reload the page.

It is also a **control experiment**. Maps here are plain relative asset paths
(`maps/x.webp`, resolved against Bevy's default `assets` root), where the app
uses rooted, authenticated `/api/canvas-assets/...` URLs. If something renders
here but not in the app, the fault is in the plumbing. If it renders in
neither, the fault is in the engine.

## Run it

```bash
pnpm -F @thunderforge/engine-sandbox dev     # http://localhost:5180
pnpm -F @thunderforge/engine-sandbox check   # headless pass/fail over every map
pnpm -F @thunderforge/engine-sandbox keyboard  # keys still work with the canvas unfocused
```

`keyboard` guards a specific regression: winit binds its key listeners to the
canvas element, so keyboard input dies the moment any UI control takes focus.
It asserts the bug is reproducible without routing, that routing fixes it,
that releasing a key stops movement, and that typing in a text field moves
nothing. The routing snippet it injects mirrors
`apps/web/src/engine/canvasKeyboard.ts` — the sandbox deliberately has no
dependency on the web app, so the two must be kept in step by hand.

`dev` extracts the maps first. Rebuild the engine separately when you change
Rust:

```bash
node -e "import('./scripts/shared.mjs').then(m => m.buildEngine())"
```

## Attach modes

The app passes `#game-canvas-container` — the id of a **`<div>`**. winit's
`with_canvas` takes an `Option<HtmlCanvasElement>`, and the cast from a
non-canvas element silently yields `None`, so winit creates its own canvas and
appends it to `<body>`. That is why the app's canvas is a child of `<body>`
rather than of its container, and why `apps/web` has to position it manually.

- `/` — Bevy attaches to the page's real `<canvas>`.
- `/?attach=body` — reproduces the app's shape.

Note that a selector matching *nothing at all* does not fall back gracefully;
bevy_winit panics with `Cannot find element: <selector>`.

## Reading the canvas

Use the compositor (a screenshot), not `gl.readPixels`.

wgpu does not request `preserveDrawingBuffer`, so a `readPixels` call landing
after the compositor has recycled the buffer returns transparent black no
matter what was drawn. Measurements taken that way alternate between the real
clear colour and `(0,0,0,0)` purely on timing — which is worse than no
measurement, because it looks like hard evidence. The in-page "Read GPU
pixels" button reports `contextLost` and the alpha channel so this failure
mode is visible rather than silent.

## What this harness found

It was built to chase a bug where **nothing rendered at all** — not an
imported map, not the engine's own demo tokens — while every indirect signal
said the renderer was healthy.

The cause was in `src/engine/Cargo.toml`: it enabled `bevy_sprite`, `bevy_ui`
and `bevy_gizmos` but none of `bevy_sprite_render`, `bevy_ui_render`,
`bevy_gizmos_render`. Bevy 0.18 splits each subsystem into a logic half and a
render half. With only the logic halves, components exist, visibility is
computed, assets load and the camera clears the target — and nothing is ever
queued to draw. No error is produced, because nothing is wrong: the renderer
for those things simply was not compiled in.

The gizmo toggle is what localised it. Gizmos draw through a different
pipeline than sprites, so "gizmos fail too" ruled out everything
sprite-specific in one step and pointed at the render graph.

`check` passes on all 8 maps and is the regression guard.
