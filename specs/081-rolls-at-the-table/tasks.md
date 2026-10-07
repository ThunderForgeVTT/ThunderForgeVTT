# Tasks: Rolls at the Table

**Input**: [spec.md](./spec.md), [plan.md](./plan.md), [research.md](./research.md),
[data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: requested by the spec's Proof section: server tests, demo tests,
the `rolls` slice and the demo's two-tab e2e. Tests come before the code
they prove inside each phase.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an open task)
- **[Story]**: US1 everyone sees a roll · US2 sheet in another tab ·
  US3 GM's eyes · US4 GM only · US5 reveal · US6 demo across tabs

---

## Phase 1: Setup

- [x] T001 Migration `crates/thunderforge-server/migrations/2026-10-07-100000-0000_roll_visibility/{up,down}.sql`: `visibility` text not null default `'everyone'` with a check on the three values, `label` text with a length check of 80, `revealed_at` timestamptz, `revealed_by` uuid references `users(id)` on delete set null, the reveal check and the `(world_id, created_at DESC)` index if absent (data-model.md); run it; update `src/schema.rs`
- [x] T002 Add the four fields to `RollRecord` and `NewRollRecord` in `crates/thunderforge-server/src/models.rs`; fill `visibility: "everyone"`, `label: None` at every existing insert (`mutations_roll.rs`, `combat/attack.rs`, `mutations_reconcile_tests.rs`); `cargo check -p thunderforge-server`

---

## Phase 2: Foundational (blocks every story)

### One visibility rule

- [x] T003 Write `crates/thunderforge-server/src/rolls/visibility_tests.rs`: `may_roll` for each visibility × player / GM / admin (FR-007 table); `view_of` for each visibility × roller / GM / admin / other player, before and after a reveal (data-model.md table)
- [x] T004 Create `crates/thunderforge-server/src/rolls/{mod.rs,visibility.rs}`: `RollVisibility` (`everyone`, `gm_eyes`, `gm_only`, parse/as_str), `Viewer { user_id, is_gm, is_admin }`, `may_roll`, `view_of -> RollView { Whole, Masked, Hidden }` (research R2); register the module in `src/lib.rs`

### Events

- [x] T005 Add `EVENT_CODE_ROLL_MADE = 36` and `EVENT_CODE_ROLL_REVEALED = 37` to `crates/thunderforge-server/src/world_events.rs`, with a `roll_event_payload(roll_id, visibility) -> serde_json::Value` that builds exactly `{rollId, visibility}`; a test asserts the key set (contracts/graphql-rolls.md)

### Fetching a roll

- [x] T006 Write `crates/thunderforge-server/src/graphql/queries/roll_feed_tests.rs`: `worldRoll` for each visibility fetched as the roller, the GM, an admin and another player (whole / masked / null); a non-member gets nothing; `worldRolls` pages newest first with `before` and `limit`, applies the same rule and omits hidden rolls without leaving a hole in the page size
- [x] T007 Create `crates/thunderforge-server/src/graphql/types_rolls.rs`: `RollVisibility` GraphQL enum, `WorldRoll`, `MaskedRoll` (built by a constructor taking only id, roller name, created_at and visibility — research R5), union `WorldRollEntry`
- [x] T008 Add `world_roll` and `world_rolls` to `crates/thunderforge-server/src/graphql/queries/roll.rs` (member check, `view_of`, roller and revealer names joined from `users`); register them; regenerate `apps/thunderforge/schema.graphql` with `thunderforge-schema`

### Publishing a roll

- [x] T009 Extend `crates/thunderforge-server/src/graphql/mutations_roll_tests.rs` (or create it): `rollDice` records one code-36 event in the same transaction with the payload of T005; a refused roll (bad formula, refused visibility, long label) records no row and no event; `visibility` defaults to `everyone`
- [x] T010 `crates/thunderforge-server/src/graphql/mutations_roll.rs`: `RollDiceInput` gains `visibility` and `label`; `roll_dice_impl` checks `may_roll` against the caller's role, stores both, and calls `record_world_event` with code 36 inside the insert's transaction; `rollCheck` passes `everyone` and the check's name as label (`mutations_roll_check.rs`)

