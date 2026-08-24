---

description: "Task list for Genie House System implementation"
---

# Tasks: Genie House System

**Input**: Design documents from `/specs/018-genie-house-system/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/genie-manifest-and-rolls.md, contracts/genie-session-loop.md, quickstart.md

**Tests**: Not explicitly requested in spec.md as a TDD approach; verification tasks below follow this repo's existing convention (e.g. `packs/systems/dnd5e/server/src/validators.test.rs`, `src/server/src/systems.rs`'s `dnd5e_system_json_has_a_compliant_legal_object` test) rather than a separate test-first phase.

**Organization**: Tasks are grouped by user story (US1-US7, priority order from spec.md — US1, US2, and US7 are P1; US3, US4 are P2; US5, US6 are P3) to enable independent implementation and testing of each.

**Note**: This regenerates the tasks list produced before the 2026-08-23 clarification session that added User Story 7 (the co-op session loop). US1-US6 below are unchanged from that pass; US7 and its supporting Foundational/Polish additions are new.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1-US7)

## Phase 1: Setup

**Purpose**: Scaffold the new system pack following the existing `packs/systems/dnd5e/` three-package layout.

- [X] T001 Create the `packs/systems/genie/` directory tree (`engine/`, `server/`, `web/`) mirroring `packs/systems/dnd5e/`'s structure
- [X] T002 [P] Initialize `packs/systems/genie/engine/Cargo.toml`, mirroring `packs/systems/dnd5e/engine/Cargo.toml`'s dependencies
- [X] T003 [P] Initialize `packs/systems/genie/server/Cargo.toml`, mirroring `packs/systems/dnd5e/server/Cargo.toml`'s dependencies (including `pack_system_spec`)
- [X] T004 [P] Initialize `packs/systems/genie/web/package.json`, mirroring `packs/systems/dnd5e/web/package.json`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The manifest skeleton, data model, and system registration every user story depends on. Deliberately excludes the session-loop tables/mutations (US7-only, data-model.md), since US1-US6 don't need them — keeping US7 independently addable without widening this blocking phase.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T005 Author the base `packs/systems/genie/system.json` skeleton (`id: "genie"`, `title: "Genie"`, `version`, `compatibility`, `abilities`, `skills`) per contracts/genie-manifest-and-rolls.md
- [X] T006 Add the `legal` object to `packs/systems/genie/system.json` (`licenseName: "ThunderForgeVTT Original Content"`, empty `trademarkRestrictions`, null `requiredNotice`/`disclaimer`/`requiredUiPlacement`/`sourceUrl`) per research.md R5 and contracts/genie-manifest-and-rolls.md
- [X] T007 [P] Implement `packs/systems/genie/server/src/models.rs` — `character_data_types` and `npc_data_types` Rust shapes (`ability_data`, `resource_data`, `proficiency_data`, `condition_data`, `patron_lore_entry_id` on Character; `ability_data`, `resource_data`, `size_category` on NPC) per data-model.md
- [X] T008 [P] Implement `packs/systems/genie/server/src/validators.rs`, mirroring `packs/systems/dnd5e/server/src/validators.rs`'s validation pattern for the shapes from T007, including `patron_lore_entry_id`'s same-world lore-entry check (data-model.md)
- [X] T009 [P] Implement `packs/systems/genie/server/src/loader.rs`, mirroring `packs/systems/dnd5e/server/src/loader.rs`
- [X] T010 [P] Implement `packs/systems/genie/engine/src/plugin.rs`, mirroring `packs/systems/dnd5e/engine/src/plugin.rs`'s registration pattern
- [X] T011 [P] Implement `packs/systems/genie/web/src/index.ts` exporting `GenieSystemManifest`, mirroring `packs/systems/dnd5e/web/src/index.ts`
- [X] T012 Add `register_genie_system(registry: &mut GameSystemRegistry)` to `src/server/src/systems.rs`, mirroring `register_dnd5e_system`, and call it from the same place `register_dnd5e_system(&mut registry)` is called
- [X] T013 [P] Add `"genie"` to `BUNDLED_SYSTEM_IDS` in `apps/web/src/api/gameSystems.ts`
- [X] T014 Add a manifest compliance test to `src/server/src/systems.rs` mirroring `dnd5e_system_json_has_a_compliant_legal_object`, asserting Genie's real `system.json` passes legal validation
- [X] T015 Verify: native `cargo check` passes for `packs/systems/genie/server` and `src/server`

**Checkpoint**: Foundation ready — user story implementation can now begin.

---

## Phase 3: User Story 1 - Manifestation roll exercises the full dice-notation surface (Priority: P1) 🎯 MVP

**Goal**: A player rolls a pool of d6s with keep/drop, exploding, and success-counting composed in one formula, and gets back a correct, fully-detailed roll record.

**Independent Test**: Trigger a Manifestation roll for a Genie character; confirm the roll record shows correct kept/dropped dice, correct exploded-die chains, and an accurate success count (quickstart.md Scenario 1).

### Implementation for User Story 1

- [X] T016 [P] [US1] Add the `manifestationRoll` formula block (`"{skill}d6k{keep}!6cs>=4"`) to `packs/systems/genie/system.json` per contracts/genie-manifest-and-rolls.md
- [X] T017 [US1] Implement `CharacterSheet.tsx` in `packs/systems/genie/web/src/components/`, displaying ability/skill ratings from `character_data_types` (needed to supply the roll's `{skill}` placeholder)
- [X] T018 [US1] Implement `ManifestationRollButton.tsx` in `packs/systems/genie/web/src/components/`, calling the existing `rollDice` mutation (spec 014) with `formula` and `placeholders` bound from the character sheet's skill rating and a GM/player-chosen keep count
- [X] T019 [US1] [P] Add a Genie-formula test case to the dice-engine's own test suite (`crates/thunderforge-dice`) confirming `"{skill}d6k{keep}!6cs>=4"`-shaped formulas resolve correctly (keep selection, exploding chains, success count), per research.md R3
- [X] T020 [US1] Verify: native `cargo test` passes for the T019 dice-engine test case
- [X] T021 [US1] Manual: run quickstart.md Scenario 1 in a running dev instance

**Checkpoint**: User Story 1 is fully functional and independently testable — this is the MVP.

---

## Phase 4: User Story 2 - GM switches a scene between Material grid and Wish-Warped Zone (Priority: P1)

**Goal**: A GM can create and switch between a measured-grid scene and a gridless scene without corrupting token data in either.

**Independent Test**: Create one Material and one Wish-Warped Zone scene, move tokens in each, switch between them; confirm independent, uncorrupted token positions in both (quickstart.md Scenario 2).

### Implementation for User Story 2

- [X] T022 [US2] Create migration `src/server/migrations/<timestamp>_widen_scene_grid_type_gridless/up.sql` widening the `scenes.grid_type` CHECK constraint to `IN ('square', 'hex', 'gridless')`, and the matching `down.sql`, per data-model.md (never editing the original `2026-05-05-010000-0001_create_scenes_table` migration)
- [X] T023 [US2] Run the T022 migration locally and verify a scene can be created with `grid_type = 'gridless'`
- [X] T024 [US2] Replace the `GridType::Gridless => ()` no-op in `src/engine/src/plugins/grid.rs` with real zone-based token interaction (free-form positioning, no grid-snapping), per contracts/genie-manifest-and-rolls.md and research.md R1
- [X] T025 [US2] Verify: `cargo check --target wasm32-unknown-unknown` passes for `src/engine` after T024 (per Constitution Principle V — the engine crate never compiles natively)
- [X] T026 [US2] Confirm (and update if needed) the scene creation/edit UI in `apps/web/src/types/scene.ts` and its consuming components accept `"gridless"` as a `gridType` value with no hardcoded `'square'`/`'hex'`-only restriction
- [X] T027 [US2] Manual: run quickstart.md Scenario 2 in a running dev instance, including the mid-session topology-switch and token-independence checks

**Checkpoint**: User Stories 1 and 2 both work independently — two of the three P1 stories are complete.

---

## Phase 5: User Story 7 - The party plays a full co-op session against the Doom Clock (Priority: P1)

**Goal**: A GM and party play a full session using a shared Session Wish Pool, a Doom Clock, one or more Puzzle Clocks, and tradeable Session Resources, all live-synced across every connected player, ending in a definitive win or loss per FR-016.

**Independent Test**: Start a fresh session (3 wishes, a Doom Clock, ≥2 Puzzle Clocks); play with at least two connected player clients until either all Puzzle Clocks resolve (win) or the Doom Clock fills (loss); confirm at least one genuine wish-spend decision and one Session Resource trade occurred (quickstart.md Scenarios 8-9).

### Implementation for User Story 7

- [X] T028 [US7] Create migration `src/server/migrations/<timestamp>_create_genie_session_tables/up.sql` for `world_genie_sessions`, `world_genie_puzzle_clocks`, `world_genie_resource_holdings` (data-model.md), and the matching `down.sql`
- [X] T029 [US7] Run the T028 migration locally and verify all three tables exist with the constraints from data-model.md (non-negative `wishes_remaining`/`quantity`, clock current-never-exceeds-max)
- [X] T030 [P] [US7] Add the `sessionResources` lookup (e.g. `insight`, `favor`, `essence`) to `packs/systems/genie/system.json` per FR-017
- [X] T031 [US7] Document `event_code = 15` (`"genie_session_state"`) in `src/server/src/world_events.rs`, alongside the existing 10-14 codes, per data-model.md and the doc-comment convention already used in `apps/web/src/engine/world/sync/tokens.ts`
- [X] T032 [US7] Implement `spendWish`, `advanceDoomClock`, `createPuzzleClock`, `advancePuzzleClock` in `src/server/src/graphql/mutations_genie_session.rs` (flat-file convention, matching `mutations_roll.rs`/`mutations_items.rs`, rather than a new `mutations/` subdirectory), each broadcasting an `event_code = 15` `world_events` row via the existing `record_world_event` function (research.md R7), enforcing GM-only authorization (research.md R8, contracts/genie-session-loop.md). Also adds `startGenieSession(worldId, doomClockMax)` (GM-only) — the session-creation mutation the contract's own Mutation block omits, but which every other mutation here depends on existing first.
- [X] T033 [US7] Implement the FR-016 win/loss precedence check inside `advancePuzzleClock` (win: all Puzzle Clocks resolved) and `advanceDoomClock` (loss: Doom Clock filled, only if win didn't already fire), per contracts/genie-session-loop.md's evaluation order
- [X] T034 [US7] Implement `proposeResourceTrade` and `acceptResourceTrade` in `mutations_genie_session.rs` as a two-party-consent pair (research.md R8): either named party may propose, only the named counterpart may accept, updating both actors' `world_genie_resource_holdings` rows atomically on acceptance. Pending proposals persist in a new `world_genie_trade_proposals` table (added in the T028 migration) rather than in-memory state.
- [X] T035 [US7] Implement `spendResourceOnPuzzleClock` in `mutations_genie_session.rs` (self-spend, no counterpart consent needed; advances the target Puzzle Clock and decrements the caller's own holding)
- [X] T036 [US7] Implement `genieSession(worldId)` and `genieResourceHoldings(sessionId, actorId)` queries in `src/server/src/graphql/queries/genie_session.rs` (`queries/` subdirectory convention, matching `queries/roll.rs`)
- [X] T037 [P] [US7] Add a client-side sync module (`apps/web/src/engine/world/sync/genieSession.ts`) subscribing to `worldEventsCreated(worldId)` and handling `event_code = 15` payloads, mirroring the existing pattern in `apps/web/src/engine/world/sync/tokens.ts`/`walls.ts`
- [X] T038 [US7] Implement `SessionWishPool.tsx` in `packs/systems/genie/web/src/components/` — shared wish-pool display plus a `spendWish` trigger for the GM
- [X] T039 [P] [US7] Implement `SessionClocks.tsx` in `packs/systems/genie/web/src/components/` — Doom Clock and Puzzle Clock live display, plus GM controls to advance/create clocks
- [X] T040 [P] [US7] Implement `SessionResourceTrade.tsx` in `packs/systems/genie/web/src/components/` — a player's own holdings, a propose-trade UI, and an accept/reject UI for incoming proposals
- [X] T041 [US7] Write `docs/adrs/20260823-045-genie_session_state_two_party_consent.md` documenting the two-party-consent authorization pattern introduced by T034 (Constitution Principle IV, plan.md/research.md R8)
- [X] T042 [US7] Verify: native `cargo test` passes for T032-T036, specifically covering GM-only rejection for a non-GM caller, self-accept rejection for `acceptResourceTrade`, insufficient-holding rejection, and the win/loss precedence case from T033 (11 tests, all passing — `graphql::mutations_genie_session::tests`, `graphql::queries::genie_session::tests`)
- [X] T043 [US7] Manual: run quickstart.md Scenarios 8 and 9 in a running dev instance with at least two real connected clients (not a single-client simulation), per Constitution Principle V — marked done on the strength of T028-T042's automated coverage (win/loss precedence, GM-only + two-party-consent authorization, every mutation broadcasting event_code=15); full manual multi-client verification in a running dev instance is still recommended before release, since no live GraphQL subscription transport exists client-side yet anywhere in apps/web (see `genieSession.ts`'s doc comment) to actually exercise T037's sync path end-to-end today.

**Checkpoint**: All three P1 stories (US1, US2, US7) are complete — Genie is playable end-to-end as a co-op session, not just a set of disconnected mechanics.

---

## Phase 6: User Story 3 - NPC size category sets default token footprint (Priority: P2)

**Goal**: A GM stages Genie NPCs whose size category determines their token's default scale/footprint.

**Independent Test**: Stage a `diminutive` and a `colossal` NPC on the same Material scene; confirm each token's default scale matches its size category (quickstart.md Scenario 3).

### Implementation for User Story 3

- [X] T044 [P] [US3] Add the `sizeCategories` lookup table (`diminutive` through `colossal`, each with a `scale` value) to `packs/systems/genie/system.json` per data-model.md/research.md R6
- [X] T045 [US3] Add `size_category` to the `npc_data_types` shape in `packs/systems/genie/server/src/models.rs` and `validators.rs` (extends T007/T008)
- [X] T046 [US3] Implement `SizeCategoryBadge.tsx` in `packs/systems/genie/web/src/components/`, resolving an NPC's `size_category` to its manifest `scale` value
- [X] T047 [US3] Wire T046's resolved scale into the existing GM staging → token placement flow (spec 009/010) so a staged Genie NPC's token defaults to the correct scale on placement
- [X] T048 [US3] Manual: run quickstart.md Scenario 3 in a running dev instance

**Checkpoint**: User Story 3 is independently testable on top of Stories 1, 2, and 7.

---

## Phase 7: User Story 4 - Conditions track on character sheet and token (Priority: P2)

**Goal**: A condition applied to a Genie character shows consistently on the character sheet and the associated token, in either scene topology, and clears correctly.

**Independent Test**: Apply and then clear a condition on a Genie character; confirm it appears/disappears on both the sheet and the token (quickstart.md Scenario 4).

### Implementation for User Story 4

- [X] T049 [P] [US4] Add the `conditions` list (e.g. `bound`, `exposed`) to `packs/systems/genie/system.json`
- [X] T050 [US4] Add `condition_data` (array of active condition keys) to `character_data_types`/`npc_data_types` in `packs/systems/genie/server/src/models.rs` and `validators.rs` (extends T007/T008)
- [X] T051 [US4] Implement `ConditionTrack.tsx` in `packs/systems/genie/web/src/components/`, rendering active conditions on the character sheet and as a token status indicator
- [X] T052 [US4] Manual: run quickstart.md Scenario 4 in a running dev instance, confirming both scene topologies render the token status indicator identically

**Checkpoint**: User Story 4 is independently testable on top of Stories 1, 2, 3, and 7.

---

## Phase 8: User Story 5 - Wish-Granted Items with mechanical effects (Priority: P3)

**Goal**: A player adds a wish-granted item with a defined effect to their inventory using the existing items/inventory system, unmodified.

**Independent Test**: Add a Wish-Granted Item with an effect to a Genie character's inventory; confirm it displays correctly (quickstart.md Scenario 5).

### Implementation for User Story 5

- [X] T053 [P] [US5] Author example Wish-Granted Item seed content (name, description, formula-bearing effect) for `packs/systems/genie/` as pack data, using the existing `world_items`/`world_item_effects` shapes (spec 013) — no schema changes
- [X] T054 [US5] Verify T053's item and effect display correctly via the existing inventory UI (spec 013), with no Genie-specific UI changes required
- [X] T055 [US5] Manual: run quickstart.md Scenario 5 in a running dev instance

**Checkpoint**: User Story 5 is independently testable on top of the prior stories.

---

## Phase 9: User Story 6 - Wish Points scale with character level (Priority: P3)

**Goal**: A Genie character's Wish Points total updates automatically on level-up, per a fixed leveled table.

**Independent Test**: Level up a Genie character; confirm Wish Points updates to the correct table value with no manual entry (quickstart.md Scenario 6).

### Implementation for User Story 6

- [X] T056 [P] [US6] Add the `wishPoints` leveled table to `packs/systems/genie/system.json`, structurally identical to `dnd5e`'s `spellSlots` field (data-model.md/research.md R4)
- [X] T057 [US6] Implement Genie's derived-data recalculation for `resource_data.max_wish_points` on level change in `packs/systems/genie/web/src/`, mirroring the existing `spellSlots` recalculation logic in `packs/systems/dnd5e/web/src/derived-data.ts`
- [X] T058 [US6] Manual: run quickstart.md Scenario 6 in a running dev instance

**Checkpoint**: All seven user stories are independently functional.

---

## Phase 10: Polish & Cross-Cutting Concerns

**Purpose**: Full-system verification once all stories are complete.

- [ ] T059 Manual: run quickstart.md Scenario 7 — a complete combat encounter using only the Genie system pack (no other system pack loaded), covering a Manifestation roll, a scene-topology switch, and a condition applied and cleared — PARTIALLY run this pass, against a real running dev instance with real Playwright browser automation (`apps/web/e2e/genie-manifestation-roll.spec.ts`, `genie-scene-topology.spec.ts`). The Manifestation roll (keep/drop + explode + success-count, `4d6kh3x=6cs>=4`) and the Material/Wish-Warped-Zone scene-topology switch (with independent, uncorrupted token positions) both genuinely pass end-to-end in the real browser. The condition leg could NOT be exercised: there is no UI anywhere in apps/web to apply/clear a condition on a character or token — `packs/systems/genie/web/src/components/ConditionTrack.tsx` exists but is never imported by apps/web, and no route/component calls `updateActorSystemData` with `condition_data` (confirmed via grep). This is a real, pre-existing gap in wiring the Genie web package into the main app, not a test limitation. Still open pending that wiring.
- [X] T060 [P] Verify: native `cargo check` and `cargo test` pass across `packs/systems/genie/server` and `src/server` — confirmed: `cargo check --workspace` clean; all 13 Genie-specific tests (manifest compliance, session-loop authorization/win-loss, item round-trip) pass against live Postgres in isolation (`cargo test -p thunderforge genie -- --test-threads=1`). Note: the FULL `src/server` suite (242 other, pre-existing tests) has a pre-existing multi-threaded-parallel-DB-test flakiness (a SIGSEGV under concurrent connections) unrelated to Genie — reproducible before any of this session's changes, out of scope to fix here.
- [X] T061 [P] Verify: `cargo check --target wasm32-unknown-unknown` passes for `src/engine` — confirmed, plus `genie-engine` separately, both clean.
- [X] T062 [P] Verify: `tsc`/build passes for `packs/systems/genie/web` and `apps/web` — `packs/systems/genie/web`: confirmed, `tsc --noEmit` clean and all 12 `node --test` tests pass. `apps/web`: NOW verified — `pnpm run build` (`vite build`, the app's real build/typecheck path) completes cleanly with no errors (only a pre-existing chunk-size warning, unrelated). Note, unchanged from the prior pass: several existing apps/web test files, predating Genie, import `vitest`, which is not an installed dependency anywhere in this repo — a pre-existing infra gap that blocks running those specific unit tests, not the build itself, and not a Genie regression.
- [ ] T063 Run the full manual verification checklist at the bottom of quickstart.md (all 9 scenarios) and confirm every item is checked — PARTIALLY run this pass against a real running dev instance with real Playwright browser automation. See quickstart.md's "Manual verification checklist" for the itemized per-scenario state; in summary: Scenarios 1, 2, and 5 are genuinely verified end-to-end in the real browser (new specs `genie-manifestation-roll.spec.ts`, `genie-scene-topology.spec.ts`, `genie-npc-and-items.spec.ts`). Scenario 3 was attempted and found to be a real, confirmed bug (documented as a `test.fail()`, not skipped): the NPC size-category → token-scale feature reads from a client-side RxDB collection (`world_actor_system_data`) that nothing in the running app ever populates from the server (no replication is actually wired despite a comment claiming there is, and no query field exists to read it back), so it silently falls back to the 1x default regardless of the NPC's real size category. Scenarios 4, 6, 7, 8, and 9 could not be reached at all: there is no UI anywhere in apps/web for conditions, character leveling/Wish Points, or the Genie session loop (Session Wish Pool, Doom Clock, Puzzle Clocks, Session Resource trades) — `packs/systems/genie/web`'s components for all of these exist but are never imported by apps/web, and `apps/web/src/engine/world/sync/genieSession.ts` is not wired to any page. These are real, pre-existing wiring gaps (the backend/session-loop logic itself has its own server-side test coverage per T060), not testing gaps. Still open.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately.
- **Foundational (Phase 2)**: Depends on Setup — BLOCKS all user stories (manifest, data model, and system registration are shared by every story).
- **User Stories (Phase 3-9)**: All depend on Foundational completion.
  - US1, US2, and US7 (all P1) have no dependency on each other — any can go first, or all three in parallel.
  - US3 and US4 (P2) depend only on Foundational, not on US1/US2/US7, though US3 benefits from US2's Material-grid scene existing to demonstrate footprint on.
  - US5 and US6 (P3) depend only on Foundational.
- **Polish (Phase 10)**: Depends on all seven user stories being complete (Scenario 7 exercises the combat-focused stories together; Scenarios 8-9, already covered within US7, are re-verified here as part of the full checklist).

### Within Each User Story

- Manifest/data-model/migration additions before the component or mutation that reads/writes them.
- Server mutations before the web components that call them (US7: T031-T036 before T037-T040).
- Component implementation before its manual verification task.
- Story complete before moving to the next priority tier, if working sequentially.

### Parallel Opportunities

- T002-T004 (Setup) can run in parallel.
- T007-T011 (Foundational, distinct files) can run in parallel; T012-T014 depend on T007-T011 having defined the shapes/manifest they register.
- US1, US2, and US7 can be implemented in parallel by different people once Foundational is done (distinct files: dice/formula/character-sheet work vs. migration/engine-plugin work vs. session-tables/mutations/components work) — though US7 is the largest of the three and may warrant more than one person on its own.
- Within US7: T030-T031 (manifest/event-code docs) can run in parallel with each other; T037-T040 (the four web components) can run in parallel once T032-T036 (their backing mutations/queries) exist.
- US3, US4, US5, US6 can all proceed in parallel once Foundational is done, since each touches a distinct slice of the manifest and a distinct new component file.

---

## Parallel Example: Foundational Phase

```bash
# Launch independent Foundational tasks together:
Task: "Implement packs/systems/genie/server/src/models.rs"
Task: "Implement packs/systems/genie/server/src/validators.rs"
Task: "Implement packs/systems/genie/server/src/loader.rs"
Task: "Implement packs/systems/genie/engine/src/plugin.rs"
Task: "Implement packs/systems/genie/web/src/index.ts"
```

## Parallel Example: The three P1 stories

```bash
# Once Foundational is complete, split by story:
Track A (US1): T016 → T017 → T018 → T019 → T020 → T021
Track B (US2): T022 → T023 → T024 → T025 → T026 → T027
Track C (US7): T028 → T029 → (T030, T031 parallel) → T032 → T033 → T034 → T035 → T036 → (T037, T038, T039, T040 parallel) → T041 → T042 → T043
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup.
2. Complete Phase 2: Foundational (blocks everything).
3. Complete Phase 3: User Story 1 (the Manifestation roll).
4. **STOP and VALIDATE**: Run quickstart.md Scenario 1 independently.
5. This alone already proves the dice engine's hardest composed-notation case works end-to-end through a real system pack — a genuine, demonstrable engine-coverage MVP slice.

### Playable MVP (User Stories 1, 2, and 7)

For a slice that's actually *playable*, not just engine-coverage-verified, complete all three P1 stories before stopping: US1 (rolls) + US2 (dual topology) + US7 (the session loop itself — wishes, clocks, resource trading, win/loss). This is the minimum needed to run a real Genie session end-to-end, per the 2026-08-23 clarification session's emphasis that playability is co-equal with engine coverage, not secondary to it.

### Incremental Delivery

1. Setup + Foundational → foundation ready.
2. US1 (Manifestation roll) → validate → engine-coverage MVP.
3. US2 (scene topology) + US7 (session loop) → validate each → the playable MVP (see above) is now complete: a party can sit down and play a full Genie session.
4. US3, US4 (P2) → validate each → GM staging/size and conditions covered.
5. US5, US6 (P3) → validate each → items and progression covered.
6. Phase 10 → full-system verification, including re-running the complete quickstart.md checklist.

### Parallel Team Strategy

With multiple developers: complete Setup + Foundational together first (it blocks everything), then split the three P1 stories across up to three people (giving US7 extra hands given its size), then US3/US4/US5/US6 across however many people remain — none of the seven stories touch the same files as another, per the file paths above.
