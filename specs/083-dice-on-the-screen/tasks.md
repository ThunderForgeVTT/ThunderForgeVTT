# Tasks: Dice on the Screen

**Input**: [spec.md](./spec.md), [plan.md](./plan.md), [research.md](./research.md),
[data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: the spec's Proof section asks for tests in the dice crate,
canvas core, the server, web and demo vitest, and e2e. Inside each phase,
the tests come before the code they prove.

**Where the engine's tests run**: the engine crate cannot compile for the
host (`plugins/frame_trace.rs`), so a `#[test]` there never runs. The
throw's logic goes in `crates/thunderforge-canvas-core/src/dice_throw/`,
and its tests are `cargo test -p thunderforge-canvas-core dice_throw`. The
engine plugin is proven by `make lint` (wasm32) and the e2e.

**File length**: `scripts/check-file-length.sh` fails a Rust file over 1000
lines.

- `crates/thunderforge-dice/src/eval.rs` is 944, so new dice tests go in
  new files.
- `crates/thunderforge-engine/src/app.rs` is 885, so it gains routing only.
- `apps/web/src/engine/bevy/index.ts` is 2305 lines of TypeScript, which the
  check does not cover. Even so, new web code goes in `diceThrow.ts`.

**Do not**: rebuild wasm from inside an e2e run; set
`THUNDERFORGE_IGNORE_E2E_LOCK`; or change how the crate totals an exploding
die (research R4).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an open
  task).
- **[Story]**: US1 a d20 lands, US2 every die kind, US3 keep/reroll/explode,
  US4 named bonuses, US5 busy table and calm screen.

---

## Phase 1: Setup

- [x] T001 Record the engine's brotli size before any change, from a release build (`node scripts/build.mjs --only-wasm`, then the research R13 one-liner on `dist/engine/engine_bg.wasm`), into `specs/083-dice-on-the-screen/tasks.md` under T068
- [x] T002 [P] Add `thunderforge-dice = { path = "../thunderforge-dice" }` to `crates/thunderforge-canvas-core/Cargo.toml`, and `pub mod dice_throw;` with an empty `crates/thunderforge-canvas-core/src/dice_throw/mod.rs` to `crates/thunderforge-canvas-core/src/lib.rs`. Then run `cargo check -p thunderforge-canvas-core` and `cargo check -p thunderforge-engine --target wasm32-unknown-unknown`

---

## Phase 2: Foundational (blocks every story)

### Dice crate: chain steps (FR-002)

- [x] T003 [P] Tests in the new `crates/thunderforge-dice/src/eval_steps_tests.rs`, wired from `eval.rs` with `#[cfg(test)] #[path = "eval_steps_tests.rs"] mod steps_tests;`, using a fixed seeded RNG:
  - `1d6r<7` gives `steps == [Reroll]`;
  - `rr` gives only `Reroll` steps;
  - `1d6xo>0` gives `[Explode]`;
  - `x` gives only `Explode` steps;
  - a die with both a reroll and an explosion gives `[Reroll, Explode]`;
  - `steps.len() == rolls.len() - 1` for every die;
  - `1d20min21` gives `final_value != *rolls.last()` and no extra step;
  - a `DieOutcome` deserialized from JSON without `steps` reads `steps == []`;
  - a resolution serialized then deserialized round-trips.
- [x] T004 Add `ChainStep { Reroll, Explode }` and `#[serde(default)] pub steps: Vec<ChainStep>` to `DieOutcome` in `crates/thunderforge-dice/src/lib.rs`, and export `ChainStep`. Push a step beside each `rolls.push` in `eval_dice_term` (`crates/thunderforge-dice/src/eval.rs:266-301`), and set `steps: Vec::new()` in the other `DieOutcome` constructors in `eval.rs`. T003 goes green.
- [x] T005 Add `steps: vec![]` to the `DieOutcome` literal in `crates/thunderforge-server/src/graphql/mutations_roll_check_tests.rs:592`, and to any other constructor `cargo check --workspace --all-targets` reports

### Dice crate: breakdown (FR-004, FR-005)