### The stream does not carry a GM only roll to a player

- [x] T011 Write tests in `crates/thunderforge-server/src/graphql/subscriptions_tests.rs` (or beside the existing subscription tests): a player's `worldEventsCreated` receives code 36 for `everyone` and `gm_eyes` and nothing for `gm_only`; the GM's receives all three; every other event code passes untouched
- [x] T012 Filter in `world_events_created` (`crates/thunderforge-server/src/graphql/subscriptions.rs`): for code 36 with payload visibility `gm_only`, ask `is_dm_of_world` (or admin) when the event arrives and skip it for anyone else (research R1)
- [x] T013 Same rule in `crates/thunderforge-server/src/graphql/queries/world_events_since.rs` as a SQL condition; extend its tests with a `gm_only` roll event caught up by a player (absent) and by the GM (present)

### The client's half

- [x] T014 [P] `apps/web/src/api/roll.ts`: `rollDice(worldId, formula, { bindings, visibility, label })`, `fetchWorldRoll`, `fetchWorldRolls`, `revealRoll`, and the `WorldRollEntry` TypeScript union; `node scripts/check-graphql-contract.mjs` passes
- [x] T015 [P] Write `apps/web/src/engine/world/sync/rolls.test.ts`: the animate decision of research R3 (live + whole + within 4 s → animate; masked, null, catch-up or late → not; a reveal animates once)
- [x] T016 Create `apps/web/src/engine/world/sync/rolls.ts`: `ROLL_MADE_EVENT_CODE = 36`, `ROLL_REVEALED_EVENT_CODE = 37`, `startRollSync({ worldId, animate, onRoll })` that fetches each roll event, decides per T015, calls `animate` (`triggerDiceRollAnimation`) and hands the entry to `onRoll`; wire codes 36/37 into `playPanels.ts` handlers and export from `sync/index.ts`

**Checkpoint**: a roll is published, fetched per role and hidden from the stream as the rules say; `cargo test -p thunderforge-server rolls roll_feed world_events_since` green.

---

## Phase 3: US1 — Everyone sees a roll (P1) 🎯 MVP

