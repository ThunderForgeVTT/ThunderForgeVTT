# Hero Builder (app)

The hero builder on a page of its own. **No server, no auth, no GraphQL** —
the library from `packages/hero-builder`, a preset picker, and the files a hero
leaves as.

## Why

Making a hero, or a preset for `packages/heroes/src/presets.ts`, should not
mean logging in, opening a world and finding an NPC. Here it is: open the page,
build, export or copy as preset.

It is also the builder's **control experiment**. If a control misbehaves here,
the fault is in the library or the catalogue; if it only misbehaves in the web
app, the fault is in the dialog around it.

## Run it

```bash
pnpm -F @thunderforge/hero-builder-app dev       # http://127.0.0.1:5190
pnpm -F @thunderforge/hero-builder-app build
pnpm -F @thunderforge/hero-builder-app preview   # http://127.0.0.1:5191
```

The ports are strict; set `HERO_BUILDER_PORT` or `HERO_BUILDER_PREVIEW_PORT`
to move them.

## What it offers

- **Start from** any preset in `PRESET_HEROES`, or a new hero.
- **Roll** with the dice, narrowed to a race or not. The seed is shown and can
  be typed back in; a locked field keeps its value. Race looks live in
  `packages/heroes/src/races.ts`.
- **Export** the portrait, the token, or the spec as `.hero.json`.
- **Import** a spec, pasted or from a file. A spec with problems is refused
  whole, and every problem is named. Drawings cannot be imported.
- **Copy as preset**: a `presets.ts` entry on the clipboard, ready to paste.

## Test it

```bash
pnpm -F @thunderforge/hero-builder-app test:e2e
```

Builds the app and runs `e2e/builder.spec.ts` against `vite preview` with one
worker. It needs no database and no backend, which is why it is not a lane of
`scripts/e2e-parallel.mjs`. Every control the suite expects is read from
`@thunderforge/heroes` at run time, so a part added to the catalogue is found,
selected and drawn without editing the suite.
