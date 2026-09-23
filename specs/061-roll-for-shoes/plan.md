# Implementation Plan: Roll for Shoes

**Branch**: `061-roll-for-shoes` | **Date**: 2026-09-22 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/061-roll-for-shoes/spec.md`

## Summary

Add Roll for Shoes — a complete roleplaying game in six rules, dedicated to the
public domain under CC0 — as a bundled system pack, and nothing else. A
character is a name, a description, an XP count and a lineage of skills that
starts at *Do Anything 1*. Rolling a skill rolls one d6 per level on the
server; beating the opposition makes the thing happen, failing pays 1 XP, and
every die showing a six grants a new, more specific skill one level higher.

The pack carries three parts, and the shape is forced rather than chosen
(research [R]):

- **`system.json`** declares what a character stores and the enforced legal
  metadata. It declares no abilities, no movement, no vision, no combat, no
  rounds and **no `checks`** — each absence is the ruleset's answer.
- **`web/src/ActorSheet.tsx`** is the game. The declarative sheet format holds
  shapes of values, never rules about them, and cannot express a lineage of
  objects at all [R:D1].
- **`server/`** exists so the system is *registered*; without a
  `SystemContribution` in the registry, every write of XP or a skill is refused
  [R:D2]. It also validates the two slots the pack stores.

No migration, no schema change, no new GraphQL root field. Rolls go through the
existing `rollDice`, whose per-die `finalValue` is what makes the all-sixes
check readable from real server dice [R:D3].

## Technical Context

**Language/Version**: Rust (2021, workspace toolchain) for `server/`;
TypeScript 5.3 + React 18 for `web/`; JSON for the manifest.

**Primary Dependencies**: `inventory` and `thunderforge_canvas_core` for the
system contribution; `serde_json` for validation; `@thunderforge/host` — the
only module the pack's web code may import — for the presentational primitives,
the actor-data hooks and `postGraphQL`. Built with `tsc && vite build`.

**Storage**: none added. The two JSONB slots `trait_data` and `resource_data`
of the existing `world_actor_system_data` table; rolls land in
`world_roll_records` as every roll does.

**Testing**: `cargo test -p roll_for_shoes_server` for the validators;
`node --test` for the pack's web logic (verdict, advancement, XP arithmetic);
one Playwright specification in the `game-systems` slice for the loop.

**Target Platform**: the existing web client and Axum server. No wasm — the
pack ships no `engine/` crate, because the game has no geometry.

**Project Type**: a bundled system pack, under the contract in
`packs/systems/README.md`.

**Performance Goals**: none specific. One roll is one mutation resolving at
most `level` d6; the sheet reads one actor's system data and refetches after
each write.

**Constraints**: the dice engine caps one resolution at `MAX_TOTAL_DICE =
1_000`, so a skill above level 1000 has its *roll* refused — rendered, never
clamped, because levels genuinely have no cap [R:D7]. Shared code must never
name this system: `check-system-registry.mjs` fails the build if it does,
allowing only the single linkage line.

**Scale/Scope**: one pack directory, three linkage lines outside it, one e2e
specification. The core six rules only; every optional rule from the source and
the whole variant ruleset are Out of Scope in the spec.

## Constitution Check

*GATE: passed before Phase 0 research; re-checked after Phase 1 design.*

- **I–III (the product's own principles)** — no conflict. The pack adds a
  ruleset and takes nothing away; it ships no AI adjudication, and by design it
  *declines* to arbitrate whether a player's new skill is specific enough
  (FR-036). That judgement stays at the table, which is the point of the game.
- **IV. ADRs and specs before divergent implementation** — satisfied by this
  spec. No new ADR is needed: the pack uses ADR-029's bundled-code allowance
  and ADR-074/ADR-044's server-resolved roll path exactly as they stand, and
  proposes no change to either.
- **V. Verify per target** — `pnpm verify` covers it: `rust-fmt`, `rust-lint`,
  `web-fmt`, `web-lint`, `registry`, `packdocs`, `filelength` and `e2e-slices`
  all apply. The pack lints for the host only; there is no wasm target here.
- **VI. Every feature is proven by its own slice** — **`game-systems`**, run as
  `pnpm e2e:game-systems`.
  - *Its own specs*: `system-settings.spec.ts`, `system-panel-slots.spec.ts`,
    `system-change-guard.spec.ts`, and this feature's new
    `system-roll-for-shoes.spec.ts`, which the slice's `system-` prefix owns.
  - *Neighbours, per seam*: `status-systems.spec.ts` — each game system
    supplies the statuses the status display shows.
  - *Standalone half*: none. Everything this feature does needs a server, a
    database and a real roll.
  - *Not cross-cutting*: the feature adds no GraphQL schema field, touches no
    auth, no migration, no harness and no shared UI primitive. Its only files
    outside the pack directory are three build-graph lines and the slice entry
    itself.

No violations. Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/061-roll-for-shoes/
├── spec.md                      # the feature specification
├── source-digest.md             # the crawl of rollforshoes.com, kept with the spec
├── plan.md                      # this file
├── research.md                  # Phase 0 — nine decisions, each grounded in code
├── data-model.md                # Phase 1 — the character, the skill, the validators
├── quickstart.md                # Phase 1 — run it, and the proof command
├── contracts/
│   ├── system-manifest.md       # what system.json declares, and what it declines to
│   ├── sheet.md                 # the contributed sheet: props, imports, roll call, test ids
│   └── server-contribution.md   # the registration and the two validators
├── checklists/
│   └── requirements.md
└── tasks.md                     # Phase 2 — written by /speckit-tasks, not here
```