- [x] T006 [P] Tests in the new `crates/thunderforge-dice/src/breakdown_tests.rs`, written against hand-built `RollResolution`s, so they need no RNG:
  - `1d20 + 5` gives `Sum[Die 0, +5]`;
  - `2d6 + 1d8 + 3` gives three dice in order, then `+3`;
  - `1d20 - 2` and `1d20 + -1` both give a negative constant;
  - `-1d4 + 10`;
  - `1d20 + MODIFIER` with `MODIFIER=3` gives `Constant{3, Some("MODIFIER")}`;
  - a missing binding gives `None`;
  - `2d20kh1 + 4` leaves out the dropped die's index;
  - `Xd6` with a bound `X` consumes X dice;
  - `(1d6 + 2) * 2` gives `None`;
  - `{1d20, 1d20}kh1 + 4` gives `None`;
  - `floor(1d6 / 2)` gives `None`;
  - `6d10cs>=8` gives `Successes` with a mark per die;
  - a resolution with fewer dice than the formula's count gives `None`.
- [x] T007 Implement `crates/thunderforge-dice/src/breakdown.rs` (data-model.md: `Breakdown`, `Addend`, `AddendKind`, `breakdown()`). It walks `ast::Expr` left to right as `eval_expr` does (`eval.rs:139-150`), and reuses `condition_matches` for success marks, made `pub(crate)` if needed. Add `pub mod breakdown;` and the re-exports to `lib.rs`. T006 goes green. Then run `cargo clippy -p thunderforge-dice --features thunderforge-dice/wasm --target wasm32-unknown-unknown -- -D warnings`.

### Server: steps and bindings (FR-002, FR-003)

- [x] T008 [P] Tests in the new `crates/thunderforge-server/src/graphql/roll_bindings_tests.rs`, wired from `types_rolls.rs`:
  - `WorldRoll::from_row` with `bindings = {"MODIFIER": 3}` gives `[RollBinding{placeholder: "MODIFIER", value: 3.0}]`;
  - `null` and a non-object both give `[]`;
  - two bindings come back sorted;
  - `GraphQLDieOutcome::from` maps `[Reroll, Explode]` to `[REROLL, EXPLODE]`;
  - a check rolled through `roll_check` (the `mutations_roll_check_tests.rs` harness) and fetched with `worldRoll` returns its bindings;
  - the same roll for GM's eyes, fetched by another player, is a `MaskedRoll`, and the query `... on MaskedRoll { bindings }` fails validation.
- [x] T009 Add `DieStep` and `GraphQLDieOutcome.steps` in `crates/thunderforge-server/src/graphql/types_dice.rs`, and `RollBinding` and `WorldRoll.bindings` (built from `row.bindings` in `from_row`) in `crates/thunderforge-server/src/graphql/types_rolls.rs`. T008 goes green under `make test-rust ARGS="-p thunderforge-server --lib roll"`.
- [x] T010 Regenerate `apps/thunderforge/schema.graphql` with `node scripts/check-graphql-contract.mjs --schema --fix`, and check that the diff is exactly contracts/graphql-rolls.md

### Web: types and queries

- [ ] T011 [P] Add `steps: ("REROLL" | "EXPLODE")[]` to `DieOutcomeRecord`, and `bindings: PlaceholderBinding[]` to `WorldRollRecord`, in `apps/web/src/types/roll.ts`. Reuse `PlaceholderBinding` (:40) if its fields match `{placeholder, value}`, and add `RollBindingRecord` if not. Select `steps` in the `dice {` fragment and `bindings { placeholder value }` in `... on WorldRoll` in `apps/web/src/api/roll.ts`. Fix the fixtures `pnpm -F @thunderforge/web exec tsc --noEmit` reports.

### Canvas core: the throw's model

