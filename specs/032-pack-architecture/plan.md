# Implementation Plan: Pack Architecture — Interfaces Shaped By Their System

**Branch**: `032-pack-architecture` | **Date**: 2026-09-02 (revised) | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/032-pack-architecture/spec.md`

**Supersedes**: the first version of this plan, written before the five
clarifications. That plan scoped User Story 1 as colours and spacing and
described the interface half as "the cheap half". It is not, and the honest
statement of that is the first thing this revision owes the reader.

## Summary

This increment builds **User Story 1**, plus the interface-pack half of User
Story 3.

> **Increment F (2026-09-03) plans User Story 2.** ADR-029 is written and the
> gate has closed: outside code is not executed, and executable extension is
> bundled-only, so a bundled pack contributing behaviour is permitted. Spec
> 031's T076 shipped separately — it turned out never to have needed pack code
> at all. See [Increment F](#f--user-story-2-a-system-brings-its-own-way-of-working)
> below; everything above this line is delivered history.

What changed: an interface pack is no longer only a palette. It also declares
**layout** over the values a system declares, which means three things have to
exist that do not:

1. **One contract every system pack implements** to supply its declared
   values, stored and derived (FR-027, FR-028). Two divergent declarations of
   this exist today and neither is used.
2. **Derived values at all.** Nothing in the product computes one value from
   another. A 5e ability modifier has no home.
3. **Layout as data** that can address a system generically or by name
   (FR-025a), so Forge composes against a system nobody has written a pack for
   while still exercising every construct the format offers (FR-025b, FR-007a).

The safety property is unchanged and still load-bearing: a pack declares, and
never computes. That is what keeps this whole increment outside ADR-029's gate
even as it grows.

**What is still cheap.** The world binding needs no migration —
`worlds.interface_pack_id` has been carried end-to-end since the phase-3
metadata migration and is read by nothing. The canvas palette needs no engine
change — `set_display_appearance` is implemented, typed in the TypeScript SDK,
and has no caller. The theme vocabulary is already runtime-swappable custom
properties, and `tokens.scss` — which looked like a build-time-only second
token system — is imported by zero files.

**What is not cheap.** The system contract, derived values, the layout format
and its validator, and reconciling two 5e implementations: roughly 1,190 dead
lines in `packs/systems/dnd5e/web/` beside roughly 1,130 live ones in
`apps/web/src/components/game-systems/dnd5e/` and `apps/web/src/systems/dnd5e/`.
`dnd5e` is the only system with a module in shared app code, which is SC-004's
violation standing in the open.

**And it grew again.** Increment E was added after three published sheets —
5e's 336 fields, Fate's 51, Cypher's 55 — were read for scope while building
A–D. Two of the three are mostly free text, Cypher's damage track has no marks
to count, and Fate has no attributes at all. The format fitted the first
ruleset that needed it, which is the mistake the attribute and resource
declarations were each written to correct; this is its third appearance. The
interface half was scoped as the cheap unblocked half and is still unblocked —
nothing in E asks a pack to execute anything — but "cheap" stopped being true
two increments ago and should not be repeated.

## Technical Context

**Language/Version**: Rust 1.98 (contract, validator, server, pack crates),
TypeScript 5.x / React 19 (web), Rust→WASM (engine)

**Primary Dependencies**: `thunderforge-canvas-core` as the contract's home —
**both `src/engine/Cargo.toml` and `src/server/Cargo.toml` already depend on
it**, which is the whole reason the contract can be stated once; `schemars` +
`serde` for manifest schema and `deny_unknown_fields`; Axum + async-graphql +
Diesel; Tailwind v4 custom properties

**Storage**: PostgreSQL. **No migration.** The binding column exists; declared
values are resolved from existing JSONB actor data plus the system's own
computation, and derived values are never stored — deriving and storing the
same number is how the two disagree.

**Testing**: `cargo test` (contract, validator, pack rules — natively, which is
the reason the contract lives in canvas-core), `vitest` (web), Playwright
(e2e, Chromium)

**Target Platform**: Chromium-only, per the constitution

**Project Type**: Web application — Rust backend, React frontend, WASM engine,
pack crates in the same workspace

**Performance Goals**: SC-001's 30 seconds is a usability bound. Applying a
pack is a custom-property write plus one engine command. Derived values are
resolved server-side per actor read, alongside the attribute and resource
resolution that already happens there.

**Constraints**: A pack switch must not reload the page and must reach every
participant. Forge must compose against a system that ships after Forge does.
No pack may compute.

**Scale/Scope**: Two interface packs at ship — Forge (generic addressing only)
and one Forged &lt;Metal&gt; targeting a real system. Two system packs carrying
declared derived values — genie first, being the only one
`IMPLEMENTED_SYSTEM_IDS` actually contains.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Assessment |
|---|---|
| **I. ECS owns simulation, React owns chrome** | **Pass.** Declared values are character data, not canvas simulation state; the canvas consumes them exactly as it consumes resource declarations today. An interface pack changes presentation only. No pack adds an entity or a system. |
| **II. Plugin-modular engine architecture** | **Pass, and this is the principle the increment serves.** FR-027 to FR-029 are Principle II applied to system packs: self-contained, independently addable, communicating through a declared contract rather than through each other's internals. The engine itself is unchanged. |
| **III. Ownership & authorization at the data boundary** | **Pass.** Changing a world's interface pack is authorized server-side by `is_dm_of_world`, mirroring `update_world_game_system_impl`. Derived values are computed server-side from stored data the server already owns. |
| **IV. Real ADRs and specs before divergent implementation** | **Pass, three ADRs required.** ADR-059: an interface pack is data, not a module. ADR-060: the system contract — one declaration, declared values not a fixed struct, and where it lives. ADR-061: how a pack's implementation is discovered rather than listed. All land in the same change set. None touches ADR-029. |
| **V. Verify before claiming done** | **Pass.** Contract and rules tested natively; engine unchanged but linted via `verify`'s wasm step; quickstart run by hand. |

**DMCA / content-moderation guardrail**: not triggered — this governs how the
product is extended, not how a world's compendium content becomes visible
outside that world. Noted separately: **FR-003b** is a copyright constraint of
a different kind, on presentation rather than distribution, and it is a
requirement rather than a guardrail check.

**Result: PASS.** Complexity Tracking is empty, but see [Risks](#risks) — the
increment is large, and saying so is not the same as violating a principle.

## Project Structure

### Documentation (this feature)

```text
specs/032-pack-architecture/
├── plan.md              # This file (revised)
├── research.md          # Phase 0 — revised; nine decisions
├── data-model.md        # Phase 1 — revised
├── quickstart.md        # Phase 1 — revised
├── contracts/
│   ├── interface-pack-manifest.md   # Revised: layout, targeting
│   ├── system-contract.md           # NEW: what a system pack implements
│   └── graphql-appearance.md        # Largely unchanged
├── checklists/requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
crates/thunderforge-canvas-core/src/
├── attributes.rs                    # exists — declaration precedent
├── resource_display.rs              # exists — declaration precedent
└── system_rules.rs                  # NEW — the one contract (FR-027)

