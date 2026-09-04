# A Bundled Pack Ships Its Own Web Surfaces, Found at Build Time

- **Date**: 2026-09-04
- **Status**: Accepted
- **Spec**: `specs/032-pack-architecture/` (FR-004, FR-005, FR-029), `T108`
- **Follows**: ADR-029 (outside code is not run), ADR-061, ADR-063

## Context

ADR-029 settled *whose* code may run: the product executes only what it
compiled, so a **bundled** pack may contribute behaviour and an outside pack
may contribute data alone. On the server that answer already had a mechanism —
`inventory`, through the linker — and ADR-063 gave a pack somewhere to put
Rust: its own crate, depending on the server as a library.

The browser had the answer and no mechanism. So pack web code that needed to
*do* something rather than merely *render* something had nowhere to live, and
it lived in `apps/web` instead:

```ts
export const SYSTEM_ACTOR_SHEETS = {
  genie: GenieActorSheet,
};
```

That map was the last place shared web code named a game system. Its own header
argued the case honestly and it was a good argument: `GenieActorSheet` is a
*data-connected container*, not the plain presentational component a manifest
carries. It fetches system data, mutates it, edits `trait_data.level` and
recomputes max Wish Points from it. The manifest format has nowhere to put any
of that, and no pack had anywhere either.

Two things were wrong underneath it, both invisible until looked at.

**`check-system-registry.mjs` only scanned Rust.** Not by decision — that was
just where the violation had first been found. FR-029 was being enforced on the
server and merely hoped for in the browser, which is how a principle becomes a
thing people remember about the backend.

**The web already had a manifest loader, and it had never worked.**
`GameSystemContext` cached manifests fetched by `await import(
"@/systems/${systemId}/index")`. `apps/web/src/systems/` has never existed, so
that call could only ever throw. Nothing mounted the provider and nothing called
the hook; the three modules importing from it wanted the `SystemManifest` *type*
and nothing else. It sat there for four months looking like the mechanism.

## Decision

**A bundled pack ships its own web surfaces, and the host discovers them at
build time.**

Three parts.

### 1. `@thunderforge/host` — a stated surface, not the whole app

A pack's web code may import from `apps/web/src/host/index.ts` and from nowhere
else in this application. Today it carries actor system-data reads and writes,
the `Card` primitive, and the `ActorSheetProps` contract.

The alternative was letting a pack import `@/anything`, which silently makes
every internal module public API, or letting it import nothing, which is what
put the container in `apps/web` to begin with. A short list someone extends on
purpose, in a diff, is the middle that actually holds.

What is deliberately excluded: routing, authentication, the world store, the
engine bridge, anything holding a session credential. A pack needing one of
those is describing a capability boundary, and ADR-029 is explicit that none
exists in this product yet. The answer there is a richer declaration format,
not a wider surface.

### 2. Discovery by build-time glob

A pack that ships a data-connected actor sheet puts it at
`packs/systems/<id>/web/src/ActorSheet.tsx`, default-exporting a component
taking `ActorSheetProps`. `systemActorSheets.ts` finds them with
`import.meta.glob(..., { eager: true })`.

This is the browser's version of `inventory`, and the resemblance is the point.
Vite expands the glob to a static import map **before anything ships**, so the
bundle holds exactly the sheets that existed when the product was compiled.
There is no fetch, no evaluation of anything the build did not see, and no way
for a pack dropped into a running deployment to be picked up. It is discovery
without runtime loading — the same trade the linker makes.

`eager` rather than lazy because the lazy form returns a promise per module,
which would make `resolveActorSheet` async and ripple through every caller, for
components already in the bundle and already paid for. Keeping the lookup
synchronous is not an optimisation; it is being honest about what it is.

### 3. The registry check covers shared web code

`check-system-registry.mjs` now scans `apps/web/src` alongside the two Rust
roots, on the same terms: a pack's own code is exempt because a pack naming
itself is the point, and tests are exempt because a test must name a system to
assert anything about one.

## Consequences

