---
description: "Task list for Roll for Shoes Extras"
---

# Tasks: Roll for Shoes Extras

**Input**: Design documents from `/specs/062-roll-for-shoes-extras/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/)

**Tests**: included. The pack's rules already live in a pure module with its own
`node --test` suite, and four of the five Extras are changes to it — writing the
arithmetic test first costs nothing and is the only cheap way to prove the core
game did not move.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on unfinished work)
- **[Story]**: which user story the task serves

## Path conventions

Everything is under `packs/systems/roll_for_shoes/` except the migration, the two
`diesel.toml` entries, the e2e spec and the ADR — see plan.md's source tree for
why those four cannot be.

---

## Phase 1: Setup

**Purpose**: the pack crate stops being table-free, and the database has somewhere to put a row.

- [X] T001 Add `diesel`, `async-graphql` and `thunderforge-server` to `packs/systems/roll_for_shoes/server/Cargo.toml`, and rewrite the header comment that currently advertises the crate as owning no tables and contributing no GraphQL — it is about to do both, and a comment that lies is worse than none
- [X] T002 Create migration `src/server/migrations/2026-09-23-000000-0000_roll_for_shoes_world_settings/up.sql` and `down.sql` for `world_roll_for_shoes_settings` per data-model.md §1: `world_id` PK/FK CASCADE, `difficulty_mode` default `'free'`, three booleans default `false`, `starting_skills` jsonb default `'[]'`, timestamps, `updated_by`
- [X] T003 [P] Add `^world_roll_for_shoes_` to `except_tables` in the root `diesel.toml`, alongside `^world_genie_.*`
- [X] T004 [P] Add `world_roll_for_shoes_settings` to `except_tables` in `src/server/diesel.toml`
- [X] T005 Verify the slice needs no edit: `game-systems` in `scripts/e2e/slices.json` already declares `own: ["system-"]` and already lists the pack, `apps/web/src/host/**` and `apps/web/src/panels/**` in `paths`. Confirm `pnpm verify`'s e2e-slices check passes unchanged and record here that no edit was needed — this is the Principle VI setup task, discharged by inspection rather than by editing

  **Done, but the answer is not the clean one the plan assumed.** The slice itself needs no edit: `own: ["system-"]` adopts `system-roll-for-shoes-extras.spec.ts` by name, and `paths` already covers `packs/systems/…/roll_for_shoes/**` and `src/server/src/test_packs.rs`. But `node scripts/e2e-slice.mjs which` over the real changed paths returns two things the plan did not predict:

  - **Three paths are cross-cutting**, so the tool routes to the full suite regardless of the slice: `src/app/src/schema_roots.rs` ("the roots of its schema"), `src/server/Cargo.toml` ("the server's features and dependencies"), and `src/server/migrations/**` ("schema history every slice reads"). This is the lookup's rule 1, which fires before any slice match. plan.md's Constitution Check claimed this change is "not cross-cutting"; that claim is wrong and is corrected there.
  - **Two paths are uncovered**: `diesel.toml` and `src/server/diesel.toml` match no slice and no rule. They are `diesel print_schema` configuration — build-time codegen input, never loaded by a browser and never read at runtime — so no e2e can prove them. What proves them is T006, which is exactly the check that the `except_tables` entries work. Recorded here rather than papering over it by adding a glob to a shared tool, per the tool's own instruction to "say how they were tested".

- [X] T006 Run `make migrate` and confirm `diesel print_schema` does **not** add the new table to `src/server/src/schema.rs` — if it does, T003/T004 are wrong

  **Passes.** `make migrate` applied `2026-09-23-000000-0000_roll_for_shoes_world_settings`, and `git diff --stat src/server/src/schema.rs` is empty: the regenerated schema is byte-identical. `world_roll_for_shoes_settings` is absent from it, as are all six `world_genie_*` tables — the same filter, proving the mechanism rather than an accident.

**Checkpoint**: the table exists and the server's schema does not know it.

---

## Phase 2: Foundational (blocking)

**Purpose**: settings can be stored, read, written and configured. Every user story needs this; none of them can start without it.

**⚠️ No user story work begins until this phase is complete.**

### Server

- [X] T007 Create `packs/systems/roll_for_shoes/server/src/settings/schema.rs` with the `table!` declaration for `world_roll_for_shoes_settings`, matching the migration exactly
- [X] T008 Create `packs/systems/roll_for_shoes/server/src/settings/mod.rs`: the row model, a `read_or_default(world_id)` that returns the defaults for a world with no row, and an upsert. **A missing row is not an error** (contracts/graphql.md)
- [X] T009 Create `packs/systems/roll_for_shoes/server/src/settings/graphql.rs` with `rollForShoesWorldSettings` (query) and `updateRollForShoesWorldSettings` (mutation) per contracts/graphql.md — whole-row upsert, all five fields required, GM-only via `auth::world_membership::is_dm_of_world`, `refuse_world_if_paused` on the write
- [X] T010 Add the three validations from contracts/graphql.md to the mutation: unknown difficulty mode, empty starting-skill name, starting-skill level below 1. Refuse with a message; never coerce
- [X] T011 Create `packs/systems/roll_for_shoes/server/src/settings/play_pause_surface.rs` submitting a `PackSurface`: the query as `reads`, the mutation as `gated` with a request document. Model it on `packs/systems/genie/server/src/session/play_pause_surface.rs`
- [X] T012 Wire the module into `packs/systems/roll_for_shoes/server/src/lib.rs` and confirm `cargo test -p thunderforge-server play_pause_surface` passes — it fails until T011 exists, which is the point

  **Done, and it needed one step the task did not name.** The module is wired into `lib.rs` and the two root types are merged in `src/app/src/schema_roots.rs`. But the test would have passed vacuously: `thunderforge-server`'s test binary links the bundled packs as dev-dependencies (`src/server/src/test_packs.rs`), and Roll for Shoes was not among them — spec 061 contributed no root fields, so it never needed to be. An `inventory` submission in a crate nothing references is never linked, so the new `PackSurface` was invisible to the very test meant to check it. Added `roll-for-shoes-server` to `src/server/Cargo.toml`'s dev-dependencies and `use roll_for_shoes_server as _;` to `test_packs.rs`. All 6 tests then pass **with the surface actually in the set**.

  **Unrelated pre-existing defect found:** `every_gated_field_refuses_a_paused_world` overflows its stack under a default `cargo test` and aborts the whole binary. It does this with Roll for Shoes unlinked too, so it predates this spec and is not caused by it. `RUST_MIN_STACK=67108864 cargo test -p thunderforge-server play_pause_surface` passes all 6. Not fixed here — it is not this feature's defect and deserves its own change.
- [X] T013 [P] Write `cargo test -p roll-for-shoes-server` cases for the settings row: a world with no row reads all defaults; an upsert round-trips; each of the three validations refuses

### Pack web

- [X] T014 [P] Add `WorldSettings`, `DEFAULT_SETTINGS`, `Band`, `BAND_DICE` and `BAND_TARGET` to `packs/systems/roll_for_shoes/web/src/game.ts` per contracts/game-rules.md. Data, not formulas — four bands with two sets of numbers, because a formula would imply a fifth band
- [X] T015 [P] Create `packs/systems/roll_for_shoes/web/src/settings.ts`: the GraphQL documents for both fields and a typed read/write through `postGraphQL` from `@thunderforge/host`. No shared web file learns the system exists
- [X] T016 Create `packs/systems/roll_for_shoes/web/src/panels/world-settings.tsx` per contracts/surfaces.md: five independent controls, read-only when `isGm` is false, `onWorldChanged()` after a write, test ids `rfs-settings*`. It reads its own settings — `world: WorldRecord` does not carry them
- [X] T017 Add `statuses` and `boughtSlots` to `data_types` in `packs/systems/roll_for_shoes/system.json`, and replace the pack-custom `startingSkill` with `startingSkills` as an array whose default is the core `Do Anything 1` (data-model.md §3)
- [X] T018 Confirm `node scripts/check-system-registry.mjs` still passes with `KNOWN` empty — nothing added so far may name the system in shared source

**Checkpoint**: a Game Master can set all five settings and they persist. No rule reads them yet.

---

## Phase 3: User Story 1 — A world that wants none of this never sees it (P1) 🎯 MVP

**Goal**: everything above is invisible to a world that touches nothing.

**Independent test**: `apps/web/e2e/system-roll-for-shoes.spec.ts` passes with no edit, and `resolve(DEFAULT_SETTINGS)` agrees with spec 061's `verdict()` everywhere.

- [X] T019 [US1] Write the tests first in `packs/systems/roll_for_shoes/web/src/game.test.ts`: for every combination of total and opposition across a representative range, `resolve()` under `DEFAULT_SETTINGS` produces the same verdict and the same XP as spec 061's `verdict()` and `xpAward()`. **This is the single most important test in the feature** — everything else it adds is optional; this is the claim that the core game did not change
- [X] T020 [US1] Add `resolve(RollInput): RollOutcome` to `game.ts` in the fixed order dice → sum → statuses → opposition → comparison (research D3, contracts/game-rules.md), and re-express the existing `verdict()` in terms of it so spec 061's tests keep testing something
- [X] T021 [US1] Add a `game.test.ts` case asserting the **order** directly: a status changes `total` and never `sum`, and `sum` is always the plain sum of `faces`
- [X] T022 [US1] Have `ActorSheet.tsx` read the world's settings once on mount and route its roll through `resolve()`, rendering exactly what it rendered before when every setting is off. Every test id spec 061 established keeps its meaning (contracts/surfaces.md)
- [X] T023 [US1] Run `pnpm e2e:game-systems` and confirm `system-roll-for-shoes.spec.ts` passes **unmodified** (FR-004). If it needed an edit, T022 is wrong

**Checkpoint**: the core game is provably unchanged, and there is somewhere for the rest to plug in.

---

## Phase 4: User Story 2 — The Game Master picks a difficulty instead of a number (P1)

**Goal**: difficulty by band, rolled or static, with the free number still available.

**Independent test**: with the mode set to `rolled`, Very Hard shows four Game Master dice that are not the character's; with `target`, Hard shows 9.

- [X] T024 [P] [US2] `game.test.ts` cases for `BAND_DICE` and `BAND_TARGET`, and for a rolled opposition being compared under the same "beat it" rule as a typed one
- [X] T025 [US2] Add `rfs-difficulty-mode` and the four `rfs-band-<band>` controls to `ActorSheet.tsx`, shown only when the mode is not `free`. The typed `rfs-opposition` stays available in every mode (FR-011)
- [X] T026 [US2] Implement the rolled difficulty as a **second `rollDice` call** with a `(BAND)d6` binding, exactly as a skill roll uses `(LEVEL)d6` (research D7). Render it as `rfs-gm-dice` / `rfs-gm-die-<index>` / `rfs-gm-total`
- [X] T027 [US2] Assert structurally that `rfs-die-<index>` never shows a Game Master die and the `faces` array passed to `isAdvancement` never contains one (FR-014)
- [X] T028 [US2] Create `apps/web/e2e/system-roll-for-shoes-extras.spec.ts` and add the difficulty scenarios from quickstart.md. Build every assertion from what is true of **every** roll — there is no dice seed. Assert `rfs-error` has count 0 wherever a write is expected

**Checkpoint**: difficulty works; the tie rule, statuses and slots are still off.

---

## Phase 5: User Story 3 — A tie stops being a loss (P1)

**Goal**: with the setting on, a tie is a success and awards no XP.

**Independent test**: one die against a target of 6 — the only way to tie — succeeds and leaves XP unchanged.

- [X] T029 [P] [US3] `game.test.ts` cases: `tieSucceeds: true` makes `total === opposition` a success with `xpAwarded: 0`; `false` keeps spec 061's failure-with-1-XP. The suppression is not a second rule — a tie is a success, and successes never award XP (FR-018)
- [X] T030 [US3] Thread `tieSucceeds` from the settings into `resolve()` in `ActorSheet.tsx`. No new UI: the setting changes what `rfs-result` says, not what the sheet shows
- [X] T031 [US3] `game.test.ts` case: the tie rule does not touch `isAdvancement` — advancement still inspects the raw rolled dice (FR-017)
- [X] T032 [US3] Add the tie scenario to `system-roll-for-shoes-extras.spec.ts` per quickstart.md

**Checkpoint**: two P1 stories done. This is a shippable increment.

---

## Phase 6: User Story 4 — Circumstance written on the character (P2)

**Goal**: named statuses carrying signed modifiers, summed, applied flat to the total.

**Independent test**: a −100 status cannot produce a positive total, and cannot change whether the advancement prompt appears.

- [X] T033 [P] [US4] `game.test.ts` cases for `statusesOf` (absent → `[]`, no read-time default), `statusModifier` (sums, including to zero and negative), `addStatus` (refuses an empty name), `removeStatus`
- [X] T034 [P] [US4] The guarantee test: for a set of faces, `isAdvancement` returns the same answer with any statuses attached — because it is never passed them (FR-023, research D4). Assert the signature has not grown
- [X] T035 [P] [US4] Add the status validator rules to `packs/systems/roll_for_shoes/server/src/validators.rs` alongside T1–T11: ids unique within the character, names non-empty, `modifier` an integer. Nothing caps count or magnitude
- [X] T036 [US4] Create `packs/systems/roll_for_shoes/web/src/components/StatusList.tsx`: `rfs-statuses`, `rfs-status-<id>`, `rfs-status-add`, `rfs-status-remove-<id>`. Labels the table writes — the system ships no list (FR-020)
- [X] T037 [US4] Wire statuses into `ActorSheet.tsx`: persist through the host's `updateActorSystemData`, show the adjustment as `rfs-modifier`, and have `rfs-total` show sum plus modifier while the dice keep showing their faces
- [X] T038 [US4] Add the status scenarios to `system-roll-for-shoes-extras.spec.ts`, including the one that proves a status cannot create or destroy an advancement

**Checkpoint**: statuses work, and provably do not leak into advancement.

---

## Phase 7: User Story 5 — A level fills up, and the character is told (P2)

**Goal**: per-level caps on how many skills sit at a level, with slots buyable at twice the level.

**Independent test**: a character with four level-2 skills who rolls an advancement is told there is no room, and 4 XP buys one.

- [X] T039 [P] [US5] `game.test.ts` cases for `SLOT_CAPS`, `capAtLevel` (levels 1 and 5+ return `null`, meaning uncapped — never a large number), `slotsUsed`, `slotsAvailable`, `slotCost` (`level * 2`)
- [X] T040 [P] [US5] The ledger test (FR-035): a sequence that spends XP on a die-into-a-six and on a slot debits the same balance honestly, and both refuse on an insufficient balance rather than going negative
- [X] T041 [P] [US5] Add the bought-slots validator rule to `validators.rs`: keys parse as integers ≥ 1, values are integers ≥ 0. **Deliberately do not** add a rule refusing a character whose skills exceed the caps — enabling the setting must not make existing characters unstorable (FR-036, research D6). Write that as a test, not only as a comment
- [X] T042 [US5] Add `buySlot` to `game.ts` returning `SlotBought | SlotRefusal`, mirroring `spendXp`'s existing refusal shape
- [X] T043 [US5] Extend `components/AdvancementPrompt.tsx` with `rfs-advancement-no-room` and `rfs-buy-slot`. The character is **told**, not silently denied — the advancement happened, the room did not exist (FR-030)
- [X] T044 [US5] Wire slot purchase into `ActorSheet.tsx` against the same `resource_data.xp`, and persist `boughtSlots` through `updateActorSystemData`
- [X] T045 [US5] Add the slot scenarios to `system-roll-for-shoes-extras.spec.ts`, including watching `rfs-xp` fall by 4 for a level-2 slot

**Checkpoint**: all P1 and P2 stories done.

---

## Phase 8: User Story 6 — A world where everybody starts as something (P3)

**Goal**: a world may define its own starting skills without disturbing anyone.

**Independent test**: change a world's starting skills, then open a character created before the change — it is unchanged.

- [X] T046 [P] [US6] `game.test.ts` cases for `startingSkills(settings)` (empty → the core `Do Anything 1`) and for `skillsOf` keeping its existing one-argument behaviour exactly
- [X] T047 [US6] The narrowing test from research D5, written explicitly: **a character with stored skills never reads the world's starting skills**, whatever the world later says. This is the mechanical rule FR-039 reduces to once the read-time default is accounted for
- [X] T048 [US6] Implement `startingSkills` and the optional `settings` argument to `skillsOf` in `game.ts`, resolving the world's set at the moment the character first stores anything — not on every read

  **⚠ Conflict found in Phase 2 that no design document names.** `validators.rs` rules **T10** ("must have exactly one starting skill" — exactly one root, `parentId: null`) and **T11** ("the starting skill must be at level 1") make two of this story's legal worlds unstorable:

  - a world declaring **two or more** starting skills produces a character with two roots → T10 refuses it;
  - a world declaring a starting skill at **level 2 or higher** produces a root above level 1 → T11 refuses it.

  Both are exactly what FR-037–041 says a world may do ("its own starting skill **or skills**, each with a name and a level"), so as it stands US6 ships a setting that creates characters the server will not save.

  The hard part is that `validate_trait_data` is **world-blind**: the registry hands it a character's JSON with no world, so it cannot ask whether that world allows several roots. Three ways out, to be decided before T048 is written:

  1. **Relax T10/T11 unconditionally** to "at least one root, each root at level ≥ 1", keeping T8/T9 (every non-root sits one level above its named parent). Cheapest, and T9 still carries the lineage invariant that actually matters. Cost: a core-rules world loses a guard it has today.
  2. **Thread the world through the validator.** Correct but large, and it changes a shared registry signature every pack implements — out of proportion to this spec.
  3. **Forbid what the validator forbids**: cap the settings at one starting skill at level 1, which reduces the Extra to renaming `Do Anything`. That contradicts the spec.

  Recommendation is (1), with T11's intent preserved as "a root is at level ≥ 1" and the change confined to this pack's own `validators.rs`. It needs its own tests replacing `t10_*`/`t11_*`, and it is a rules change, so it wants a line in the ADR.
- [X] T049 [US6] Add the starting-skills editor to `panels/world-settings.tsx` (`rfs-setting-starting-skills`): a list of name-and-level rows, with an empty list meaning the core default rather than "no skills"
- [X] T050 [US6] Add the starting-skills scenario to `system-roll-for-shoes-extras.spec.ts`, including opening a character made before the change

**Checkpoint**: all six stories done.

---

## Phase 9: Polish & cross-cutting

- [X] T051 Write `docs/adrs/20260923-108-<slug>.md`: declining to build a generic world-settings surface at the moment ADR-063's own "one pack is a case, two is a shape" threshold was met, what was built instead, and what the third case should do. Per constitution Principle IV it lands in this change set, not after it. Reasoning is in research D1
- [X] T052 [P] Update `packs/systems/roll_for_shoes/README.md` (and the pack's entry in `packs/systems/README.md` if it names the crate as table-free) to describe the five settings and the new table
- [X] T053 [P] Add the `world-settings` panel slot to `packs/systems/README.md`'s pack contract — the four slots are currently documented only in `apps/web/src/host/index.ts`, which is why this feature had to go read source to find them. A second pack has now used one; that makes it contract, not trivia
- [X] T054 Run the interaction scenarios from quickstart.md that cross two Extras: statuses with the tie rule, slots with XP spending, a rolled difficulty with a status. Each pair must behave as the fixed roll order says, with no combination silently changing a third setting (FR-002)
- [X] T055 Run `node scripts/check-system-registry.mjs` and confirm `KNOWN` is still empty (FR-006)
- [X] T056 Run `pnpm verify` (rustfmt, clippy, prettier, eslint) and fix what it reports **in the code this feature added**. Keep it to that — a repo-wide lint pass folded in here buries the feature work. `pnpm verify:fix` handles the mechanical part
- [X] T057 Proof: run `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1 pnpm e2e:game-systems --record-durations` alone, grep the log for ✘, and record the `Totals:` line and wall time here. Commit the changed line in `scripts/e2e/slice-durations.json`. The full suite is for releases and cross-cutting changes (`pnpm e2e:which` says which)
  - **Measured 2026-09-23** on a quiet machine: `Totals: 26 passed, 0 failed, 0 flaky, 0 skipped.` in **189s** for 6 specs (was 176s / 5 specs / 18 passed). `scripts/e2e/slice-durations.json` re-recorded.
  - Two earlier runs failed, in two different tests, on one shared defect: neither could tell that the dice on screen belonged to the roll it had just clicked. `rfs-total` is already visible from the previous attempt, so waiting for it returns instantly on stale content, and `setAttempt` replaces the whole attempt the moment the next roll lands. The status test now rolls against 99 so every roll pays 1 XP and the ledger is the signal; the cross-Extra test cannot use that — the outcome it hunts is a success, and successes pay nothing — so it reloads first, discarding the component state that holds the last result.

---

## Dependencies & execution order

### Phase dependencies

- **Setup (1)** → **Foundational (2)** blocks everything. Settings that cannot be stored cannot vary by world, and every story reads them.
- **US1 (3)** is not optional and is not parallel with the rest: it establishes `resolve()`, which US2–US4 all route through, and it proves the core game did not move. Everything else builds on it.
- **US2 (4)**, **US3 (5)**, **US4 (6)**, **US5 (7)**, **US6 (8)** are independent of each other once US1 is done, and each is a shippable increment. They touch `ActorSheet.tsx` in common, so run them in sequence unless split across worktrees.
- **Polish (9)** last, except T051 — the ADR may be written any time after Phase 2 and must land in this change set.

### Within each story

Tests before implementation. Pure `game.ts` rules before the sheet that calls them. The e2e scenario last, because it proves the rule reached a screen rather than re-deriving the arithmetic.

### Parallel opportunities

- T003 ‖ T004 (two different `diesel.toml` files)
- T013 ‖ T014 ‖ T015 (Rust test, pure rules, network module — three files, no overlap)
- T024, T029, T033, T034, T039, T040, T046 are all `game.test.ts` additions for different stories; parallel across worktrees, sequential in one
- T035 ‖ T041 both touch `validators.rs` — **not** parallel with each other
- T052 ‖ T053 (different documents)

---

## Implementation strategy

**MVP is Phases 1–3 plus Phase 5.** Setup, Foundational, US1 and US3 give a table the tie rule — the smallest Extra, the one the site itself offers as the first thing to change — on top of a proof that nothing else moved. That is shippable on its own.

Then US2 (difficulty), US4 (statuses), US5 (slots), US6 (starting skills), in that order — spec priority, and also increasing cost. US5 is the largest: it is the only Extra that touches both the stored shape and the XP ledger.

## Notes

- There is **no dice seed**. Every e2e assertion is built from what is true of every roll — one d6 cannot beat 6, four d6 cannot reach 25, a −100 status cannot yield a positive total.
- A player driving the play dock needs Editor granted **explicitly**; claiming a character grants no write access today (FR-045). That host gap is its own spec: **spec 063**, which closes it and withdraws this constraint.
- Assert `rfs-error` has count 0 wherever a write is expected to land. The sheet badges refusals rather than throwing, so a lost write otherwise reads as a disagreement about a number.
- Read only the harness's `Totals:` line.
