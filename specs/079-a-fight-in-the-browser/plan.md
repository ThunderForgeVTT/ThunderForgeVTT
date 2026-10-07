# Implementation Plan: A Fight in the Browser

**Branch**: `worktree-agent-a58ebbadbf18fc98d` | **Date**: 2026-10-06 | **Spec**: [spec.md](./spec.md)

**Depends on**: [spec 046, A Fight That Resolves](../046-a-fight-that-resolves/spec.md), [spec 074, A World to Try](../074-a-world-to-try/spec.md)

**Input**: Feature specification from `/specs/079-a-fight-in-the-browser/spec.md`

## Summary

Spec 046's fight lives in `crates/thunderforge-server/src/combat/`, and nearly
every file there holds a rule and the database reads and writes around it. The
demo cannot run that. The owner decided the rules should be shared, not
copied: one Rust crate, compiled into the server and compiled to wasm for the
browser ([ADR-113](../../docs/adrs/20261006-113-the_rules_of_a_fight_are_one_crate.md)).

The work comes in two halves, and the first half is done.

1. **The extraction.** `crates/thunderforge-combat` holds the rules: what an
   attack decides, the hit point arithmetic, turn order, whose turn holds a
   token back, the turn budget, reach and line of sight, sizes, and the
   pack's `combat` and `turnStructure` blocks. The server keeps what touches
   storage: it loads, calls the crate, and saves. Its tests were not edited.
2. **The demo.** The demo's backend answers the combat operations through the
   crate's `wasm` façade, holding the fight in its own persisted state. Not
   started; Phase 4 of [tasks.md](./tasks.md) lists it.

Three choices shaped the extraction.

- **The server's tests are the contract, so they did not move.** Every server
  module kept its `#[cfg(test)]` module and its `_tests.rs` file, and re-exports
  what moved (`pub use thunderforge_combat::…`). Where a signature changed —
  `resolve` takes a `Spent` rather than a diesel row, `turn_check` asks a trait —
  the server keeps a wrapper with the old signature. A test reaches what it
  reached before by the path it used before.
- **Types the schema exposes are defined once.** `ActionCost`,
  `HitPointChangeKind`, `BudgetLine` and `TurnBudget` appear in the GraphQL
  schema. The crate derives `async_graphql` on them behind a `graphql` feature
  the server turns on, rather than the server mirroring them; the schema did
  not change. The manifest shapes carry `JsonSchema` the same way behind
  `schema`, for `pack_system_spec`.
- **The crate does not know what a combatant row is.** Turn order and the turn
  check take a trait (`order::Seat`, `turn::Party`) that the server implements
  on its `Combatant` and the demo's façade on whatever it holds.

## Technical Context

**Language/Version**: Rust 2024 (the new crate), Rust 2021 (server, unchanged edition), TypeScript 5 (the demo, Phase 4)

**Primary Dependencies**: `thunderforge_dice` (rolls, with an injected RNG), `thunderforge_canvas_core` (grid, walls, line of sight, movement cost), `serde`, `rand_core` 0.10. Optional: `async-graphql` (`graphql`), `schemars` (`schema`), `wasm-bindgen` (`wasm`).

**Storage**: None in the crate. The server's tables are unchanged; the demo keeps its fight in the state it already persists (spec 074 FR-012).

**Testing**: the server's existing combat tests, unedited; the crate's own unit tests; a parity test of one scripted fight through both paths (`combat/parity_tests.rs`); the `combat` e2e slice; and, for Phase 4, a demo e2e test in `apps/demo`.

**Target Platform**: Linux server; the browser through `wasm32-unknown-unknown`.

**Constraints**: no diesel, network, bevy or tokio in the crate (FR-001). No behaviour change on the server (FR-003). 5e arithmetic stays in `packs/systems/dnd5e` (FR-002): the crate names no system's fields and reads everything it needs from the pack's manifest (spec 046 contract M5).

### What moved, and what stayed

