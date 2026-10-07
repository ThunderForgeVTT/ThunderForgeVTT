---
description: "Task list for A Fight in the Browser"
---

# Tasks: A Fight in the Browser

**Input**: Design documents from `/specs/079-a-fight-in-the-browser/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [spec 046](../046-a-fight-that-resolves/spec.md), [spec 074](../074-a-world-to-try/spec.md), [ADR-113](../../docs/adrs/20261006-113-the_rules_of_a_fight_are_one_crate.md)

**Tests**: included. US2's test is the server's existing combat tests,
unedited; US3's is a new parity test; US1's is a demo e2e test.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished work)

---

## Phase 1: Baseline

**Purpose**: write down what "unchanged" means before anything changes.

- [X] T001 [US2] On main e9e7e8a5, list the server library's tests (`cargo test -p thunderforge-server --lib -- --list`): 1,873
- [X] T002 [US2] Run `cargo test -p thunderforge-server --lib combat`: 111 passed

**Checkpoint**: the numbers the extraction is held to.

---

## Phase 2: The crate (US2)

**Purpose**: the rules in one place, with no storage, network or engine.

- [X] T010 `crates/thunderforge-combat`: new workspace member, edition 2024, `cdylib` + `rlib`; depends on `thunderforge_dice`, `thunderforge_canvas_core` (no default features), `serde`, `serde_json`, `rand_core`. Features `graphql`, `schema`, `wasm`
- [X] T011 [P] `manifest.rs`: the pack's `combat` block shapes, moved from `pack_system_spec`, `JsonSchema` behind `schema`; `combat_from_manifest`, `turn_budget_from_manifest`
- [X] T012 [P] `turn_structure.rs`: `TurnStructure`, `from_manifest`
- [X] T013 [P] `records.rs`: outcome, flag, kind and offer names
- [X] T014 [P] `hit_points.rs`: `HitPoints`, `apply_to`, read and write by the pack's fields, `standing_after` (C8)
- [X] T015 [P] `size.rs`: `footprint_from`
- [X] T016 [P] `reach.rs`: `Reach`, `Measured`, `flags_for`, `scene_grid`, `measure`
- [X] T017 [P] `budget.rs`: `Spent` (the row without its keys), `resolve`, `Spend`, `flags_after_spend`, `flags_before_spend`, `step_cells`, `move_cost`
- [X] T018 [P] `attack.rs`: `ActionCost`, `attack_formulas`, `roll`, `judge`, `damage_source`, `offered_amount`, `auto_apply_holds`. The order dice are drawn in is part of the rule: to-hit, then damage only on a hit
- [X] T019 [P] `order.rs`: the `Seat` trait, `sort_seats`, `next_turn`
- [X] T020 [P] `turn.rs`: the `Party` trait, `held_by`, `turn_refusal`
- [X] T021 [P] `dice.rs`: `SeededDice` (SplitMix64) and `ScriptedDice` (named faces), both a `rand_core` RNG with no OS entropy (FR-007)

**Checkpoint**: `cargo test -p thunderforge_combat` — 12 passed.

---

## Phase 3: The server calls the crate (US2, US3)

**Purpose**: the server keeps load and persist; behaviour does not change.

- [X] T030 [US2] `pack_system_spec/src/combat.rs`: re-export the manifest shapes; keep validation. 111 tests pass
- [X] T031 [US2] Server `combat/{records,manifest,hit_points,size,reach,budget,attack,weapon,turn}.rs`, `turn_structure.rs`, `graphql/mutations_combat.rs`: re-export or call the crate. A changed signature keeps a server wrapper with the old one; an import only tests use is `#[cfg(test)]`
- [X] T032 [US2] The test list after the change is the 1,873 names of T001, byte for byte; `combat` runs 111, all passing; `git diff` on `*_tests.rs` is empty. **No test moved; none was edited**
- [X] T033 [US2] `make lint` clean, with `thunderforge_combat --features wasm` linted for `wasm32-unknown-unknown` in `lint-wasm`
- [X] T034 [US2] Full `make test-rust`: the server library 1,868 passed, 6 ignored, 0 failed (1,873 plus T035)
- [X] T035 [US3] `combat/parity_tests.rs`: a scripted fight under `ScriptedDice` — three of Aria's swings at the ogre (a tie that hits, a miss, a hit) and the ogre's one back for all of Aria's hit points — through `make_attack` and `resolve_offer`, and through the crate alone. Every to-hit total, outcome and offered amount, and both hit point totals at the end, agree
- [X] T036 `packs/systems/dnd5e/server/Cargo.toml`: drop `tokio` and `async-trait`, declared and never used. Separate commit
- [ ] T037 [US2] The `combat` e2e slice (`THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1 pnpm e2e:combat`, `--workers=1`). **Not run in the extraction's worktree**: the worktree has no `dist/engine`, and the runner would build one, which this change was asked not to do. Run it on main after the merge, where the engine is already built. The change is Rust only and touches nothing in the slice's web paths

