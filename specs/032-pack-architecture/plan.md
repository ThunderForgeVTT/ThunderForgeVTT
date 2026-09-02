# Implementation Plan: Pack Architecture — Interface Packs Are Themes

**Branch**: `032-pack-architecture` | **Date**: 2026-09-02 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/032-pack-architecture/spec.md`

## Summary

This increment builds **User Story 1 only** — the interface-pack half — plus the
interface-pack portions of User Story 3 (FR-018, FR-022, FR-023), because
shipping a pack selector while two screens still describe the binding as
"Unbound placeholder" would leave the feature telling a lie about itself.

**User Story 2 (system packs) is not in this increment.** It is gated on
ADR-029, which is an empty stub. Spec 031's T076 — turn structure becoming
system-supplied — sits inside that gate and stays blocked. This is a
deliberate scope decision, not an omission: see [Deferred](#deferred).

The technical approach, in one sentence: **an interface pack is a data file, not
a module**, declaring values for the CSS custom properties the application
already themes with and for the engine's existing `AppearanceOverride` — which
makes FR-003's "an interface pack MUST NOT contribute behaviour" structural
rather than policed, because the format has nowhere to put code.

Three things this feature needs already exist and are unused, which is most of
why the interface half is the cheap half:

| Exists | State today |
|---|---|
| `worlds.interface_pack_id` (nullable varchar) | Validated on write, carried through GraphQL and the web types, **read by nothing** but two labels |
| Engine `set_display_appearance` command + `AppearanceOverride` | Typed in `apps/web/src/engine/sdk/commands.ts`, **no caller** |
| CSS custom properties in `globals.css` (`:root` / `.dark`) | The app's entire theme vocabulary, already runtime-swappable |

## Technical Context

**Language/Version**: Rust 1.98 (server, `pack_system_spec`), TypeScript 5.x /
React 19 (web), Rust→WASM (engine, unchanged by this increment)

**Primary Dependencies**: Axum + async-graphql + Diesel/PostgreSQL; React +
Tailwind v4 + Radix; `schemars` for manifest schema, already used by
`pack_system_spec`

**Storage**: PostgreSQL. `worlds.interface_pack_id` already exists — **no
migration for the binding**. Bundled packs are read from disk, mirroring how
`src/server/src/systems.rs` serves `system.json`; no `interface_packs` table in
this increment (see research.md §3).

**Testing**: `cargo test` (server + `pack_system_spec` validator), `vitest`
(web unit), Playwright (e2e, Chromium)

**Target Platform**: Chromium-only, per the constitution's supported-browsers
constraint

**Project Type**: Web application — Rust backend, React frontend, WASM engine

**Performance Goals**: SC-001's 30 seconds is a usability bound, not a
throughput one. A pack switch is a custom-property write on `document
.documentElement` plus one engine command; no reload, no re-fetch of scene
content.

**Constraints**: A pack must be applicable without a page reload (SC-001) and
must reach every participant in the world without one (SC-001). The look must
cover both the React chrome and the Bevy canvas, which are two different
renderers with two different palettes.

**Scale/Scope**: Two packs at ship (Forge, plus one visibly different pack that
exists to prove the mechanism and to make SC-002 testable). ~30 CSS custom
properties, 7 `AppearanceOverride` fields.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Assessment |
|---|---|
| **I. ECS owns simulation, React owns chrome** | **Pass, and load-bearing.** An interface pack changes only presentation, which is precisely the half React owns. The canvas half reaches the engine as `set_display_appearance` — a palette override on an existing resource, not simulation state. No pack may add an entity, a system, or a rule. |
| **II. Plugin-modular engine architecture** | **Pass, no engine change.** `StatusDisplayPlugin` already owns the `Appearance` resource and already accepts an override. This increment adds a caller, not a plugin. |
| **III. Ownership & authorization at the data boundary** | **Pass, with new surface.** Changing a world's interface pack is a mutation on `worlds`, authorized server-side by `is_dm_of_world`, mirroring `update_world_game_system_impl` exactly (FR-010). |
| **IV. Real ADRs and specs before divergent implementation** | **Pass, with one required artifact.** The pack-as-data decision is architecturally significant and gets **ADR-046**, landing in the same change set. This plan does **not** touch ADR-029, which governs pack-supplied *code* — the decision this increment is designed to not need. |
| **V. Verify before claiming done** | **Pass.** Per-crate targets as usual; the engine is unchanged but is now linted via `verify`'s wasm step. FR-012a's contrast rule gets unit tests in Rust and an e2e that proves a failing pack is refused. |

**DMCA / content-moderation guardrail**: not triggered. This feature governs how
the product is extended, not how one world's compendium content becomes visible
outside that world. Recorded because the guardrail names "any future content
type" and a pack is a content type in the loose sense — but a pack ships with
the product or is installed by an operator, and nothing here exposes a world's
content to another world.

**Result: PASS.** No entries in Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/032-pack-architecture/
├── plan.md              # This file
├── research.md          # Phase 0 — the seven decisions this design rests on
├── data-model.md        # Phase 1 — manifest shape, binding, degraded state
├── quickstart.md        # Phase 1 — how to prove it works by hand
├── contracts/
│   ├── interface-pack-manifest.md   # What a pack author writes
│   └── graphql-appearance.md        # Mutation, query field, world event
├── checklists/
│   └── requirements.md  # Answered 2026-09-02
└── tasks.md             # NOT created by /speckit-plan
```

