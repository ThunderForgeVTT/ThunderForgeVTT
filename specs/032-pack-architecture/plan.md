# Implementation Plan: Pack Architecture — Interfaces Shaped By Their System

**Branch**: `032-pack-architecture` | **Date**: 2026-09-02 (revised) | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/032-pack-architecture/spec.md`

**Supersedes**: the first version of this plan, written before the five
clarifications. That plan scoped User Story 1 as colours and spacing and
described the interface half as "the cheap half". It is not, and the honest
statement of that is the first thing this revision owes the reader.

## Summary

This increment builds **User Story 1**, plus the interface-pack half of User
Story 3. User Story 2 remains gated on ADR-029; spec 031's T076 stays blocked.

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
| **IV. Real ADRs and specs before divergent implementation** | **Pass, three ADRs required.** ADR-046: an interface pack is data, not a module. ADR-047: the system contract — one declaration, declared values not a fixed struct, and where it lives. ADR-048: how a pack's implementation is discovered rather than listed. All land in the same change set. None touches ADR-029. |
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
- **The layout vocabulary is the part that can be got wrong quietly.** Too thin
  and Forged &lt;Metal&gt; cannot express a nine-level slot grid; too rich and it
  acquires conditionals, at which point it is a language and FR-003 is gone.
  FR-007a's conformance test is the guard, and it only guards what Forge uses.

## Deferred

- **User Story 2 (system packs mounting their own surfaces), and spec 031's
  T076.** Gated on ADR-029. Note the distinction this increment relies on:
  ADR-029 governs loading *third-party* code at runtime. Bundled pack crates
  are Cargo workspace members compiled into the product, which is why
  Increment A is not gated.
- **The system-pack half of User Story 3** (FR-019 to FR-021).
- **An `interface_packs` table and an upload flow.** Bundled packs only.
- **Third-party system packs**, per FR-017's interim restriction.
- **Removing `apps/web/src/styles/tokens.scss`** — zero importers, unrelated
  tidying.

## Complexity Tracking

No constitution violations to justify. See [Risks](#risks) for where this is
expected to be hard.
