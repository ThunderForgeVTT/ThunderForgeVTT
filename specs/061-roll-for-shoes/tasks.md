---
description: "Task list for Roll for Shoes"
---

# Tasks: Roll for Shoes

**Input**: Design documents from `/specs/061-roll-for-shoes/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/)

**Tests**: included. The spec asks for one end-to-end proof (FR-047), and the
plan puts the game's rules in a pure module precisely so they can be tested
without a browser or a server. Both are below.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel — different files, no dependency on unfinished work
- **[Story]**: the user story from spec.md this serves

## Path Conventions

Everything this feature adds lives in `packs/systems/roll_for_shoes/`, except
one e2e specification, one slice entry, and the three build-graph lines the
pack contract allows.

---

## Phase 1: Setup

**Purpose**: the pack exists, is discovered, and is linked into the build.

- [X] T001 Create `packs/systems/roll_for_shoes/system.json` exactly as [contracts/system-manifest.md](./contracts/system-manifest.md) specifies — identity, `legal` (CC0 1.0, crediting Ben Wray and rollforshoes.com and asserting nothing further), `data_types` for `trait_data` and `resource_data`, the `xp` counter in `resources`, one `description` entry in `sheet`, `turnStructure: { rounds: false }`, and `startingSkill`. Declare no `abilities`, no `movement`, no `vision`, no `combat`, no `appearance` and no `checks`.
- [X] T002 [P] Scaffold `packs/systems/roll_for_shoes/web/` — `package.json` (`@thunderforge/roll-for-shoes`, `tsc && vite build`, the `node --test` script), `tsconfig.json`, `vite.config.ts`. Copy Genie's shape; depend on nothing beyond React and `@thunderforge/host`.
- [X] T003 [P] Scaffold `packs/systems/roll_for_shoes/server/Cargo.toml` (package `roll-for-shoes-server`), depending on `inventory`, `serde_json` and `thunderforge_canvas_core`.
- [X] T004 Add the three lines outside the pack directory and no others: the `members` entry in the root `Cargo.toml`, the dependency in `src/app/Cargo.toml`, and `use roll_for_shoes_server as _;` in `src/app/src/system_packs.rs`. Confirm `scripts/check-system-registry.mjs` still passes.
- [X] T005 Add `packs/systems/roll_for_shoes/**` to the `game-systems` slice's `paths` in `scripts/e2e/slices.json`, extending the existing brace list of pack directories. `pnpm verify`'s `e2e-slices` check must report 0 orphans. No new `package.json` script is needed — `e2e:game-systems` already exists.

**Checkpoint**: the stack builds, `/api/systems` offers Roll for Shoes, and its manifest passes the enforced legal check.

---

## Phase 2: Foundational (blocking)

**Purpose**: the system can be registered and can store data, and the rules of
the game exist as testable functions. **No user story can begin until this
phase is done** — without registration every write is refused.

- [X] T006 Implement `packs/systems/roll_for_shoes/server/src/validators.rs`: `validate_resource_data` (rules R1–R3) and `validate_trait_data` (rules T1–T11) from [data-model.md](./data-model.md#validation), plus the `*_for_registry` adapters that map the internal error to `String`. Judge no skill name's specificity or relevance, and impose no level cap.
- [X] T007 [P] Write `packs/systems/roll_for_shoes/server/src/validators_tests.rs` covering every numbered rule, including the two that are easy to get wrong: a second root is refused (T10), and a level that is not one above its parent is refused (T9). Add a test asserting a cycle is unreachable because T9 forbids it.
- [X] T008 Implement `packs/systems/roll_for_shoes/server/src/lib.rs`: `SYSTEM_ID` equal to the manifest's `id`, and the `inventory::submit!` `SystemContribution` naming only `trait_data` and `resource_data`. Leave `ability_data`, `proficiency_data`, `spell_data` and `rules` absent.
- [X] T009 Prove registration works: bring the stack up and confirm a write to `updateActorSystemData` for a `roll_for_shoes` actor is accepted, rather than refused with `System 'roll_for_shoes' not registered`. This is the failure research D2 predicts if T004 or T008 is wrong, and it is silent until exercised.
- [X] T010 [P] Implement `packs/systems/roll_for_shoes/web/src/game.ts` as pure functions over plain data, with no React and no network: `verdict(total, opposition)` (strictly greater wins, equal fails, absent is unjudged), `xpAward(verdict)`, `sixesShown(dice, bought)`, `isAdvancement(dice, bought, level)`, `grantSkill(skills, parent, name)`, `spendXp(balance, dice, bought)`, and the lineage ordering the sheet renders.
- [X] T011 [P] Write `packs/systems/roll_for_shoes/web/src/game.test.ts` (`node --test`). Cover the tie as a failure, the unjudged roll paying nothing, all-sixes reached by buying, and a grant recording its parent at one level higher.
- [X] T012 Create `packs/systems/roll_for_shoes/web/src/ActorSheet.tsx` and `index.ts` — the shell the glob discovers. It takes `{ actor, canEdit }` and nothing else, imports only from `@thunderforge/host`, carries the `rfs-sheet` test id, and renders every refusal through a `StatusBadge` at `rfs-error`. Every write spreads the slot it read and is followed by `refetch()`.

**Checkpoint**: the sheet mounts for a Roll for Shoes actor, the game's rules are tested, and data can be saved.

---

## Phase 3: User Story 1 — A game that starts with a name (P1) 🎯 MVP

**Goal**: a character exists after typing a name, with nothing to fill in.

**Independent Test**: create an actor in a Roll for Shoes world, open its sheet, and see XP 0 and one skill at level 1 without having entered anything.

- [X] T013 [US1] In `web/src/game.ts`, state the starting skill as a constant matching the manifest's `startingSkill`, with a comment pointing at research D6 for why it is stated in both places. A character whose `skills` are absent or empty reads as holding exactly that skill.
- [X] T014 [US1] In `ActorSheet.tsx`, render the XP balance at `rfs-xp`, reading `resource_data.xp` and treating an absent value as 0.
- [X] T015 [US1] Render the description from `trait_data.description` — editable when `canEdit`, shown as text otherwise — and the character's name from the actor's own label, which the pack does not own.

**Checkpoint**: US1 works. A character is a name and one skill.

---

## Phase 4: User Story 2 — Roll the skill, beat the opposition (P1)

**Goal**: rolling a skill rolls its dice on the server and says what happened.

**Independent Test**: roll the starting skill against an opposition of 6 and see one die, its face, the total, the 6 it was judged against, and a failure.

- [X] T016 [US2] Build `web/src/components/SkillLineage.tsx`: every skill the character holds, each with its level and a roll button at `rfs-roll-<id>`, rows at `rfs-skill-<id>`. Every skill is rollable (FR-024).
- [X] T017 [US2] Add the opposition control at `rfs-opposition` — one optional number, labelled as what the Game Master said. Empty is a real state, not zero.
- [X] T018 [US2] Implement the roll in `ActorSheet.tsx`: `postGraphQL` a `rollDice` with formula `(LEVEL)d6` and a `LEVEL` binding of the skill's level, against `actor.worldId`. Render the dice at `rfs-die-<n>`, the total at `rfs-total`, and the verdict at `rfs-result`. Catch `GraphQLRequestError` and render it — including the dice-engine refusal a level above 1000 produces, which is shown and never clamped (research D7).
- [X] T019 [US2] Render an unjudged roll as unjudged: the dice and total are shown, no success and no failure is claimed, and nothing is written (FR-023).

**Checkpoint**: US2 works. The dice on screen are the server's dice.

---

## Phase 5: User Story 3 — Failure is the only thing that pays (P1)

**Goal**: failing is how a character grows.

**Independent Test**: roll against an opposition of 6 twice and watch XP go 0 → 1 → 2; roll unjudged and watch it stay put.

- [X] T020 [US3] On a judged failure, write `resource_data` with `xp` increased by exactly 1, then `refetch()`. A success and an unjudged roll write nothing. A tie is a failure and pays (FR-022, FR-025, FR-026). Surface a rejected write at `rfs-error` rather than losing it.

**Checkpoint**: US3 works. The three P1 stories together are a playable game.

---

## Phase 6: User Story 4 — All sixes, and a skill that did not exist before (P1)

**Goal**: the moment the game is named for.

**Independent Test**: drive a roll whose dice all show six and be offered a new skill one level higher, beneath the one rolled.

- [X] T021 [US4] Detect advancement from `dice[].finalValue` plus any bought conversions — never from `resultValue`, which cannot tell you whether every die showed a six (FR-034).
- [X] T022 [US4] Build `web/src/components/AdvancementPrompt.tsx` at `rfs-advancement`: a name input at `rfs-advancement-name`, confirm and decline at `rfs-advancement-confirm` / `rfs-advancement-decline`. An empty name is refused with the reason shown and no skill created (FR-037). Declining creates nothing and changes nothing already settled (FR-038).
- [X] T023 [US4] On confirm, append a skill at the rolled skill's level plus one, recording the rolled skill as its parent, and write `trait_data`. The parent is never removed or replaced (FR-039).
- [X] T024 [US4] Make the two outcomes independent: a roll that both fails and shows all sixes awards the XP *and* offers the advancement (FR-041).

**Checkpoint**: all four P1 stories work. This is the MVP.

---

## Phase 7: User Story 5 — Spending XP buys the skill, never the outcome (P2)

**Goal**: XP buys a chance at advancement and cannot buy success.

**Independent Test**: fail a roll, spend the XP it just paid to turn its die into a six, and watch the verdict stay a failure while the advancement is offered.

- [X] T025 [US5] Add the spend control at `rfs-spend-xp`: one XP converts one die that is not already a six, offered while non-six dice remain. XP awarded by this roll is spendable on this roll (FR-029). A spend beyond the balance is refused before any mutation is sent, and the refusal says the balance is too low (FR-030). XP buys nothing else (FR-031).
- [X] T026 [US5] Assert in `game.test.ts` that a spend leaves the verdict, the total and the XP the roll awarded untouched (FR-028) — the property that keeps this from becoming a way to buy success.

**Checkpoint**: US5 works.

---

## Phase 8: User Story 6 — The table decides what "more specific" means (P2)

**Goal**: the product prompts and records; it does not arbitrate.

**Independent Test**: name a new skill something absurdly broad and watch it be accepted without comment.

- [X] T027 [US6] Word the advancement prompt so it states the new skill should be more specific than the skill rolled and relevant to what was attempted — as guidance to the table, not a rule the product enforces. Accept any non-empty name (FR-036). No validation of specificity or relevance exists anywhere in the pack, server side or web side.

**Checkpoint**: US6 works.

---

## Phase 9: User Story 7 — A sheet that shows where a skill came from (P3)

**Goal**: the lineage is legible, because it is the character's history.

**Independent Test**: after two advancements, read the sheet and see which skill each one grew out of.

- [X] T028 [US7] Render the lineage nested — each skill beneath the one it advanced from, the root first, each showing its level (FR-042, FR-043).
- [ ] T029 [US7] Make the sheet work in the play dock: mounted with `canEdit: false`, compacted to roughly 22rem, no crash without edit permission, and rolling still available — the dock is where a player sits during play. There is no declarative fallback behind it.

**Checkpoint**: every user story works.

---

## Phase 10: Polish & Proof

- [X] T030 Write `apps/web/e2e/system-roll-for-shoes.spec.ts` — the name puts it in the `game-systems` slice, which its `system-` prefix owns. Play the loop of FR-047: register, create a world on `roll_for_shoes` through GraphQL, create an actor, open the sheet, roll the starting skill against an opposition of 6, assert the failure and the XP it paid, then drive an advancement and name the skill it grants. Assert the dice as invariants — one die per level, six sides, faces in range, total equal to the sum — because **no seed exists and a roll cannot be forced** (research D9).
- [X] T031 [P] Assert the sheet is accessible with `expectNoAxeViolations` from the e2e fixtures.
- [X] T032 [P] Fix the stale path in `packs/systems/README.md`: the linkage line lives in `src/app/src/system_packs.rs`, not `src/server/src/system_packs.rs`. One line, found while writing this plan, kept separate from the feature's own code.
- [ ] T033 Walk [quickstart.md](./quickstart.md) end to end on a real stack, including the play-dock check and the reload.
- [X] T034 Proof: run `pnpm e2e:game-systems` alone with `--record-durations`, grep the log for `✘`, and record the result and wall time here. The full suite is for releases and cross-cutting changes; this feature is neither.

  **Result (2026-09-22)**: `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1 pnpm e2e:game-systems --record-durations` — **17 passed, 0 failed, 0 flaky, 0 skipped** across 5 specs on 1 shard, **152s** wall (recorded to `scripts/e2e/slice-durations.json`). No `✘` in the log. The first attempt crashed before any test ran because the Postgres container was down (`Totals: 0 passed, 0 failed`); the second surfaced one real defect, a pre-existing unnamed `<select>` in the host's Ownership block, fixed separately.
- [X] T035 Run `pnpm verify` and fix what it reports **in the code this feature added** — `rust-fmt`, `rust-lint`, `web-fmt`, `web-lint`, `registry`, `packdocs`, `filelength`, `e2e-slices`. `pnpm verify:fix` rewrites what can be rewritten mechanically. Repo-wide lint work gets its own commit.

---

## Dependencies & Execution Order

### Phase dependencies

- **Phase 1 (Setup)** — start immediately.
- **Phase 2 (Foundational)** — depends on Setup, and **blocks every story**. T009 in particular: until registration is proven, no write works and every story would fail for the same hidden reason.
- **Phases 3–9 (Stories)** — all depend on Phase 2. In priority order they build a playable game at the end of Phase 6.
- **Phase 10 (Polish)** — depends on the stories it proves.

### Within the stories

US1 → US2 → US3 → US4 is a genuine chain: you cannot roll a skill before a character has one, cannot award XP before a verdict exists, and cannot buy an advancement before dice exist to buy. US5 depends on US3 (there must be XP) and US4 (there must be something to buy). US6 refines US4's prompt. US7 is presentation over what US4 stores.

### Parallel opportunities

- T002 and T003 (two scaffolds, two directories).
- T007, T010 and T011 — the server's tests and the web's rules module touch nothing in common.
- T031 and T032 are independent of each other and of T030.

---

## Implementation Strategy

**MVP is Phases 1–6**: a character, a roll, XP from failure, and the
advancement. That is the whole game as the source states it; everything after
it is refinement of how the game is spent and shown.

Stop at the Phase 6 checkpoint and play it. Roll for Shoes is short enough that
a full session takes minutes, and the quickstart's step 4 — a single d6 against
an opposition of 6 — is the one outcome the dice cannot refuse you.

## Notes

- Commit after each task or logical group. Every commit is signed.
- The pack's whole surface is its own directory plus three build-graph lines.
  If a task tempts you to name `roll_for_shoes` anywhere else in shared code,
  the registry check will fail and it is telling you the truth.
- No migration, no GraphQL schema change, no new root field. If one appears to
  be needed, re-read research D3 and D5 before writing it.
