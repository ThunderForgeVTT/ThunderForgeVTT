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
delivers. Phase 4 (turn order) serves US1's "a player on their turn". Phase 5
(links) is the hit-point record US2 writes to.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: US1–US6 from spec.md

---

## Phase 1: Setup

**Purpose**: a baseline and the working set.

- [X] T001 Run `pnpm playtest --only=combat-5e` on `main` and keep its report as the before-state; list its FINDING lines (263, 380, 416, 439, 481, 522, 548, 606 on 2026-09-14) in a comment at the top of `specs/046-a-fight-that-resolves/tasks.md`'s Notes section
- [X] T002 [P] Read `specs/046-a-fight-that-resolves/contracts/fight.md` and `research.md` into the working set; every task below is measured against contract clauses C1–C10, the redaction rule in §3, and manifest clauses M1–M5

---

## Phase 2: Foundational (blocks every story)

**Purpose**: the pack's `combat` block and the server's `combat` module, which every phase reads.

**⚠️ No story phase starts before T003–T008 are done.**

- [X] T003 Add the optional typed `combat` block (`hitPoints`, `defence`, `sizes`, `legendary`) and `turnStructure.budget` to `SystemManifest` in `crates/pack_system_spec/src/lib.rs`, following the `vision` precedent; `validate_system_manifest` enforces M2 (named slot and field exist in `data_types`), M3 (footprint ≥ 0.5, unique ids) and M4 (budget movement speed names a `movement` key)
- [X] T004 [P] Tests for T003 in `crates/pack_system_spec/src/` (the crate's existing test file): each of M2, M3 and M4 refused with a named field; a manifest with no `combat` block accepted (M1). *Note (implement): the crate has no single test file; these live in a new sibling `combat_tests.rs`, beside `combat.rs`, since `lib.rs` sits near the 1000-line check. `turnStructure` and `data_types` are read from the untyped manifest, because bundled packs do not carry the typed schema's other fields.*
- [X] T005 Create module `src/server/src/combat/mod.rs` with `manifest.rs`, which reads a world's system `combat` block from `system.json` under `systems_dir` (the way `vision_profiles.rs` and `attributes.rs::movement_declarations_for_system` read theirs), cached per system id; register the module in `src/server/src/lib.rs`
- [X] T006 [P] Add `"combat": {"hitPoints": {"slot": "resourceData", "current": "current_hp", "max": "max_hp", "temporary": "temporary_hp"}}` to `packs/systems/dnd5e/system.json`; the rest of the block lands with its phase
- [X] T007 [P] Document the `combat` block and `turnStructure.budget` in `packs/systems/README.md`: shape, M1–M5, and that shared code never names a system's fields; keep `pnpm verify`'s pack-contracts link check green
- [X] T008 `cargo test -p thunderforge-server combat::manifest` and `pnpm verify`; record results in this file. *Result 2026-09-14: `cargo test -p pack_system_spec` 82 passed (10 new in `combat::tests`); `combat::manifest` 6 passed; `cargo clippy` clean on both crates; verify's rust-fmt, web-fmt, registry, seam, graphql-ops, filelength and `check-pack-docs` green.*

**Checkpoint**: a pack can declare combat; the server can read it.

---

## Phase 3: Damage lands, bars move, zero is out (Priority: P1) — US2, US3

**Goal**: a Game Master damages or heals any creature from the tracker; every board's bars move within a second; a creature at zero leaves the turn order and returns when healed.

**Independent test**: the goblin has 7; the GM applies 5; both players' bars read 2 within a second, no reload. 2 more: marked out, skipped. Heal 3: back in. Manual Down then heal: stays down.

- [X] T009 [US3] Migration `src/server/migrations/<date>-046-combatant-downed-by/` (up/down): `world_combatants.downed_by TEXT NULL CHECK (downed_by IN ('hit_points','game_master'))`; regenerate `src/server/src/schema.rs`
- [X] T010 [US2] Implement `apply_hit_point_change` in `src/server/src/combat/hit_points.rs` per research R5: lock the row it writes (`SELECT … FOR UPDATE` on `world_actor_system_data` for now; Phase 5 adds copies), spend temporary first, bound at 0 and max, validate with the pack validator, write, record event 26 with the existing payload shape
- [X] T011 [US3] In `hit_points.rs`, after a write that crosses zero, update the creature's combatant in the running combat (match `token_id`, else `actor_id`): to 0 → `active = false, downed_by = 'hit_points'`; above 0 → `active = true, downed_by = NULL` only when `downed_by = 'hit_points'`; record event 18
- [X] T012 [US2] GraphQL mutation `changeHitPoints(tokenId, kind, amount)` → `TokenHitPoints`, Game Master only (`is_dm_of_world`), `refuse_if_paused`, in `src/server/src/graphql/mutations_combat_hit_points.rs`; register in `src/server/src/graphql.rs`
- [X] T013 [US3] Make the tracker's Down/Up write `downed_by = 'game_master'` / `NULL` in `update_combatant` in `src/server/src/graphql/mutations_combat.rs`; expose `downedBy` on `GraphQLCombatant`
- [X] T014 [P] [US2] Server tests in `src/server/src/combat/hit_points_tests.rs`: temporary absorbs first (C7); current never below 0; healing never above max; two concurrent changes both land; a pack without `combat.hitPoints` refuses with a clear message (M1)
- [X] T015 [P] [US3] Server tests in `src/server/src/combat/hit_points_tests.rs` for C8: to zero marks out; heal reactivates only `hit_points`; a GM Down is not undone by healing
- [X] T016 [P] [US2] Re-read token status on event 26 as well as 14 and 19 in `apps/web/src/engine/world/sync/tokenStatus.ts` (research, correction 1)
- [X] T017 [US2] Add `changeHitPoints` to `apps/web/src/api/combat.ts`, and Damage / Heal controls on each combatant row, for Game Masters only, in `apps/web/src/components/world/PlayDock/CombatPanel.tsx` (number input + two buttons, keyboard operable, labelled)
- [X] T018 [US3] Show a combatant marked out by hit points distinctly from a GM Down in `CombatPanel.tsx`, with an accessible label ("Out: 0 hit points" / "Down")
- [X] T019 [US2] Regenerate `src/app/schema.graphql` (`node scripts/check-graphql-contract.mjs --schema --fix`); `pnpm verify` green including `graphql-ops`. *Result 2026-09-14: `pnpm verify` 13 of 14 green on first run (graphql schema and operations included); web lint caught the two new e2e specs importing `test` from `@playwright/test` instead of `e2e/fixtures/test` (a rule that landed on main during this phase), fixed, and both specs re-run through the harness: 2 passed, no `✘`.*
- [X] T020 [US2] e2e `apps/web/e2e/combat-hit-points.spec.ts` covering the independent test above across a GM and two player pages
- [X] T021 [US2] In `apps/web/playtest/combat-5e.playtest.ts`, turn FINDINGs 522 (bar doesn't move, and correct its stale "emits no event" wording) and 548 (zero doesn't mark down) into hard checks using `changeHitPoints`
- [X] T022 [US2] Prove: T020 through the harness (`✘` grep), the playtest, `tsc --noEmit`; record results here. *Result 2026-09-14: `node scripts/e2e-parallel.mjs --shards=1 --only=combat-hit-points,combat-turn-order,combat-panel,token-movement-walls,status-display,world-cache-offline`: `combat-hit-points` passed (38 s; all three boards drew 2 without a reload, polled with a 5 s budget — the one-second figure is annotated per run, not asserted), and `combat-panel`, `status-display`, `token-movement-walls` passed; the log's only `✘` were two `world-cache-offline` tests failing at registration with a 502 while the machine was loaded (load 54), which passed 3/3 when re-run alone. Playtest: FINDINGs 522 and 548 now hard and passing. `cargo test` for `combat::` (hit points C7, C8, M1, concurrency) passed; `tsc --noEmit` and the web vitest suites for PlayDock and sync clean.*

**Checkpoint**: a fight can end.

---

## Phase 4: Turn order holds (Priority: P1) — US1

**Goal**: a player cannot move or attack on someone else's turn; the reason names whose turn it is; Game Masters and non-combatants are not held.

**Independent test**: on the ogre's turn, a player's move is refused with "It is Ogre's turn" on every path, and the token doesn't move on any board; hidden name reads "Unknown".

- [X] T023 [US1] Implement `turn_check(conn, scene_id, acting_token_id, user) -> TurnCheck` in `src/server/src/combat/turn.rs` per research R12 and C1: running combat in the scene, acting token is a combatant, not active, user not GM → refused with the active combatant's label under `load_combat`'s "Unknown" rule
- [X] T024 [US1] Call `turn_check` in `move_own_token` in `src/server/src/graphql/mutations_tokens.rs` before `judge_against_walls`
- [X] T025 [US1] Call `turn_check` for queued token moves in `src/server/src/graphql/mutations_reconcile.rs` (token write ~688-712), reporting a refusal the way a queued conflict is reported today. *Note (implement): a queued outcome carries an enum reason and no text, so this adds `NOT_YOUR_TURN` to `GraphQLRejectionReason` (and `thunderforge_cache_core::queue::RejectionReason`) and a `refusal: String` field on the outcome with the same sentence; `ReconcileReport.tsx` shows it.*
- [X] T026 [P] [US1] Server tests in `src/server/src/combat/turn_tests.rs`: refused off-turn; allowed on-turn; GM never refused; non-combatant token free; hidden active name → "Unknown"; offline replay refused
- [X] T027 [US1] Show the refusal text where a refused move is shown today in `apps/web/src/engine/world/sync/tokens.ts` (the move snaps back, and the message names whose turn it is). *Note (implement): already true of `applyMoveRefusal` since spec 045 — it re-reads the server's position and toasts the server's own message — so this is a comment, not code; `combat-turn-order.spec.ts` proves the text appears.*
- [X] T028 [US1] e2e `apps/web/e2e/combat-turn-order.spec.ts` covering the independent test, including the GM moving the ogre freely
- [X] T029 [US1] Turn FINDING 606 (a player moved out of turn) in `apps/web/playtest/combat-5e.playtest.ts` into a hard check
- [X] T030 [US1] Prove: T028 through the harness, `cargo test` for T026, the playtest; record results here. *Result 2026-09-14: `combat-turn-order` passed in the same harness run as T022 (46 s: drag, direct `moveOwnToken` and queued replay each refused with "It is Ogre's turn", "Unknown" with the name hidden, the Game Master and a non-combatant free, Aria's move landing on her turn). `cargo test` `combat::turn` 6 passed and `mutations_reconcile::…somebody_elses_turn…` passed, alongside the play-pause surface test covering `changeHitPoints`. Playtest: FINDING 606 hard and passing.*

**Checkpoint**: a round has an order that means something.

---

## Phase 5: A token is its actor, or a copy of it (Priority: P1) — US2

**Goal**: linked tokens share their actor's hit points; unlinked copies hold their own; unique NPCs place linked; `tokens.health` is gone; two hundred copies need no actors.

**Independent test**: Aria (linked) and two goblins (copies of one NPC). Damage goblin A: B and the NPC unchanged. Damage Aria: her sheet changes. "Boblin" marked unique places linked. Relink A: it takes the NPC's hit points.

- [X] T031 [US2] Write `docs/adrs/<date>-102-a_token_is_its_actor_or_a_copy.md` (PROPOSED) from research R4 and data-model.md; add it to `docs/adrs/README.md`
- [X] T032 [US2] Migration `src/server/migrations/<date>-046-token-links/` per data-model.md: add `tokens.linked`, `tokens.system_data`, the CHECK; backfill `linked`; null dangling `actor_id`s and add the FK `ON DELETE SET NULL`; add `world_actors.is_unique`; `down.sql` reverses what it can
  - *Corrected against real rows (2026-09-14):* the backfill keys on the actor's `is_npc`, not on `token_type = 'character'` — the dev database holds 19 NPC-actor tokens typed `character` and 2,366 `actor_type = 'character'` actors flagged `is_npc`, so the type is not evidence. An NPC's existing token becomes a copy **seeded from the NPC's current `resource_data`** (otherwise every placed goblin would lose its bars on migration); a player character's stays linked; an actorless token is an unlinked marker. `linked` defaults to `true` for insert paths that do not decide (bring-the-party, tests), which is what every token meant before. Dangling ids are counted with `RAISE NOTICE`; the dev database had 0 of 141,621 tokens.
- [X] T033 [US2] Second migration `src/server/migrations/<date>-046-retire-token-health/`: copy non-null `health`/`max_health` into `system_data` of unlinked tokens through the pack's declared hit-point fields (resolved per world's system in SQL, or in a one-off Rust step in the migration runner if SQL cannot reach the manifest), then drop both columns; `down.sql` restores them from `system_data`
  - *How the fields were resolved:* SQL cannot read `system.json`, and a Rust step in the runner would be the only migration that is not SQL. dnd5e is the only bundled pack declaring `combat.hitPoints`, so the migration names its fields (`current_hp`, `max_hp`) for tokens whose system (the actor's, else the world's) is `dnd5e`. Every other value — other systems, installed packs SQL cannot know, and linked tokens (whose record is the actor) — is **archived, not invented into undeclared fields**: every non-null value goes to a new `retired_token_health` table first, and `down.sql` restores the columns exactly from it rather than from `system_data`. The dev database held 0 non-null values.
- [X] T034 [US2] Placement defaults in `create_token` in `src/server/src/graphql/mutations_tokens.rs` per data-model.md: resolve `linked`, seed `system_data` for copies, derive `token_type` from `world_actors.actor_type` when the caller gives none; add `linked: Boolean` to `GraphQLCreateTokenInput` in `src/server/src/graphql/input_types.rs`
  - *Note:* the kind is derived from `is_npc` (NPC → `npc`) and `actor_type` (`vehicle` → vehicle, `hazard`/`prop`/`light_source` → object), for the same reason as T032. `create_token` also now refuses an actor from another world — a copy is seeded from its sheet, so the unchecked id would have carried another world's data onto the board. The resolver body moved to `create_token_impl` so placement is tested through it.
- [X] T035 [US2] `setTokenLink(tokenId, linked)` and `setActorUnique(actorId, unique)`, Game Master only, in `src/server/src/graphql/mutations_token_links.rs`; relink rules per data-model.md; record event 14 / 26
  - *Note:* the rules and `_impl`s live in `mutations_token_links.rs`; the two resolvers sit on `TokenMutation` in `mutations_tokens.rs`. Asking for what a token already is changes nothing (so "unlink" on a copy cannot re-seed and quietly heal it). Both are gated by the play pause and listed in `play_pause_surface_tables.rs`.
- [X] T036 [US2] Extend `apply_hit_point_change` in `src/server/src/combat/hit_points.rs` to write an unlinked copy's `tokens.system_data` (lock the token row; event 14) and a linked token's actor data (as phase 3)
  - *Note:* a copy's zero never falls back to `actor_id` (`follow_zero` takes the actor only for a linked token); `a_copy_at_zero_marks_out_its_own_combatant_and_no_other_sharing_its_npc` pins it, and fails when the fallback is put back. A copy holds only a resource slot: a pack declaring hit points in another slot has no damage operation on copies, and says so.
- [X] T037 [US2] Read a copy's `system_data` in `src/server/src/graphql/queries/token_status.rs`, and stop skipping actorless tokens that carry `system_data`
- [X] T038 [US2] Remove `health`/`max_health` from `models::Token`, `GraphQLToken` (`types_scene.rs`), `input_types.rs`, `mutations_reconcile.rs`; remove the engine's `health` fields and the unread `DerivedStats.is_dead` in `src/engine/src/components.rs`, `src/engine/src/app.rs`, `src/engine/src/derived_data.rs`; delete the orphan `src/server/src/token_systems.rs`
  - *Note:* `world_tokens.health` (the legacy world-scoped table, `GraphQLWorldToken`) is a different table and is untouched. The engine's `RollbackCache.last_server_health`, `DerivedStats.health_percentage`/`is_full_health` and `derived_data`'s health helpers went with `is_dead`, since all three read only `Token.health`; so did the `health`/`maxHealth` fields of the engine's token payload.
- [X] T039 [US2] Remove the legacy health bar and the `health`/`maxHealth` create fields from `apps/web/src/components/TokenPanel.tsx`, and `health` from `apps/web/src/engine/world/sync/tokens.ts` and the engine payload types in `apps/web/src/engine/world/types.ts`
- [X] T040 [US2] Set a character token's owner from the claim when one exists in `bring_party_to_scene` in `src/server/src/graphql/mutations_party.rs` (research R7 known gap)
- [X] T041 [P] [US2] Server tests in `src/server/src/graphql/mutations_token_links_tests.rs`: placement defaults for character, NPC, unique NPC, actorless; copy damage isolated; relink discards `system_data`; unlink seeds it; non-GM refused
  - *Also:* a copy at zero marks out only its own combatant (not one sharing its NPC with no token), a linked token's damage writes its actor, an actor from another world is refused, and T040's claimed character arrives owned by its claimant. 11 tests; with the rest of `thunderforge-server --lib`, 1,611 passed against a scratch database.
- [X] T042 [US2] "Linked / Copy" control on a selected token and "Unique" toggle on the NPC editor, Game Master only, in `apps/web/src/components/TokenPanel.tsx` and the NPC edit page under `apps/web/src/pages/world/`
  - *Note:* the NPC page is `pages/world/actor/ActorDetailPage.tsx`; the Unique toggle shows on view and edit for a Game Master, like the claim block.
- [X] T043 [US2] Regenerate `src/app/schema.graphql`; `pnpm verify`; `tsc --noEmit`; `make lint-wasm`
- [X] T044 [US2] e2e `apps/web/e2e/token-links.spec.ts` covering the independent test
  - *Found by it:* the token panel's details popover had no height limit, so opened near the top of a 720px screen its first controls (photo, and now the link choice) sat above the viewport; existing specs forced their clicks. It now scrolls within Radix's available height, and the spec clicks unforced.
- [X] T045 [US2] Add a 200-copy level to `apps/web/e2e/engine-status-limits.spec.ts` that places copies of one NPC through `createToken`; assert no new actors, measure load-to-drawn time and fps against research R17's targets (SC-008)
  - *Measured 2026-09-14* (32 cores, desktop session open, load average 3–5, nothing else under test; release engine; median of three opens, load = navigation to the last change in sprite count with all 200 displaying): empty scene 759–806 ms; **200 copies 1,425–1,483 ms, +686 ms**; the same 200 relinked to the NPC 1,467–1,957 ms, +694 ms; **copies over linked −8 ms** (previous run +4 ms). Tokens and their bars land together (sprite count reached 200 at the same moment). `tokens` answers in 15 ms and `tokenStatus` in 13 ms for the scene. Steady state **60 fps, 16.7 ms frame**, 20 samples, matching the 400-token baseline (60 fps in the same run).
  - **R17's load target is not met as written** ("adds under 500 ms to the same scene without them"): two hundred tokens add ~690 ms, but copies contribute nothing to it — the cost is the engine bringing two hundred tokens and bars onto a board, which predates this phase. The spec's hard gate is therefore on what this phase owns (copies open within 500 ms of the same tokens linked) and on 60 fps; the absolute figure is logged every run. Whether 500 ms against an empty scene is the right target is for whoever owns engine loading.
- [X] T046 [US2] Add a hard check to `apps/web/playtest/combat-5e.playtest.ts`: two goblin copies take damage separately
  - *Note:* the second goblin is placed from the same NPC but kept off the tracker's roster. NPCs are written before they are placed (`placeCast`'s new `sheet`), because a copy starts from the sheet it was placed with.