| Rule | Crate module | Server keeps |
| --- | --- | --- |
| Outcome, flag, kind and offer names | `records` | the diesel `AttackRecord` / `OfferRecord` |
| Pack `combat` / `turnStructure` shapes | `manifest`, `turn_structure` | reading the pack from disk, the cache |
| Damage and healing, standing after a change | `hit_points` | the locked read, validation, `follow_zero`'s writes |
| Size to footprint | `size` | — |
| Reach, distance, line of sight, flags | `reach` | `SceneMeasure` loads tokens and walls |
| Budget lines, spends, flags, move cost | `budget` | the budget row, `is_own_turn` |
| Formulas, roll, judge, damage, auto-apply | `attack` | the transaction, roll records, offers, events |
| Turn order, next turn | `order` | `sort_combatants`, `next_turn_index` wrappers |
| Whose turn holds a token back | `turn` | `turn_check` loads and redacts |
| Seeded and scripted dice | `dice` | its own `StdRng` |

Not moved, and on purpose for now: redaction (what a player may see),
legendary and lair actions' bookkeeping, controllers and offer resolution. Each
is a database question first; the rule inside each is a line or two. Phase 4
decides, per operation, whether the demo needs the rule from the crate or
already has what it needs (the demo has one viewer at a time and knows which).

### Measured

**Tests (US2, SC-002)**. Before the extraction (main e9e7e8a5): the server
library listed 1,873 tests; `cargo test -p thunderforge-server --lib combat`
ran 111, all passing. After: the same 1,873 names, byte for byte
(`cargo test -- --list`, diffed); 111 combat tests passing; `git diff` on
every `*_tests.rs` file empty, and the inline test modules in `reach.rs`,
`size.rs` and `manifest.rs` untouched. The full `make test-rust` run: 1,868
passed, 6 ignored, 0 failed in the server library (1,873 plus the parity test).

**Size (FR-009)**. `wasm-pack build --release --target web -- --features wasm`
on 2026-10-06:

| File | Raw | gzip -9 | brotli 11 |
| --- | --- | --- | --- |
| `combat_bg.wasm` | 412,578 B | 148,598 B | 118,073 B |
| `combat.js` (glue) | 27,823 B | — | — |

About 115 KiB over the wire with brotli: noise beside the engine, and far
under the 1 GB cap. The module is its own download, separate from the
engine's; Phase 4 loads it only when a fight starts, so a visitor who never
fights never fetches it.

## Constitution Check

- **I. A change is described before it is made** — spec.md, this plan and
  tasks.md. PASS.
- **II. The product is one product** — this is the point: the demo fights by
  the server's rules, not a copy. PASS.
- **III. Data has one owner** — the server's tables still own a real fight;
  the demo's state owns a demo fight. The *rules* now have one owner, which
  they did not have the day a second copy would have been written. PASS.
- **IV. A significant decision leaves an ADR** — ADR-113. PASS.
- **V. Licensed content carries its licence** — no new content. PASS.
- **VI. Every feature is proven by its own slice** — the server half by the
  `combat` slice and its unit tests; the demo half by `apps/demo`'s own suite.
  PASS for the half that exists.

## Project Structure

```text
crates/thunderforge-combat/        # new: the rules
├── Cargo.toml                     # features: graphql, schema, wasm
└── src/
    ├── lib.rs
    ├── attack.rs  budget.rs  dice.rs  hit_points.rs  manifest.rs
    ├── order.rs   reach.rs   records.rs  size.rs  turn.rs  turn_structure.rs
    └── wasm.rs                    # the browser façade (feature `wasm`)

crates/thunderforge-server/src/combat/   # load → call the crate → persist
crates/thunderforge-server/src/combat/parity_tests.rs   # new: US3
crates/pack_system_spec/src/combat.rs    # re-exports the manifest shapes

apps/demo/src/backend/handlers/combat.ts # Phase 4: the demo's combat operations
```

## Complexity Tracking

| Choice | Why | Simpler alternative rejected because |
| --- | --- | --- |
| `graphql` and `schema` features on the crate | the schema and the install-time JSON Schema keep one type each | mirrored types in the server would have to be kept in step by hand, and a drift would change the schema silently |
| Traits for a combatant | the crate cannot name a diesel row, and the demo's combatant is a JSON object | passing tuples would have rewritten `turn_check` and the order functions, and the tests that call them |
| A JSON façade rather than `serde-wasm-bindgen` | one dependency fewer, and the demo already moves JSON strings | the faster path is not needed at a few calls per turn |
