---
description: "Task list for spec 103, Tokens Talk"
---

# Tasks: Tokens Talk

**Input**: Design documents from `specs/103-tokens-talk/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/graphql.md, contracts/engine.md, quickstart.md

**Tests**: Requested by the spec (Playwright e2e with two browsers) and by the constitution (VI: its own slice). The server and engine rules are TDD: the test comes first and must fail.

**Organization**: By user story; US1–US3 are P1, US4–US5 P2.

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup

- [ ] T001 Add the `talk` slice to scripts/e2e/slices.json (own prefix `tokens-talk-`; neighbours `chat-panel.spec.ts`, `rolls-everyone.spec.ts`, `rolls-gm-only.spec.ts`, `rolls-gm-eyes.spec.ts`, `rolls-reveal.spec.ts`, `dice-roll.spec.ts`, each with its seam per plan.md; `paths` per the plan's tree; `standalone`: `pnpm -F @thunderforge/demo e2e tokens-talk`). Add `e2e:talk`, `e2e:talk:standalone` and `e2e:talk:integration` to the root package.json, modelled on `e2e:rolls`. `node scripts/check-e2e-slices.mjs` must pass.
- [ ] T002 [P] Write the ADR docs/adrs/20261016-117-speech_is_drawn_by_the_engine.md from research.md R1–R2: the engine draws bubbles and emits an edge placement; the page owns only the right-edge stack. Add it to docs/adrs/README.md.

---

## Phase 2: Foundational (blocks every story)

### Server: the speaker columns and rule

- [ ] T003 Add migration crates/thunderforge-server/migrations/2026-10-16-120000-0000_speaker_token/{up,down}.sql. It adds `speaker_token_id UUID NULL REFERENCES tokens(id) ON DELETE SET NULL` and `speaker_label TEXT NULL` to `world_chat_messages` and `world_roll_records`; down drops them. Update crates/thunderforge-server/src/schema.rs and `ChatMessage`/`NewChatMessage`/`RollRecord` (and its insertable) in crates/thunderforge-server/src/models.rs.
- [ ] T004 Write the failing tests first in crates/thunderforge-server/src/graphql/speaker_tests.rs:
  - the GM may speak as any token of the world
  - a player may speak as a token `controllers::may_move` allows, and not as another's token, a token of another world, or a token off the given `sceneId`
  - a nonexistent id gives the same FORBIDDEN as a forbidden one
  - `speaker_label_for(viewer)` returns the real name to the GM, and "Someone" to a player when `npc_visibility::player_may_read_token_name` is false
- [ ] T005 Implement crates/thunderforge-server/src/graphql/speaker.rs, registered in graphql/mod.rs:
  - `resolve_speaker(conn, world_id, scene_id: Option<Uuid>, user_id, token_id) -> Result<(Uuid, String)>` returns the token id and its real name, or FORBIDDEN "You can't speak as that token". It uses `crate::combat::controllers::may_move` and `is_dm_of_world`.
  - `speaker_label_for(conn, is_gm, token_id, stored_label)` applies research R5.
  - T004 must pass.
- [ ] T006 Wire the chat side in crates/thunderforge-server/src/graphql/mutations_chat.rs:
  - `SendChatMessageInput.speakerTokenId: Option<Uuid>`
  - `send_chat_message_impl` calls `resolve_speaker` before the insert and stores both columns
  - `GraphQLChatMessage` gains `speaker_token_id` and `speaker_label`, the latter resolved per viewer with `speaker_label_for` in the query that lists messages
  - add tests to the existing chat tests for a stored speaker and a refused one
- [ ] T007 Wire the roll side:
  - `RollDiceInput.speakerTokenId` in crates/thunderforge-server/src/graphql/mutations_roll.rs (`roll_dice_impl` calls `resolve_speaker`)
  - output fields `speakerTokenId`/`speakerLabel` in crates/thunderforge-server/src/graphql/types_dice.rs and queries/roll.rs, with the label withheld per viewer
  - a test in mutations_roll_tests.rs
- [ ] T008 Regenerate apps/thunderforge/schema.graphql (the repo's schema export) and confirm the diff is only the spec 103 fields. Run `cargo check -p thunderforge-server` and `cargo test -p thunderforge-server speaker` on the checkout's own test DB.

### Client types and the demo backend

- [ ] T009 [P] Add `speakerTokenId?`/`speakerLabel?` in apps/web/src/api/chat.ts (the query fields, and `sendChatMessage(input)` accepts `speakerTokenId`) and apps/web/src/api/roll.ts (`rollDice` input). Add them to the `RollRecord` type in apps/web/src/types/roll.ts.
- [ ] T010 [P] Mirror the rule in apps/demo/src/backend/handlers.ts:
  - `sendChatMessage` and `rollDice` accept `speakerTokenId`
  - the same checks against the in-memory tokens (GM: any token of the world; player: tokens they may move, using the move rule the demo already uses), with the same FORBIDDEN code
  - store and return the label, withheld as "Someone" for a player view when the token's name is hidden
  - tests in apps/demo/src/backend/chat.test.ts
  - run `pnpm -F @thunderforge/demo test`

### Engine: SpeechPlugin

- [ ] T011 Write the failing tests first in crates/thunderforge-engine/src/plugins/speech_tests.rs (pattern: token_culling_tests.rs):
  - `say` on a visible, in-view token spawns one bubble child and emits `placement: over`
  - a second `say` on the same token replaces it
  - a token with `Visibility::Hidden` gets no bubble and emits `edge`
  - a token outside the view shrunk by `set_view_insets` emits `edge`
  - a bubble despawns after `ms`
  - `clear_speech` despawns all
  - a viewer change that hides a token despawns its bubble
- [ ] T012 Implement crates/thunderforge-engine/src/plugins/speech.rs per contracts/engine.md:
  - the `SpeechPlugin`
  - `Speech` and `ViewInsets` resources
  - the bubble: `Text2d` on a dark backing sprite, child of the token, upright, above the nameplate's z, at most ⅓ of the view's width, 3 lines then "…"
  - roll emphasis
  - `motion` flag
  - expiry
  - `speech_placed` through `sdk.rs::emit_event`
  Register it in plugins/mod.rs and app.rs.
- [ ] T013 Parse `say`, `clear_speech` and `set_view_insets` in crates/thunderforge-engine/src/sdk.rs `parse_command`, with tests beside the `set_rendering` test, and route them in app.rs. Export the wasm `speech_bubbles() -> String` modelled on `nameplate.rs::token_nameplates`. Run `cargo check -p thunderforge-engine --target wasm32-unknown-unknown` and `cargo test -p thunderforge-engine speech`. T011 must pass.
- [ ] T014 [P] Add the `speech_placed` event type and parser to packages/encounter/src/engineEvents.ts, in the `EngineEvent` union, with a unit test beside the existing parser tests. Expose `speech_bubbles` on the engine type the page uses (apps/web engine loader and apps/landing/src/engine/loadEngine.ts if typed there).

**Checkpoint**: The server stores and checks speakers, the engine draws and places bubbles, and the page can hear placements.

---

## Phase 3: User Story 1, a character says it on the map (P1) 🎯 MVP

**Goal**: A player's line rises over their character for every viewer who may see it, with the character's name, and fades after ~6 s.

**Independent Test**: Two contexts, a player and the GM. The player sends a line, and the GM's `speech_bubbles()` lists it over the player's token with the character's name; it is gone after ~6 s and still in the log.

- [ ] T015 [P] [US1] Write the e2e apps/web/e2e/tokens-talk-says.spec.ts: two browser contexts (GM, player) on one world and scene, using the existing world/actor/token fixtures. It covers US1 acceptance 1–3 and 5 via `speech_bubbles()` (bubble over the player's token, name, gone by ~7 s, a second line replaces the first, the bubble still listed after the GM pans) and FR-006/US1-4 (no character → edge float under the player's name).
- [ ] T016 [US1] Create apps/web/src/components/world/TokenTalk/useTalk.ts, the page hook:
  - keeps the ids seen at mount plus a 10 s freshness window (research R6)
  - hears chat arrivals through `startPlayPanelEventSync({ onChatMessage })` + `subscribeToWorldEvents(worldId)` and refetches via `getWorldChatMessages`
  - resolves the token on the current scene
  - sends `{ type: "say", id: "chat:<id>", tokenId, name, text, kind: "line", ms: TALK_FLOAT_MS, motion }` through the engine send, or pushes an edge float when there is no token
  - listens for `speech_placed` `edge` and moves that float to the stack
  - exports `TALK_FLOAT_MS = 6000`
  - unit-tests the freshness and seen-set logic in TokenTalk/__tests__/useTalk.test.ts
- [ ] T017 [US1] Create apps/web/src/components/world/TokenTalk/EdgeFloats.tsx:
  - a right-edge stack, newest at the bottom, max 4
  - each float a `<button data-testid="edge-float">` with name and text
  - dark backing, AA contrast
  - the container `aria-live="polite"`, never focusing
  - sits above spec 102's bottom row on the phone and above the dice tray on the desktop (it reads the same layout the phone board uses)
  - no transitions under `prefers-reduced-motion`
  Run impeccable detect on it.
- [ ] T018 [US1] Mount `useTalk` and `<EdgeFloats>` in apps/web/src/pages/world/WorldPage.tsx (phone and desktop boards). Send `set_view_insets` from apps/web/src/components/world/PhoneBoard/PhoneBoard.tsx (bottom row + open sheet height) and from the desktop dock layout (open dock width).
- [ ] T019 [US1] Send the default speaker from ChatPanel apps/web/src/components/world/PlayDock/ChatPanel.tsx: the player's own token on the scene, else none (FR-004; the picker comes in US5). Show the log line as "<token name> (<typist>)" when `speakerLabel` is set (FR-006). Update apps/web/e2e/chat-panel.spec.ts only where it asserts the author text (SC-007).
- [ ] T020 [US1] Run `pnpm e2e:talk:integration` for tokens-talk-says. Fix until green.

**Checkpoint**: US1 works alone. This is a demoable MVP.

---

## Phase 4: User Story 2, rolls show over the character (P1)

**Goal**: A roll floats over its character with name/type and result, never the formula. GM-only rolls float for the GM only, and reveals float on reveal.

**Independent Test**: The player rolls Perception from the sheet, and the GM sees "Perception <n>" over the token with no formula.

- [ ] T021 [P] [US2] Create apps/web/src/components/world/TokenTalk/floatText.ts: `rollFloatText(roll)` returns `"<label ?? roll_kind name> <result>"`, with a word result kept as the word and never the formula or dice. Unit-test each roll kind SC-002 lists (check, save, skill, attack, tray, coin) in TokenTalk/__tests__/floatText.test.ts.
- [ ] T022 [US2] Feed rolls into useTalk.ts:
  - listen to the `startRollSync` upsert (the source `useWorldRolls` uses)
  - floats only rolls this viewer can see, those new and fresh by `createdAt`, plus reveals fresh by `revealedAt` (id `roll:<id>:reveal`)
  - resolves the token per data-model.md "Roll → token resolution"
  - `kind: "roll"`
- [ ] T023 [US2] Have the dice tray pass the composer's speaker: apps/web/src/components/world/DiceRollerPanel/useDiceRoller.ts sends `speakerTokenId` (read from the speaker hook from T019, later US5's picker).
- [ ] T024 [P] [US2] Write the e2e apps/web/e2e/tokens-talk-rolls.spec.ts:
  - a sheet Perception roll floats for the GM with label and number, with no `d20` and no `+` in the bubble text
  - a tray roll floats over the speaker's token
  - a `GM_ONLY` roll never floats for the player (SC-004), then reveal → it floats for the player
- [ ] T025 [US2] Run `pnpm e2e:talk:integration` + `pnpm e2e:rolls:integration` (the neighbours). Fix until green.

---

## Phase 5: User Story 3, someone speaks out of view (P1)

**Goal**: Out-of-view or hidden tokens float at the right edge, never located; up to 4, without overlaps; tapping one goes to the token or opens chat.

**Independent Test**: Pan the token out of the GM's view, the player speaks, and an edge float appears with the name while `speech_bubbles()` is empty.

- [ ] T026 [US3] In EdgeFloats.tsx, handle a tap: when the float's token is one the viewer may see, centre the view on it (the existing camera focus command the board uses for a token); otherwise open chat (the phone sheet "chat" or the desktop dock chat section, through the callbacks WorldPage passes). Make the stack drop the oldest past 4 and lay floats out in a column with gaps, so none overlap each other or a control.
- [ ] T027 [P] [US3] Write the e2e apps/web/e2e/tokens-talk-edge.spec.ts:
  - out of view → edge float, no bubble
  - the GM speaks as a hidden goblin (set via GraphQL with `speakerTokenId`) → the player gets an edge float named "Someone" and no bubble (SC-003)
  - five quick lines → 4 floats with bounding boxes that do not intersect each other or the bottom controls (SC-006), at desktop and at 412×915
  - tapping an edge float for a visible token moves the view so the next line bubbles over it
- [ ] T028 [US3] Run `pnpm e2e:talk:integration`. Fix until green.

---

## Phase 6: User Story 4, opening chat quiets the board (P2)

**Goal**: Opening chat clears every float, and nothing floats while it is open or replays after.

**Independent Test**: Floats showing → open chat → zero floats; a line sent while open never floats.

- [ ] T029 [US4] In useTalk.ts, take a `chatOpen` flag from WorldPage (the phone sheet id `"chat"` from PhoneBoard, or the desktop dock's chat section being open). On rising edge send `{ type: "clear_speech" }` and empty the stack; while open, mark arrivals seen without floating them.
- [ ] T030 [P] [US4] Write the e2e apps/web/e2e/tokens-talk-chat-open.spec.ts: a bubble and an edge float showing → open chat → `speech_bubbles()` empty and no `edge-float`; a line while open → none; close → the next line floats (SC-005).
- [ ] T031 [US4] Run `pnpm e2e:talk:integration`. Fix until green.

---

## Phase 7: User Story 5, choosing who speaks (P2)

**Goal**: The composer shows "Speaking as" and lets a player pick their tokens or themselves, and the GM "Game Master" or any scene token. The choice is remembered per device.

**Independent Test**: The GM picks the goblin and sends, and the player sees the bubble over the goblin with its name.

- [ ] T032 [US5] Create apps/web/src/components/world/TokenTalk/SpeakerPicker.tsx and a `useSpeaker(worldId, sceneId, isGameMaster)` hook beside it:
  - persists to localStorage `tf.speaker.<worldId>` in try/catch
  - falls back per FR-004 when the stored token is gone or not controlled
  - player options are the scene tokens the move UI lets them move, plus "self"
  - GM options are "Game Master" plus every scene token
  - touch targets ≥44px on the phone
  Replace T019's default with it in ChatPanel.tsx and T023's in useDiceRoller.ts. Run impeccable detect.
- [ ] T033 [P] [US5] Write the e2e apps/web/e2e/tokens-talk-speaker.spec.ts:
  - player default = their character
  - the picker lists only their tokens + self
  - GM picks the goblin → the player sees the bubble over the goblin with its name
  - reload keeps the choice
  - a direct GraphQL send as the GM's token from the player → FORBIDDEN
- [ ] T034 [US5] Run `pnpm e2e:talk:integration` + `pnpm e2e:which` neighbours (chat, rolls). Fix until green.

---

## Phase 8: Polish & Cross-Cutting

- [ ] T035 [P] Write the demo standalone e2e apps/demo/e2e/tokens-talk.spec.ts: one person, the GM view and a player view (switching as the demo does), covering a line bubble over the character, a roll float without formula, an edge float when panned away, and chat open clearing floats, at phone size (412×915) and desktop. Run `pnpm e2e:talk:standalone`.
- [ ] T036 [P] Verify reduced motion: an e2e step with `reducedMotion: "reduce"` asserts edge floats have no transition and `say` carries `motion: false` (spy on the engine send in the demo).
- [ ] T037 Run `cargo check -p thunderforge-engine --target wasm32-unknown-unknown`, `make lint`, `pnpm -F @thunderforge/web typecheck`, and the demo build. Run impeccable detect on the TokenTalk files and fix its findings.
- [ ] T038 Proof: run `pnpm e2e:talk` alone with `--record-durations`, grep the log for ✘, then run the slices `pnpm e2e:which` names for the diff (expected: chat via rolls, phone). Record the results and wall time here.
- [ ] T039 Owner phone test (SC-008): serve the demo through the tunnel, and the owner sees their character talk and roll and tunes `TALK_FLOAT_MS` if needed. Nothing is pushed until they greenlight.

---

## Dependencies & Execution Order

- Phase 1 → Phase 2 (T003→T004→T005→T006/T007→T008; T009 and T010 can start once T003's fields are fixed; T011→T012→T013; T014 is parallel) → stories.
- US1 (Phase 3) is the MVP and is needed by US2–US4, since they extend useTalk/EdgeFloats. US2 and US3 can run in parallel after US1 (different concerns in useTalk: rolls vs taps/stack). US4 needs US1. US5 needs US1 and replaces T019/T023's default speaker.
- Polish after the stories you ship.
- Do not edit sources while an e2e run is in progress (the commit hook's lock).

## Parallel Opportunities

- Phase 2: the server (T003–T008), the engine (T011–T013) and the demo (T010) are independent tracks; T009 and T014 are small and parallel.
- Within stories, the e2e spec writing (T015, T024, T027, T030, T033) runs beside the implementation of the same story.
- After US1: US2 ∥ US3.

## Implementation Strategy

1. **MVP**: Phases 1–3. A line over a character, with edge fallback for no token. Show the owner on the phone.
2. Add US2 (rolls), then US3 (out of view, taps); both are P1 and complete the owner's ask.
3. Add US4 (chat quiets) and US5 (picker).
4. Polish, slice proof, owner greenlight. Commit as we go (signed), and push nothing before the greenlight.
