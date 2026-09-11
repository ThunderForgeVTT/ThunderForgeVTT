# Research: The Hero Builder

**Spec**: [spec.md](./spec.md) · **Date**: 2026-09-10

This file records the decisions the spec states and the options that lost.
Each entry has the same four parts: decision, why, alternatives and
consequences.

## R1: The shape of "a frontend project like engine"

**Decision**: the builder is a library and an app.

- **The library** is `packages/hero-builder` (`@thunderforge/hero-builder`), a
  set of React components that takes React as a peer dependency.
- **The app** is `apps/hero-builder`, a thin standalone host for phase (a).
- **`apps/web`** imports the library directly and loads it on demand.
- **`packages/heroes`** stays framework-free.

**Why**:

- **It is the engine's own triangle, adjusted.** The engine is one artefact
  (`@thunderforge/engine`, built into `dist/engine`) with two consumers:
  `apps/engine-sandbox`, which drives it with no server, no auth and no React,
  and `apps/web`, which mounts it through `mountEngine` and `useCanvasEngine`.
  The builder repeats that shape. It is one package, the standalone app is its
  sandbox, and the web app is its product consumer.
- **The builder has to be a library, not a mounted app.** The engine is mounted
  as an opaque runtime because it is one: Bevy in wasm, owning a canvas, driven
  through three exported functions. The builder is ordinary React on the same
  React as the web app. Mounting it the way the engine is mounted, as a separate
  bundle or an iframe, would bring a second React. `apps/web/vite.config.mts`
  already carries aliases to stop `@thunderforge/genie` doing exactly that
  ("Invalid hook call"). It would also stop the builder looking like the rest of
  the product, and make "Save to this NPC" a message across a frame boundary
  instead of a function call.
- **A standalone app is needed as well as the library.** Phase (a) is for us,
  and it has to be fast to iterate on: change a part, reload, and see it. That
  can't wait on a database, a login and a world. The sandbox README makes the
  same case for the engine. A standalone app can also be e2e-tested with no
  backend, so the builder's own proof never waits on the full stack.
- **`packages/heroes` stays free of React.** The CLI, `seed-demo-art.mjs` and
  the Playwright e2e all import it under plain Node. Putting React components
  into it would make every one of them carry React for no reason.

**Alternatives**:

| Option | Why not |
|---|---|
| **App only** (`apps/hero-builder`, with web embedding it) | A second React, or an iframe with messaging. The builder could not look like ThunderForge, and saving would cross a frame. |
| **Library only** (no standalone app) | Phase (a) would need the whole stack to iterate on a hat. The builder's e2e would need a backend it does not use. |
| **Components inside `packages/heroes`** | Forces React on the CLI, the seed script and the e2e imports, which is the framework-free property the package exists to keep. |
| **Components inside `apps/web`, with the standalone app importing from web** | Couples the tool to the product's routing, auth and aliases (`@/…`). A developer tool that needs the product to build is not standalone. |

**Consequences**:

- The library can't use `apps/web`'s `@/components/ui` directly. It carries its
  own structure and takes its look from the design tokens the host provides. The
  plan decides how those tokens are shared with the standalone app.
- The standalone app uses the web app's toolchain (Vite, React 19, Tailwind)
  rather than the sandbox's older Vite, so the builder renders the same in both.
- `apps/web` gains a workspace dependency on `@thunderforge/hero-builder`, and
  through it on `@thunderforge/heroes`.

## R2: What `packages/heroes` gains, and why there and not in the builder

**Decision**: phase (a) adds five things to `packages/heroes`. Each is a pure
function or a data export, and each has a node test.

1. **Labels** for every field and every choice. The package test asserts that
   every one exists.
2. **A palette per colour field** that `SKIN_TONES` does not already cover.
3. **A seeded randomiser**, `randomHero(seed, locked)`. The test asserts that
   every result validates and that one seed always gives one hero.
4. **A minimal-spec function.** It drops a field exactly when removing it leaves
   the resolved hero unchanged.
5. **A preset-source writer** for "copy as preset". It uses `SKIN_TONES` names
   where they match.

**Why**:

- **One source for everything that uses these.** Quick NPC, the builder, the
  seed script and a future CLI all randomise and export. If this logic lived in
  the builder, the next caller would copy it.
- **The failure moves to the right place.** Labels and palettes in the package
  mean that a new choice without a label fails `pnpm -F @thunderforge/heroes
  check`, not a builder e2e three layers away (US2 scenario 3).
- **The minimal-spec rule needs the package's defaults.** Those include the
  dependent ones (trim from outfit, ring from outfit, beard from hair, accent
  from headgear), which only `validateHero` knows. Copying them into the builder
  would be a second definition of the defaults, and the two would drift.

**Alternatives**:

| Option | Why not |
|---|---|
| **Builder-local labels and palettes** | A list the builder keeps, which is what the package's data exports exist to prevent. |
| **`Math.random` randomise** | Not reproducible. A GM who liked a face and lost it can't get it back, and a test can't assert it. |
| **Emit the full resolved hero as "the spec"** | It would pin every derived colour. A pasted preset would stop following its outfit when the outfit changed, which is the opposite of how the existing presets are written. |

