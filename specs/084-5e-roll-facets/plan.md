# Implementation Plan: 5e Roll Facets

**Branch**: `084-5e-roll-facets` | **Date**: 2026-10-07 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `specs/084-5e-roll-facets/spec.md`

## Summary

A 5e character's sheet records the facets that change its rolls, and the
server applies them. Advantage is a per-roll argument. Halfling Luck and
Great Weapon Fighting rewrite the formula before it is rolled. Heroic
Inspiration and a Luck Point are spent after the roll through a new
`rerollRoll`, which replays the recorded dice and changes only what the
spend allows.

- **Dice crate.**
  - `rewrite.rs` prints a parsed formula back after adding a count or
    modifiers to chosen dice terms (research R2).
  - `replay.rs` re-evaluates a stored roll and feeds each die its recorded
    chain. It can reroll one die, or draw the extra dice of a reshaped term
    (R3).
  - `wasm.rs` exposes both to the demo.
- **Canvas core.** A `RollFacets` slot on `SystemContribution`, made of plain
  functions and label tables (R1, contracts/pack-roll-facets.md).
- **5e pack.**
  - `roll_facets.rs` holds the transform and the spend rules.
  - `trait_data` gains `facets` and `luck_points_used`.
  - `system.json` declares `itemProperties`.
  - The sheet shows facet checkboxes and the Luck Points left.
- **Server.**
  - One migration. The roll record gains its actor, kind, check id, facets
    and reroll link. Attacks gain a reroll link, and items gain properties.
  - `rollCheck` and `makeAttack` shape their formulas and take `advantage`.
  - `rerollRoll` spends, replays, re-judges and records the new roll, in one
    transaction.
  - A missed attack's reroll is a new attack row, judged against the stored
    defence, sharing the hit path with `record_attack` through a new
    `combat/attack_hit.rs`.
  - `WorldRoll` gains `facets`, `rerollOf`, `rerolledBy`, `spent`,
    `rerollOffers` and `rerollUntil`.
  - A reveal reveals the whole chain.
- **Web.**
  - The check and attack buttons get an Advantage picker.
  - `RollEntry` shows facet tags, both values of a changed die, a struck-
    through chain, and Reroll buttons for the roller.
  - The attack fields editor gets property checkboxes.
- **Demo.** The transform is mirrored in TypeScript. Rerolls go through the
  crate's wasm replay, with the same refusals and the same seeded tests.

### What is deliberately not built

- Conditions that grant advantage on their own (spec Assumptions). The
  transform takes the choice as an input, so a condition can supply it
  later.
- Lucky's defensive half, Inspiration on damage, Versatile weapons held in
  two hands, and a long-rest button.
- A weapon compendium. SRD weapons are not in the repository. The GM marks
  Two-Handed by hand (research R6).
- How a reroll is _drawn_ on the board. Spec 083 owns the throw and the
  chain's step kinds. This spec only makes a reroll a `ROLL_MADE` like any
  roll, and pushes `ChainStep::Reroll` if 083's `steps` already exists.

## Technical Context

**Language/Version**: Rust 2024 (dice crate, canvas core, the 5e pack's
server crate, server); TypeScript 5 / React 19 (web, the 5e pack's web
sheet, demo)
**Primary Dependencies**: no new crates. The 5e pack's server crate gains a
path dependency on `thunderforge-dice`. async-graphql and Diesel stay on the
server.
**Storage**: PostgreSQL. One migration, `2026-10-07-120000-0000_roll_facets`
(data-model.md). It comes after spec 082's `…110000…_authoring_tool_revocations`,
which is uncommitted in this tree.
**Testing**:

- `cargo test -p thunderforge-dice`;
- `cargo test -p thunderforge-system-dnd5e`;
- `make test-rust ARGS="-p thunderforge-server"`;
- `pnpm -F @thunderforge/demo test`;
- `pnpm -F @thunderforge/web test` for `RollEntry`;
- `pnpm e2e:rolls`;
- then every slice that `pnpm e2e:which --diff` names. The GraphQL schema
  and a migration change, so it prints FULL SUITE; the named slices run
  instead. The full suite is never the gate.

**Target Platform**: the server on Linux; the web and demo in Chromium, as
the e2e suite runs.
**Project Type**: a pnpm and Cargo workspace (apps, crates, packs).
**Performance Goals**: a reroll reaches every chat and board within 1 s on a
local stack (SC-004), which is the same path as any roll. A replay is one
re-evaluation of the formula with no extra draws for the recorded dice.
**Constraints**:

- Shared code never names 5e. The facet and spend ids are opaque strings,
  and their labels come from the pack.
- Other systems roll byte-for-byte as before (SC-005).
- No Rust file over 1000 lines. `attack.rs` (830) and `eval.rs` (944) take
  nothing new.
- No client-side database.

**Scale/Scope**: six stories, about 21 functional requirements, and 3 new e2e specs.

## Constitution Check