### Source Code (repository root)

```text
packs/systems/roll_for_shoes/
├── system.json                  # identity, enforced legal metadata, data_types,
│                                #   resources (the XP counter), one sheet entry,
│                                #   turnStructure { rounds: false }
├── server/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs               # SYSTEM_ID, the inventory::submit! contribution
│       ├── validators.rs        # trait_data and resource_data, rules T1-T11 / R1-R3
│       └── validators_tests.rs
└── web/
    ├── package.json             # @thunderforge/roll-for-shoes
    ├── tsconfig.json
    ├── vite.config.ts
    └── src/
        ├── index.ts             # the module entry
        ├── ActorSheet.tsx       # discovered by the glob; the whole game
        ├── components/          # SkillLineage, RollResult, XpSpend, AdvancementPrompt
        ├── game.ts              # verdict, XP award, all-sixes check, skill grant
        └── game.test.ts         # node --test, no browser, no server

apps/web/e2e/
└── system-roll-for-shoes.spec.ts

# The three lines outside the pack directory, and nothing more:
Cargo.toml                       # members: packs/systems/roll_for_shoes/server
src/app/Cargo.toml               # dependency on roll_for_shoes_server
src/app/src/system_packs.rs      # use roll_for_shoes_server as _;
```

**Structure Decision**: a bundled system pack under `packs/systems/`, following
the contract in `packs/systems/README.md` and modelled on Genie, which is the
only existing pack that both stores structured data and contributes its own
sheet. The rules of the game live in `web/src/game.ts` as pure functions so
they can be tested without a browser or a server, and `ActorSheet.tsx` draws
them.

**Slice**: `scripts/e2e/slices.json` gains
`packs/systems/roll_for_shoes/**` in the `game-systems` slice's `paths` —
extending the existing brace list of pack directories — and the new
specification is named `system-roll-for-shoes.spec.ts` so that slice's `system-`
prefix owns it and `pnpm verify`'s `e2e-slices` check reports no orphan. The
root `package.json` already has `e2e:game-systems` and
`e2e:game-systems:integration`, and needs no new script.

## Complexity Tracking

> No Constitution Check violations. Nothing to justify.
