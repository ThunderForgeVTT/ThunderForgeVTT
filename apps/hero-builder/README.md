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

It needs no stack: no `make dev`, no Postgres, no RustFS, no Rust server. A
`pnpm install` at the repository root is enough; the two packages it uses are
read from source, so there is nothing to build first.

```bash
pnpm -F @thunderforge/hero-builder-app dev        # http://127.0.0.1:5190
pnpm -F @thunderforge/hero-builder-app build      # into dist/
pnpm -F @thunderforge/hero-builder-app preview    # the build, http://127.0.0.1:5191
pnpm -F @thunderforge/hero-builder-app typecheck
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

## Where the looks live

The page draws nothing of its own. Everything it offers comes from the
catalogue in `packages/heroes/src`, so tuning a look means editing there and
reloading this page:

| File | What it holds |
|---|---|
| `spec.ts` | Every field and choice a hero has |
| `parts.ts` | How each choice is drawn |
| `palettes.ts` | The swatches offered, and the only colours the dice pick |
| `labels.ts` | What a person reads for each field and choice |
| `races.ts` | Race looks: aliases, the choices and swatches a race narrows the dice to, and `matchRace` |
| `presets.ts` | `PRESET_HEROES`, the "Start from" list |

In the web app, the race that narrows the dice is read from the sheet field a
system names in its manifest's `appearance.race.source` (see
[`packs/systems/README.md`](../../packs/systems/README.md#appearance)) and
matched to a key or alias here.

## Test it

```bash
pnpm -F @thunderforge/hero-builder-app test:e2e
```

Builds the app and runs `e2e/builder.spec.ts` against `vite preview` with one
worker. It needs no database and no backend, which is why it is not a lane of
`scripts/e2e-parallel.mjs`. Every control the suite expects is read from
`@thunderforge/heroes` at run time, so a part added to the catalogue is found,
selected and drawn without editing the suite.