| Principle                                        | How this plan holds it                                                                                                                                                                                                                |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| I. The server is the authority                   | The formula is shaped on the server from the sheet it reads. The client sends only a choice (`advantage`) or a spend id. The reroll draws on the server, and the spend is checked and written in the same transaction as the roll.    |
| II. React and Bevy are isolated from the network | The Advantage picker and the Reroll button call the existing API layer (`apps/web/src/api`), as `CharacterRollButtons` already does. The new roll reaches the store and the board through `ROLL_MADE`, as every roll does (spec 081). |
| III. Optimistic updates with rollback            | A roll is not optimistic, and it never was. The chat shows what the server recorded. The Reroll button disables while in flight and shows the refusal sentence.                                                                       |
| IV. Base data vs derived data                    | The Luck Point maximum is derived from the level and never stored. The labels are joined at read. `rerollOffers` is derived per viewer at read and never stored.                                                                      |
| V. One pub/sub backplane                         | The new events are the existing codes 26, 29, 30, 36 and 37, through `record_world_event`. There is no new channel.                                                                                                                   |
| VI. Every feature is proven by its own slice     | The new specs are `rolls-facets-*.spec.ts`, which the `rolls` slice already owns by prefix. `pnpm e2e:rolls` proves the feature. The schema and migration are cross-cutting, so every slice `pnpm e2e:which --diff` names runs too; the full suite is never the gate.              |

## Project Structure

### Documentation (this feature)

```text
specs/084-5e-roll-facets/
├── spec.md
├── plan.md              # this file
├── research.md          # R1–R15
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── graphql-rolls-facets.md
│   └── pack-roll-facets.md
└── tasks.md
```

### Source code (touched)

```text
crates/thunderforge-dice/src/
├── lib.rs                     # pub mod rewrite, replay
├── rewrite.rs, rewrite_tests.rs   # new: term rewrite + canonical printer
├── replay.rs, replay_tests.rs     # new: replay a stored roll
└── wasm.rs                    # replayRoll, lowestDie

crates/thunderforge-canvas-core/src/
├── roll_facets.rs             # new: the slot's types
└── system_contribution.rs     # roll_facets: Option<&'static RollFacets>

packs/systems/dnd5e/
├── system.json                # trait_data.facets, luck_points_used; itemProperties
├── server/Cargo.toml          # + thunderforge-dice
├── server/src/lib.rs          # register roll_facets
├── server/src/roll_facets.rs, roll_facets_tests.rs   # new
├── server/src/validators.rs, validators_tests.rs     # facets, luck_points_used
└── web/src/components/RollFacetsSection.tsx          # new; mounted from ActorSheet.tsx

crates/thunderforge-server/
├── migrations/2026-10-07-120000-0000_roll_facets/{up,down}.sql
└── src/
    ├── schema.rs, models.rs
    ├── rolls/facets.rs, facets_tests.rs        # new: shape_roll, facet_labels
    ├── rolls/reroll.rs, reroll_tests.rs        # new: REROLL_WINDOW, chain rules, offers
    ├── graphql/mutations_roll.rs               # roll_and_settle takes RollMeta; reveal the chain
    ├── graphql/mutations_roll_check.rs         # advantage; shape before roll_and_settle
    ├── graphql/mutations_reroll.rs, _tests.rs  # new: rerollRoll
    ├── graphql/types_rolls.rs                  # WorldRoll fields, RollFacet, Advantage
    ├── graphql/queries/roll.rs                 # rerolledBy, offers for the viewer (entry_for)
    ├── graphql/mutations_attacks.rs, types_attacks.rs   # advantage; properties
    ├── graphql/play_pause_surface_tables.rs    # rerollRoll gated
    ├── combat/weapon.rs                        # Part.properties
    ├── combat/item_properties.rs               # new: the pack's declared vocabulary
    ├── combat/attack.rs                        # shape to-hit and damage; call settle_hit
    ├── combat/attack_hit.rs                    # new: settle_hit, moved out of record_attack
    └── combat/attack_reroll.rs, _tests.rs      # new: reroll a missed attack

apps/web/src/
├── api/roll.ts, api/systemChecks.ts, api/attacks.ts   # rerollRoll; advantage; properties
├── hooks/useWorldRolls.ts                     # new fields
├── components/world/RollAdvantage/AdvantagePicker.tsx   # new
├── components/world/PlayDock/CharacterRollButtons.tsx   # picker, reset after a roll
├── components/world/PlayDock/RollEntry.tsx    # tags, two values, chain, Reroll buttons
├── components/world/PlayDock/RerollButtons.tsx          # new
└── pages/world/ability/AttackFieldsEditor.tsx # property checkboxes

apps/demo/src/backend/
├── handlers/facets.ts, facets.test.ts         # new: transform mirror, reroll rules
├── handlers/dice.ts, dice.test.ts             # record fields, rerollRoll, chain reveal
├── handlers/combatAttacks.ts, combat.test.ts  # advantage, attack reroll
└── actors.ts                                  # rollCheck advantage

apps/web/e2e/
├── rolls-facets-advantage.spec.ts
├── rolls-facets-inspiration.spec.ts
└── rolls-facets-attack.spec.ts

docs/guides/rolls.md, docs/CONTRIBUTING.md (Rolls section)
```

## Complexity Tracking

| Addition                              | Why it is needed                                                                                          | Simpler alternative rejected because                                                                                       |
| ------------------------------------- | --------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| A formula printer in the dice crate   | The pack has to edit a formula it does not own, and the AST is private.                                   | String splicing breaks on the first formula with an existing modifier (R2).                                                |
| `replay` instead of a fresh roll      | The rules reroll _one_ die (Inspiration) or add _one_ die (Luck). Everything else must stand.             | A fresh roll redraws the damage and the other d20. Editing the JSON would duplicate keep and clamp outside the crate (R3). |
| A new attack row for an attack reroll | The reroll keeps the first miss on record and is judged against the stored defence.                       | Updating the row in place erases the miss that the chain shows struck through (R7).                                        |
| `settle_hit` moved into its own file  | The reroll and `record_attack` must offer damage through one path (FR-014), and `attack.rs` is 830 lines. | Copying the block would let the two drift. Growing `attack.rs` risks the 1000-line limit.                                  |