- [x] T012 In `crates/thunderforge-canvas-core/src/dice_throw/mod.rs`:
  - `ThrowSpec`, the parsed payload in core types: roll id, roller, label, formula, bindings, result kind and value, and dice with sides, rolls, steps, kept and final value;
  - `ThrowDie`, `ShapeKind`, `Segment`;
  - `TIMINGS` (research R9), with `tumble_ms: 1200`, `step_ms: 500`, `hold_ms: 2500`, `fade_ms: 400` and `reduced_ms: 150`;
  - `expand(spec) -> (Vec<ThrowDie>, hidden_count)`, following data-model.md's expansion rules and the 20-die cap.

  Tests in `dice_throw/expand_tests.rs`, written first:
  - `[1, 5]` with `[Reroll]` gives one die with two segments, the first struck;
  - `[6, 6, 2]` with `[Explode, Explode]` gives three dice, the last two with `explosion_of: Some(0)`;
  - an empty `steps` gives all rerolls;
  - a clamp gives a struck segment with no tumble;
  - `40d6` gives 20 drawn dice and 20 hidden;
  - explosions count toward the cap.

### Engine: the payload

- [ ] T013 Replace `ExternalCommand::TriggerDiceRoll { dice }` and `DiceRollDiePayload` in `crates/thunderforge-engine/src/payloads.rs` (:529) with `TriggerDiceRoll { roll: DiceRollPayload }` (contracts/engine-dice.md, camelCase, `steps` and `bindings` defaulted), and add `SetReducedMotion { reduced: bool }`. Parse both in `crates/thunderforge-engine/src/sdk.rs` (:314). In `crates/thunderforge-engine/src/app.rs` (:211-213, :872), push into the new `DiceQueue` resource and set the `DiceMotion` resource; both are declared in T019. Convert the payload to `ThrowSpec` in a `From` impl in `payloads.rs`.

**Checkpoint**: crate, server and canvas core tests are green; the schema is regenerated; the engine compiles for wasm32.

---

## Phase 3: User Story 1, a d20 tumbles and lands on the server's number (P1) MVP

### Tests first

- [x] T014 [P] [US1] Tests in `crates/thunderforge-canvas-core/src/dice_throw/shapes_tests.rs`:
  - the icosahedron has 20 faces labelled 1 to 20, each once;
  - every face is a planar convex polygon whose outward normal points away from the centroid;
  - vertices are unit-scale.
- [x] T015 [P] [US1] Tests in `dice_throw/landing_tests.rs`: for every value 1 to 20, the face's normal rotated by `landing(shape, value, spin)` is `+z` within 1e-5, for several spins.
- [x] T016 [P] [US1] Tests in `dice_throw/tumble_tests.rs`:
  - `seed("0d9b…")` is stable (a literal);
  - `path(seed, index, t)` is identical on two calls;
  - at `t = 1`, the orientation equals the landing orientation and the position equals the resting place;
  - two dice of one throw get different resting places that do not overlap at the die size;
  - every resting place is inside the lower third of a 1280×720 viewport.
- [x] T017 [P] [US1] Tests in `dice_throw/readout_tests.rs`:
  - `Ayla: Stealth   17 + 5 = 22`;
  - with no label, `Ayla   17 + 5 = 22`;
  - a negative constant reads `12 - 1 = 11`;
  - addends that do not sum to the total fall back to `formula = total`;
  - a non-integer total prints two decimals;
  - every output byte is ASCII.
- [ ] T018 [P] [US1] Web tests in the new `apps/web/src/engine/bevy/__tests__/diceThrow.test.ts`: `buildDiceThrow(roll)` keeps every field of contracts/engine-dice.md from a `WorldRollRecord` fixture, with `steps` and `bindings` defaulting to `[]` when the record has none. In `apps/web/src/engine/world/sync/__tests__/rolls.test.ts`, change the `animate` assertions to receive the whole `WorldRoll`, and to never receive a `MaskedRoll`.

### Implementation

- [x] T019 [US1] Implement `dice_throw/shapes.rs` (the icosahedron first, with the table and face labels), `landing.rs`, `tumble.rs` (FNV-1a and splitmix64, research R3) and `readout.rs`, until T014 to T017 are green
- [ ] T020 [US1] Replace `crates/thunderforge-engine/src/plugins/dice_roll.rs` with `crates/thunderforge-engine/src/plugins/dice/`:
  - **`mod.rs`**: `DicePlugin`, plus the `DiceQueue`, `DiceMotion` and `DiceStage` resources and markers, with the system order after the camera systems.
  - **`mesh.rs`**: one `Mesh2d` per die, built at spawn and mutated in place with `Assets<Mesh>::get_mut`. It rotates, projects, culls by normal z and shades by `n · L` with `ATTRIBUTE_COLOR` (research R1).
  - **`throw.rs`**: spawn under the stage, animate from `tumble::path`, land, hold, fade, then despawn and drop the mesh handles.
  - **`readout.rs`**: the `Text2d` readout above the dice, and the face number at the projected face centre on landing.

  Update `plugins/mod.rs` (:10, :43) and the `lib.rs` (:46) registration. `SETTLE_DURATION_SECS` goes.

