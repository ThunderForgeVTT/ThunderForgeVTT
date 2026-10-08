# Tasks: 5e Roll Facets

**Input**: [spec.md](./spec.md), [plan.md](./plan.md), [research.md](./research.md),
[data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: the spec's Proof section asks for dice-crate, pack, server, demo
and e2e tests. Inside each phase, the tests come before the code they
prove.

**File length**: `scripts/check-file-length.sh` fails a Rust file over 1000
lines. Three files are near the limit and take no new logic:

- `thunderforge-dice/src/eval.rs` (944);
- `combat/attack.rs` (830);
- `combat/attack_tests.rs` (789).

The new code goes in the new files named below. `mutations_roll_check_tests.rs`
(638) takes the advantage cases, and the reroll tests get their own file.

**Spec 083** is planned in parallel and touches `DieOutcome` (`steps`) and
the throw. This spec does not draw anything. T012 is the only point where the
two specs meet.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an open task)
- **[Story]**: the user story a task serves:
  - US1 advantage and disadvantage;
  - US2 Halfling Luck;
  - US3 Inspiration on a check;
  - US4 a missed attack's reroll;
  - US5 Lucky;
  - US6 Great Weapon Fighting.

---

## Phase 1: Setup

- [x] T001 Migration `crates/thunderforge-server/migrations/2026-10-07-120000-0000_roll_facets/{up,down}.sql` as in data-model.md:
  - on `world_roll_records`: `actor_id`, `roll_kind`, `check_id`, `facets`, `reroll_of` (partial unique index), `reroll_spent`, and the two checks;
  - `world_attacks.reroll_of` (partial unique index) and `world_attacks_to_hit_roll_id_idx`;
  - `world_items.properties`.

  Run it, and update `crates/thunderforge-server/src/schema.rs`. It must sort after spec 082's `…110000…`.

- [x] T002 In `crates/thunderforge-server/src/models.rs`:
  - add the new fields to `RollRecord`, `NewRollRecord` (`:1808`), `AttackRecord` and the item model;
  - add a `NewRollRecord::plain(world_id, triggered_by, …)` constructor with the new fields defaulted;
  - move every `NewRollRecord { … }` literal to it: `graphql/mutations_roll.rs`, `combat/attack.rs` `roll_and_record` `:263`, and the tests that `cargo check --workspace --tests` names.
  - Run `cargo check --workspace`.
- [x] T003 [P] `packs/systems/dnd5e/server/Cargo.toml`: add a path dependency on `thunderforge-dice`, then `cargo check -p thunderforge-system-dnd5e`.

---

## Phase 2: Foundational (blocks every story)

### Dice crate: rewrite and replay

- [x] T004 [P] Tests `crates/thunderforge-dice/src/rewrite_tests.rs`:
  - an edit returning `None` everywhere gives the formula back byte for byte;
  - `1d20 + MODIFIER` becomes `2d20kh1 + MODIFIER` and `2d20kl1 + MODIFIER`;
  - `1d20r1` is accepted, and so is `2d20r1kh1` in canonical order;
  - every term of `2d6 + 1d8 + 3` gets `min3`;
  - only the first d20 term is touched in `1d20 + 1d20`;
  - `parse(print(ast)) == ast` for every modifier kind in `ast.rs`.
- [x] T005 `crates/thunderforge-dice/src/rewrite.rs`: `TermView`, `AddModifier`, `TermEdit` and `rewrite_dice_terms`, with a canonical printer for `Expr` (contracts/pack-roll-facets.md). Export it from `src/lib.rs`, and register the tests with `#[cfg(test)] #[path]`.
- [x] T006 [P] Tests `crates/thunderforge-dice/src/replay_tests.rs`, covering guarantees 1–5 of contracts/pack-roll-facets.md:
  - an identity replay with a panicking rng, over formulas covering keep, drop, `r`, `rr`, `x`, `xo`, `min` and `max`, and counting successes;
  - `RerollDie` under `2d20kh1`, where the keep moves;
  - `RerollDie` under `min3`;
  - a fresh face that is not rerolled under `r1`;
  - a reshape from `1d20 + 5` to `2d20kh1 + 5`;
  - a reshape from `2d20r1kh1` to `3d20r1kh1`, with a fresh 1 rerolled;
  - a misaligned reshape is an error;
  - `lowest_die` picks the first of tied lowest d20s and ignores d6s.