- [x] T017 [US1] E2E `apps/web/e2e/rolls-everyone.spec.ts`: GM and two players in three contexts on one world's play view; one player rolls `1d20` from the dice roller; within 1 s every board's dice overlay shows and every chat panel lists the roll with the same total (SC-001); a reload lists it once and does not animate it (catch-up)
- [x] T018 [US1] Start the roll sync in the play view where the engine mounts (beside the other `start*Sync` calls); pass `triggerDiceRollAnimation` as `animate`
- [x] T019 [P] [US1] Remove the local `triggerDiceRollAnimation` calls from `DiceRollerPanel.tsx`, `InPaneCharacterSheet.tsx` and `AttackFlow/AttackFlow.tsx`; each keeps showing its result from the mutation's answer (FR-013)
- [x] T020 [US1] `crates/thunderforge-server/src/combat/attack.rs::roll_and_record` records code 36 (`everyone`, the attack's name as label) for each roll it inserts; extend `combat/attack_tests.rs` (research R4)
- [x] T021 [P] [US1] Create `apps/web/src/hooks/useWorldRolls.ts`: first page from `fetchWorldRolls`, older pages on demand, entries added or replaced by id from the roll sync's `onRoll`, `refetch()`
- [x] T022 [US1] `apps/web/src/components/world/PlayDock/ChatPanel.tsx`: interleave roll entries with messages by time; a roll entry shows roller, label, formula, dice and total
- [x] T023 [US1] Run `dice-roll.spec.ts`, `combat-attack.spec.ts`, `chat-panel.spec.ts`, `roll-check.spec.ts`; fix what they report

**Checkpoint**: US1 complete; `rolls-everyone.spec.ts` green.

---

## Phase 4: US2 — The sheet in another tab rolls onto the board (P1)

- [x] T024 [US2] E2E `apps/web/e2e/rolls-sheet-tab.spec.ts`: one player, two tabs of one context — the play view and `/world/:id/actor/:actorId/view`; roll Stealth on the sheet page; the play tab's board animates it and its chat lists "Stealth" (SC-002); an attack button on the sheet page says to pick the target on the board
- [x] T025 [US2] `apps/web/src/pages/world/actor/ActorDetailPage.tsx`: a roll section built from `statRolls` / `abilityRolls` in `PlayDock/characterRolls.ts`, shown to anyone who may roll for the actor, calling `rollDice` with the roll's label; attack entries show "Pick the target on the board" instead of rolling (FR-014)
- [x] T026 [P] [US2] Lift the roll buttons from `InPaneCharacterSheet.tsx` into a shared `PlayDock/CharacterRollButtons.tsx` used by both sheets, if the two would otherwise duplicate more than the list

**Checkpoint**: US2 complete; `rolls-sheet-tab.spec.ts` green.

---

## Phase 5: US3 — A player rolls for the GM's eyes (P1)

- [x] T027 [US3] E2E `apps/web/e2e/rolls-gm-eyes.spec.ts`: GM, player A, player B. A rolls "GM's eyes": A and the GM see the number and the GM's board animates; B's panel shows `****` and B's board does not animate. A network capture on B (every WebSocket frame and every response body) contains none of the roll's formula, label or total (SC-003)
- [x] T028 [P] [US3] Create `apps/web/src/components/world/RollVisibility/{RollVisibilityPicker.tsx,storedChoice.ts}`: options by role (player: Everyone, GM's eyes; GM: Everyone, GM only), the choice in `localStorage` wrapped in try/catch (research R8); a Vitest for the stored choice
- [x] T029 [US3] Put the picker on `DiceRollerPanel.tsx`, `InPaneCharacterSheet.tsx` (or `CharacterRollButtons.tsx`) and the sheet page; pass the choice to `rollDice`
- [x] T030 [US3] Render `MaskedRoll` in `ChatPanel.tsx` as "<name> rolled for the GM: \*\*\*\*"; mark the roller's and GM's view of a `gm_eyes` roll "GM's eyes"

**Checkpoint**: US3 complete; `rolls-gm-eyes.spec.ts` green with the capture clean.

---

## Phase 6: US4 — The GM rolls behind the screen (P2)

- [x] T031 [US4] E2E `apps/web/e2e/rolls-gm-only.spec.ts`: GM rolls "GM only"; the GM's board animates and the feed shows it marked "GM only"; the player's feed, board and network capture show nothing — no `MaskedRoll`, no code-36 frame (SC-003, FR-005a); the player reloads and catch-up still shows nothing
- [x] T032 [US4] Mark `gm_only` entries in the GM's `ChatPanel.tsx`; confirm the picker's GM options from T028 reach `rollDice`

**Checkpoint**: US4 complete; `rolls-gm-only.spec.ts` green.

---

## Phase 7: US5 — The GM reveals (P2)

- [x] T033 [US5] Server tests in `mutations_roll_tests.rs`: `revealRoll` by the GM and by an admin sets `revealed_at`/`revealed_by` and records one code-37 event; a second reveal and a reveal of an `everyone` roll record nothing (FR-010); a player's reveal is refused; dice, total and `created_at` unchanged (FR-011)
- [x] T034 [US5] `revealRoll` in `crates/thunderforge-server/src/graphql/mutations_roll.rs`; regenerate the schema
- [x] T035 [US5] A "Reveal" button on the GM's `gm_eyes` and `gm_only` entries in `ChatPanel.tsx`; a revealed entry stays at its time and reads "revealed by <GM>"
- [x] T036 [US5] E2E `apps/web/e2e/rolls-reveal.spec.ts`: a `gm_eyes` and a `gm_only` roll revealed; every member's feed shows the whole roll and every board animates within 1 s (SC-004); a second reveal sends no frame to the other contexts

**Checkpoint**: US5 complete; `rolls-reveal.spec.ts` green.

---

## Phase 8: US6 — The demo across tabs (P2)

- [x] T037 [US6] Demo tests `apps/demo/src/backend/handlers/dice.test.ts`: the server's visibility cases (T003, T006, T033) against `handlers/dice.ts` as GM and as player
- [x] T038 [US6] `apps/demo/src/backend/handlers/dice.ts` and `events.ts`: visibility and label on `rollDice`, `worldRoll`, `worldRolls`, `revealRoll`, codes 36/37 in `EVENT`, a roll without `visibility` read as `everyone` (data-model.md); per-viewer delivery of a `gm_only` roll event
- [x] T039 [US6] Viewer per tab: move the GM / player switch from the saved world to `sessionStorage` (`apps/demo/src/backend/state.ts`, `actors.ts`, `DemoNotice.tsx`); a saved world's viewer is read once as the first tab's (research R7)
- [x] T040 [US6] Demo tests `apps/demo/src/backend/tabs.test.ts` with a fake lock and channel: a guest's request runs on the holder as the guest's viewer; events reach every tab filtered per viewer; when the holder goes, a guest takes over from the saved world and resends what was open; without locks or channels each tab stands alone
- [x] T041 [US6] Create `apps/demo/src/backend/tabs.ts` (contracts/demo-tabs.md) and route the demo's request entry and event subscription through it
- [x] T042 [US6] Demo E2E `apps/demo/e2e/rolls-across-tabs.spec.ts`: two tabs of one context; one switched to player; a roll in either animates on both boards; a `gm_only` roll from the GM tab does not reach the player tab; closing the first tab leaves the second working; after edits in both and a reload of both, one identical world (SC-005)

**Checkpoint**: US6 complete; `pnpm -F @thunderforge/demo test` and the demo's `rolls-across-tabs.spec.ts` green.

---

## Phase 9: Proof and polish

- [x] T043 Register the slice: `scripts/e2e/slices.json` entry `rolls` (`own: ["rolls-"]`, `standalone: "pnpm -F @thunderforge/demo test && pnpm -F @thunderforge/demo e2e rolls-across-tabs"`, neighbours from plan.md confirmed with `pnpm e2e:which --diff`, `paths` for every file touched); root `package.json` scripts `e2e:rolls`, `e2e:rolls:standalone`, `e2e:rolls:integration`; `node scripts/check-e2e-slices.mjs` passes
- [x] T044 `cargo fmt`, `cargo clippy -p thunderforge-server`, `pnpm -F web typecheck`, `pnpm -F @thunderforge/demo typecheck`, `node scripts/check-graphql-contract.mjs`
- [x] T045 `cargo test -p thunderforge-server` green
- [ ] T046 Run `pnpm e2e:rolls`; record the result here
- [x] T047 [P] User guide `docs/guides/rolls.md` (who sees what, GM's eyes, GM only, reveal, the sheet in another tab); CONTRIBUTING note on the roll events, the one visibility rule and the demo's tab holder

---

## Dependencies

- Phase 1 → Phase 2 → stories. Every story needs T004, T010 and T016.
- US2 needs US1's sync running in the play view (T018). US3 needs T012 only
  for its capture, and the feed from US1 (T022). US4 needs T012/T013 and
  the picker (T028). US5 needs the feed (T022). US6 needs the server's rules
  settled (T003, T033) to copy them.
- Order of delivery: US1 (MVP) → US3 → US2 → US4 → US5 → US6 → Phase 9.

## Parallel opportunities

- T014 and T015 beside the server work of Phase 2.
- T019 and T021 are separate files; T028 beside T024–T026.
- US6's T037–T040 beside US4 and US5 once T033 is written.

## Implementation strategy

MVP is US1: every roll published and animated on every board. US3 and US4
make hidden rolls safe before any of them ships; the SC-003 capture is the
proof that matters most. US6 brings the demo along last, since it copies
rules the server has already settled.