- [ ] T021 [US1] Screen anchoring in `plugins/dice/mod.rs` (research R8). It copies the `Camera2d` translation into `DiceStage` at z 950 and its projection scale into the stage's scale, and reads the viewport for the lower third.
- [ ] T022 [US1] `plugins/dice/probe.rs`:
  - the landed log (50 entries);
  - `#[wasm_bindgen] dice_landed() -> String`, `dice_timings() -> String` and `dice_entity_count() -> u32`.

  Export them from `sdk.rs` as `frame_trace` is exported (`sdk.rs:52`).

- [ ] T023 [US1] Web side, in the new `apps/web/src/engine/bevy/diceThrow.ts`: `buildDiceThrow(roll)` and `engineDiceTimings()`. In `apps/web/src/engine/bevy/index.ts`:
  - `triggerDiceRollAnimation(roll: WorldRollRecord)` (:1768) sends `{type: "trigger_dice_roll", roll: buildDiceThrow(roll)}`, and still pushes the final values to `dicePlayed` (:115);
  - `installEngineProbe` (~:132-200) gains `diceLanded()` and `diceEntities()`.

  T018 goes green.

- [ ] T024 [US1] Widen `animate` in `apps/web/src/engine/world/sync/rolls.ts` to `(roll: WorldRollRecord) => void`, and update both callers: `apps/web/src/pages/world/WorldPage.tsx:1958-1966` and `apps/web/src/hooks/useWorldRolls.ts:103`
- [ ] T025 [US1] Run `make lint` (lint-host, lint-wasm, file length). Then rebuild with `ENGINE_PROFILE=dev node scripts/build.mjs --only-wasm`, and check a `1d20 + 5` by hand in `make dev`.

### E2E

- [ ] T026 [US1] `diceLanded(page)` and `diceEntities(page)` in `apps/web/e2e/fixtures/rolls.ts`, beside `dicePlayed` (:36)
- [ ] T027 [US1] The new `apps/web/e2e/rolls-dice-on-screen.spec.ts`, owned by the `rolls-` prefix of the `rolls` slice (`scripts/e2e/slices.json:765`), with the US1 tests:
  - `1d20 + 5` lands one die of sides 20 on the server's `finalValue`, read with `worldRoll`, and the readout ends `<v> + 5 = <total>`;
  - the same roll on a second member's board has identical faces and resting places;
  - after a pan far from the origin and a zoom, the readout still appears (`diceLanded` grows) and `restingPlace` is unchanged;
  - `diceEntities()` returns to 0 within `tumbleMs + holdMs + fadeMs + 1 s`.

**Checkpoint**: a d20 is a real die on every board. This is the MVP.

---

## Phase 4: User Story 2, several dice of several kinds (P1)

- [x] T028 [P] [US2] Extend `dice_throw/shapes_tests.rs` and `landing_tests.rs` to every `ShapeKind` in data-model.md:
  - d4, d6, d8, d12 and d10 face counts and labels;
  - the d10 labels `0` for 10;
  - the d100 pair labels `00` to `90` and `0` to `9`, with 100 reading `00` and `0`;
  - the d3 and Fate cube labels, with Fate values `1, 1, 0, 0, -1, -1`;
  - the coin labels `H` and `T`;
  - a disc for d2 and for any other size;
  - every value of every shape lands with its face toward `+z`.
- [x] T029 [P] [US2] In `dice_throw/expand_tests.rs`:
  - `2d6 + 1d8 + 3` expands to `[6, 6, 8]` in resolution order;
  - `1d100` is one throw die with two meshes, and reports `sides: 100` once;
  - `1d7` is a disc showing 7's value.