crates/pack_system_spec/src/
├── lib.rs                           # system manifest (unchanged)
├── interface.rs                     # NEW — InterfaceManifest: tokens, layout, targets
├── layout.rs                        # NEW — layout vocabulary, generic + specific
└── contrast.rs                      # NEW — the FR-012a legibility floor

packs/interface/                     # NEW
├── forge/interface.json             # base; generic addressing only (FR-025b)
└── forged-steel/interface.json      # targets a real system; specific addressing

packs/systems/genie/server/          # gains its SystemRules implementation
packs/systems/dnd5e/server/          # gains its SystemRules implementation
packs/systems/dnd5e/web/             # the surviving 5e presentation (FR-030)

src/engine/src/systems/core.rs       # RETIRED into canvas-core (FR-027)
packs/systems/dnd5e/engine/src/plugin.rs  # its duplicate trait, likewise

src/server/src/
├── interface_packs.rs               # NEW — list/manifest routes
├── systems.rs                       # register_*_system list → discovery (FR-029)
├── graphql.rs                       # updateWorldInterfacePack; declared values
└── world_events.rs                  # EVENT_CODE_WORLD_APPEARANCE_CHANGED

apps/web/src/
├── appearance/                      # NEW — context, provider, resolution
├── layout/                          # NEW — renders a Layout Declaration
├── pages/world/settings/WorldAppearanceSettingsCard.tsx
├── components/game-systems/dnd5e/   # REMOVED or reduced (FR-030)
└── systems/dnd5e/                   # REMOVED (FR-030)