- [x] T007 `crates/thunderforge-dice/src/replay.rs`:
  - `Recorded`, `ReplayEdit`, `lowest_die` and `replay` (research R3);
  - an `EvalCtx` draw source that pops recorded chains term by term;
  - keep and clamp reapplied through the existing `apply_keep_drop` and `apply_clamp`. Make them `pub(crate)` in `eval.rs`, with no new logic there.

  Export from `src/lib.rs`.

- [x] T008 [P] `crates/thunderforge-dice/src/wasm.rs`: `replayRoll` and `lowestDie` (contracts/pack-roll-facets.md), with a host test calling the inner functions.

### Canvas core: the slot

- [x] T009 `crates/thunderforge-canvas-core/src/roll_facets.rs`:
  - the types in contracts/pack-roll-facets.md;
  - `pub roll_facets: Option<&'static RollFacets>` on `SystemContribution` (`system_contribution.rs`), set to `None` in `new`;
  - a test that `new` leaves it `None`.

  Fix every literal that `cargo check --workspace --tests` names.

### Server: the record and the shaping hook

- [x] T010 Tests `crates/thunderforge-server/src/rolls/facets_tests.rs`:
  - `shape_roll` for a system with no slot gives the formula untouched with no facets, and refuses `Advantage` with `This system does not roll with advantage.`;
  - a test-registered slot (`inventory::submit!` under `#[cfg(test)]`, id `test-facets`) is called with the input it should get;
  - `facet_labels` maps ids to labels and passes an unknown id through as its own label.
- [x] T011 `crates/thunderforge-server/src/rolls/facets.rs`: `shape_roll` and `facet_labels` (contracts/pack-roll-facets.md). Register it in `rolls/mod.rs`.
- [x] T012 `RollMeta { actor_id, roll_kind, check_id, facets, reroll_of, reroll_spent }`, threaded through internally:
  - `roll_and_settle` (`graphql/mutations_roll.rs:112`) gains a `meta: RollMeta` parameter and writes it to the record. `rollDice` passes `RollMeta::default()`, and its tests prove nothing about a free roll changes.
  - `roll_and_record` (`combat/attack.rs:247`) gains the same parameter.
  - **Spec 083 meet point**: if `DieOutcome.steps` exists by now, `replay` copies it and pushes `ChainStep::Reroll` for the new face, and T006 gains that case.
- [x] T013 [P] `crates/thunderforge-server/src/graphql/types_rolls.rs`:
  - `RollFacet { id, label }` and the `Advantage` enum;
  - `WorldRoll` gains `facets`, `reroll_of`, `rerolled_by`, `spent`, `reroll_offers` and `reroll_until`, filled in `from_row`. Offers and until are empty or null for now; US3 fills them.
  - `graphql/queries/roll.rs` `entries` batch-loads `rerolled_by` with one `reroll_of = ANY(ids)` query.

  Add a test in `graphql/queries/roll_feed_tests.rs` that a feed of two chained rolls shows both links.

**Checkpoint**: the crate can rewrite and replay, packs have a slot, every
roll record carries its meta, and `cargo test --workspace` is green with
nothing visible changed.

---

## Phase 3: US1: Advantage and disadvantage (P1, MVP)

### 5e pack

- [x] T014 [P] [US1] Tests `packs/systems/dnd5e/server/src/roll_facets_tests.rs`:
  - `shape` on a check and a to-hit with Normal gives `Ok(None)`;
  - Advantage gives `2d20kh1 + MODIFIER` with `["advantage"]`;
  - Disadvantage gives `2d20kl1 + MODIFIER`;
  - a formula with no d20 refuses with `This roll has no d20 to roll twice.`;
  - Damage with Normal is untouched.