- [x] T030 [US2] Implement the remaining vertex and face tables in `dice_throw/shapes.rs`: tetra, cube, octa, dodeca, the pentagonal trapezohedron and the disc prism. Implement the d100 pair in `expand`. T028 and T029 go green.
- [ ] T031 [US2] Draw a d100 as two meshes in `plugins/dice/mesh.rs` and `throw.rs`, and add the disc flip (a rotation about x) in `throw.rs`
- [ ] T032 [US2] In `rolls-dice-on-screen.spec.ts`, US2:
  - `2d6 + 1d8 + 3` reports sides `[6, 6, 8]` and the faces match the server, with readout `a + b + c + 3 = total`;
  - `1d100` reports one die of sides 100 whose face is the server's;
  - `4dF` reports four `"F"` dice;
  - `1d7` reports sides 7.

---

## Phase 5: User Story 3, advantage, rerolls and explosions (P1)

- [x] T033 [P] [US3] In `dice_throw/readout_tests.rs`:
  - `2d20kh1 + 4` sums only the kept die;
  - `6d10cs>=8` reads `3 successes` with marks, and `1 success` for one;
  - an exploded chain's readout uses the crate's `final_value`, and its total is the server's (research R4).
- [x] T034 [P] [US3] In `dice_throw/tumble_tests.rs`:
  - a die with two segments takes `tumble_ms + step_ms` to land;
  - an explosion die enters at `tumble_ms + k·step_ms`;
  - a clamp segment adds no tumble time.
- [ ] T035 [US3] In `plugins/dice/throw.rs` and `readout.rs`:
  - a dropped die dims to grey at half alpha;
  - a failed success-count die is dimmed;
  - a struck value is a `Text2d` plus a thin sprite bar, shown above the die;
  - a reroll tumbles again;
  - an explosion spawns its die at its step time;
  - a clamp swaps the number with no tumble.

  The probe gains `rerolled`, `clamped`, `explosionOf` and `succeeded`. T033 and T034 go green.

- [ ] T036 [US3] In `rolls-dice-on-screen.spec.ts`, US3. These rolls are deterministic whatever the server's RNG (research R5):
  - `2d20kh1 + 4`: one die reports `kept: false`, and the readout uses the other;
  - `1d6r<7`: `rerolled` has one value, and `face` is the server's `finalValue`;
  - `1d6xo>0`: two dice, the second with `explosionOf: 0`;
  - `1d20min21`: `clamped` is the server's `rolls[0]`, and `face` is 21;
  - `6d10cs>=1`: `6 successes`, every die with `succeeded: true`.

---

## Phase 6: User Story 4, bonuses are named numbers (P2)

- [x] T037 [P] [US4] In `dice_throw/readout_tests.rs`:
  - `1d20 + MODIFIER` with `MODIFIER=3` reads `13 + 3 = 16`;
  - `(1d6 + MOD) * 2` with `MOD=2` reads `(1d6 + 2) * 2 = 14`, the placeholder replaced as a whole identifier, so `MODX` is untouched;
  - a roll stored before this spec, with no bindings and a placeholder formula, reads `formula = total`.
- [x] T038 [US4] Substitute the placeholders in the fallback in `dice_throw/readout.rs`. T037 goes green.
- [ ] T039 [US4] In `rolls-dice-on-screen.spec.ts`, US4: a Stealth check rolled from the sheet in the dock (the `rolls-sheet-tab` setup) has a readout bonus equal to `worldRoll.bindings[0].value`, and the readout does not contain `MODIFIER`

---

## Phase 7: User Story 5, a busy table and a calm screen (P2)

- [x] T040 [P] [US5] Tests in `dice_throw/queue_tests.rs`:
  - push A, B, C, D and E while A plays, and B is skipped, never A;
  - `landed()` starts the next throw in arrival order;
  - fading throws are independent of the playing one;
  - a sixth push while four wait skips the oldest again.