scripts/check-system-registry.mjs    # NEW — FR-029, modelled on the seam check
```

**Structure Decision**: the contract lives in `thunderforge-canvas-core`
because the dependency graph already permits it and nothing else does — the
5e engine pack re-declared the trait explicitly "to avoid cross-package
dependency", and canvas-core is the dependency both sides already have. It is
also where this codebase has twice put exactly this kind of thing, and the
reason it gives each time is that its tests execute natively while the engine's
do not.

## Increments

Four, each a checkpoint. The first two are prerequisites with nothing a Game
Master can see; that is stated rather than disguised.

### A — Declared values end to end (foundational)

One contract in canvas-core; genie implements it; stored and derived values
resolve to one `identifier → value` set the server returns. Retires both
existing trait declarations and the fixed `DerivedStats`. Replaces the
`register_*_system` list with discovery.

*Checkpoint*: an actor in a genie world reports its declared values, derived
ones included, through one path. Nothing looks different.

### B — The pack format, its validator, and Forge

`InterfaceManifest` with tokens, canvas, layout and targets; the five
validation checks plus compatibility and the Forge conformance test; Forge
authored with generic addressing only, reproducing today's look exactly.

*Checkpoint*: `cargo test -p pack_system_spec` passes; a malformed or
illegible or over-claiming pack is refused by name. Still nothing looks
different.

### C — User Story 1: the Game Master dresses the table 🎯 MVP

Routes, mutation, world event, the appearance provider, the layout renderer,
the picker, propagation without reload.

*Checkpoint*: **demonstrable.** A GM picks Forge for a genie world, the table
sees it, an actor renders through generic layout, and nothing else moved.

### D — A targeted pack, and one 5e implementation

Forged &lt;Metal&gt; targeting a real system with specific addressing; dnd5e's
`SystemRules`; the duplicate 5e presentation resolved in the pack's favour;
User Story 3's interface half.

*Checkpoint*: two packs, two shapes, and `apps/web/src/systems/` is gone.

### E — Every shipping system has a usable sheet (User Story 4)

Added 2026-09-02, after three published sheets were read for scope while
building A–D and each disagreed with the format differently. The declaration
vocabulary grows from numbers and pools to cover a whole character sheet: text,
player-named blank slots, ordered lists, marked tracks, named ordered states,
and groups whose parts belong together. Forge renders every kind, which is what
makes "usable sheet for every system" a property rather than a promise. The
bundled manifests are completed to declare what their sheets actually track.

*Checkpoint*: bind a world to each bundled system in turn and open an actor
under the base pack alone. Every system has a sheet worth reading, and none of
them needed a pack written for it.

### F — User Story 2: a system brings its own way of working

Planned 2026-09-03, after ADR-029 closed the gate. **Three of the six
acceptance scenarios are already satisfied** by Increments A–E, and saying so
is what keeps this increment honest about its actual size:

| Scenario | State |
|---|---|
| 1. A system's sheet is presented with no shared branch naming it | **Done** — `PackActorSheet` renders from declarations |
| 2. Two worlds, two systems, visibly different sheets | **Done** — `world-appearance.spec.ts` T080 proves it |
| 3. A system declaring no sheet gets a system-agnostic default | **Done** — Forge, plus a named empty state |
| 4. Installation is validated against the recorded security terms | **Open**, and now trivial: ADR-029 says bundled-only |
| 5. A failing pack surface is contained and names the pack | **Open** — nothing contains it today |
| 6. A published contract exists for pack authors | **Open** — no author-facing document |

So what is left is **not** "mount a sheet". It is four things: stop the shared
application naming systems, let a pack contribute *behaviour* rather than only
declarations, contain a pack's failure, and write the contract down.

#### F1 — The application stops knowing which systems exist (FR-005, SC-004)

`apps/web/src/api/gameSystems.ts` holds `BUNDLED_SYSTEM_IDS` and
`BUNDLED_SYSTEM_LABELS`: two hand-kept lists naming all seven systems and
their titles, in shared web code. SC-004 says adding a system must touch only
that system's own pack directory, and today it demands an edit here.

The lists exist for a stated reason — the `/api/systems` route reads the
`game_systems` **database table**, and that table has **0 rows**. It has never
been seeded with the bundled packs. So the honest server answers an empty list
and the client compensates with a literal.

The fix already exists one directory over: `/api/interface-packs` lists packs
by reading `packs/interface/`, which is why nothing on the client hardcodes
interface pack names. `/api/systems` does the same for `packs/systems/`, and
both web lists are deleted.

This makes ADR-028 (*Game Systems DB Model and Ownership Rules*, also an empty
stub) a live question — what is the `game_systems` table **for**, if the
directory is the source of truth? That question is in scope here, and its
answer is likely "installed third-party systems, which ADR-029 says there are
none of" — i.e. the table is premature and the row of record is the directory.

#### F2 — A pack contributes behaviour, not only declarations (FR-004, T014a2)

`graphql.rs` branches on `game_system_id == "genie"` at world creation and
inserts a `world_genie_sessions` row. That is the last system name in shared
server code, and the only remaining entry in `check-system-registry.mjs`'s
`KNOWN` list.

The real obstacle is not the hook — it is **table ownership**. `genie-server`
has no `diesel` dependency, and `world_genie_sessions` lives in the server's
generated `schema.rs`, produced by `diesel print-schema` from migrations that
are also the server's. A pack cannot write to its own table because it does
not have one, in any sense the code recognises.

Three sub-decisions, all genuinely open, all in Phase 0's remit:

- **Where a pack's migrations live.** Diesel expects one migrations directory.
  A pack owning `migrations/` under its own directory needs either a merged
  migration source or an embedded-migrations-per-pack arrangement.
- **Whether `schema.rs` keeps pack tables.** It is generated from the database,
  so it will keep producing them. Two `table!` declarations for one table is
  exactly the drift this spec has spent five increments removing.
- **The hook's signature.** `inventory` + a typed `fn(&mut PgConnection, ...)`
  is the proven pattern from `SystemContribution`, and it requires `diesel` in
  the pack crate — which is the decision above, restated.

**This is the increment's hard part and its main risk.** A cheap answer here
produces the schema duplication the whole feature exists to retire.

#### F3 — A pack's failure is contained and named (FR-016, SC-009)

Nothing contains a pack-contributed surface today. `PackActorSheet` catches a
*fetch* failure and says the sheet could not load, which is not the same as a
surface that throws while rendering — that takes the page with it, and the
message names nothing.

SC-009 measures both halves: the rest of the session stays usable, and the
message names the responsible pack. An error boundary around each mounted
surface, told which pack it wraps, is the shape. `MissingPackNotice`'s
precedent applies — say it plainly, block nothing.

#### F4 — The contract exists as a document (FR-015, SC-010)

SC-010: an author can produce a working pack **from the published contract
alone, without reading shared application source**, and the contract has zero
references to documents that do not exist.

`packs/interface/README.md` does this for interface packs. There is no
equivalent for system packs, and `contracts/system-contract.md` in this spec
directory is a design artefact for us rather than a document for an author.
The deliverable is an author-facing `packs/systems/README.md` describing every
declaration block — `abilities`, `resources`, `movement`, `sheet`, `groups`,
`turnStructure` — and every hook a pack may contribute.

Note this is testable rather than merely assertable: a check that every
document the contract references exists is the same shape as
`check-system-registry.mjs`.

*Checkpoint*: a second bundled pack contributes a world-creation behaviour and
a surface; `check-system-registry.mjs` reports **zero** outstanding violations;
an injected failure in a pack surface leaves the session usable and names the
pack; and `packs/systems/README.md` describes what a pack may declare with no
dangling references.

## Risks

Recorded because the previous plan's confidence did not survive contact with
the spec, and a plan that does not say where it is likely to be wrong is worse
than one that does.

- **Increment D is the one that will slip.** Reconciling two 5e
  implementations touches live UI covered by e2e specs, and spec 031's own
  history says a control that moves breaks tests that reach it by placeholder
  and accessible name rather than testid. Grep for both before moving anything.
- **FR-029's "discovered rather than listed" has no settled mechanism.** The
  precedent the spec names — the interaction seam — still gathers a
  `contributions()` list; what it actually enforces is that the *core* owns no
  effect. Research §6 picks a mechanism; it is the decision most likely to need
  revisiting.
- **The layout vocabulary is the part that can be got wrong quietly** — and it
  was, four times, each found by building something against it rather than by
  review. `barStack` had no maximum, `slotGrid` could not address its own
  per-level numbers, `tracker` fitted one of three systems, and there was no
  way to declare text at all. The guard that worked was writing a real pack and
  a real renderer; the guard that did not was reading the type and thinking it
  looked complete. Increment E should expect the same, and the same remedy:
  build a Fate pack and a Cypher pack against it before calling it settled.
- **Increment E's genuine risk is the opposite of the earlier one.** Too thin
  and Fate renders two numbers; too rich and the format acquires conditionals
  and becomes a language, at which point FR-003 is gone and the whole increment
  is behind ADR-029. Every kind FR-031 admits is a *shape of value*, never a
  rule about one — that is the line, and it is easier to hold when stated
  before the work than argued afterwards.

## Deferred

- **The system-pack half of User Story 3** (FR-019 to FR-021).
- **An `interface_packs` table and an upload flow.** Bundled packs only.
- **Third-party system packs**, per FR-017's interim restriction.
- **Removing `apps/web/src/styles/tokens.scss`** — zero importers, unrelated
  tidying.

## Complexity Tracking

No constitution violations to justify. See [Risks](#risks) for where this is
expected to be hard.
