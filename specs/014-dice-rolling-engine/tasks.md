---

description: "Task list for feature implementation"
---

# Tasks: Dice Rolling Engine

**Input**: Design documents from `/specs/014-dice-rolling-engine/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/graphql-roll.md, quickstart.md (all present)

**Tests**: The crate itself is test-heavy by design (plan.md's Testing section) — every modifier category gets a deterministic unit test using a seeded/mock `RngCore`, folded into the Foundational phase rather than split into a separate contract-test phase, following spec 012/013's established inline-test convention.

**Organization**: Tasks are grouped by user story (spec.md priorities P1/P1/P2/P2). Unlike most specs in this repo, User Story 2 (full grammar breadth) is inherently the same work as the Foundational parser/evaluator — there is no meaningful way to ship a "partial grammar" MVP — so Foundational directly implements the complete grammar from spec.md FR-004-FR-009a, and US2's own task section is verification-only (mirroring spec 015's US2/Foundational relationship).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1-US4)
- Every task includes an exact file path

## Path Conventions

New workspace crate `crates/thunderforge-dice/` (path-dependency from both `src/server` and `src/engine`, following `crates/thunderforge-canvas-core`'s exact precedent). Existing `src/server` (Rust/Axum/Diesel/async-graphql), `src/engine` (Bevy/wasm32-unknown-unknown), `apps/web` (React/TS).

---

## Phase 1: Setup

- [X] T001 Add `crates/thunderforge-dice` to the root `Cargo.toml`'s `[workspace] members` list (alongside `crates/thunderforge-canvas-core`, `crates/pack_system_spec`)
- [X] T002 Create `crates/thunderforge-dice/Cargo.toml`: package name `thunderforge_dice`, edition 2024, deps `serde` (+ `derive`), `serde_json`, `rand_core` only — no `bevy`/`wasm-bindgen`, matching `thunderforge-canvas-core`'s minimal-dependency precedent (research.md §1/§3)
- [X] T003 [P] Create `crates/thunderforge-dice/src/error.rs`: `FormulaError` enum (`ParseError { message, position }`, `DivisionByZero`, `NonFiniteResult`, `MissingPlaceholder(String)`, `DiceCountExceeded`, `IterationCapExceeded` — data-model.md's `FormulaError` table), implementing `std::error::Error`/`Display`
- [X] T004 [P] Create `crates/thunderforge-dice/src/ast.rs`: formula AST types per the FR-004-FR-009a grammar — `Expr` (arithmetic/dice-term tree), `DiceTerm` (count, sides: `Numeric(u32)`/`Fate`/`Coin`, or nested `Expr` for `1d(1d20)`/`(2d4)d8`), `Modifier` enum (KeepHighest/KeepLowest/DropHighest/DropLowest{count}, Reroll/RerollRecursive/Explode/ExplodeOnce/ExplodeCapped{condition}, Min/Max{n}, CountSuccesses/CountFailures/DeductFailures/SubtractFailures{condition}, Even/Odd, MarginOfSuccess{n}), `Condition` (Eq/Gt/Gte/Lt/Lte, n), `Pool(Vec<Expr>, Option<Modifier>)`, `Placeholder(String)`, `MathFn` (Floor/Ceil/Round/Abs)

**Checkpoint**: Crate scaffold exists, AST types defined — parser/evaluator implementation can begin.

---

## Phase 2: Foundational (Blocking Prerequisites — implements the full grammar, User Story 2's actual content)

**Purpose**: A complete, correct, bounded, RNG-injected `resolve()` — the crate's entire reason to exist. No user story's acceptance scenarios (US1's persistence, US3's placeholders, US4's animation) can be verified without this.

**⚠️ CRITICAL**: This phase implements ALL of spec.md's grammar (FR-004 through FR-009a) — do not treat it as a "basic version to expand later."

- [X] T005 Implement the tokenizer + hand-rolled recursive-descent parser in `crates/thunderforge-dice/src/parser.rs`: `DiceFormula::parse(source: &str) -> Result<DiceFormula, FormulaError>` — base `NdM`/`dF`/`dc`/`dcc{n}` dice, arithmetic (`+ - * /`) via precedence climbing, parenthetical sub-expressions (including dice-count/dice-size as a nested `1d(1d20)`/`(2d4)d8` expression), dice pools `{term1, term2, ...}modifier`, math functions (`floor()`/`ceil()`/`round()`/`abs()`), named placeholders (bare identifiers like `STAT`), and every modifier from T004's `Modifier` enum with its comparison-condition (`=`/`>`/`>=`/`<`/`<=`) or count argument (research.md §2)
- [X] T006 [P] Add `crates/thunderforge-dice/src/lib.rs`'s public API surface: re-export `DiceFormula`, `PlaceholderBindings` (`HashMap<String, f64>`), `RollResolution`, `DieOutcome`, `DieSides`, `ResolutionKind`, `FormulaError` (data-model.md's crate-types table) — all `#[derive(Serialize, Deserialize)]` where they cross the GraphQL boundary (depends on T003, T004)
- [X] T007 Implement `crates/thunderforge-dice/src/eval.rs`'s `resolve(formula: &DiceFormula, bindings: &PlaceholderBindings, rng: &mut impl rand_core::RngCore) -> Result<RollResolution, FormulaError>`: walks the AST, substitutes placeholders (erroring on any unbound name, never defaulting to 0 — FR-010), rolls dice via the injected `rng`, applies every modifier (keep/drop, reroll/explode, clamp, success/failure counting), tracks every individual `DieOutcome` (full `rolls` chain, `kept` flag, `final_value`) per FR-013, and produces `ResolutionKind::Total` or `::SuccessCount` depending on the formula's own notation (never a caller's expectation) (depends on T005, T006)
- [X] T008 Enforce the FR-012 bounded-iteration guarantee inside `eval.rs`: two hard `pub const` caps (max total dice per resolution, max reroll/explosion iterations per die — research.md §4), checked continuously during evaluation (not just up front), aborting with `FormulaError::DiceCountExceeded`/`IterationCapExceeded` rather than truncating or hanging (depends on T007)
- [X] T009 [P] Unit tests in `crates/thunderforge-dice/src/eval.rs` (or a sibling `tests.rs`) using a seeded/mock `RngCore` for every grammar category from quickstart.md US2: `4d6+2` (arithmetic), `4d6kh3` (keep-highest, dropped dice present with `kept: false`), `2d6r1` (reroll shows multi-entry `rolls`), `1d6x` (explode chain), `8d10cs>=7` (`SuccessCount`), `4dF` (fate die faces in `{-1,0,1}`), `(2d4)d8` (nested dice-size), `floor(1d20/2)` (math fn), malformed `1d20 +` (parse error, zero dice rolled), and `1d6x>=1` (iteration-cap rejection, not a hang) (depends on T007, T008)
- [X] T010 [P] Unit tests for placeholder substitution in `eval.rs`'s test module: `1d20 + STAT` with `STAT` bound resolves correctly and two different bindings produce results differing by exactly the delta (deterministic RNG); `1d20 + STAT` with no binding returns `MissingPlaceholder`; a formula with no placeholders resolves identically whether or not `bindings` is empty (quickstart.md US3) (depends on T007)
- [X] T011 [P] Confirm `cargo check --target wasm32-unknown-unknown -p thunderforge_dice` succeeds standalone (quickstart.md's "wasm32 build" cross-cutting check) — this is a verification task, not new code; if it fails, the fix belongs in whichever T002-T008 task introduced the non-wasm-clean dependency/code (depends on T002-T008)

**Checkpoint**: `thunderforge_dice` is a complete, correct, bounded, independently-testable (no DB/server needed) crate. Server/engine integration can begin.

---

## Phase 3: User Story 1 - A roll is requested and resolved fairly, with no client-supplied outcome ever trusted (Priority: P1) 🎯 MVP

**Goal**: `rollDice` is the sole path to an authoritative result — world-membership-gated, always re-resolves server-side with a real OS-backed RNG, structurally incapable of accepting a client-supplied outcome, and durably recorded.

**Independent Test**: Trigger `rollDice` with `1d20` as a world member; confirm the response and a matching `world_roll_records` row exist; confirm `RollDiceInput` has no field that could express a pre-computed result; confirm two concurrent rolls from different users are independent (quickstart.md US1).

### Implementation for User Story 1

- [ ] T012 [US1] Add `rand` (full crate, OS-backed) to `src/server/Cargo.toml`, and `thunderforge_dice = { path = "../../crates/thunderforge-dice" }` to both `src/server/Cargo.toml` and `src/engine/Cargo.toml` (path dependency, per plan.md's Project Structure)
- [ ] T013 [US1] Create Diesel migration `create_world_roll_records` (`id`, `world_id` FK cascade, `triggered_by` FK users, `formula` text, `bindings` jsonb nullable, `detail` jsonb not null, `result_kind` text, `result_value` double precision, `created_at`) in `src/server/migrations/<ts>_create_world_roll_records/{up,down}.sql` (data-model.md), run it, and regenerate `src/server/src/schema.rs`
- [ ] T014 [P] [US1] Add `RollRecord`/`NewRollRecord` Diesel `Queryable`/`Insertable` structs to `src/server/src/models.rs` (depends on T013)
- [ ] T015 [P] [US1] Add `GraphQLDieOutcome`, `DieSidesKind`, `RollResultKind`, `GraphQLRollResolution`, `GraphQLRollRecord` GraphQL types to `src/server/src/graphql/types.rs`, and `RollDiceInput`/`PlaceholderBindingInput` input types to `src/server/src/graphql/input_types.rs` (contracts/graphql-roll.md's shape — note `RollDiceInput` has ONLY `worldId`/`formula`/`bindings`, no result-shaped field, by design) (depends on T014)
- [ ] T016 [US1] Implement the `rollDice` mutation in `src/server/src/graphql/mutations_roll.rs`: verifies caller is a member of `worldId` (any role, per contracts/graphql-roll.md's Authorization — reuse the existing world-visibility check pattern), constructs a real OS-backed RNG (`rand::rngs::StdRng::from_os_rng()` or equivalent), calls `thunderforge_dice::resolve()`, on success inserts a `world_roll_records` row and returns the `GraphQLRollResolution`; on any `FormulaError` returns a specific GraphQL error and inserts nothing (contracts/graphql-roll.md's Behavior) (depends on T012, T015, T007)
- [ ] T017 [P] [US1] Implement `worldRollRecords(worldId, limit)` query in `src/server/src/graphql/queries/roll.rs`: DM-only (contracts/graphql-roll.md's stated floor), newest-first, default page size ~50, reconstructing `GraphQLRollResolution` from each row's persisted `detail` JSONB (depends on T014, T015)
- [ ] T018 [P] [US1] Implement `validateDiceFormula(formula)` query in `src/server/src/graphql/queries/roll.rs`: pure `DiceFormula::parse` check, no RNG/persistence, any authenticated caller (contracts/graphql-roll.md) (depends on T015)
- [ ] T019 [US1] Wire `mutations_roll`/`queries::roll` into the GraphQL schema root in `src/server/src/graphql.rs` (`pub mod`/`pub use` + `QueryRoot`/`MutationRoot` field additions) (depends on T016, T017, T018)
- [ ] T020 [US1] Resolver tests in `mutations_roll.rs`/`queries/roll.rs` (inline `#[tokio::test]`, matching existing convention): non-member rejected before any roll happens; two rolls (even identical formulas) from different simulated calls produce independent `world_roll_records` rows; a malformed formula produces zero rows; `worldRollRecords` denies a non-DM caller (depends on T016, T017)

**Checkpoint**: User Story 1 is fully functional and independently testable — the trust boundary this entire feature exists for is real and enforced.

---

## Phase 4: User Story 2 - Any formula the table's game system needs can be rolled (Priority: P1)

**Goal**: The full grammar breadth (spec.md's explicit notation categories) resolves correctly.

**Independent Test**: See quickstart.md US2's nine formula examples plus the two rejection cases.

*(Implementation for this story is entirely covered by Phase 2's T005-T011 — the parser/evaluator IS the grammar, there is no separable "US2 code" beyond the crate itself. This section exists so the story is independently checkable per spec.md's priority ordering, and so `rollDice` (US1) has something to actually resolve through the full grammar rather than a toy subset.)*

**Checkpoint**: A user (via `rollDice`, US1) can resolve any formula in spec.md's supported grammar, not just `NdM`.

---

## Phase 5: User Story 3 - A formula can reference a character's stat instead of a hard-coded number (Priority: P2)

**Goal**: `rollDice`'s `bindings` input actually flows through to placeholder substitution end-to-end via the real GraphQL mutation (the crate-level logic itself is already covered by T010's unit tests — this phase is the integration path).

**Independent Test**: Call `rollDice` with `1d20 + STAT` and a binding, and again with a different binding, confirming the totals differ by exactly the delta; call it with a placeholder and no binding and confirm a clear rejection (quickstart.md US3).

### Implementation for User Story 3

- [ ] T021 [US3] Confirm (resolver test in `mutations_roll.rs`) that `RollDiceInput.bindings` (a list of `PlaceholderBindingInput{name, value}`) is correctly converted to the crate's `PlaceholderBindings` map before calling `resolve()`, and that a `MissingPlaceholder` error surfaces as a specific, distinguishable GraphQL error (not a generic failure) — this is primarily a wiring/mapping check since T007/T010 already prove the underlying logic (depends on T016)
- [ ] T022 [US3] [P] Resolve an actual spec 013 Item Effect formula (e.g. a "Longsword" damage effect's stored `2d8`, or an attack-roll's `1d20 + STAT + MODIFIERS` with test bindings) through `rollDice` in a resolver test, confirming zero changes are needed to spec 013's `world_item_effects` schema (SC-005, quickstart.md's "spec 013 unlock" cross-cutting check) (depends on T016)

**Checkpoint**: User Stories 1-3 all work independently — a formula authored once (e.g. spec 013's Item Effects) resolves correctly for different characters' different stat values.

---

## Phase 6: User Story 4 - Players and the DM watch dice physically resolve (Priority: P2)

**Goal**: Triggering a roll plays a dice-bouncing animation that reveals — never precedes or disagrees with — the server's already-resolved per-die outcomes; the result remains fully available even if the animation can't play.

**Independent Test**: Trigger a roll from the play canvas; confirm the animation lands each die on its real `finalValue` (including reroll/explosion chains); confirm no total is shown before the animation completes; confirm a missing animation surface still delivers the result (quickstart.md US4).

### Implementation for User Story 4

- [ ] T023 [US4] [P] Create `apps/web/src/api/roll.ts`: `rollDice(worldId, formula, bindings?)`, `getWorldRollRecords(worldId, limit?)`, `validateDiceFormula(formula)` — fetch-based GraphQL calls mirroring `api/items.ts`'s `postGraphQL`/CSRF pattern (contracts/graphql-roll.md)
- [ ] T024 [US4] [P] Add `apps/web/src/types/roll.ts`: `RollResolutionRecord`, `DieOutcomeRecord`, `RollRecordRecord` TS types (contracts/graphql-roll.md) (depends on T023)
- [ ] T025 [US4] Create the `src/engine/src/plugins/dice_roll/` Bevy plugin (`mod.rs`, `systems/`, `resources/`, per Constitution Principle II's per-capability-plugin convention): a resource holding the current roll's `GraphQLDieOutcome[]` (received from the React/game-shell layer the same way other server-pushed world state already reaches the engine — check `src/engine/src/network/mod.rs`'s existing `ExternalCommand` pattern for precedent), and systems that spawn/animate one die entity per outcome, settling each on its real `final_value` — the plugin only ever renders already-known outcomes, it never calls `resolve()` itself to produce one (research.md §6, Constitution Principle I) (depends on T012)
- [ ] T026 [US4] Register `DiceRollPlugin` in `src/engine/src/lib.rs`'s plugin list, alongside the other existing plugins (`BackgroundPlugin`, `CameraPlugin`, etc.) (depends on T025)
- [ ] T027 [US4] Wire a "roll trigger → animation → reveal" flow: the React trigger (wherever a roll is initiated — e.g. an Item Effect's "use" affordance, or a manual roll control) calls `rollDice` (T023), forwards the response's per-die detail into the engine via the existing `ExternalCommand`-style bridge (T025's resource), and gates displaying the final `resultValue` in the UI until the animation reports completion — never shown before (FR-015/FR-016, quickstart.md US4 step 2) (depends on T023, T025)
- [ ] T028 [US4] Ensure the result is still delivered/displayed when no animation surface is available (e.g. the engine/canvas isn't mounted) — the UI's result display must not be gated *only* on an animation-completion event with no fallback/timeout (quickstart.md US4 step 3, FR-016) (depends on T027)
- [ ] T029 [US4] [P] Playwright e2e in `apps/web/e2e/dice-roll.spec.ts`: trigger a roll, confirm a result is displayed, confirm `worldRollRecords` (as DM) shows the matching record afterward (depends on T027)

**Checkpoint**: All four user stories are independently functional.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [ ] T030 Author `docs/adrs/<next-number>-dice-rolling-engine-shared-crate-and-trust-boundary.md` per plan.md's Constitution Check (Principle IV, Complexity Tracking): documents the `crates/thunderforge-dice` shared-crate pattern (native + wasm32, RNG-injected, no target-detection), the server-only-authoritative-RNG trust boundary, and explicitly records that this supersedes `packs/systems/dnd5e/engine/src/dice.rs` (research.md §5) — including the recommended follow-up (migrate `roll_d20`/`RollAdvantage` call sites onto `thunderforge_dice::resolve()` with `1d20`/`2d20kh1`/`2d20kl1` formulas, then delete `dice.rs`) as noted, explicitly-out-of-scope future work, not silently implied
- [ ] T031 [P] Run `cargo test -p thunderforge_dice` standalone (no DB/server) and confirm it passes with zero external dependencies (Constitution Principle II, quickstart.md's cross-cutting check)
- [ ] T032 [P] Run `cargo check`/`cargo test` in `src/server` (native) and `cargo check --target wasm32-unknown-unknown -p thunderforge_engine -p dnd5e-engine` (Constitution Principle V)
- [ ] T033 [P] Run `cargo clippy --all-targets` on `thunderforge_dice`, `thunderforge`, `thunderforge_core` and fix any new warnings, keeping the workspace at 0 (per the recent full clippy pass)
- [ ] T034 [P] Run `pnpm --filter @thunderforge/web build` and a scoped `eslint` check on new/touched frontend files
- [ ] T035 Execute every scenario in `specs/014-dice-rolling-engine/quickstart.md` against a running local dev stack (including the wasm32 build and standalone-crate-test cross-cutting checks) — actually trigger rolls and watch the animation in a browser, not just compile
- [ ] T036 [P] Confirm `./scripts/check-file-length.sh` shows no new failures introduced by this feature's files

---

## Dependencies & Execution Order

- **Setup (Phase 1)**: No dependencies — start immediately.
- **Foundational (Phase 2)**: Depends on Setup — BLOCKS everything else; this phase IS User Story 2's actual implementation.
- **US1 (Phase 3)**: Depends on Foundational (needs a working `resolve()` to call). Independent of US3/US4 beyond that.
- **US2 (Phase 4)**: Fully satisfied by Phase 2; no additional code, verification-only.
- **US3 (Phase 5)**: Depends on US1's `rollDice` mutation existing (T016) to integration-test placeholder wiring against; the underlying logic is already proven in Phase 2.
- **US4 (Phase 6)**: Depends on US1's `rollDice` (needs real responses to animate) and T012 (engine's crate dependency). Independent of US3.
- **Polish (Phase 7)**: Depends on all desired user stories being complete.

### Parallel Opportunities

- T003, T004 (Phase 1) can run in parallel.
- T009, T010, T011 (Phase 2) can run in parallel once T007/T008 land.
- T014, T015 (Phase 3) can run in parallel once T013 lands.
- T017, T018 (Phase 3) can run in parallel once T015 lands.
- T023, T024 (Phase 6) can run in parallel.
- T031-T034, T036 (Phase 7) can run in parallel.

---

## Implementation Strategy

### MVP First

1. Phase 1 (Setup) + Phase 2 (Foundational) — the crate exists, is correct, is bounded, is independently testable.
2. Phase 3 (US1) — a real, trustworthy `rollDice` mutation exists.
3. **STOP and VALIDATE**: quickstart.md US1/US2 against a running server (no engine/animation needed yet).
4. Phase 5 (US3) — placeholder wiring, unlocks spec 013.
5. Phase 6 (US4) — the visible payoff (animation).
6. Polish, including the ADR.

### Suggested Task Ordering for a Single Implementer

Sequential by phase (T001→T036) is safe; [P]-marked tasks within a phase may be reordered/batched freely. Phase 6 (the Bevy plugin) is the highest-uncertainty phase — if time-constrained, a minimal "dice settle into place with no bounce physics" animation still satisfies every US4 acceptance scenario (spec.md's own Assumptions explicitly leave animation visual design as an implementation decision) and is preferable to an incomplete, more ambitious physics simulation.