- [ ] T041 [P] [US5] Web test in `apps/web/src/engine/bevy/__tests__/diceThrow.test.ts`: `watchReducedMotion` sends the initial value, sends on `change`, and stops after its disposer runs (a `matchMedia` stub)
- [ ] T042 [US5] Implement `dice_throw/queue.rs` and use it from `plugins/dice/throw.rs`. Write skipped throws to the landed log. T040 goes green.
- [ ] T043 [US5] Reduced motion:
  - in `plugins/dice/throw.rs`, `DiceMotion.reduced` spawns dice landed, fades them in over `reduced_ms`, and shows the readout at once;
  - in `diceThrow.ts`, `watchReducedMotion`;
  - in `WorldPage.tsx`, start it when the engine is ready, send `set_reduced_motion`, and stop it on teardown.

  T041 goes green.

- [ ] T044 [US5] The `+N more` chip in `plugins/dice/readout.rs`, using `readout::chip`
- [ ] T045 [US5] In `rolls-dice-on-screen.spec.ts`, US5:
  - five rolls within a second give five `diceLanded` entries in arrival order, of which at most one is skipped, and the playing throw never is;
  - every skipped roll is in the chat;
  - a context with `reducedMotion: "reduce"` lands within 150 ms plus the event's delivery, and the entry reports `reducedMotion: true`;
  - `40d6` reports 20 dice, `chip: "+20 more"`, and a readout ending in the server's total.

---

## Phase 8: Timings owned by the engine (FR-017)

- [ ] T046 [P] Web test in the new `apps/web/src/components/world/DiceRollerPanel/__tests__/DiceRollerPanel.test.tsx`: the result appears after `engineDiceTimings().tumbleMs` (stubbed to 300), after `reducedMs` under reduced motion, and after 1200 ms when no engine is loaded
- [ ] T047 Read the delay from `engineDiceTimings()` in `apps/web/src/components/world/DiceRollerPanel/DiceRollerPanel.tsx` (:37, :63), and delete `ANIMATION_REVEAL_MS`. T046 goes green.

---

## Phase 9: The demo (FR-020)

- [ ] T048 [P] [US4] Tests in `apps/demo/src/backend/handlers/dice.test.ts`, beside the `rollCheck` tests (:151, :360):
  - `worldRoll` of a check returns `bindings` sorted;
  - a plain roll returns `[]`;
  - a masked entry has no `bindings` key.
- [ ] T049 [P] [US3] Tests in the same file: `resolutionRow` maps the crate's `steps` to `REROLL`/`EXPLODE`, and a stored detail without `steps` maps to `[]`
- [ ] T050 In `apps/demo/src/backend/handlers/dice.ts`:
  - `recordRoll` stores `bindings` (it gains a `bindings` option, passed by `resolveAndRecord`);
  - `entryFor` emits `bindings`;
  - `Resolution` gains `steps?`;
  - `resolutionRow` (:60-85) maps them.

  Rebuild `dist/dice` with `node scripts/build.mjs --only-wasm`. T048 and T049 go green.

- [ ] T051 Add a test to `apps/demo/e2e/rolls-across-tabs.spec.ts` that keeps its `dicePlayed` assertions unchanged, and asserts on both tabs that `diceLanded()` has an entry for the open roll, with the same `face` and `restingPlace`

---

## Phase 10: The sandbox

- [ ] T052 Add `"@thunderforge/dice": "workspace:^"` to `apps/engine-sandbox/package.json`, and a formula field with a **Roll** button in `apps/engine-sandbox/src/main.ts`, beside the buttons at ~:181 and :236. It rolls with the dice wasm `roll(formula, "{}", seed)`, builds a `WorldRoll`-shaped payload, and sends it through `apply_world_command` (:60). Add a reduced-motion checkbox that sends `set_reduced_motion`. Document both in `apps/engine-sandbox/README.md`.

---

## Phase 11: Docs

- [ ] T053 [P] `docs/guides/rolls.md`: a new section, "Dice on the board". It says:
  - what plays;
  - what the readout means;
  - how dropped, rerolled, exploded and clamped dice look;
  - that bursts queue and skip;
  - that reduced motion is the OS setting;
  - that a GM's eyes or GM only roll never plays where it is hidden.