### Source Code (repository root)

```text
packs/
└── interface/                       # NEW — sibling of packs/systems/
    ├── forge/interface.json         # The base pack (FR-007)
    └── <second>/interface.json      # Exists to make SC-002 testable

crates/pack_system_spec/src/
├── lib.rs                           # System manifest (unchanged)
├── interface.rs                     # NEW — InterfaceManifest, validation
└── contrast.rs                      # NEW — the FR-012a legibility floor

src/server/src/
├── interface_packs.rs               # NEW — list/manifest routes, disk-backed
├── graphql.rs                       # update_world_interface_pack mutation
├── world_events.rs                  # EVENT_CODE_WORLD_APPEARANCE_CHANGED
└── main.rs                          # route mount

apps/web/src/
├── appearance/                      # NEW
│   ├── appearance-context.ts        # The context + hook (split, per the
│   │                                #   fast-refresh rule this repo now enforces)
│   ├── AppearanceProvider.tsx       # Applies custom properties; feeds the engine
│   └── packs.ts                     # Fetch + cache a world's pack
├── pages/world/settings/
│   └── WorldAppearanceSettingsCard.tsx  # NEW — the GM's picker and preview
├── pages/world/WorldDashboardPage.tsx   # FR-022: one wording
└── pages/world/components/WorldCard.tsx # FR-022: the same wording

apps/web/e2e/
└── world-appearance.spec.ts         # NEW — US1 scenarios 1, 2, 3, 6
```

**Structure Decision**: `packs/interface/` as a sibling of `packs/systems/`,
because the spec's FR-002 makes the type exclusive and a directory that cannot
hold both is the cheapest possible enforcement of it. The validator lives in
`pack_system_spec` rather than a new crate: it is the same question ("is this
manifest acceptable?") asked of a second manifest shape, and splitting it would
put the two pack types' validation rules where they cannot see each other —
which is exactly where FR-002's exclusivity check needs to stand.

## Deferred

Recorded so a later reading does not mistake absence for oversight.

- **User Story 2 (system packs), entirely.** Gated on ADR-029. FR-017's interim
  restriction — no system packs from any source but the product itself — is
  already the de facto state and needs no code to hold in this increment.
- **Spec 031 T076 (system-supplied turn structure).** Inside US2's gate. Spec
  031 cannot close on this increment.
- **`interface_packs` database table and an upload/install flow.** Bundled packs
  only. See research.md §3.
- **Removing `tokens.scss`.** It is imported by nothing (verified: zero
  importers) and is a fossil of a previous design system, but deleting it is
  unrelated tidying and belongs in its own change.

## Complexity Tracking

No constitution violations. Table intentionally empty.