**Four violations became visible the day the check could see them, and were
retired the next.** `ActorDetailPage`, `WorldStagingPage`,
`WorldSystemSettingsPage` and `ClocksPanel` each branched on one system's id to
mount that system's panel from a page every system shares. They waited on the
check's `KNOWN` list against `032/T108` for a day; that list is now empty on
both halves for the first time.

They were not fixed alongside the sheet, and the reason is a real difference
rather than fatigue — though the first version of this paragraph got that
difference wrong, and the wrong version is worth correcting in place rather
than quietly replacing.

It said a filename is the wrong place to encode which slot a panel fills. That
is overstated: `packs/systems/<id>/web/src/panels/<slot>.tsx` carries both the
system and the slot perfectly well, and the same glob reads them off the path.

The actual difference is that panels need a **host-declared vocabulary** where
the sheet did not. One mount point needs one props contract, and a filename
convention is the whole of it. Four mount points need an agreed set of slot
names and a typed contract for each, because a pack has to know which slots
exist and what each one is handed. `@thunderforge/host` declares `PanelSlot`
and `PanelSlotProps`; `apps/web/src/panels/systemPanels.ts` is the registry,
keyed `${systemId}:${slot}`. Two slots may share one component, and Genie's
staging and clocks panels do, because the session loop is one panel reached
from two places.

**Doing it widened this file, and that was the point of the exercise rather
than a cost of it.** A pack imports from `@thunderforge/host` or from nowhere,
so a pack panel could not call `useGenieSession` where it lived — in
`apps/web/src/hooks` — and exporting a system-named hook from the host would
have been the same violation wearing a different hat. The system's whole client
layer moved into the pack, and the host gained `postGraphQL` and
`subscribeToWorldEvents` so it could. ADR-063 had already given that pack its
tables and the GraphQL over them; a pack that owns a schema and may not call it
is not a boundary, it is a pack whose two halves have to meet in `apps/web`.
Routing, authentication, the world store and anything holding a session
credential are still absent, and a panel is told who is looking through its
props rather than being handed a way to ask.

**The check learned to read filenames, and found three more.** Content matching
could not see `GenieShopPanel.tsx`, `useGenieSession.ts` or
`engine/world/sync/genieSession.ts`, because not one of them quoted the id
inside itself: a component can be entirely about one system without ever
spelling it in a string literal, and the name is where it says so. Paths are
now compared against every bundled id with separators flattened, so
`YearZeroEnginePanel.tsx` trips on `year_zero_engine`.

**Pack web code was type-checked by nobody, including the sheet this ADR is
about.** The pack's own `tsc --noEmit` answered "Cannot find module
'@thunderforge/host'" on every file that used it. Mapping the alias from inside
the pack drags `apps/web` under the pack's React 18 types and fails on the
version skew, so `apps/web/tsconfig.json` includes
`../../packs/systems/*/web/src` instead — which checks pack code against the
React the bundle actually resolves to. A typed contract nothing checks is not a
contract.

**`GameSystemContext` was deleted rather than repaired**, along with
`game-system-context.ts`; the two manifest types moved to
`apps/web/src/types/systemManifest.ts`. The same reasoning as spec 032 T107,
which deleted `useSystemHooks` for `await import("/api/systems/<id>/<path>")`:
a per-system dynamic import is the wrong mechanism now, not merely a broken
one. Leaving a dead implementation of the rejected approach in the tree is how
it comes back.

**Renaming anything in `@thunderforge/host` means updating packs in the same
commit.** That is possible precisely because every pack lives in this
repository — which is the cost ADR-029 accepted when it ruled that only bundled
packs may contribute behaviour, now being paid rather than merely agreed to.

**The glob can silently stop matching.** A pattern that matches nothing yields
an empty registry, and every system quietly loses its sheet with no error
anywhere. `systemSheetResolution.test.ts` asserts the registry is non-empty for
that reason; the assertion was mutation-tested by breaking the pattern, and it
fails with the message that names the cause.
