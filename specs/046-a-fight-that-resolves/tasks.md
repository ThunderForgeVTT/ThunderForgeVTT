---
description: "Task list for spec 046, a fight that resolves"
---

# Tasks: A Fight That Resolves

**Input**: Design documents from `/specs/046-a-fight-that-resolves/`

**Prerequisites**: plan.md, spec.md (clarified 2026-09-14), research.md,
data-model.md, contracts/fight.md, quickstart.md

**Tests**: Required. FR-070 names the playtest and FR-071 names the e2e
coverage. Every phase ends with its e2e spec run through the harness, and the
playtest FINDINGs it clears turned into hard checks. Server rules C1–C10
(contracts/fight.md §2) each get a `cargo test`.

**Proof per phase**: `node scripts/e2e-parallel.mjs --shards=1 --only=<specs>`
with no other e2e or playtest running; search the log for `✘`, don't trust the
summary. Then `pnpm playtest --only=combat-5e`. `pnpm verify` does **not**
type-check the web app, so every web phase also runs
`pnpm -F @thunderforge/web exec tsc --noEmit`. Engine changes are linted with
`make lint-wasm` only.

**Phases follow plan.md, offset by two** (tasks Phase 3 is plan phase 1, and so on to tasks Phase 9 = plan phase 7; research, data-model and quickstart use plan numbering). Each is labelled with the user story it primarily
delivers. Phase 3 (turn order) serves US1's "a player on their turn". Phase 4
(links) is the hit-point record US2 writes to.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US6 from spec.md

---

## Phase 1: Setup

**Purpose**: a baseline and the working set.

- [ ] T001 Run `pnpm playtest --only=combat-5e` on `main` and keep its report as the before-state; list its FINDING lines (263, 380, 416, 439, 481, 522, 548, 606 on 2026-09-14) in a comment at the top of `specs/046-a-fight-that-resolves/tasks.md`'s Notes section
- [ ] T002 [P] Read `specs/046-a-fight-that-resolves/contracts/fight.md` and `research.md` into the working set; every task below is measured against contract clauses C1–C10, the redaction rule in §3, and manifest clauses M1–M5

---

## Phase 2: Foundational (blocks every story)

**Purpose**: the pack's `combat` block and the server's `combat` module, which every phase reads.

**⚠️ No story phase starts before T003–T008 are done.**