- [X] T047 [US2] Prove: T044, T045 and the token/combat/status e2e specs through the harness; the playtest; record results and the measured numbers here
  - *Result 2026-09-14:* `--shards=1 --only=token-links,combat-hit-points,combat-turn-order,combat-panel,token-authoring,token-names,token-attributes,token-keyboard-move,token-movement-walls,status-display,status-disclosure,status-gm-control,status-many-tokens,status-placement,status-sdk,status-systems,status-appearance,genie-npc-and-items`: 39 passed, 0 failed, no `✘`. `engine-status-limits` (with `STATUS_CAPACITY_LEVELS=400` for the existing sweep): 2 passed, no `✘`; numbers under T045. `pnpm playtest --only=combat-5e` played to Round 2 and the end with no hard failure; the new copies check passes; five soft FINDINGs remain (263, 380, 416, 439, 481), as before this phase. `cargo test -p thunderforge-server --lib` 1,611 passed on a scratch database migrated to this branch; `pnpm verify` 14/14; `tsc --noEmit` clean; `make lint-wasm` clean. Migrations round-tripped up/down/up on a scratch database with a fixture covering every backfill case (see the T032/T033 commit).
  - *Pre-existing, not this phase:* `cargo test -p thunderforge` aborts with a stack overflow in `play_pause_surface_tests::every_gated_pack_field_refuses_a_paused_world` at `--test-threads=1`; the same test overflows identically on `main` at 9cdd2ce (checked in a detached worktree), and passes under `RUST_MIN_STACK=67108864`.

