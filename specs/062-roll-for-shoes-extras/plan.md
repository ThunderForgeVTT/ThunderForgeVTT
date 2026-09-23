# Implementation Plan: Roll for Shoes Extras

**Branch**: `main` | **Date**: 2026-09-23 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/062-roll-for-shoes-extras/spec.md`

## Summary

Five independent, per-world, off-by-default settings for the `roll_for_shoes`
pack: difficulty modes, the tie rule, statuses, skill slots, and customised
starting skills. Spec 061 shipped the core six rules and named each of these as
later work.

The approach follows what the pack already is. Four of the five Extras are
rules, and the pack's rules live in one pure module, `web/src/game.ts`, tested by
`node --test` with no browser and no stack. Those four are changes there plus
the sheet that calls them. The fifth thing — the *settings* — has nowhere to
live: no per-world, per-system configuration surface exists in the product. Per
ADR-063 the pack grows a table of its own, `world_roll_for_shoes_settings`, and
two root GraphQL fields to read and write it, which turns `roll-for-shoes-server`
from a validator-only crate into one that owns storage. A pack-contributed
`world-settings` panel gives the Game Master somewhere to set them.

The reasoning for every one of those choices, and the four storage candidates
rejected, is in [research.md](./research.md).

## Technical Context

**Language/Version**: Rust 2021 (server, pack crate), TypeScript 5 (pack web, host), SQL (migration)

**Primary Dependencies**: Diesel + PostgreSQL, async-graphql, `inventory` for pack discovery, React 18, Playwright

**Storage**: PostgreSQL. One new pack-owned table, `world_roll_for_shoes_settings`, keyed by `world_id`. Per-character state (statuses, bought slots) extends the existing `world_actors` JSON slots — no new actor table.

**Testing**: `node --test` for `packs/systems/roll_for_shoes/web/src/game.test.ts` (the rules); `cargo test` for the pack's validators and the play/pause surface; Playwright for `apps/web/e2e/system-roll-for-shoes-extras.spec.ts` (the proof).

**Target Platform**: Linux server, browser (Chromium under Playwright)

**Project Type**: Web application — Rust/Axum backend, React frontend, with a bundled system pack spanning both.

**Performance Goals**: None specific. The settings are read once per sheet mount and cached with the sheet's other reads; no new per-roll round trip.

**Constraints**: No shared file under `src/server/src`, `src/app/src` or `apps/web/src` may name the system (FR-006, enforced by `scripts/check-system-registry.mjs`). Every Extra defaults off, and a world touching none of them must play exactly the core game — `apps/web/e2e/system-roll-for-shoes.spec.ts` passes unmodified (FR-004).

**Scale/Scope**: 45 functional requirements in 6 groups, 6 user stories. One migration, one new pack Rust module, one new pack panel, ~5 new pure functions in `game.ts`, one new e2e spec file.

## Constitution Check

*GATE: passed before Phase 0 research; re-checked after Phase 1 design.*

- **I. A change is described before it is made** — spec.md and this plan precede
  any code. PASS.
- **II. The product is one product** — the Extras are pack-local. Nothing in the
  host learns that Roll for Shoes exists; the `world-settings` slot and
  `import.meta.glob` discovery were built for exactly this. PASS.
- **III. Data has one owner** — the settings row is owned and written by
  `roll-for-shoes-server`, not by the host. Per-character state stays in the
  actor JSON the host already owns and the pack already validates. PASS.
- **IV. A significant decision leaves an ADR** — **ADR-108** lands in this change
  set: declining to build a generic world-settings surface at the moment
  ADR-063's own "two is a shape" threshold was met, and what the third case
  should do instead. See research D1.
- **V. Licensed content carries its licence** — no new content. Roll for Shoes is
  CC0 1.0 (Ben Wray, rollforshoes.com) and the pack's existing `legal` block
  already carries it. These Extras are published on the same site under the same
  dedication, so no attribution obligation changes. PASS.

- **VI. Every feature is proven by its own slice** —

  - **Slice**: `game-systems`, run as `pnpm e2e:game-systems`.
  - **Its own specs**: `apps/web/e2e/system-roll-for-shoes-extras.spec.ts`,
    new. The slice's `own` prefix is `system-`, so the file joins by being
    named, with no edit to `scripts/e2e/slices.json` and none to
    `package.json`. `apps/web/e2e/system-roll-for-shoes.spec.ts` stays in the
    slice and must pass unchanged (FR-004).
  - **Surfaces changed, and the neighbouring specs that cross each seam**:
    - *The pack's actor sheet and its stored JSON* — no seam outside the pack;
      the host reads `system_data` opaquely. Covered by the slice's own specs.
    - *Statuses* — the slice already carries `status-systems.spec.ts` as a
      neighbour, with the seam recorded as "each game system supplies the
      statuses the status display shows". Roll for Shoes statuses are pack-local
      narrative labels and do not feed that display, but the neighbour stays:
      the claim that they do *not* leak is worth a passing spec.
    - *The `world-settings` panel slot* — `slices.json` already lists
      `apps/web/src/panels/**`, `apps/web/src/host/**` and
      `WorldSystemSettingsPage.tsx` in the slice's `paths`. Genie fills the same
      slot and its specs are in this slice.
  - **Standalone half**: none. `e2e:game-systems:standalone` does not exist and
    this feature does not add one — every scenario needs a world, an actor, a
    persisted setting and a server-rolled die.
  - **Cross-cutting after all** (corrected during implementation, T005). This
    block originally claimed the change touches nothing cross-cutting. Running
    `node scripts/e2e-slice.mjs which` over the paths the change actually
    touches contradicts that: three of them match the lookup's `crossCutting`
    rules, which fire *before* any slice match, so the tool's verdict is the
    full suite.

    - `src/server/migrations/**` — "schema history every slice reads". A
      pack-owned table still needs a migration in the server's one migrations
      directory, so owning the *declaration* elsewhere does not keep the
      *migration* out of a shared path. Unavoidable for any feature that adds
      a table.
    - `src/app/src/schema_roots.rs` — "the roots of its schema". Contributing a
      root field means naming the type at the composition root. Unavoidable for
      any pack that contributes GraphQL.
    - `src/server/Cargo.toml` — "the server's features and dependencies". This
      one was not foreseen at all: linking the pack into the server's *test*
      binary is what makes `play_pause_surface` see the new `PackSurface`
      rather than pass vacuously. See T012.

    The correction does not change the plan's shape. `game-systems` remains the
    slice that proves the feature's behaviour, and it still needs no edit. What
    changes is the merge gate: the honest statement is that the full suite is
    the pre-merge check here, per Principle VI's own rule that the full suite
    stays the cross-cutting check. The slice is the per-change gate, and the
    full suite is currently deferred by the project owner.

    Two further paths — `diesel.toml` and `src/server/diesel.toml` — are
    **uncovered**: no slice and no rule claims them. They are `print_schema`
    codegen configuration that nothing loads at runtime, so no e2e can prove
    them; T006 is what proves them. Recorded rather than resolved by widening a
    shared tool's globs.

  Unit coverage sits underneath, not instead: `game.test.ts` proves the
  arithmetic of bands, ties, status sums and slot costs without a stack, so the
  e2e proves those rules reached a screen rather than re-deriving them.

## Project Structure

### Documentation (this feature)

```text
specs/062-roll-for-shoes-extras/
├── spec.md              # Feature specification
├── checklists/
│   └── requirements.md  # Spec quality checklist (all items pass)
├── plan.md              # This file
├── research.md          # Phase 0 — D1..D8
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1
├── contracts/           # Phase 1
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
packs/systems/roll_for_shoes/
├── system.json                          # + statuses data_type, + startingSkills
├── web/src/
│   ├── game.ts                          # + bands, tie rule, statuses, slots; resolve()
│   ├── game.test.ts                     # + unit coverage for each of the above
│   ├── settings.ts                      # NEW — the settings type, defaults, GraphQL reads
│   ├── ActorSheet.tsx                   # reads settings; GM dice; slot prompt; statuses
│   ├── components/
│   │   ├── StatusList.tsx               # NEW
│   │   ├── DifficultyPicker.tsx         # NEW
│   │   └── AdvancementPrompt.tsx        # + "no room at this level"
│   └── panels/
│       └── world-settings.tsx           # NEW — the five toggles, GM-only
└── server/
    ├── Cargo.toml                       # + diesel, async-graphql, thunderforge-server
    └── src/
        ├── lib.rs                       # + settings module
        ├── validators.rs                # + rules for statuses and bought slots
        └── settings/
            ├── mod.rs                   # NEW — row model, read/write
            ├── schema.rs                # NEW — table! for world_roll_for_shoes_settings
            ├── graphql.rs               # NEW — the read field and the gated write
            └── play_pause_surface.rs    # NEW — PackSurface classification

src/server/migrations/
└── 2026-09-2X-XXXXXX-0000_roll_for_shoes_world_settings/{up,down}.sql   # NEW

diesel.toml                              # + ^world_roll_for_shoes_ to except_tables
src/server/diesel.toml                   # + the table name to except_tables
src/server/Cargo.toml                    # already depends on the pack crate
src/app/src/system_packs.rs              # already carries `use roll_for_shoes_server as _;`

apps/web/e2e/
├── system-roll-for-shoes.spec.ts        # unchanged — FR-004
└── system-roll-for-shoes-extras.spec.ts # NEW — the proof

docs/adrs/
└── 20260923-108-<slug>.md               # NEW — see Constitution Check IV
```

**Structure Decision**: the pack directory is the unit. Everything the feature
adds lives under `packs/systems/roll_for_shoes/` except four things that
cannot: the migration (migrations are one directory by Diesel's design), the
two `diesel.toml` `except_tables` entries (which exist precisely so a pack's
tables stay out of the server's schema), the e2e spec (the suite is one
directory), and the ADR. None of those four names the system inside shared
*source*, so `check-system-registry.mjs` stays green with an empty `KNOWN` list.

`src/app/src/system_packs.rs` and `src/server/Cargo.toml` already carry the
pack, so the "one line outside your directory" is already written — adding
GraphQL to the crate does not add a second.

**Slice**: `scripts/e2e/slices.json` needs **no edit**. The `game-systems` entry
already declares `own: ["system-"]`, which adopts
`system-roll-for-shoes-extras.spec.ts` by name, and its `paths` already cover
`packs/systems/{...,roll_for_shoes,...}/**`, `apps/web/src/host/**`,
`apps/web/src/panels/**` and `WorldSystemSettingsPage.tsx`. The root
`package.json` already has `e2e:game-systems`. The slice measured 176s for 18
specs at its last recording (`scripts/e2e/slice-durations.json`); this feature
re-records it.

## Complexity Tracking

One deviation is worth naming, though it is a cost rather than a violation.

| Deviation | Why needed | Simpler alternative rejected because |
|---|---|---|
| `roll-for-shoes-server` gains `diesel`, `async-graphql` and `thunderforge-server`, and stops being the table-free crate its `Cargo.toml` advertises | The five Extras are *per-world settings*. A setting that lives nowhere cannot vary by world, and no per-world, per-system configuration surface exists in the product | A column on `worlds` — the Genie shape, which Genie's own source files as a defect and which `check-system-registry.mjs` cannot catch. A generic world-settings registry — now justified by ADR-063's "two is a shape" test, but a spec of its own, not a paragraph of this one (research D1, ADR-108) |