**Checkpoint**: the server fights exactly as before, and a fight made by the
crate alone is the same fight.

**Not moved, by decision** (plan.md, "What moved"): redaction, legendary and
lair bookkeeping, controllers, offer resolution. Each is a database question
with a small rule inside; Phase 4 takes from them only what the demo needs.

---

## Phase 4: The demo fights (US1)

**Purpose**: the visitor plays a round of the ambush with no server.

- [X] T040 `crates/thunderforge-combat/src/wasm.rs`: the JSON façade — `Dice` (seeded or scripted), `roll`, `attackPart`, `attackFormulas`, `autoApplyHolds`, `measure`, `readHitPoints`, `writeHitPoints`, `applyHitPoints`, `standingAfter`, `footprintFrom`, `combatFromManifest`, `roundLabel`, `sortSeats`, `nextTurn`, `heldBy`, `turnRefusal`, `resolveBudget`, `spendForAttack`, `takeSpend`, `startTurn`, `spendFlags`, `moveCost`
- [X] T041 Measure the release wasm (FR-009): 412,578 B raw, 118,073 B brotli; recorded in plan.md
- [ ] T042 Build the module for the demo: a `scripts/shared.mjs` step beside the pdf crate's (`wasm-pack build ./ --release --target web --out-dir ../../dist/combat --scope thunderforge --out-name combat -- --features wasm`), and the demo's Vite config resolving `@thunderforge/combat`
- [ ] T043 `apps/demo/src/backend/handlers/combat.ts` (new): the demo's combat operations, registered in `apps/demo/src/backend/handlers.ts` with one import and one spread, nothing else in that file changed. Load the wasm module lazily, on the first combat operation
- [ ] T044 `apps/demo/src/backend/state.ts`: the fight in the demo's persisted state — combat, combatants, budgets, attacks, offers, the auto-apply settings, the dice seed's position — so a reload keeps it and "Start over" clears it (US1 scenario 5, spec 074 FR-012)
- [ ] T045 [US1] Answer, through the crate, every operation of FR-005:
  - `activeCombat`; `startCombat`, `endCombat`
  - `addCombatant`, `updateCombatant`, `removeCombatant`, `addLairCombatant`
  - `advanceTurn` (`nextTurn`, `startTurn` on the new combatant's budget)
  - `changeHitPoints` (`applyHitPoints`, `writeHitPoints`, `standingAfter`)
  - `makeAttack`, `previewAttack`, `pendingOffers`, `resolveOffer` (`attackFormulas`, `measure`, `attackPart`, `spendForAttack`, `spendFlags`, `autoApplyHolds`)
  - `attack` (one attack) and `sceneAttacks`, replacing today's empty stub
  - `setCombatAutoApply`, `updateWorldAutoApplyNpcDamage`
  - `worldAbilities` and `actorAbilities`, from the ambush's stat blocks, replacing today's empty lists: the tracker's attack list reads `worldAbilities`
- [ ] T046 [US1] Dice: `Dice.seeded` from `crypto.getRandomValues` in play; a fixed seed or `Dice.scripted` when the demo's e2e asks for one (FR-007)
- [ ] T047 [US1] Viewing as a player: hide what the server hides (FR-008) — a monster's exact hit points, a hidden combatant's name; port the server's `combat/redaction.rs` rule into the crate if the demo needs more than the viewer's role
- [ ] T048 [US1] Anything the fight UI asks for that the demo does not support (`setAbilityAttack`, `setItemAttack` — editing an attack's cost and reach — unless T045 finds them cheap) is refused with the "not part of the demo" answer (`notInDemo.ts`), never silently empty
- [ ] T049 [US1] `apps/demo` e2e: start a fight on the Grassy Path Ambush, attack a goblin on Brannoc's turn, advance a full round, reload, and read back the turn order, the attack log and the hit points (`cd apps/demo && pnpm run e2e`)
- [ ] T050 [US1] SC-004: each of start, attack and advance shows its result in under half a second in that e2e

**Checkpoint**: SC-001 — a round of the ambush, with no server.