- [ ] T003 Add the optional typed `combat` block (`hitPoints`, `defence`, `sizes`, `legendary`) and `turnStructure.budget` to `SystemManifest` in `crates/pack_system_spec/src/lib.rs`, following the `vision` precedent; `validate_system_manifest` enforces M2 (named slot and field exist in `data_types`), M3 (footprint ≥ 0.5, unique ids) and M4 (budget movement speed names a `movement` key)
- [ ] T004 [P] Tests for T003 in `crates/pack_system_spec/src/` (the crate's existing test file): each of M2, M3 and M4 refused with a named field; a manifest with no `combat` block accepted (M1)
- [ ] T005 Create module `src/server/src/combat/mod.rs` with `manifest.rs`, which reads a world's system `combat` block from `system.json` under `systems_dir` (the way `vision_profiles.rs` and `attributes.rs::movement_declarations_for_system` read theirs), cached per system id; register the module in `src/server/src/lib.rs`
- [ ] T006 [P] Add `"combat": {"hitPoints": {"slot": "resourceData", "current": "current_hp", "max": "max_hp", "temporary": "temporary_hp"}}` to `packs/systems/dnd5e/system.json`; the rest of the block lands with its phase
- [ ] T007 [P] Document the `combat` block and `turnStructure.budget` in `packs/systems/README.md`: shape, M1–M5, and that shared code never names a system's fields; keep `pnpm verify`'s pack-contracts link check green
- [ ] T008 `cargo test -p thunderforge-server combat::manifest` and `pnpm verify`; record results in this file

**Checkpoint**: a pack can declare combat; the server can read it.

---

## Phase 3: Damage lands, bars move, zero is out (Priority: P1) — US2, US3

**Goal**: a Game Master damages or heals any creature from the tracker; every board's bars move within a second; a creature at zero leaves the turn order and returns when healed.

**Independent test**: the goblin has 7; the GM applies 5; both players' bars read 2 within a second, no reload. 2 more: marked out, skipped. Heal 3: back in. Manual Down then heal: stays down.

- [ ] T009 [US3] Migration `src/server/migrations/<date>-046-combatant-downed-by/` (up/down): `world_combatants.downed_by TEXT NULL CHECK (downed_by IN ('hit_points','game_master'))`; regenerate `src/server/src/schema.rs`
- [ ] T010 [US2] Implement `apply_hit_point_change` in `src/server/src/combat/hit_points.rs` per research R5: lock the row it writes (`SELECT … FOR UPDATE` on `world_actor_system_data` for now; phase 4 adds copies), spend temporary first, bound at 0 and max, validate with the pack validator, write, record event 26 with the existing payload shape
- [ ] T011 [US3] In `hit_points.rs`, after a write that crosses zero, update the creature's combatant in the running combat (match `token_id`, else `actor_id`): to 0 → `active = false, downed_by = 'hit_points'`; above 0 → `active = true, downed_by = NULL` only when `downed_by = 'hit_points'`; record event 18
- [ ] T012 [US2] GraphQL mutation `changeHitPoints(tokenId, kind, amount)` → `TokenHitPoints`, Game Master only (`is_dm_of_world`), `refuse_if_paused`, in `src/server/src/graphql/mutations_combat_hit_points.rs`; register in `src/server/src/graphql.rs`
- [ ] T013 [US3] Make the tracker's Down/Up write `downed_by = 'game_master'` / `NULL` in `update_combatant` in `src/server/src/graphql/mutations_combat.rs`; expose `downedBy` on `GraphQLCombatant`
- [ ] T014 [P] [US2] Server tests in `src/server/src/combat/hit_points_tests.rs`: temporary absorbs first (C7); current never below 0; healing never above max; two concurrent changes both land; a pack without `combat.hitPoints` refuses with a clear message (M1)
- [ ] T015 [P] [US3] Server tests in `src/server/src/combat/hit_points_tests.rs` for C8: to zero marks out; heal reactivates only `hit_points`; a GM Down is not undone by healing
- [ ] T016 [P] [US2] Re-read token status on event 26 as well as 14 and 19 in `apps/web/src/engine/world/sync/tokenStatus.ts` (research, correction 1)
- [ ] T017 [US2] Add `changeHitPoints` to `apps/web/src/api/combat.ts`, and Damage / Heal controls on each combatant row, for Game Masters only, in `apps/web/src/components/world/PlayDock/CombatPanel.tsx` (number input + two buttons, keyboard operable, labelled)
- [ ] T018 [US3] Show a combatant marked out by hit points distinctly from a GM Down in `CombatPanel.tsx`, with an accessible label ("Out: 0 hit points" / "Down")
- [ ] T019 [US2] Regenerate `src/app/schema.graphql` (`node scripts/check-graphql-contract.mjs --schema --fix`); `pnpm verify` green including `graphql-ops`
- [ ] T020 [US2] e2e `apps/web/e2e/combat-hit-points.spec.ts` covering the independent test above across a GM and two player pages
- [ ] T021 [US2] In `apps/web/playtest/combat-5e.playtest.ts`, turn FINDINGs 522 (bar doesn't move, and correct its stale "emits no event" wording) and 548 (zero doesn't mark down) into hard checks using `changeHitPoints`
- [ ] T022 [US2] Prove: T020 through the harness (`✘` grep), the playtest, `tsc --noEmit`; record results here

**Checkpoint**: a fight can end.

---

## Phase 4: Turn order holds (Priority: P1) — US1

**Goal**: a player cannot move or attack on someone else's turn; the reason names whose turn it is; Game Masters and non-combatants are not held.

**Independent test**: on the ogre's turn, a player's move is refused with "It is Ogre's turn" on every path, and the token doesn't move on any board; hidden name reads "Unknown".

- [ ] T023 [US1] Implement `turn_check(conn, scene_id, acting_token_id, user) -> TurnCheck` in `src/server/src/combat/turn.rs` per research R12 and C1: running combat in the scene, acting token is a combatant, not active, user not GM → refused with the active combatant's label under `load_combat`'s "Unknown" rule
- [ ] T024 [US1] Call `turn_check` in `move_own_token` in `src/server/src/graphql/mutations_tokens.rs` before `judge_against_walls`
- [ ] T025 [US1] Call `turn_check` for queued token moves in `src/server/src/graphql/mutations_reconcile.rs` (token write ~688-712), reporting a refusal the way a queued conflict is reported today
- [ ] T026 [P] [US1] Server tests in `src/server/src/combat/turn_tests.rs`: refused off-turn; allowed on-turn; GM never refused; non-combatant token free; hidden active name → "Unknown"; offline replay refused
- [ ] T027 [US1] Show the refusal text where a refused move is shown today in `apps/web/src/engine/world/sync/tokens.ts` (the move snaps back, and the message names whose turn it is)
- [ ] T028 [US1] e2e `apps/web/e2e/combat-turn-order.spec.ts` covering the independent test, including the GM moving the ogre freely
- [ ] T029 [US1] Turn FINDING 606 (a player moved out of turn) in `apps/web/playtest/combat-5e.playtest.ts` into a hard check
- [ ] T030 [US1] Prove: T028 through the harness, `cargo test` for T026, the playtest; record results here

**Checkpoint**: a round has an order that means something.

---

## Phase 5: A token is its actor, or a copy of it (Priority: P1) — US2

**Goal**: linked tokens share their actor's hit points; unlinked copies hold their own; unique NPCs place linked; `tokens.health` is gone; two hundred copies need no actors.

**Independent test**: Aria (linked) and two goblins (copies of one NPC). Damage goblin A: B and the NPC unchanged. Damage Aria: her sheet changes. "Boblin" marked unique places linked. Relink A: it takes the NPC's hit points.

- [ ] T031 [US2] Write `docs/adrs/<date>-102-a_token_is_its_actor_or_a_copy.md` (PROPOSED) from research R4 and data-model.md; add it to `docs/adrs/README.md`
- [ ] T032 [US2] Migration `src/server/migrations/<date>-046-token-links/` per data-model.md: add `tokens.linked`, `tokens.system_data`, the CHECK; backfill `linked`; null dangling `actor_id`s and add the FK `ON DELETE SET NULL`; add `world_actors.is_unique`; `down.sql` reverses what it can
- [ ] T033 [US2] Second migration `src/server/migrations/<date>-046-retire-token-health/`: copy non-null `health`/`max_health` into `system_data` of unlinked tokens through the pack's declared hit-point fields (resolved per world's system in SQL, or in a one-off Rust step in the migration runner if SQL cannot reach the manifest), then drop both columns; `down.sql` restores them from `system_data`
- [ ] T034 [US2] Placement defaults in `create_token` in `src/server/src/graphql/mutations_tokens.rs` per data-model.md: resolve `linked`, seed `system_data` for copies, derive `token_type` from `world_actors.actor_type` when the caller gives none; add `linked: Boolean` to `GraphQLCreateTokenInput` in `src/server/src/graphql/input_types.rs`
- [ ] T035 [US2] `setTokenLink(tokenId, linked)` and `setActorUnique(actorId, unique)`, Game Master only, in `src/server/src/graphql/mutations_token_links.rs`; relink rules per data-model.md; record event 14 / 26
- [ ] T036 [US2] Extend `apply_hit_point_change` in `src/server/src/combat/hit_points.rs` to write an unlinked copy's `tokens.system_data` (lock the token row; event 14) and a linked token's actor data (as phase 3)
- [ ] T037 [US2] Read a copy's `system_data` in `src/server/src/graphql/queries/token_status.rs`, and stop skipping actorless tokens that carry `system_data`
- [ ] T038 [US2] Remove `health`/`max_health` from `models::Token`, `GraphQLToken` (`types_scene.rs`), `input_types.rs`, `mutations_reconcile.rs`; remove the engine's `health` fields and the unread `DerivedStats.is_dead` in `src/engine/src/components.rs`, `src/engine/src/app.rs`, `src/engine/src/derived_data.rs`; delete the orphan `src/server/src/token_systems.rs`
- [ ] T039 [US2] Remove the legacy health bar and the `health`/`maxHealth` create fields from `apps/web/src/components/TokenPanel.tsx`, and `health` from `apps/web/src/engine/world/sync/tokens.ts` and the engine payload types in `apps/web/src/engine/world/types.ts`
- [ ] T040 [US2] Set a character token's owner from the claim when one exists in `bring_party_to_scene` in `src/server/src/graphql/mutations_party.rs` (research R7 known gap)
- [ ] T041 [P] [US2] Server tests in `src/server/src/graphql/mutations_token_links_tests.rs`: placement defaults for character, NPC, unique NPC, actorless; copy damage isolated; relink discards `system_data`; unlink seeds it; non-GM refused
- [ ] T042 [US2] "Linked / Copy" control on a selected token and "Unique" toggle on the NPC editor, Game Master only, in `apps/web/src/components/TokenPanel.tsx` and the NPC edit page under `apps/web/src/pages/world/`
- [ ] T043 [US2] Regenerate `src/app/schema.graphql`; `pnpm verify`; `tsc --noEmit`; `make lint-wasm`
- [ ] T044 [US2] e2e `apps/web/e2e/token-links.spec.ts` covering the independent test
- [ ] T045 [US2] Add a 200-copy level to `apps/web/e2e/engine-status-limits.spec.ts` that places copies of one NPC through `createToken`; assert no new actors, measure load-to-drawn time and fps against research R17's targets (SC-008)
- [ ] T046 [US2] Add a hard check to `apps/web/playtest/combat-5e.playtest.ts`: two goblin copies take damage separately
- [ ] T047 [US2] Prove: T044, T045 and the token/combat/status e2e specs through the harness; the playtest; record results and the measured numbers here