- [x] T015 [US1] `packs/systems/dnd5e/server/src/roll_facets.rs`: `shape` for the choice, using `rewrite_dice_terms`, plus `RollFacets` with `labels` and a `reroll` that refuses everything for now. Register it in `packs/systems/dnd5e/server/src/lib.rs`, and in `registration_tests.rs` assert that 5e registers it.

### Server

- [x] T016 [US1] In `crates/thunderforge-server/src/graphql/mutations_roll_check_tests.rs`, `rollCheck` on a 5e world:
  - `advantage: ADVANTAGE` records `2d20kh1 + MODIFIER`, two d20 dice, `facets = {advantage}`, `actor_id`, `roll_kind = 'check'` and `check_id`;
  - the GraphQL default is `NORMAL`, and the formula is byte-identical to before;
  - a Roll for Shoes world refuses `ADVANTAGE`, and its `NORMAL` formula is unchanged (SC-005).
- [x] T017 [US1] `crates/thunderforge-server/src/graphql/mutations_roll_check.rs`:
  - `roll_check_impl` and the resolver (`:398`) take `advantage: Option<Advantage>`;
  - read the actor's sheet (the existing `actor_slots`, `:163`);
  - call `shape_roll` between `bindings_for_check` and `roll_and_settle`, and pass `RollMeta`.
- [x] T018 [US1] Tests in a new `crates/thunderforge-server/src/combat/attack_facets_tests.rs`, registered from `combat/mod.rs`:
  - `make_attack` with `advantage: Advantage` records a `2d20kh1 + …` to-hit with `roll_kind = 'to_hit'`, the attacker's `actor_id` and `["advantage"]`;
  - the hit or miss is judged on the kept die (seeded rng);
  - damage is untouched;
  - a lair's attack takes no actor and refuses advantage.
- [x] T019 [US1] `AttackRequest` (`combat/attack.rs:142`) gains `advantage`, and `AttackInput` (`graphql/types_attacks.rs`) gains `advantage: Option<Advantage>` mapped in `into_request`. `record_attack` (`:583`) shapes each part's to-hit with the attacker's sheet before `roll_and_record`, and passes `RollMeta`. Keep `attack.rs` under 900 lines: the sheet read goes in `rolls/facets.rs` as `sheet_of(conn, actor_id)`.

### Web