**Checkpoint**: one record of hit points per creature.

---

## Phase 6: An attack is aimed at something (Priority: P1) — US1, US2

**Goal**: an attack chooses a target, rolls against its defence, is shown to every seat within a second with the attacker redacted per viewer, and a hit's damage is offered to the target's controller (or auto-applied to the GM's NPCs).

**Independent test**: see quickstart.md phase 4 — attack shown to all seats; offer taken by the GM; offer to Aria's absent player resolved on her behalf; auto-apply for an encounter; a hidden attacker reads "Unknown" with no id or name in the player's traffic.

- [X] T048 [US1] Write `docs/adrs/<date>-101-an_attack_is_resolved_on_the_server.md` (PROPOSED) from research R1, R7, R8, R15; add to `docs/adrs/README.md`
- [X] T049 [US1] Migration `src/server/migrations/<date>-046-attacks-and-offers/`: `world_attacks`, `world_offers` (with the pending index), `worlds.auto_apply_npc_damage`, `world_combats.auto_apply`, and the attack fields on `world_abilities` and `world_items` (`reach`, `range_normal`, `range_long`, `needs_line_of_sight`, `action_cost`, `legendary_cost`, `multiattack`) per data-model.md
  - *Note (implement):* beyond data-model.md, `world_attacks` keeps `attacker_label`, `target_label` and `ability_name` (server-side snapshots, so a deleted token still reads in the Game Master's log; sent only unredacted), and `world_offers` keeps `target_linked` (the relink rule, research R18). Token, ability and roll references are `ON DELETE SET NULL`, so the ability/item CHECK is "at most one". Round-tripped up/down/up on a scratch database (`tf_p6_scratch`) with a fixture row in every table and column touched; the CHECKs refuse a negative amount, a long range shorter than normal, and an unknown outcome; deleting a target token takes its offer and nulls the attack's target.
- [X] T050 [P] [US1] Add `defence` to 5e: `armor_class` (integer, min 0) in a data type of `packs/systems/dnd5e/system.json` and its `server/src/validators.rs`, and `combat.defence` naming it
  - *Note:* `armor_class` is in `ability_data` (the slot data-model.md's example names), optional; the validator refuses a non-integer or negative value.
- [X] T051 [US1] Add `EVENT_CODE_ATTACK_MADE = 29` and `EVENT_CODE_OFFER_CHANGED = 30` to `src/server/src/world_events.rs` with their doc comments; payloads ids only (contract §4)
- [X] T052 [US1] Controllers per research R7 in `src/server/src/combat/controllers.rs`: `controllers_of(token)` and `may_act_for(user, token)`, reusing the check in `move_own_token` rather than copying it
  - *Note:* `move_own_token` now calls `controllers::may_move` instead of its inline check (same rule: owner, or Owner on the actor, which a Game Master holds implicitly). `player_controllers` is R15's "no controller other than Game Masters"; `controlled_tokens_in_scene` is a viewer's eyes for §3.
- [X] T053 [US1] Redaction per contract §3 in `src/server/src/combat/redaction.rs`: `party_for_viewer(viewer, token, scene)` using `vision::visibility_of` from each of the viewer's controlled tokens, `wall_set_from_rows`, the scene's lights and ambient light, and `vision_profiles.rs`; `name_visible_to_players`; Game Masters see all
  - *Note:* `SceneSight::for_viewer` loads a scene once per viewer and answers many tokens. Beyond the task text it lights the scene with each creature's *carried* light and gives each eye its declared darkvision (both resolved as `tokenVision` does), and honours the engine's "a token never hides from its own square".
- [X] T054 [US1] Resolution in `src/server/src/combat/attack.rs`: `make_attack` in contract order — C2 control, C1 turn (`turn.rs`, REACTION exempt), resolve the ability's `ATTACK_ROLL` formula through `roll_dice_impl`, read defence, outcome, damage roll on hit, write `world_attacks`, then C4/C5 offer or auto-apply (research R15) through `apply_hit_point_change` in one transaction; multiattack makes one row per part with `multiattack_of`; record events 29 and 30
  - *Note (implement):* the rolls go through the dice crate inside `make_attack`'s transaction and are written as ordinary `world_roll_records` rows, rather than through `roll_dice_impl`, which is async, takes its own pooled connections and could not share the transaction. Refusals are a `FightRefusal` (paused, not your turn, not controlled, not found, invalid) so the offline path can report each as its own reason. A copy reads its defence from its NPC's sheet and a copy whose NPC is gone has none (`NO_DEFENCE`). Auto-apply runs in a savepoint: a target with no hit points recorded leaves the offer pending rather than losing the attack. Several `DAMAGE` effects roll as one formula, `(a)+(b)`.
- [X] T055 [US2] Offers in `src/server/src/combat/offers.rs`: `resolve_offer(offer, take, user)` — C6 once only, controllers or GM, `resolved_on_behalf`, take → `apply_hit_point_change` in the same transaction; `pending_offers(world, user)`
  - *Note:* the relink rule (research R18, contract C6a) is here: taking an offer whose token was relinked or unlinked since it was made is refused, declining is not; the offer row and then the token row are locked before the comparison.
- [X] T056 [US1] GraphQL in `src/server/src/graphql/mutations_attacks.rs` and `src/server/src/graphql/queries/attacks.rs`: `makeAttack`, `previewAttack` (flags and turn check, no writes), `attack(id)`, `sceneAttacks`, `resolveOffer`, `pendingOffers`, `updateWorldAutoApplyNpcDamage`, `setCombatAutoApply`; types per contract §1; every answer built per viewer through `redaction.rs`; `refuse_if_paused` on every mutation (C10); register in `graphql.rs`; expose the new ability and item fields on their types and inputs
  - *Note (implement):* the ability and item attack fields are exposed flattened on `Ability` and `Item`, and set through `setAbilityAttack` / `setItemAttack` (Editor) with an `AttackFieldsInput`, rather than on `updateAbility`/`updateItem`'s inputs, whose "omitted means unchanged" cannot clear a reach. `Combat` gains `autoApply` and `effectiveAutoApply`; `World` gains `autoApplyNpcDamage`. All six mutations are in `play_pause_surface_tables.rs` (the surface test seeds a pending offer). ~~The fields are on `WorldAbility`/`WorldItem`, so every path that clones those rows (collection copies) carries them.~~ *Corrected in Phase 7:* they were not carried — every copy path inserts from a named list of fields; see T079's note.
- [X] T057 [US1] Offline attack intents in `src/server/src/graphql/mutations_reconcile.rs` (research R16): a queued attack resolves through `make_attack` at replay; a turn refusal spends nothing and is reported as a refused queued action
  - *Note:* a queued `{"type": "make_attack", "token": {"id": <attacker>}, "attack": {…}}` is resolved in `reconcileQueuedChanges` through `make_attack`, made as the person who queued it; `thunderforge_cache_core::queue::ATTACK_INTENT_TYPE` names the kind, beside T025's `NotYourTurn`. `NOT_YOUR_TURN` carries the live sentence; nothing is rolled, recorded or offered.
- [X] T058 [P] [US1] Server tests `src/server/src/combat/attack_tests.rs`: C1 (off-turn refused, reaction allowed), C2, C4 (miss and no target create no offer), C5 (pending; auto-apply only to GM-run NPCs; player-controlled always pending), multiattack rows, pause refusal (C10)
  - *Note:* also C3 (nothing flagged or refused), event 29's payload, a player using only what their creature has, a copy's defence from its NPC and `NO_DEFENCE` when its NPC is gone, the encounter override both ways, and auto-apply's line-of-sight rule. The offline replay test lives in `mutations_reconcile_tests.rs`, where `apply_attack_intent` can be reached; shared fixtures are `combat/fixtures.rs`.
- [X] T059 [P] [US2] Server tests `src/server/src/combat/offers_tests.rs`: C6 once only, by a controller, by a GM on behalf, by a stranger refused; taken offer changes hit points atomically
  - *Note:* also two takes at once on separate connections resolving once, a failed hit-point change leaving the offer pending, C10, `pendingOffers` per viewer, and the relink rule.
- [X] T060 [P] [US1] Server tests `src/server/src/combat/redaction_tests.rs`: hidden name → Unknown; unseen through a wall → Unknown; seen → named; target redaction nulls defence; attacker redaction nulls ability name; no redacted id in any field; GM unredacted
  - *Note:* also darkness hidden and revealed by a light, a viewer's own creature always known, a viewer with no token seeing nobody, redaction following a token that walks round a wall, offers read by their controller, and no redacted id or name in any event payload. Mutation-checked: making `may_know` always true fails seven of the ten; removing the relink comparison fails its test.
- [X] T061 [US1] Ability and item editors gain reach, normal/long range, "needs line of sight", action cost, legendary cost and multiattack in `apps/web/src/pages/world/ability/` and `apps/web/src/pages/world/item/` (effect editors' neighbouring form)
  - *Note:* one `AttackFieldsEditor` in `pages/world/ability/`, used by both detail pages beside their effect editors; it reads and writes through its own query and `setAbilityAttack` / `setItemAttack`. Multiattack is entered as ability ids for now — a picker is polish.
- [X] T062 [US1] `apps/web/src/api/attacks.ts`: `makeAttack`, `previewAttack`, `attack`, `sceneAttacks`, `resolveOffer`, `pendingOffers`, auto-apply setters
- [X] T063 [US1] Attack flow in `apps/web/src/components/world/PlayDock/AttackFlow/`: from an attack on the in-pane sheet (`InPaneCharacterSheet.tsx`, replacing the bare `rollDice` for `ATTACK_ROLL` abilities), choose a target on the board or from a list, show `previewAttack`'s warning, confirm, roll; FR-033's warning before rolling
  - *Note:* the sheet's `ATTACK_ROLL` buttons keep their test ids and open `AttackFlow` instead of calling `rollDice` (`CharacterRoll.attackAbilityId`); the attacker is this character's token on the scene in play (the viewer's own, else the first), and the button is disabled with a reason when there is none. `ActorsPanel` now takes `sceneId`. "This is a reaction" sends `REACTION`. Offline, the attack is queued as an intent (`OfflineEditKind` gains `attack`).
- [X] T064 [US1] Attack log for the table in `apps/web/src/components/world/PlayDock/AttackLog/`: subscribe to event 29, read `attack(id)`, show attacker, target, total vs defence, outcome and flags; "Unknown" as served; wire into the dice-roll animation where `handleRoll` does today
  - *Note (implement):* mounted over the board in `WorldPage` with the offer prompt (`table-feed`), not in the dock: the dock mounts only its open section, and an attack has to reach a player looking at something else. The dice animation plays on the attacker's client when `makeAttack` answers, as `handleRoll` does for a free roll. An offer changing re-reads the page, so who resolved it reaches every seat.
- [X] T065 [US2] Offer prompt in `apps/web/src/components/world/PlayDock/OfferPrompt/`: on event 30 and on load (`pendingOffers`), show "Take 5 damage?" with Take / Decline; a GM sees every pending offer with "resolve on behalf"; the table sees who resolved it
- [X] T066 [US1] Auto-apply toggles: world default in the world settings page under `apps/web/src/pages/world/` (GM only), and the per-encounter override in `CombatPanel.tsx`
  - *Note:* the world default is on the world dashboard's campaign settings (`components/campaign/CampaignSettingsPanel.tsx`, beside "player-created characters"), which is where this app's Game-Master world settings live; the per-encounter override is a three-way choice on the tracker.
- [X] T067 [US1] Regenerate `src/app/schema.graphql`; `pnpm verify`; `tsc --noEmit`
  - *Result 2026-09-14:* `pnpm verify` 14/14 (graphql schema and operations included; the registry check made the test fixtures a `_tests.rs` file, since they name `dnd5e`); `pnpm -F @thunderforge/web exec tsc --noEmit` clean; web vitest for PlayDock and world sync 137 passed.
- [X] T068 [US1] e2e `apps/web/e2e/combat-attack.spec.ts` per quickstart.md phase 4, including capturing the player page's network responses and asserting the hidden ogre's token id and name are absent
  - *Note:* the weapons hit on any die (`1d20+100`) for fixed damage, so the arithmetic is asserted rather than rolled for. Steps: Aria attacks the goblin from her sheet and all three boards show "Aria → Goblin · Longsword · N vs 13: hit · 5 damage offered"; the Game Master takes it and the goblin's bar reads 2 on every board; the ogre hits Aria, her offer survives her page going away and coming back, and the Game Master takes it on her behalf ("taken by Game Master" on both players' boards); auto-apply on for the encounter applies a hit on the goblin and still offers one on Aria; then a wall and a hidden name, and the ogre's attack reads "Unknown → Aria" with no weapon on both players' boards.
  - *Network capture:* each player page records every GraphQL response body and every subscription frame. After the ogre is hidden, every response to `attack`, `sceneAttacks`, `pendingOffers`, `makeAttack`, `resolveOffer` or `previewAttack` and every frame carrying `attackId`/`offerId` (20 per player) is asserted to contain neither the ogre's token id, its actor id, its name nor its weapon, and the check is asserted non-vacuous (the attack was read and event 29 arrived). The rest of the traffic is annotated, not asserted: 8 of 15 other messages carried the ogre's token id (the scene's token list and status, which spec 045 sends and does not draw — research R8's accepted consistency note) and 0 carried its name.
- [X] T069 [US1] Turn FINDINGs 263 (roll not shown), 416 (no AC or target) and 439 (longsword not rollable) in `apps/web/playtest/combat-5e.playtest.ts` into hard checks, using `apps/web/playtest/combat.ts` helpers for target selection and offers
  - *Note:* FINDING 263 is now the attack reaching the other seats (a free roll in the dice roller stays the roller's own, and is annotated); 416 is Aria's longsword swung from her own sheet at the goblin and judged against its armour class (goblin 15, ogre 11, Aria 16, Brom 12, kept with the scores); 439 is the longsword on Aria's sheet, which needed each player to *claim* their character (`claimFor`), since only a claimed character's sheet opens in the dock. A miss offers nothing; Aria swings until she lands one (at most eight), and the Game Master declines the goblin's offer so the later damage step meets it whole, and Brom's board is told who declined. The playtest found the attack log over the dice roller's Roll button at the bottom left, and then the Game Master's tool flyout over the offer prompt at the top left; the feed sits at the bottom centre.
- [X] T070 [US1] Prove: T068 through the harness; `cargo test` for T058–T060; the playtest; accept ADR-101 and ADR-102 once proven; record results here
  - *Result 2026-09-14:* `node scripts/e2e-parallel.mjs --shards=1 --only=combat-attack,combat-hit-points,combat-turn-order,combat-panel,token-links,token-names,status-display,dice-roll,abilities-compendium,abilities-ux,actor-claim,genie-npc-and-items,world-cache-offline,demo-first-session,gm-staging-page,world-compendium`: 42 passed, 0 failed, no `✘` in the log, 10.2 minutes. `cargo test -p thunderforge-server --lib` on a scratch database migrated to this branch: 1,655 passed, 0 failed (`RUST_MIN_STACK=16777216`); C1–C8, C10 and C6a each have named tests (C9, budgets, is Phase 8's) in `combat::attack`, `combat::offers`, `combat::redaction`, `combat::hit_points`, `graphql::mutations_reconcile` and `play_pause_surface_tests`. `pnpm playtest --only=combat-5e` played to the end with no hard failure: FINDINGs 263, 416 and 439 are hard and pass (the run's attack: "Aria → Goblin · Longsword · 19 vs 15: hit · 7 damage offered", declined by the Game Master); the two soft FINDINGs left are 380 (economy, Phase 8) and 481 (size and reach, Phase 7). ADR-101 and ADR-102 accepted.

**Checkpoint**: an attack does something, and the table sees it — MVP complete.

---

## Phase 7: Size fills squares, and attacks have reach (Priority: P1) — US4

**Goal**: a creature's size decides the squares it fills for drawing, snapping, hit-testing and movement; reach, range and line of sight are measured from footprint to footprint and flagged, never refused.

**Independent test**: a Large ogre fills 2×2 on every board; a hero one square away swings unflagged; four squares away is warned, still swings, flagged "out of reach"; shortbow beyond normal flagged long range; through a closed door flagged no line of sight and not auto-applied.

- [X] T071 [US4] `footprint_distance` in `crates/thunderforge-canvas-core/src/grid.rs` per research R11 (square: min Chebyshev between covered rectangles; hex: centre axial; gridless: Euclidean ÷ cell) and `footprint_line_of_sight` in `crates/thunderforge-canvas-core/src/wall.rs` (any pair of covered cell centres visible)
  - *Note (implement):* `GridSpec::footprint_distance` returns `f32` cells, not R11's `i32`: a gridless scene has no whole number to give (square and hex answers are integral). Gridless subtracts how far each creature's half-width reaches past half a cell, so touching creatures are 1 apart at any size, and two one-cell creatures read their plain Euclidean centre distance. The blocks come from a new `GridSpec::covered_cells` (corner rounded half-up, as `snap_footprint` rounds it), and the points line of sight tries from `GridSpec::footprint_points` (a per-cell lattice on gridless). Line of sight is walls only (`is_visible`), not light: whether a sword reaches through a door is a question about the door.
- [X] T072 [P] [US4] Tests in `crates/thunderforge-canvas-core/src/grid.rs` tests and `wall_tests.rs`: 1×1 adjacent = 1; 2×2 adjacent from any of its squares; 4×4; hex centre; gridless; a 2×2 peering past a corner sees
  - *Note:* the grid tests are a sibling `grid_footprint_tests.rs` (15 tests; `grid.rs` is near the 1000-line check); six more in `wall_tests.rs`, the corner case asserting that the two centres are blocked while the footprints are not. `cargo test -p thunderforge_canvas_core --lib`: 460 passed.
- [X] T073 [P] [US4] 5e sizes: `size` in `trait_data` of `packs/systems/dnd5e/system.json` and its validator; `combat.sizes` with Tiny 0.5, Small 1, Medium 1, Large 2, Huge 3, Gargantuan 4
  - *Note:* both packs' validators read their size ids from the `system.json` compiled in beside them (`declared_size_ids`, `include_str!`), so 5e has no list of its own either. 5e's `trait_data` still requires `class` and `level`, so a monster's size is written with them.
- [X] T074 [P] [US4] Genie: move `sizeCategories` under `combat.sizes` in `packs/systems/genie/system.json`; make `packs/systems/genie/server/src/validators.rs` read the manifest instead of its hard-coded list; update `apps/web/src/utils/sizeCategory.ts` and `packs/systems/genie/web/src/lib/sizeCategory.ts` to read the new location
  - *Note:* the old table was `{ id: { scale } }` and the new block is `categories: [{ id, label, footprint }]` with a `source`, so both `sizeCategory.ts` files became `declaredSizesOf` / `resolveSizeFootprint` rather than a changed lookup path; `SizeCategoryBadge` shows the footprint. Genie's `size_category` keeps its field name (the block's `source` names it). `pack_system_spec`'s shipped-manifest tests now cover Genie's block too.
- [X] T075 [US4] Query `tokenGrid(sceneId)` in `src/server/src/graphql/queries/token_grid.rs` per contract §1 and research R10 (linked → actor's size; copy → its NPC's; none → omitted); one query over `tokens` joined to `world_actors`
  - *Note:* the resolution is `combat::size::footprints_in_scene`, shared with the attack measurement, and is three reads rather than one join (tokens; their actors' systems; their sheets), each once per scene however many tokens. A size is read from the actor's system, else the world's, as defence is; a pack that kept size among resources would read a copy's own `system_data`.
- [X] T076 [US4] `sync/tokenGrid.ts` in `apps/web/src/engine/world/sync/`, shaped like `tokenVision.ts`: dispatch `SetTokenGridCommand` for every token (explicit 1 for omitted), re-read on events 14 and 26; add `SetTokenGridCommand` to `apps/web/src/engine/world/types.ts`; export from `sync/index.ts`
  - *Note (implement):* event 14 is served where `tokens.ts` re-reads the tokens (on load and on every token change), beside and in parallel with `tokenVision`, so a new token is in the engine before it is sized; event 26 joins `WorldPage`'s fan-out. Two engine changes the task did not name: `snap_tokens_to_grid` now also runs on `Changed<TokenGridBehaviour>`, because a token arrives, snaps as one square, and is told a moment later it is Large — keyed on the transform alone it stayed centred in one cell; and a `token_footprints()` probe (footprint, centre, sprite size, name height, bar width as drawn) on `__engineProbe.tokenFootprints`, for T083.
- [X] T077 [US4] Make `TokenPanel.tsx` stop passing a size-derived `scale` to `createToken` (size now comes from `tokenGrid`); keep `scale` as an art multiplier
  - *Note:* the create dialog's hint now says what the NPC will fill ("Fills 4×4 squares", test id `token-create-npc-size-hint`), read through the system's `combat.sizes` source rather than a named `size_category` field. `genie-npc-and-items.spec.ts` asserted the old scales (0.5 and 4 sent to `createToken`) and was rewritten to assert no scale is sent and `tokenGrid` answers 0.5 and 4. Nothing else derives scale from size (`resolveSizeScale` is gone). The engine's `MIN_TOKEN_SCALE = 1` resize clamp no longer meets Genie's Small 0.75 / Diminutive 0.5, which are footprints now, and `Footprint` floors at 0.5.
- [X] T078 [US4] In `src/server/src/combat/attack.rs`, measure `distance` with `footprint_distance` × `units_per_cell` and set `OUT_OF_REACH`, `LONG_RANGE`, `BEYOND_RANGE`, `NO_REACH_DECLARED` and `NO_LINE_OF_SIGHT` (skipped when `needs_line_of_sight = false`); exclude no-line-of-sight attacks from auto-apply (FR-007); same in `previewAttack`
  - *Note (implement):* the measurement is a new `combat/reach.rs` (`SceneMeasure`, `flags_for`), since `attack.rs` sits near the 1000-line check; both `make_attack` and `preview_attack` call one `measure_parts`. The scene's grid is built from `scenes` exactly as the engine builds it (anchored to the map's corner). Flag rules, the walls-only line of sight and the **redaction decision** (redaction stays centre-based, as the engine draws; the flag is footprint-based) are recorded in research R11 and contract §3. C1 stays first: `make_attack` refuses for the turn before it measures, and `previewAttack` returns the turn check alongside the flags. `AttackPreview` gains `reach`, `rangeNormal`, `rangeLong` and `unit` for T080's sentence (contract §1 updated).
- [X] T079 [P] [US4] Server tests in `src/server/src/combat/attack_tests.rs`: each flag; no flag refuses (C3); a Large creature's reach is its attack's, not 10 ft; auto-apply skipped without line of sight unless the ability ignores it
  - *Note:* seven tests there (each flag through `makeAttack` and `previewAttack` agreeing; four squares out of reach still rolled and offered; a Large ogre adjacent from every square and out of reach at two; auto-apply skipped behind a wall and applied for an attack that ignores walls; the turn refusal standing in front of a reach warning; `footprints_in_scene` for linked, copy, marker and a changed sheet), and `redaction_follows_the_board_by_centres_while_an_attacks_sight_follows_footprints` in `redaction_tests.rs`. Mutation-checked: measuring every token as one square fails the Large-ogre test. `combat::` 87 passed on a scratch database (`tf_p7_scratch`, migrated to this branch; Phase 7 adds no migration).
  - *Copy and export (the risk Phase 6 named):* a test that a collection copy carries `reach`/ranges/`needs_line_of_sight`/`action_cost`/`legendary_cost`/`multiattack` **failed** — `copy_ability`, `copy_item`, `copy_shared_ability_to_world_impl` and `copy_shared_item_to_world_impl` each insert from a named list without them, and the personal export omitted them. Fixed through one `combat/attack_fields.rs` every copy path writes with (a multiattack's parts point at the copies made alongside, and a part not copied is dropped rather than left naming an ability in another world), and `ExportedAbility`/`ExportedItem` gain `attack`. Tests: `collections::copy::tests::fidelity::attack_fields_travel_with_a_copy_and_an_export` (fails with the carry removed) and the ability-share copy test's new assertions.
- [X] T080 [US4] Show flags in `AttackLog` and the pre-roll warning in `AttackFlow` ("Out of reach: 20 ft, reach 5 ft")
  - *Note:* `AttackLog` already rendered an attack's flags (Phase 6 wired it against empty lists), so the change is the warning: `warningTexts` in `attackText.ts` makes one sentence per flag from `previewAttack`'s distance, reach, ranges and unit, shown under the turn warning (never in front of it); vitest `attackWarnings.test.ts`.
- [X] T081 [US4] Replace the playtest ogre's `scale = 2` with `size: large` in `apps/web/playtest/combat-5e.playtest.ts` and `apps/web/playtest/table.ts`; turn FINDING 481 (no size or reach) into a hard check
  - *Note:* `placeCast` loses its `scale` option and its `sheet` gains `traits` (5e's `trait_data` requires `class` and `level`, so the ogre is `{ class: "monster", level: 5, size: "large" }`). "Sees the ogre at its size" read the store's `scale`; it now asks each board's engine (`footprintOn`, the new probe) that the ogre fills two squares and is drawn two squares wide. FINDING 481 is hard: the ogre's sheet carries its size, and Aria's longsword, given a five-foot reach (`setAbilityReach`), swung across the room is warned "Out of reach: N ft, reach 5 ft" before rolling, made anyway, and shown flagged on the Game Master's and Brom's logs. New helpers in `playtest/combat.ts`: `setAbilityReach`, `footprintOn`, `attackWarningsFromSheet`.
- [X] T082 [US4] Regenerate `src/app/schema.graphql`; `pnpm verify`; `tsc --noEmit`; `make lint-wasm`
  - *Result 2026-09-14:* schema regenerated with `tokenGrid`, `TokenGrid` and `AttackPreview`'s four new fields; `pnpm verify` 14/14; `pnpm -F @thunderforge/web exec tsc --noEmit` clean; `make lint-wasm` clean (it caught one `type_complexity` in the re-snap query, factored out); web vitest for PlayDock, sync and `sizeCategory` 123 passed; Genie's `node --test` size tests 5 passed.
- [X] T083 [US4] e2e `apps/web/e2e/combat-reach.spec.ts` per the independent test, including snapping, hit-testing and a keyboard move of the 2×2 ogre
  - *Note:* the scene is given a 64-unit grid on a 1280-square map so a vertex sits on the world origin. Every board's engine draws the ogre two squares wide on its vertex, its bars two squares wide and its name above them (Game Master's board, which draws both for both); a press on its far square selects it; a drag lands on a vertex one square on and every board and the server agree. The keyboard check moves **Brom, made Large**, not the ogre: keyboard movement is only ever the player's own primary token (`token_move.rs`), and the ogre is the Game Master's. Then from footprint to footprint: beside the ogre's corner square unflagged; four squares away "Out of reach: 20 ft, reach 5 ft", made, flagged on all three logs; a scaled shortbow "Long range: 20 ft, normal range 10 ft", then "Beyond range: 30 ft, range 20 ft"; through a closed door "No line of sight", offered and **not** applied with auto-apply on for the encounter, the ogre's hit points untouched, and Aria's log reading "Aria → Unknown" (her board does not draw the ogre behind the door; research R11).
  - *Found by it, fixed:* (1) a Large token was drawn one square up and right of the server's position on every board: it was snapped as one square on arrival, and the new re-snap on a footprint change snapped from there rather than from where it was put (`snap_tokens_to_grid` now remembers each token's unsnapped position; commit "Draw a Large token where the server put it"). (2) Preparing the keyboard check: a keyboard step snapped the next cell's centre for the footprint, which for a 2×2 turned west into north and south into east; canvas-core `step_token` and `PlannedPath::destination` / `world_points_from` carry the whole footprint (commit "Let a Large token step west and south with the keyboard", four canvas-core tests). The engine still draws a planned route from cell centres, not a Large token's vertex — cosmetic, left.
- [X] T084 [US4] Prove: T083 plus the canvas, token and lighting specs through the harness; `cargo test -p thunderforge_canvas_core`; the playtest; record results here
  - *Result 2026-09-14:* `node scripts/e2e-parallel.mjs --shards=1 --only=combat-reach,combat-attack,combat-hit-points,combat-turn-order,combat-panel,token-links,token-names,token-authoring,token-keyboard-move,token-movement-walls,token-attributes,canvas-authoring,carried-light,darkvision-range,interactive-lighting,scene-lighting,scene-exploration,status-display,status-placement,status-many-tokens,genie-npc-and-items,genie-full-encounter,content-collections,scene-default-grid-type`: **59 passed, 0 failed, no `✘` in the log**, 15.6 minutes. `cargo test -p thunderforge_canvas_core` 464 passed; `pack_system_spec` 83, `dnd5e-server` 46, `genie-server` validators 14 passed. `cargo test -p thunderforge-server --lib` on `tf_p7_scratch` (`RUST_MIN_STACK=16777216`): 1,671 passed, 1 failed — `compendium_tests::a_re_import_is_recognised_from_a_different_world`, which builds a "different" hash by replacing the last hex digit with `f` and so fails whenever the random hash already ends in `f` (1 in 16; unrelated to this phase, passed on re-run). `pnpm playtest --only=combat-5e` played to Round 3 and the end with no hard failure: FINDING 481 is hard and passes; the one soft FINDING left is 380 (economy, Phase 8).
  - *Load (T045's level, machine idle after the proof run, load average 1.6 falling):* `STATUS_CAPACITY_LEVELS=400 --only=engine-status-limits` 2 passed. Empty scene 733–788 ms; 200 copies 1,487–1,535 ms, **+757 ms** (Phase 5: +686); the same 200 linked +711 ms (Phase 5: +694); copies over linked +46 ms (gate 500 ms); 60 fps, 16.6 ms. Footprint sync is one `set_token_grid` per token beside `tokenVision`'s two, read in parallel with vision; the ~+20–70 ms against Phase 5's figures is within the run-to-run spread seen there, and R17's absolute +500 ms target stays unmet as recorded under T045.

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

<!-- T001 baseline, 2026-09-14, `pnpm playtest --only=combat-5e` on main at 0e8b3ac
     (before any spec 046 code): the scenario played to the end (reached Round 3)
     with no hard failure and eight soft FINDINGs:
       263 a roll is not shown to the table
       380 a round is not an economy
       416 no armour class or target
       439 the longsword is not rollable from Aria's sheet
       481 no size or reach
       522 the goblin's bar does not move on a hit-point write
       548 zero hit points does not mark a combatant out
       606 a player moved on somebody else's turn
     After Phases 3 and 4 (same command, this branch): 522, 548 and 606 are hard
     checks and pass; 263, 380, 416, 439 and 481 remain.
     After Phase 7: 481 is hard as well; 380 alone remains. -->

- Commits are signed, stage explicit paths, and each names the phase and task ids.
- `.e2e-shards-durations.json` is rewritten by the harness; don't commit it with feature work.
- Migrations get real timestamps when written; `<date>` above is a placeholder for that, not a directory name.