**Checkpoint**: one record of hit points per creature.

---

## Phase 6: An attack is aimed at something (Priority: P1) — US1, US2

**Goal**: an attack chooses a target, rolls against its defence, is shown to every seat within a second with the attacker redacted per viewer, and a hit's damage is offered to the target's controller (or auto-applied to the GM's NPCs).

**Independent test**: see quickstart.md phase 4 — attack shown to all seats; offer taken by the GM; offer to Aria's absent player resolved on her behalf; auto-apply for an encounter; a hidden attacker reads "Unknown" with no id or name in the player's traffic.

- [ ] T048 [US1] Write `docs/adrs/<date>-101-an_attack_is_resolved_on_the_server.md` (PROPOSED) from research R1, R7, R8, R15; add to `docs/adrs/README.md`
- [ ] T049 [US1] Migration `src/server/migrations/<date>-046-attacks-and-offers/`: `world_attacks`, `world_offers` (with the pending index), `worlds.auto_apply_npc_damage`, `world_combats.auto_apply`, and the attack fields on `world_abilities` and `world_items` (`reach`, `range_normal`, `range_long`, `needs_line_of_sight`, `action_cost`, `legendary_cost`, `multiattack`) per data-model.md
- [ ] T050 [P] [US1] Add `defence` to 5e: `armor_class` (integer, min 0) in a data type of `packs/systems/dnd5e/system.json` and its `server/src/validators.rs`, and `combat.defence` naming it
- [ ] T051 [US1] Add `EVENT_CODE_ATTACK_MADE = 29` and `EVENT_CODE_OFFER_CHANGED = 30` to `src/server/src/world_events.rs` with their doc comments; payloads ids only (contract §4)
- [ ] T052 [US1] Controllers per research R7 in `src/server/src/combat/controllers.rs`: `controllers_of(token)` and `may_act_for(user, token)`, reusing the check in `move_own_token` rather than copying it
- [ ] T053 [US1] Redaction per contract §3 in `src/server/src/combat/redaction.rs`: `party_for_viewer(viewer, token, scene)` using `vision::visibility_of` from each of the viewer's controlled tokens, `wall_set_from_rows`, the scene's lights and ambient light, and `vision_profiles.rs`; `name_visible_to_players`; Game Masters see all
- [ ] T054 [US1] Resolution in `src/server/src/combat/attack.rs`: `make_attack` in contract order — C2 control, C1 turn (`turn.rs`, REACTION exempt), resolve the ability's `ATTACK_ROLL` formula through `roll_dice_impl`, read defence, outcome, damage roll on hit, write `world_attacks`, then C4/C5 offer or auto-apply (research R15) through `apply_hit_point_change` in one transaction; multiattack makes one row per part with `multiattack_of`; record events 29 and 30
- [ ] T055 [US2] Offers in `src/server/src/combat/offers.rs`: `resolve_offer(offer, take, user)` — C6 once only, controllers or GM, `resolved_on_behalf`, take → `apply_hit_point_change` in the same transaction; `pending_offers(world, user)`
- [ ] T056 [US1] GraphQL in `src/server/src/graphql/mutations_attacks.rs` and `src/server/src/graphql/queries/attacks.rs`: `makeAttack`, `previewAttack` (flags and turn check, no writes), `attack(id)`, `sceneAttacks`, `resolveOffer`, `pendingOffers`, `updateWorldAutoApplyNpcDamage`, `setCombatAutoApply`; types per contract §1; every answer built per viewer through `redaction.rs`; `refuse_if_paused` on every mutation (C10); register in `graphql.rs`; expose the new ability and item fields on their types and inputs
- [ ] T057 [US1] Offline attack intents in `src/server/src/graphql/mutations_reconcile.rs` (research R16): a queued attack resolves through `make_attack` at replay; a turn refusal spends nothing and is reported as a refused queued action
- [ ] T058 [P] [US1] Server tests `src/server/src/combat/attack_tests.rs`: C1 (off-turn refused, reaction allowed), C2, C4 (miss and no target create no offer), C5 (pending; auto-apply only to GM-run NPCs; player-controlled always pending), multiattack rows, pause refusal (C10)
- [ ] T059 [P] [US2] Server tests `src/server/src/combat/offers_tests.rs`: C6 once only, by a controller, by a GM on behalf, by a stranger refused; taken offer changes hit points atomically
- [ ] T060 [P] [US1] Server tests `src/server/src/combat/redaction_tests.rs`: hidden name → Unknown; unseen through a wall → Unknown; seen → named; target redaction nulls defence; attacker redaction nulls ability name; no redacted id in any field; GM unredacted
- [ ] T061 [US1] Ability and item editors gain reach, normal/long range, "needs line of sight", action cost, legendary cost and multiattack in `apps/web/src/pages/world/ability/` and `apps/web/src/pages/world/item/` (effect editors' neighbouring form)
- [ ] T062 [US1] `apps/web/src/api/attacks.ts`: `makeAttack`, `previewAttack`, `attack`, `sceneAttacks`, `resolveOffer`, `pendingOffers`, auto-apply setters
- [ ] T063 [US1] Attack flow in `apps/web/src/components/world/PlayDock/AttackFlow/`: from an attack on the in-pane sheet (`InPaneCharacterSheet.tsx`, replacing the bare `rollDice` for `ATTACK_ROLL` abilities), choose a target on the board or from a list, show `previewAttack`'s warning, confirm, roll; FR-033's warning before rolling
- [ ] T064 [US1] Attack log for the table in `apps/web/src/components/world/PlayDock/AttackLog/`: subscribe to event 29, read `attack(id)`, show attacker, target, total vs defence, outcome and flags; "Unknown" as served; wire into the dice-roll animation where `handleRoll` does today
- [ ] T065 [US2] Offer prompt in `apps/web/src/components/world/PlayDock/OfferPrompt/`: on event 30 and on load (`pendingOffers`), show "Take 5 damage?" with Take / Decline; a GM sees every pending offer with "resolve on behalf"; the table sees who resolved it
- [ ] T066 [US1] Auto-apply toggles: world default in the world settings page under `apps/web/src/pages/world/` (GM only), and the per-encounter override in `CombatPanel.tsx`
- [ ] T067 [US1] Regenerate `src/app/schema.graphql`; `pnpm verify`; `tsc --noEmit`
- [ ] T068 [US1] e2e `apps/web/e2e/combat-attack.spec.ts` per quickstart.md phase 4, including capturing the player page's network responses and asserting the hidden ogre's token id and name are absent
- [ ] T069 [US1] Turn FINDINGs 263 (roll not shown), 416 (no AC or target) and 439 (longsword not rollable) in `apps/web/playtest/combat-5e.playtest.ts` into hard checks, using `apps/web/playtest/combat.ts` helpers for target selection and offers
- [ ] T070 [US1] Prove: T068 through the harness; `cargo test` for T058–T060; the playtest; accept ADR-101 and ADR-102 once proven; record results here

**Checkpoint**: an attack does something, and the table sees it — MVP complete.

---

## Phase 7: Size fills squares, and attacks have reach (Priority: P1) — US4

**Goal**: a creature's size decides the squares it fills for drawing, snapping, hit-testing and movement; reach, range and line of sight are measured from footprint to footprint and flagged, never refused.

**Independent test**: a Large ogre fills 2×2 on every board; a hero one square away swings unflagged; four squares away is warned, still swings, flagged "out of reach"; shortbow beyond normal flagged long range; through a closed door flagged no line of sight and not auto-applied.

- [ ] T071 [US4] `footprint_distance` in `crates/thunderforge-canvas-core/src/grid.rs` per research R11 (square: min Chebyshev between covered rectangles; hex: centre axial; gridless: Euclidean ÷ cell) and `footprint_line_of_sight` in `crates/thunderforge-canvas-core/src/wall.rs` (any pair of covered cell centres visible)
- [ ] T072 [P] [US4] Tests in `crates/thunderforge-canvas-core/src/grid.rs` tests and `wall_tests.rs`: 1×1 adjacent = 1; 2×2 adjacent from any of its squares; 4×4; hex centre; gridless; a 2×2 peering past a corner sees
- [ ] T073 [P] [US4] 5e sizes: `size` in `trait_data` of `packs/systems/dnd5e/system.json` and its validator; `combat.sizes` with Tiny 0.5, Small 1, Medium 1, Large 2, Huge 3, Gargantuan 4
- [ ] T074 [P] [US4] Genie: move `sizeCategories` under `combat.sizes` in `packs/systems/genie/system.json`; make `packs/systems/genie/server/src/validators.rs` read the manifest instead of its hard-coded list; update `apps/web/src/utils/sizeCategory.ts` and `packs/systems/genie/web/src/lib/sizeCategory.ts` to read the new location
- [ ] T075 [US4] Query `tokenGrid(sceneId)` in `src/server/src/graphql/queries/token_grid.rs` per contract §1 and research R10 (linked → actor's size; copy → its NPC's; none → omitted); one query over `tokens` joined to `world_actors`
- [ ] T076 [US4] `sync/tokenGrid.ts` in `apps/web/src/engine/world/sync/`, shaped like `tokenVision.ts`: dispatch `SetTokenGridCommand` for every token (explicit 1 for omitted), re-read on events 14 and 26; add `SetTokenGridCommand` to `apps/web/src/engine/world/types.ts`; export from `sync/index.ts`
- [ ] T077 [US4] Make `TokenPanel.tsx` stop passing a size-derived `scale` to `createToken` (size now comes from `tokenGrid`); keep `scale` as an art multiplier
- [ ] T078 [US4] In `src/server/src/combat/attack.rs`, measure `distance` with `footprint_distance` × `units_per_cell` and set `OUT_OF_REACH`, `LONG_RANGE`, `BEYOND_RANGE`, `NO_REACH_DECLARED` and `NO_LINE_OF_SIGHT` (skipped when `needs_line_of_sight = false`); exclude no-line-of-sight attacks from auto-apply (FR-007); same in `previewAttack`
- [ ] T079 [P] [US4] Server tests in `src/server/src/combat/attack_tests.rs`: each flag; no flag refuses (C3); a Large creature's reach is its attack's, not 10 ft; auto-apply skipped without line of sight unless the ability ignores it
- [ ] T080 [US4] Show flags in `AttackLog` and the pre-roll warning in `AttackFlow` ("Out of reach: 20 ft, reach 5 ft")
- [ ] T081 [US4] Replace the playtest ogre's `scale = 2` with `size: large` in `apps/web/playtest/combat-5e.playtest.ts` and `apps/web/playtest/table.ts`; turn FINDING 481 (no size or reach) into a hard check
- [ ] T082 [US4] Regenerate `src/app/schema.graphql`; `pnpm verify`; `tsc --noEmit`; `make lint-wasm`
- [ ] T083 [US4] e2e `apps/web/e2e/combat-reach.spec.ts` per the independent test, including snapping, hit-testing and a keyboard move of the 2×2 ogre
- [ ] T084 [US4] Prove: T083 plus the canvas, token and lighting specs through the harness; `cargo test -p thunderforge_canvas_core`; the playtest; record results here

**Checkpoint**: a board is a board.

---

## Phase 8: A round is an economy (Priority: P2) — US5

**Goal**: every seat sees each creature's action, bonus action, reaction and movement; spending is recorded and shown, never refused; it resets at the owner's turn.

**Independent test**: Aria's four lines unspent on every seat; she attacks → action spent; attacks again → overspent, not refused; moves 20 ft → 10 ft left; turn passes and returns → fresh.

- [ ] T085 [US5] Migration `src/server/migrations/<date>-046-combatant-budgets/`: `world_combatant_budgets` per data-model.md; create a row for every existing combatant; `down.sql` drops it
- [ ] T086 [P] [US5] 5e `turnStructure.budget` in `packs/systems/dnd5e/system.json` (action 1, bonus action 1, reaction 1, movement from `walk`)
- [ ] T087 [US5] `src/server/src/combat/budget.rs`: create a budget with each combatant; `spend(combatant, cost)`; reset on `advance_turn` in `mutations_combat.rs` per data-model.md state transitions; `TurnBudget` resolved against the pack's declared allowances and the creature's speed
- [ ] T088 [US5] Spend by `action_cost` in `make_attack` (a multiattack spends one action) and set `OVERSPENT` when a line goes negative; spend movement in `move_own_token` with `movement_budget::cost_path` from `thunderforge-canvas-core` when the token is a combatant in a running combat
- [ ] T089 [P] [US5] Server tests `src/server/src/combat/budget_tests.rs`: C9 overspend recorded not refused; reset on own turn only; reaction returns at own turn start; movement counted against speed; multiattack one action
- [ ] T090 [US5] Budget per combatant in `CombatPanel.tsx` (action, bonus, reaction, movement remaining; debt shown as negative), visible to every seat; `budget` added to the combat query in `apps/web/src/api/combat.ts`
- [ ] T091 [US5] Regenerate schema; `pnpm verify`; `tsc --noEmit`
- [ ] T092 [US5] e2e `apps/web/e2e/combat-economy.spec.ts` per the independent test
- [ ] T093 [US5] Turn FINDING 380 (no turn economy) in `apps/web/playtest/combat-5e.playtest.ts` into a hard check
- [ ] T094 [US5] Prove: T092 through the harness; `cargo test` for T089; the playtest; record results here

**Checkpoint**: the tracker knows what a turn is.

---

## Phase 9: A legendary creature acts between turns (Priority: P3) — US6

**Goal**: legendary actions show how many remain, spend between other creatures' turns, refill at the owner's; a lair sits at initiative 20.

**Independent test**: three legendary actions spent across three players' turns (2, 1, 0), refilled to 3 at its own turn; a GM-added lair at 20, losing ties.

- [ ] T095 [US6] Migration `src/server/migrations/<date>-046-lair-combatants/`: `world_combatants.kind` per data-model.md
- [ ] T096 [P] [US6] 5e `legendary_actions` (integer, min 0) in `trait_data` of `packs/systems/dnd5e/system.json` and its validator; `combat.legendary` naming it
- [ ] T097 [US6] In `budget.rs`, read `legendary_per_round` when a combatant is added and refill `legendary_remaining` at the start of its turn; in `make_attack`, spend `legendary_cost` for `action_cost = legendary` and flag `LEGENDARY_ON_OWN_TURN` when made on its own turn
- [ ] T098 [US6] `addLairCombatant(combatId, label)` in `src/server/src/graphql/mutations_combat.rs`: `kind = 'lair'`, no token or actor, initiative 20, tiebreak −1; lair actions made through `makeAttack` with no attacker token by a Game Master
- [ ] T099 [P] [US6] Server tests `src/server/src/combat/legendary_tests.rs`: spend and refill; own-turn flag; lair ordering and tie loss; lair action by GM only
- [ ] T100 [US6] Legendary pips and "Spend legendary action" (GM only) on a combatant row, and "Add lair" in `CombatPanel.tsx`
- [ ] T101 [US6] Regenerate schema; `pnpm verify`; `tsc --noEmit`
- [ ] T102 [US6] e2e `apps/web/e2e/combat-legendary.spec.ts` per the independent test
- [ ] T103 [US6] Add a hard check to `apps/web/playtest/combat-5e.playtest.ts`: a legendary creature spends three legendary actions across turns and has three again at its own (SC-006)
- [ ] T104 [US6] Prove: T102 through the harness; `cargo test` for T099; the playtest; record results here

**Checkpoint**: every story in the spec is real.

---

## Phase 10: Polish & cross-cutting

- [ ] T105 [P] Update `specs/046-a-fight-that-resolves/spec.md`'s status and Context to what shipped, phase by phase, correcting the three claims research found stale
- [ ] T106 [P] Confirm every FINDING in `apps/web/playtest/combat-5e.playtest.ts` is now a hard check or explicitly deferred with a reason in the file
- [ ] T107 [P] Accessibility pass on AttackFlow, AttackLog, OfferPrompt and CombatPanel additions: keyboard only, focus return on dialogs, accessible names for every control and pip
- [ ] T108 Confirm FR-071's e2e list is covered: reach flagged, out-of-turn refused, "Unknown" with no leak, damage reaching another client's bars within a second, dropping at zero, an action spent and refilled, a legendary action between turns — naming the spec file for each
- [ ] T109 Full run: `node scripts/e2e-parallel.mjs --shards=2` with nothing else running; search the log for `✘`; record time and result beside the 2026-09-14 run
- [ ] T110 `pnpm verify`, `pnpm -F @thunderforge/web exec tsc --noEmit` and `make lint-wasm` before the final commit of the feature

---

## Dependencies

```text
Phase 1 (setup)
  └─ Phase 2 (combat block, combat module) ─── blocks everything below
       ├─ Phase 3 (hit points, zero)       ── ships first
       ├─ Phase 4 (turn order)             ── independent of 3
       ├─ Phase 5 (links)                  ── needs 3 (the damage operation)
       ├─ Phase 6 (attacks, offers)        ── needs 3, 4, 5  ← MVP complete
       ├─ Phase 7 (size, reach)            ── needs 6 (flags live on the attack)
       ├─ Phase 8 (economy)                ── needs 4 and 6
       └─ Phase 9 (legendary, lair)        ── needs 8
Phase 10 (polish) ── after every phase that ships
```

Within a phase: migration → server module → GraphQL → tests → web → e2e → playtest → proof.

## Parallel opportunities

- **Phase 2**: T004, T006, T007 alongside T003/T005.
- **Phase 3**: T014, T015, T016 are separate files, alongside T010–T013.
- **Phase 4**: T026 alongside T024/T025.
- **Phase 5**: T041 alongside T036–T040.
- **Phase 6**: T050 alongside the migration. T058, T059 and T060 are separate test files. T061, T062 and the web components (T063–T066) split across files once T056's schema is regenerated.
- **Phase 7**: T072, T073 and T074 alongside T071.
- **Phase 4 and Phase 3** can run side by side: turn order touches `move_own_token` and `reconcile`, hit points touch `combat/hit_points.rs` and `CombatPanel.tsx`.
- **Never in parallel**: two e2e or playtest runs, or `cargo test` beside either (shared database and global settings rows).

## Implementation strategy

**First shippable slice: Phase 3.** The Game Master's damage button, bars that
move and a creature that drops at zero. It is small, clears two FINDINGs, and
makes a fight end even before anyone can attack.

**MVP: Phases 3–6.** A player attacks a creature, the table sees it, the target's
controller takes the hit, bars move, and a creature at zero is out. Turn order
holds, and hit points have one record. That is spec US1–US3 complete, and SC-001,
SC-002, SC-003, SC-007 and SC-008.

**Then Phase 7** (US4: size and reach, SC-004), **Phase 8** (US5: economy,
SC-005) and **Phase 9** (US6: legendary, SC-006), each shippable alone.

## Notes

- Commits are signed, stage explicit paths, and each names the phase and task ids.
- `.e2e-shards-durations.json` is rewritten by the harness; don't commit it with feature work.
- Migrations get real timestamps when written; `<date>` above is a placeholder for that, not a directory name.
