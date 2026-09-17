# Hero Builder (library)

The hero builder as a React component: a hero spec in, a portrait and a token
drawn beside every control that changes them. **No server, no network, no
storage, no routing** — the host decides what happens to the hero.

```tsx
import { HeroBuilder, renderHero } from "@thunderforge/hero-builder";

<HeroBuilder
  initialSpec={{ name: "Sir Pip" }}
  idPrefix="npc-42" // unique in the document; every SVG id and radio group derives from it
  initialRace={null} // or matchRace(sheet race): where the dice's race picker starts
  onChange={(spec) => keep(spec)}
  onInvalid={(problems) => show(problems)}
  actions={<button onClick={save}>Save</button>}
/>;
```

## What it knows, and what it does not

Every control is **generated** from `@thunderforge/heroes`: the parts and
their choices, the colours and their swatches, the flags, the sizes, the
labels, and the races the dice can narrow to. A part added there appears here
with no edit, and `src/catalogue-free.test.ts` fails if this package's source
ever names a catalogue key itself.

Race looks live in `packages/heroes/src/races.ts`. A race only narrows a roll;
it is never written into the spec, never reported through `onChange`, and never
exported.

## React comes from the host

React and `react-dom` are **peer** dependencies. The library bundles no React
of its own, so a host that resolves a second copy gets "Invalid hook call" —
dedupe it (`resolve.dedupe` in Vite).

The styles are mounted with the component and scoped under `.tfhb`, so a host
needs no stylesheet or Tailwind source. Retheme through the `--tfhb-*` custom
properties.

## Files in and out

`heroFiles(spec)` gives the portrait SVG, the token SVG and the minimal spec as
JSON; `saveHeroFile` hands one to the browser. `parseHeroText` reads a spec
back, refusing it whole with every problem named. There is **no SVG import**:
a drawing is not a hero.

## Check

```bash
pnpm -F @thunderforge/hero-builder check   # tsc + the catalogue-free guard
```

The builder's behaviour is proved end to end by `apps/hero-builder`'s suite.
