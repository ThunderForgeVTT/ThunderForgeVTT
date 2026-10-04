---
description: "Task list for One Way to Be a Pack"
---

# Tasks: One Way to Be a Pack

**Input**: [spec.md](./spec.md), [the pack contract](../../packs/systems/README.md), [ADR-029](../../docs/adrs/20260504-029-runtime_module_loading_and_security.md), [ADR-062](../../docs/adrs/20260902-062-packs_extend_the_engine_with_data_not_code.md)

**Tests**: the removals are proved by what still passes. The guard is written
test-first, because a guard nobody has seen fail proves nothing.

**Baseline** (commit `4a45504d`): 39 Cargo workspace members; `make test-rust`
runs the default members and passed 1,867. The seven engine crates hold 18
tests between them, none in the default members, so that number does not move.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished work)

---

## Phase 1: Nothing unread (Story 1)

**Purpose**: every file in a pack is one the product reads.

- [X] T001 [US1] Compare `packs/systems/dnd5e/engine/src/dice.rs` with `crates/thunderforge-dice`; move any case the dice crate does not cover into its tests — nothing to move: it rolled a d20 with advantage, which the dice crate's own tests cover
- [X] T002 [US1] Delete `packs/systems/{blades_in_the_dark,cypher_system,dnd5e,fate_core,genie,pathfinder2e,year_zero_engine}/engine/`; remove the seven members from the root `Cargo.toml`; drop `-p dnd5e-engine` from `lint-wasm` in the `Makefile`
- [X] T003 [US1] Delete `web/` from Blades, Cypher, Fate, Pathfinder and Year Zero; regenerate `pnpm-lock.yaml`
- [X] T004 [US1] In 5e and Genie `web/src/`, remove any file not reachable from `ActorSheet.tsx`, `StatBlocks.ts`, `panels/*.tsx` or an alias `apps/web` imports — 5e lost `index.ts`, `schema.ts` and four components (989 lines); Genie had none
- [X] T005 [US1] In each of the seven packs with `server/src/loader.rs`: delete the no-op registration function and its re-export, keep the two tests, and rename the file `registration_tests.rs` under `#[cfg(test)]`

**Checkpoint**: `cargo check --all-targets`, `make lint`, `make test-rust` and the web typecheck pass.

---

## Phase 2: The manifest and the template (Stories 3 and 4)

- [X] T010 [US4] `crates/pack_system_spec/src/lib.rs`: `esmodules`, `styles` and `packs` default to empty when absent; test that a manifest without them validates and one with them still does
- [X] T013 [US4] Found in T010: the schema also required `authors`, where the contract documents `author` and every bundled manifest carries that, so no bundled manifest passed the validation an installed pack must. `authors` defaults to empty too, and `bundled_manifests_tests.rs` walks every bundled manifest through `validate_system_manifest`
- [X] T011 [US4] Remove `esmodules`, `styles` and `packages` from all nine `system.json` files
- [X] T012 [US3] `basic-game-system`: delete `module/`, `styles/`, `package.json` and `rollup.config.js`, leaving `system.json`; the T013 walk covers it

**Checkpoint**: `cargo test -p pack_system_spec` passes; `/api/systems` still omits the template.

---

## Phase 3: The guard (Story 2)

- [X] T020 [US2] `scripts/__tests__/check-packs.test.mjs`: one failing case per fault in FR-005, and a passing case shaped like Roll for Shoes
- [X] T021 [US2] `scripts/check-packs.mjs`: the check, exporting pure functions the tests call; it holds a `server/` crate to both linkage modules, the application's and the server library's test binary's
- [X] T022 [US2] `scripts/verify.mjs` step `packs`; add it to `.hooks/pre-commit`
- [X] T023 [US2] `scripts/check-system-registry.mjs`: it already scanned `apps/web/src`; widen to every app, `packages/*/src` and the three engine-side crates (FR-006), with a test for each root

**Checkpoint**: `pnpm test:scripts` passes; the check passes on the tree.

---

## Phase 4: The contract (Story 5)

- [X] T030 [US5] `packs/systems/README.md`: remove `engine/` from the shape and say why; state the `web/` rule; replace "the one line outside your directory" with the full list, split by what the pack contributes; drop the `esmodules`/`styles` mentions if any
- [X] T031 [US5] `node scripts/check-pack-docs.mjs` passes

---

## Phase 5: Proof

- [X] T040 `cargo check --all-targets`, `make lint`, `make test-rust` — 1,867 passed, as at the baseline
- [X] T041 `pnpm --filter @thunderforge/web typecheck`, web unit tests (727 passed), `pnpm test:scripts` (129 passed)
- [ ] T042 From the main checkout after the fast-forward: the system e2e slices