- [x] T020 [P] [US1] `apps/web/src/components/world/RollAdvantage/AdvantagePicker.tsx`, with a vitest test: a three-way segmented control with `data-testid="roll-advantage-picker"`. It offers Normal, Advantage and Disadvantage, and has an accessible name.
- [x] T021 [US1] Wire the picker up:
  - `apps/web/src/api/systemChecks.ts` `rollCheck` and `apps/web/src/api/attacks.ts` `makeAttack` take `advantage`;
  - `apps/web/src/components/world/PlayDock/CharacterRollButtons.tsx` renders the picker, but only when the world's system declares facets. Expose `rollsWithAdvantage: Boolean!` on the system checks query, true when a `roll_facets` slot is registered, and read it there.
  - The choice is passed to every check and attack, and goes back to Normal after each one is sent (FR-018).
  - `apps/web/src/pages/world/actor/ActorRollsPanel.tsx` gets the same, through `CharacterRollButtons`.
  - *As built*: `CharacterRollButtons` sends free `rollDice` formulas and hands attacks to the attack flow, so it rolls no server check. The picker is in the two places a d20 test reaches the server, which are the ones FR-018 names: `SystemChecksPanel` (the sheet page's checks, via `rollCheck`) and `AttackFlow` (via `makeAttack`). `rollsWithAdvantage(worldId)` is a root query beside `systemChecks`. A lair or a queued attack is not offered the choice.
- [x] T022 [US1] `apps/web/src/components/world/PlayDock/RollEntry.tsx`: tags from `facets` (`data-testid="roll-facet"`). `apps/web/src/hooks/useWorldRolls.ts` and `apps/web/src/api/roll.ts` fetch `facets { id label }`. Test the tags in `RollEntry`'s vitest file.

### Demo

- [ ] T023 [P] [US1] Tests `apps/demo/src/backend/handlers/facets.test.ts` and `dice.test.ts`:
  - `rollCheck` with `ADVANTAGE` and `DISADVANTAGE` records `2d20kh1 ± n` and `2d20kl1 ± n`, with facets, under `seedDice`;
  - `makeAttack` with advantage judges on the kept die.
- [ ] T024 [US1] Mirror the transform:
  - `apps/demo/src/backend/handlers/facets.ts` holds `shapeD20` and `shapeDamage`;
  - `apps/demo/src/backend/actors.ts` `rollCheck` (`:254`) and `handlers/combatAttacks.ts` `makeAttack` (`:412`) apply it;
  - `recordRoll` in `handlers/dice.ts` stores `actorId`, `rollKind`, `checkId` and `facets`, and returns them on the `WorldRoll` row.

### Proof

- [ ] T025 [US1] `apps/web/e2e/rolls-facets-advantage.spec.ts`, with a GM and a player (the `apps/web/e2e/fixtures/rolls.ts` helpers):
  - the player rolls Stealth with Advantage, and both chats show two d20s, one struck, and the Advantage tag;
  - the picker reads Normal again;
  - a Disadvantage attack's formula shows `kl1`.

  Run `pnpm e2e:rolls`.

**Checkpoint**: US1 works end to end on the server and in the demo.

---

## Phase 4: US2: Halfling Luck (P1)

- [ ] T026 [P] [US2] Tests in `packs/systems/dnd5e/server/src/validators_tests.rs`:
  - `trait_data.facets` accepts the three ids;
  - it refuses an unknown id and a duplicate;
  - `luck_points_used` accepts 0 and above, and refuses a negative or a fraction.
- [ ] T027 [US2] `packs/systems/dnd5e/server/src/validators.rs` `validate_trait_data` (`:502`) checks both fields. Declare them in `packs/systems/dnd5e/system.json` `data_types.trait_data` (data-model.md).
- [ ] T028 [P] [US2] Tests in `roll_facets_tests.rs`:
  - `halfling_luck` gives `1d20r1 + MODIFIER`, and `2d20r1kh1 + MODIFIER` with Advantage;
  - damage gets no `r1`;
  - a sheet without the facet is untouched.
- [ ] T029 [US2] `roll_facets.rs` `shape`: add `r1` for `halfling_luck` on d20 tests.
- [ ] T030 [US2] Server tests in `mutations_roll_check_tests.rs`: a halfling's check under a seeded rng whose first d20 is a 1 records `rolls [1, n]`, a total using `n` and `facets {halfling_luck}`. A plain character keeps the 1.
- [ ] T031 [US2] The sheet: `packs/systems/dnd5e/web/src/components/RollFacetsSection.tsx`, mounted from `ActorSheet.tsx` beside Heroic Inspiration (`:212`):
  - three checkboxes (`data-testid="roll-facet-<id>"`) written through `writeTraits`, for whoever may edit;
  - when Lucky is ticked, the Luck Points left (proficiency bonus minus `luck_points_used`, computed in `derived-data.ts`) and a **Reset** button that writes `luck_points_used: 0`.

  Test it with vitest beside the sheet's existing tests.

- [ ] T032 [US2] `RollEntry.tsx`: a die whose `rolls` has more than one value, or whose `finalValue` differs from its last roll, shows the rolled values struck beside the used one (FR-016). Test it in vitest.
- [ ] T033 [P] [US2] Demo: `facets.ts` adds `r1`, and `facets.test.ts` seeds a 1 and asserts the chain.
- [ ] T034 [US2] In `rolls-facets-advantage.spec.ts`, add a second test: tick Halfling Luck on the player's sheet, roll a check, and assert the formula contains `r1` and the Halfling Luck tag shows in both chats (research R13).

---

## Phase 5: US3: Heroic Inspiration buys a reroll (P1)

### 5e pack

- [ ] T035 [P] [US3] Tests in `roll_facets_tests.rs`:
  - `reroll("inspiration")` with `inspiration: true` gives `RerollLowest { sides: 20 }` and a sheet with it false;
  - it refuses when the flag is false, when the setting `inspiration` is false, and on a damage roll;
  - an unknown spend is refused.
- [ ] T036 [US3] `roll_facets.rs` `reroll` for `inspiration`, plus `spends`.

### Server

- [ ] T037 [US3] Tests `crates/thunderforge-server/src/rolls/reroll_tests.rs`:
  - the chain walk;
  - the window: accepted at 1:59 and refused at 2:01, with an injected clock;
  - the spend already used in the chain;
  - "already rerolled";
  - `offers_for(viewer, roll)`, which is empty for a non-maker, for a reroll's original and after the window.
- [ ] T038 [US3] `crates/thunderforge-server/src/rolls/reroll.rs`: `REROLL_WINDOW = 2 min`, `chain_of`, `may_reroll`, and `offers_for`, which calls the pack's `reroll` and discards the result. Fill `WorldRoll.reroll_offers` and `reroll_until` in `graphql/queries/roll.rs` `entry_for` (`:96`) for the viewer.
- [ ] T039 [US3] Tests `crates/thunderforge-server/src/graphql/mutations_reroll_tests.rs`:
  - an Inspiration reroll of a check, in one transaction:
    - `trait_data.inspiration` is false;
    - a new record has `reroll_of`, `reroll_spent = 'inspiration'`, facets plus `inspiration`, the same visibility, label, actor and check id;
    - only the lowest d20 is changed;
    - events 36 and 26 are recorded.
  - Advantage `[14, 6]` rerolls the 6.
  - A second reroll is refused, and nothing is spent.
  - Refusals:
    - another member's roll;
    - the GM on a player's roll;
    - a roll with no actor;
    - a free `rollDice` roll;
    - a damage roll;
    - the setting off;
    - paused;
    - outside the window (an injected clock);
    - lost Editor.
  - A GM's eyes roll rerolled stays GM's eyes, and `view_of` masks the reroll for another player.
  - **SC-003**: 100 concurrent `reroll_roll_impl` calls on one roll leave exactly one new record and one sheet write. Use `tokio::spawn` over the test pool.
  - A check on a test system with an adjudicator is judged again (research R14).
- [ ] T040 [US3] `crates/thunderforge-server/src/graphql/mutations_reroll.rs`: `reroll_roll_impl` and `RerollMutation::reroll_roll` (contracts/graphql-rolls-facets.md):
  - the refusals in order;
  - then, in one transaction:
    1. lock the roll row, then the sheet row;
    2. call the pack's `reroll`;
    3. validate the new `trait_data` through the pack's `trait_data` validator and write it;
    4. `ACTOR_SHEET_CHANGED` with the payload `mutations_actor_system_data.rs` writes;
    5. `replay`;
    6. insert;
    7. `judge_check`;
    8. `ROLL_MADE`.
  - A unique violation maps to `This roll has already been rerolled.`

  Register the mutation in the schema root.

- [ ] T041 [US3] Make `rerollRoll` gated in `crates/thunderforge-server/src/graphql/play_pause_surface_tables.rs`, beside `rollCheck` (`:316`). Run the pause surface tests.
- [ ] T042 [US3] `revealRoll` (`graphql/mutations_roll.rs:249`) reveals the whole chain and emits `ROLL_REVEALED` per roll. Test it in `mutations_roll_tests.rs`: revealing the reroll reveals the original, and the reverse.
- [ ] T043 [US3] Run `node scripts/check-graphql-contract.mjs --schema --fix` and commit the regenerated schema with this phase.

### Web

- [ ] T044 [P] [US3] `apps/web/src/components/world/PlayDock/RerollButtons.tsx`, with a vitest test:
  - one button per `rerollOffers` entry (`data-testid="roll-reroll-<id>"`, "Reroll (<label>)");
  - it is hidden once `rerollUntil` has passed (a timer);
  - it is disabled while in flight;
  - it shows the server's refusal sentence.

  `apps/web/src/api/roll.ts` gains `rerollRoll`.

- [ ] T045 [US3] `RollEntry.tsx`:
  - a roll with `rerolledBy` is struck through (`data-testid="roll-rerolled"`);
  - its replacement shows "Rerolled with <spent.label>" (`data-testid="roll-spent"`);
  - `RerollButtons` sits on the roller's own entry, in the chat and in the in-pane result.
  - `useWorldRolls.ts` refetches the original when a `ROLL_MADE` arrives for a roll with `rerollOf`, so its `rerolledBy` and offers update.

### Demo

- [ ] T046 [P] [US3] Tests in `apps/demo/src/backend/handlers/dice.test.ts`: `rerollRoll` with Inspiration through `replayRoll` (seeded), the sheet flag off, and each refusal from T039 that the demo can reach (another viewer, spent, setting off, window, a damage roll). Revealing a chain reveals both.
- [ ] T047 [US3] `apps/demo/src/backend/handlers/dice.ts`:
  - a `rerollRoll` mutation, with `facets.ts` `rerollPlan` for the rules;
  - `rerollOffers`, `rerollUntil`, `rerollOf` and `rerolledBy` on the row;
  - a chain reveal.

  `events.ts` needs no change.

### Proof

- [ ] T048 [US3] `apps/web/e2e/rolls-facets-inspiration.spec.ts`:
  1. The GM grants the player's character Inspiration on its sheet.
  2. The player rolls a check, and the GM's chat shows no Reroll button.
  3. The player clicks Reroll (Heroic Inspiration).
  4. Both chats show the first roll struck through and the new one marked "Heroic Inspiration".
  5. The sheet's Inspiration toggle is off.
  6. The button is gone.
  7. A direct `rerollRoll` on the same roll through the page's GraphQL helper is refused with "already been rerolled".

  Run `pnpm e2e:rolls`.

**Checkpoint**: a spendable facet works end to end. US4 and US5 reuse
`rerollRoll` unchanged.

---

## Phase 6: US5: The Lucky feat (P2)

- [ ] T049 [P] [US5] Tests in `roll_facets_tests.rs`:
  - `reroll("luck_point")` with `lucky` at level 5 and 2 points used gives `Reshape { "2d20kh1 + MODIFIER" }` and `luck_points_used: 3`;
  - it refuses at 3 used, without `lucky`, and on a roll whose facets include `disadvantage`;
  - from `2d20r1kh1 + MODIFIER` it gives `3d20r1kh1 + MODIFIER`;
  - an NPC's proficiency bonus comes from its challenge.
- [ ] T050 [US5] `roll_facets.rs` `reroll` for `luck_point`, using `rules::proficiency_bonus` and `proficiency_bonus_for_challenge`.
- [ ] T051 [US5] Server tests in `mutations_reroll_tests.rs`:
  - a Luck reroll adds one d20 and keeps the highest, keeping the original dice' chains (seeded);
  - `luck_points_used` goes up by one;
  - three rerolls on three rolls succeed and the fourth is refused;
  - Inspiration and then Luck on the new roll is allowed (US5.4);
  - Luck twice in a chain is refused;
  - Disadvantage is refused, and `rerollOffers` omits Luck there.
- [ ] T052 [US5] Nothing new in `mutations_reroll.rs`. If T051 needs a code change, it goes in the `Reshape` branch only: replay with `reshaped`, and store the new formula.
- [ ] T053 [P] [US5] Demo: `facets.ts` `rerollPlan` for `luck_point`, with the matching cases in `dice.test.ts`.
- [ ] T054 [US5] In `rolls-facets-inspiration.spec.ts`, add a second test:
  1. Tick Lucky.
  2. Roll.
  3. Reroll (Luck Point).
  4. The new formula shows `2d20kh1`, and the sheet's points left drop by one.
  5. A roll at Disadvantage offers no Luck button.

---

## Phase 7: US4: A missed attack can be rerolled (P2)

- [ ] T055 [US4] Move out of `record_attack` (`combat/attack.rs:648–790`), with no change in behaviour: the per-part block "roll damage on a hit → insert the offer → auto-apply → `OFFER_CHANGED`" becomes `settle_hit(conn, systems_dir, user_id, ctx, part, target, attack_id, flags, rng)` in a new `crates/thunderforge-server/src/combat/attack_hit.rs`. Existing `attack_tests.rs` and `parity_tests.rs` must stay green unchanged.
- [ ] T056 [P] [US4] Tests `crates/thunderforge-server/src/combat/attack_reroll_tests.rs`:
  - a seeded miss rerolled with Inspiration into a hit:
    - one new `world_attacks` row with `reroll_of`, the old `defence` (even after the target's AC is changed in between), and the same target, labels and distance;
    - one damage roll and one offer, or an auto-applied change;
    - events 29, 30, 36 and 26;
    - the action budget unchanged.
  - A reroll that misses too: no damage and no offer, and the resource is spent.
  - A hit is refused with `A hit cannot be rerolled.`
  - One part of a multiattack: only that part gets a new row, and the others stand.
  - A lair's to-hit has no actor and cannot be rerolled.
- [ ] T057 [US4] `crates/thunderforge-server/src/combat/attack_reroll.rs`: `reroll_attack(conn, …, roll, new_resolution)`, called from `mutations_reroll.rs` when `roll_kind = 'to_hit'`. It:
  - finds the attack by `to_hit_roll_id`;
  - refuses unless it missed;
  - judges with `judge(target.is_some(), attack.defence, total)`;
  - inserts the new row;
  - on a hit, rebuilds the part with `find_weapon` and `parts_of` (`combat/weapon.rs:140`, `:219`) from the row's `item_id` or `ability_id`, and calls `settle_hit`;
  - never calls `spend_for_attack` (research R7).
- [ ] T058 [P] [US4] The attack feed and the type: `graphql/types_attacks.rs` gains `rerollOf` on the attack, and `queries/attacks.rs` returns it. Regenerate the GraphQL contract.
- [ ] T059 [P] [US4] Demo: `combatAttacks.ts` handles a `to_hit` reroll the same way (a new attack row, the stored defence, damage, an offer). Test it in `combat.test.ts` with a seeded miss and then a hit.
- [ ] T060 [US4] `apps/web/e2e/rolls-facets-attack.spec.ts`:
  1. The GM sets a goblin's AC to 99.
  2. The player's character, with Inspiration, attacks it with a weapon from the dock sheet and misses.
  3. The player clicks Reroll on the to-hit.
  4. The chain shows in both chats, the reroll is a miss, no damage offer exists, and Inspiration is off.

---

## Phase 8: US6: Great Weapon Fighting (P3)

- [ ] T061 [P] [US6] Declare `itemProperties` in `packs/systems/dnd5e/system.json` (data-model.md). Tests `crates/thunderforge-server/src/combat/item_properties_tests.rs`:
  - `item_properties_for_system` reads 5e's nine;
  - a pack without the key gives none;
  - `setItemAttack` with `properties: ["two_handed"]` stores it, and an undeclared id is refused with the contract sentence.
- [ ] T062 [US6] `crates/thunderforge-server/src/combat/item_properties.rs`. In `types_attacks.rs`, `AttackFieldsInput` gains `properties: Option<Vec<String>>`. `set_attack_fields_impl` (`mutations_attacks.rs:197`) validates and writes it, for items only; an ability with properties is refused. Add the `systemItemProperties` query, and expose `properties` on the item's attack fields.
- [ ] T063 [P] [US6] Tests in `roll_facets_tests.rs`:
  - `great_weapon_fighting` with `melee` and `two_handed` gives `2d6min3 + STR` with `["great_weapon_fighting"]`;
  - not melee, or no `two_handed`, leaves it untouched;
  - `2d6 + 1d8` gets `min3` on both terms;
  - a d20 test is unaffected.
- [ ] T064 [US6] `roll_facets.rs` `shape` for damage. `Part` (`combat/weapon.rs:113` `part_from`) carries `properties` from the item. `record_attack` shapes the damage source with `melee`, which holds when the part has a reach and `Measured.distance` is within it (research R6), and passes `roll_kind = 'damage'`. `settle_hit` does the same for a reroll's damage.
- [ ] T065 [US6] Server test in `attack_facets_tests.rs`: a seeded `[1, 5]` greatsword hit records final values `[3, 5]` with the GWF tag. The same weapon at range, or a one-handed weapon, records `[1, 5]`.
- [ ] T066 [P] [US6] Web: `apps/web/src/pages/world/ability/AttackFieldsEditor.tsx` shows a checkbox per `systemItemProperties` entry (`data-testid="item-property-<id>"`) for an item, saved through `setItemAttack` (`apps/web/src/api/attacks.ts`). Test it with vitest.
- [ ] T067 [P] [US6] Demo: `facets.ts` `shapeDamage`, and the demo's items carry `properties`. Add a seeded `[1, 5]` → `[3, 5]` case in `combat.test.ts`.

---

## Phase 9: Polish and proof

- [ ] T068 [P] `docs/guides/rolls.md`: a "Facets and rerolls" section for players and GMs. It covers:
  - Advantage;
  - what the sheet's facets do;
  - the Reroll buttons and the two-minute window;
  - that a reroll keeps its visibility and a reveal shows the chain;
  - the GM marking a weapon Two-Handed.
- [ ] T069 [P] `docs/CONTRIBUTING.md`, in the Rolls section: the `roll_facets` slot, `rewrite_dice_terms` and `replay`, and the rule that shared code carries facet ids as opaque strings.
- [ ] T070 Run `make lint` (host and wasm32, plus the file-length check) and `pnpm verify`. Confirm `attack.rs` is smaller than 830 lines after T055.
- [ ] T071 Run `cargo test -p thunderforge-dice`, `cargo test -p thunderforge-system-dnd5e`, `make test-rust ARGS="-p thunderforge-server"`, `pnpm -F @thunderforge/demo test` and `pnpm -F @thunderforge/web test`. All must be green.
- [ ] T072 **Proof**: `pnpm e2e:rolls` is green, including `rolls-facets-advantage.spec.ts`, `rolls-facets-inspiration.spec.ts` and `rolls-facets-attack.spec.ts`, which the `rolls` slice owns by its `rolls-` prefix in `scripts/e2e/slices.json`. Then run every slice that `pnpm e2e:which --diff` names. The schema and a migration changed, so it prints FULL SUITE: run the slices it names instead. The owner has ruled out the full suite as a gate; never run `node ./scripts/e2e-parallel.mjs` on its own. Record each slice's result here.

---

## Dependencies

- Phase 1 comes before everything else. Phase 2 blocks every story.
- US1 (Phase 3) comes before US2, because both use `shape`, and before US3, because the record meta is set by `rollCheck`.
- US3 (Phase 5) comes before US5 and US4: both reuse `rerollRoll`.
- US4 needs T055, the `settle_hit` move. US6's damage shaping (T064) also runs through `settle_hit`, so T055 comes before T064.
- US6 is otherwise independent of US3 to US5, and can run after US1.
- T072 comes last.

## Parallel opportunities

- Phase 2:
  - T004 and T006 can run together (dice tests);
  - T008 can start once T007 is done;
  - T009 can run beside the dice work;
  - T013 can run beside T010 and T011.
- Inside each story, the pack tests, the web component and the demo tasks marked [P] touch different files.
- Once US3 lands, US5 (Phase 6) and US6 (Phase 8) can proceed side by side. US4 (Phase 7) can too, as long as T055 lands before T064.

## Implementation strategy

1. **MVP**: Phases 1–3, then Advantage on checks and attacks, server and
   demo, proven by `rolls-facets-advantage.spec.ts`.
2. Add Halfling Luck (Phase 4), the first passive facet.
3. Add Inspiration (Phase 5), the reroll path everything else reuses.
4. Then Lucky, attack rerolls and Great Weapon Fighting, each with its own
   proof.
5. Finish with Phase 9. The merge gate is `pnpm e2e:rolls` plus every slice
   `pnpm e2e:which --diff` names, never the full suite.