## R3: Upload SVG, or rasterise in the browser

**Decision**: the builder uploads SVG, and the server rasterises it.

**Why**:

- **One rasteriser.** `svg.rs` draws every SVG at a fixed 1024 px edge,
  whatever client sent it, so every stored hero is drawn by the same code.
- **The path is already proven.** The P8 e2e uploads a preset's token as SVG and
  gets WebP back.
- **The hardening already exists.** The server's refusal of external references
  and its size ceiling were written for exactly this.
- **The upload is small.** An SVG is a few kilobytes, where a 1024 px PNG is a
  few hundred.

**Alternatives**:

| Option | Why not |
|---|---|
| **Canvas rasterise in the browser, upload PNG or WebP** | Results would differ by browser and font stack. It would be a second rasteriser to keep in agreement with the first, and the server re-encodes to WebP anyway. |
| **Upload both SVG and raster** | Twice the bytes for no product gain. The server would still have to decide which to trust. |

**Consequences**: the builder is offline-capable up to the moment of saving.
Saving is two ordinary uploads, one per role, reported separately (FR-025).
There is no multi-role transaction. `uploadActorImage` is per role, and a
partial result is reported rather than hidden. That is simpler than a new
batch mutation and honest about what happened.

## R4: Where a saved spec lives (phase d)

**Decision**: add a nullable `hero_spec` JSON column on `world_actor_images`.
It means "this image was drawn from this spec". It is written by the same
upsert that stores the image, through a new optional `heroSpec` argument on
`uploadActorImage`. An upload without the argument clears it.

**Why**:

- **A spec describes an image, not an actor.** The column records exactly that,
  so the spec and the image can't disagree. If a GM replaces the built token
  with a file, the token's spec is cleared by the same write. The builder can
  then say truthfully that the token no longer comes from a built hero
  (US6 scenario 2), instead of opening a spec that describes a picture nobody is
  looking at.
- **It travels for free.** Spec 026 FR-018 already copies an actor's imagery
  rows with the actor. A column on those rows is carried by the copy the moment
  the copy carries the row, which is FR-039.
- **It keeps ADR-057's bargain.** Imagery-related data stays on imagery rows.
  Nothing is added to `world_actors`, which ADR-057 deliberately kept free of
  image columns.
- **It fits the authorisation already in place.** Writing the spec is writing
  the image, under the same Editor gate or the phase (c) holder right. No new
  permission surface is needed.

**Alternatives**:

| Option | For | Against |
|---|---|---|
| **`hero_spec` on `world_actors`** | One spec per actor, and the simplest read. | It can silently disagree with the images the moment either role is replaced by a file. It also puts presentation data on the actor row, against ADR-057. |
| **A new `world_actor_hero_specs` table** (unique per actor) | Keeps both tables untouched. | The same disagreement problem as a column on the actor, plus another join and another table for 026's copy to learn about. |
| **Inside the uploaded SVG** (a `<metadata>` element) | No schema change. | The server rasterises and discards the SVG, so the spec would be lost at the moment it was meant to be kept. |
| **Browser storage only** | No server change at all. | It is per device and per browser, it is gone on another machine, and it can't travel in a collection. That is not "saved". |

**Trade-offs accepted**:

- **Duplication.** The normal case stores one spec twice, once on the portrait
  row and once on the token row. A spec is well under a kilobyte, and the
  duplication is what lets the two roles diverge honestly.
- **Validation lives on both sides.** The server is the data boundary
  (Constitution Principle III), so the client-side `validateHero` can't be the
  only gate for stored data. `packages/heroes` publishes a JSON Schema derived
  from its own data. A package test fails if the schema and `HERO_PARTS` /
  `HERO_COLORS` / `HERO_FLAGS` disagree, and the server checks each spec against
  that schema and a size cap. Clients still validate before drawing.
- **The catalogue becomes a compatibility surface.** Once specs are stored,
  renaming or removing a choice breaks every stored spec that names it. So
  choices become append-only (FR-038). Changing how a choice draws stays free,
  because stored pixels don't change until they are re-saved.
- **An ADR is needed.** Under Constitution Principle IV this decision gets an
  ADR, landed with phase (d), recorded as an extension of ADR-057.

## R5: Where each phase's proof lives

**Decision**:

- **Phase (a)** is proven by a Playwright spec in `apps/hero-builder/e2e`,
  against the standalone app with no backend.
- **Phases (b) to (d)** are proven by Playwright specs in `apps/web/e2e`,
  against the full stack, with the stack's usual rate-limit bypass and a single
  worker.

**Why**: the repository favours e2e proof over unit tests. The builder's own
behaviour needs none of the stack, and making its proof wait on one would slow
the loop phase (a) exists to speed up. The in-world phases are about the stack:
mutations, permissions, collections. They belong with the suite that already
proves those.

The phase (a) spec takes its expectations from `HERO_PARTS`, `HERO_COLORS` and
`HERO_FLAGS` at run time, not from hard-coded counts, so it grows with the
catalogue (SC-002). It also checks the page for duplicated ids after every
preset (SC-005) and walks the builder by keyboard alone (SC-004).