- [ ] T054 [P] `docs/CONTRIBUTING.md` Rolls section (:76). It covers:
  - where the throw is built: `dice_throw` in canvas core and `plugins/dice/` in the engine;
  - why the logic lives in canvas core;
  - the payload contract;
  - `diceLanded()` for tests;
  - tuning in the sandbox;
  - the exploding-total follow-up (research R4).

---

## Phase 12: Polish and proof

- [ ] T055 `cargo test -p thunderforge-dice` and `cargo test -p thunderforge-canvas-core dice_throw`, green
- [ ] T056 `make test-rust ARGS="-p thunderforge-server --lib roll"`, green
- [ ] T057 `make lint`, green (host, wasm32 and file length)
- [ ] T058 `node scripts/check-graphql-contract.mjs --schema`, clean
- [ ] T059 Web vitest `pnpm -F @thunderforge/web exec vitest run src/engine src/components/world/DiceRollerPanel`, and `pnpm -F @thunderforge/web exec tsc --noEmit`, green
- [ ] T060 Demo vitest `pnpm -F @thunderforge/demo exec vitest run`, green
- [ ] T061 `pnpm verify` green, including `e2e-slices`, which checks that the new spec is owned by `rolls`
- [ ] T062 Release rebuild with `node scripts/build.mjs --only-wasm`, outside any e2e run
- [ ] T063 The existing `rolls-everyone`, `rolls-gm-only`, `rolls-sheet-tab`, `rolls-reveal` and `rolls-gm-eyes` specs pass unchanged on `dicePlayed` (FR-018)
- [ ] T064 **Proof**: `pnpm e2e:rolls`, green. It runs the demo's unit tests and `rolls-across-tabs`, then the `rolls` slice with `rolls-dice-on-screen.spec.ts`.
- [ ] T065 `pnpm e2e:which --diff`, and run every slice it names
- [x] T066 ~~Full suite `node ./scripts/e2e-parallel.mjs`, green~~ — skipped by owner decision 2026-10-07: slices are the gate (T064 and T065 are the proof)
- [ ] T067 SC-004: in `rolls-dice-on-screen.spec.ts`, 50 consecutive `1d6` rolls end with `diceEntities() == 0`. SC-007: a `20d6` throw keeps the frame times from `frame_trace()` at or under 18.2 ms (55 fps) at the median, as `engine-limits.spec.ts` reads them.
- [ ] T068 SC-006: the release `engine_bg.wasm` brotli size after the change, minus T001's, is under 150 KB. Record both numbers here. Before (T001, release, pre-change): 5,010,820 B brotli (raw 30,140,764 B).

---

## Dependencies

- Phase 2 blocks every story.
  - T003 → T004 → T005.
  - T006 → T007.
  - T008 → T009 → T010.
  - T012 needs T004 and T007, because `ThrowSpec` holds steps and the readout needs `Breakdown`.
  - T013 needs T012.
- US1 (Phase 3) blocks US2 to US5, because they extend its files.
- US2, US3 and US4 touch different parts of `dice_throw` and can proceed in
  parallel after US1, but they share `rolls-dice-on-screen.spec.ts`, so
  their e2e tasks are serial.
- US5 needs US1. The demo phase needs T004 and T009. The panel phase needs
  T022 and T023.
- Polish runs last. T062 comes before T063 to T067.

## Parallel examples

```text
Phase 2: T003, T006, T008, T011 together; then T004, T007, T009 in their own crates.
Phase 3: T014, T015, T016, T017, T018 together.
After US1: T028/T029 (US2), T033/T034 (US3), T037 (US4), T040/T041 (US5), T048/T049 (demo).
```

## Implementation strategy

1. **MVP.** Phases 1 to 3. A d20 lands on every board with its readout,
   anchored to the screen and proven by e2e.
2. Then US2 and US3, which complete the ask: "of that variety", and what
   happened to each die.
3. Then US4 and US5, the timings, the demo, the sandbox and the docs.
4. The proof is `pnpm e2e:rolls`, then every slice `pnpm e2e:which --diff`
   names. Slices are the gate (owner decision 2026-10-07).

Commit at each checkpoint, signed, with explicit paths.
