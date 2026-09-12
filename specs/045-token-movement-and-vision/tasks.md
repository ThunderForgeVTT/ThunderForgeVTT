---
description: "Task list for spec 045, token movement and vision"
---

# Tasks: Token Movement and Vision

**Input**: Design documents from `/specs/045-token-movement-and-vision/`

**Prerequisites**: plan.md, research.md, data-model.md,
contracts/movement-and-vision.md, quickstart.md

**Proof**: `pnpm playtest` is the acceptance test. Every phase ends by naming
the FINDINGs it clears. `pnpm verify` does **not** type-check the web app, so
every web phase runs `tsc --noEmit` of its own.

## Phase 1: Setup

- [ ] T001 Record the baseline: run `pnpm playtest --only=dungeon-crawl` and keep its report, so each phase can be compared against what the product did before it
- [ ] T002 [P] Read `specs/045-token-movement-and-vision/contracts/movement-and-vision.md` into the working set — it is the contract every phase below is measured against

## Phase 2: Foundational (blocks Phase 5)

- [X] T003 `movement_blocked_by` and `path_blocked_by` in `crates/thunderforge-canvas-core/src/wall.rs`, beside `is_visible` and sharing its `segments_intersect` and door rule, plus `WallSet::movement_blocking_walls`. They return **the wall**, not a bare `false`: the server has to name what refused the move and the engine has to draw the stop somewhere
- [X] T004 [P] Ten tests in `wall_tests.rs`, including the two that carry the design: a route is judged leg by leg (a there-and-back whose *endpoints* read as legal), and walking around a wall is allowed (whose straight line does not). Also: the joint where two walls meet, and a move that goes nowhere — a token standing on a wall would otherwise be frozen, because the touching rule counts its own position as a crossing
- [X] T005 [P] `docs/adrs/20260911-095-server_side_movement_adjudication.md`, PROPOSED. Both sides judge from one shared test, the server's answer counts, and a Game Master is never judged (spec decision 1). Five accepted ADRs were missing from `docs/adrs/README.md`; added with it
- [X] T006 `cargo test -p thunderforge_canvas_core`: 405 passed, 0 failed (the package id is underscored, not hyphenated)

## Phase 3 (US3): A door change reaches every board

**Goal**: A door designated, opened, closed, locked or revealed reaches every
client's board within a second — for sight, for light and for passage.

**Independent test**: Two browsers on one scene. The Game Master designates a
wall a door and opens it; the other browser shows it open without a reload.

- [X] T007 [US3] `announce_door` in `src/server/src/graphql/mutations_interactives_support.rs` records a wall-changed event (code 10) as well as the door-changed one, because a door lives on a wall row and code 10 is what clients re-read walls on
- [X] T008 [P] [US3] Every door change goes through that one announcer: designation, lock and secret already did; the direct activation path and the **approval** path now do too (`mutations_interactives.rs`). The approval path was missed at first and T009 is what caught it
- [X] T009 [P] [US3] `opening_a_door_announces_a_wall_change_too` in `src/server/src/graphql/mutations_interactives_tests.rs` — takes a high-water mark of world events before the activation, so it proves the *opening* announced both
- [X] T010 [US3] Confirmed: `apps/web/src/engine/world/sync/walls.ts` re-reads a scene's walls on code 10 and needed no change
- [X] T011 [US3] `apps/web/e2e/interactive-doors.spec.ts` no longer calls `loadWallsIntoStore` itself; it polls the store and so proves the change reached the page
- [X] T012 [US3] The playtest's door checks are hard in `apps/web/playtest/dungeon-crawl.playtest.ts`: the door is open on Aria's board, the Game Master's and Brom's
- [X] T013 [US3] Proved: 17 interactives server tests pass, and `pnpm playtest --only=dungeon-crawl` went from 7 findings to 2 in both game systems, with no hard failures — the five door-and-light findings are gone; what remains is the keyboard (Phase 4) and the wall (Phase 5)

## Phase 4 (US1): A player's keyboard moves their own token

**Goal**: A movement key moves the token a player owns, on gridded and
gridless scenes, and everyone sees it.

**Independent test**: A player presses D; their token is one cell east on every
client and on the server, and still there after a reload.

- [X] T014 [US1] `SetControlledToken` added to `payloads.rs` and `sdk.rs`, and a `set_controlled_token` wasm export in `systems/token_move.rs` — the export is what the web calls, since the web talks to the engine through exports and never through the command queue
- [X] T015 [US1] Applied by `reconcile_controlled_token`, which keeps exactly the named token tagged. A **resource reconciled every frame**, not a one-shot: the application names a token before the engine necessarily holds it, and the first version dropped the request in exactly that case — the playtest caught it moving nothing
- [X] T016 [US1] `setup_scene`'s placeholder no longer carries `PlayerControlled`
- [X] T017 [US1] The gridless branch emits its move
- [X] T018 [P] [US1] Superseded by the probe below and the playtest's hard check; an engine unit test of a `single_mut` query would not have caught either real cause
- [X] T019 [US1] `setControlledToken` wraps it in `apps/web/src/engine/bevy/index.ts` and `WorldPage.tsx` names the player's primary owned token beside the viewer token (`null` for a Game Master)
- [X] T020 [US1] **This is where the second cause was.** The engine announced a keyboard move as `update_token`, a shape no web module handles; a drag emits `upsert_token`. All three keyboard emissions now use one helper emitting that event with the whole transform. `pathCells` came out with it — the web dropped it anyway, and a route belongs in `moveOwnToken` in Phase 5
- [X] T020a [US1] Added `movement_state()` to the engine and `movementState` to `__engineProbe`, reported in the playtest's failure message: the token named, found, tagged, grid, runs, presses and the token's own x. It read `runs: 223, presses: 1, x: 7.5` against a store still on 0, which is what identified the cause
- [X] T021 [US1] The playtest's keyboard check is hard
- [X] T022 [P] [US1] `apps/web/e2e/token-keyboard-move.spec.ts` watches the whole round trip — the press, the server's stored position, then the Game Master's board — because either half of the fix can regress on its own. Run, and run again with control disabled to prove it fails without it: it does, reporting `{"named":null,"found":false,"tagged":0,"grid":true,"runs":174,"presses":0,"x":null}`
- [X] T023 [US1] Proved: engine checks clean, `tsc --noEmit` clean, and `pnpm playtest --only=dungeon-crawl` goes from 2 findings to **1** in both systems with no hard failures — sessions run 58s instead of stopping at 21s. Only the wall (Phase 5) remains

## Phase 5 (US2): A wall stops a hero

**Goal**: No player's move crosses a wall that blocks movement or a closed
door, whichever client asks, and the stop is shown before anything is sent.

**Independent test**: A player tries to cross by keyboard, by route and by
drag, and a script signed in as them sends the crossing move straight to the
server. The token ends every attempt on its own side.

- [X] T024 [US2] `src/server/src/movement/mod.rs`: `judge(from, to, path, walls)` returning `Allowed` or `Refused`. The path is **anchored** to the token's real position and the requested destination, so a short innocent route cannot be sent as cover for a move that crossed a wall
- [X] T025 [P] [US2] 12 tests in `src/server/src/movement/tests.rs`. Beyond the 64-point bound: a path carrying NaN or an infinity is refused, because NaN compares false against everything and would slip past the intersection test entirely rather than merely giving a strange answer
- [X] T026 [US2] `moveOwnToken` takes an optional `path` and is judged by `judge_against_walls`; refused with "A wall is in the way" and nothing more — a closed secret door stops a player like any wall (FR-019), and one test asserts the sentence names no door
- [X] T027 [US2] `updateToken` left unjudged, with a comment naming decision 1: it is the Game Master's path, and this is the rule rather than a gap to tighten later
- [X] T028 [P] [US2] The engine's committed route travels as `path` on `UpsertTokenCommand` — on the **command**, not on `WorldToken`, because a token does not *have* a path, it took one. A drag and a step send nothing, and the server judges the straight line, which is what they are
- [X] T029 [US2] `applyMoveRefusal` re-reads the server's position rather than restoring the client's memory of it — that memory is exactly what the refused move overwrote. Dispatched as `sync`, or it would bounce straight back out as another move and loop for as long as the wall is there. A transport failure is translated instead of shown raw
- [X] T030 [US2] `refuse_at_wall` in `token_move.rs`, on the step and the gridless nudge
- [X] T031 [US2] The drag returns the token to where it began. `DraggingToken` now remembers each token's `origin`, because by release the transform has already been written every frame of the drag and nothing else remembers. A Game Master's drag is not judged (FR-017)
- [X] T032 [US2] A route refuses to extend through a wall — judged **before** `get_or_insert_with` creates the plan. Writing it the obvious way left an empty plan behind every time a player pressed shift into a wall: a route that exists, has no steps, and was never started. Caught by T033
- [X] T033 [P] [US2] Six tests driving the real Bevy system in `token_move.rs`. The one that earns its place is `a_step_away_from_a_wall_still_moves` — a check wired backwards passes every "is it blocked" test and freezes the token in all four directions
- [X] T034 [US2] Both playtest wall checks are hard. The wander's is the stricter: the aimed attempt is one drag at a known wall, this is 12 rounds of random movement from wherever the heroes had got to
- [X] T034a [US2] Added `tryDrag` to the playtest fixture. `drag` insists the token move, because a press that grabbed nothing is the worst failure this harness can have — but a refused drag leaves the token exactly where it was, and the two are indistinguishable. The first run after the engine change failed **both scenarios** on this, and it was the wall working
- [X] T035 [P] [US2] `apps/web/e2e/token-movement-walls.spec.ts` sends the crossing move straight to the server from the player's session — no canvas, because a test that drags proves only that the engine works. Also asserts the Game Master's `updateToken` across the same wall still succeeds
- [X] T036 [US2] Proved: server and wasm engine check clean, `tsc --noEmit` clean, 1367 server tests and 213 engine tests pass, and `pnpm playtest --only=dungeon-crawl` reports **`0 wall crossing(s)`** in both game systems with no findings and no failures — down from 7 findings at the start of spec 045
- [X] T036a Mounted sonner's `<Toaster />` in `apps/web/src/App.tsx`. It had never been mounted anywhere, so `MissingPackNotice`'s warning about an uninstalled interface pack had never been seen by anyone. Spec 045 needed somewhere to say "A wall is in the way"; an audit of what else shouts into a void is queued separately

## Phase 6 (US3, US4): The rules of sight and light, held

**Goal**: What the spec says about sight and light is checked, not assumed —
including the cases the crawl does not cover today.

**Independent test**: A player with no token sees the lit board; a token in
darkness with no wall between is hidden; the Game Master sees every token, and
one no player can see is marked for them.

- [X] T037 [P] [US3] Two steps added. **A wraith** hidden by darkness alone (FR-031), placed past the torch's reach — inside it the step would pass without darkness doing anything. **Carl** joins mid-session with no token (FR-035), and the subject is a *lit* sentry behind the wall: unlit it would be hidden from Carl by darkness and from the players by the wall, and two rules reaching the same answer is not evidence about either. `joinLate` added to the fixture
- [X] T038 [P] [US4] Aria walks to the wraith in a dark scene and her lantern reveals it (FR-042). A light that stayed where it was placed would never reach it
- [X] T039 [US3] **It was not what `Perceived::Dim` meant.** The Game Master's branch computed *illumination at the token*, which is a different question that only sometimes agrees: a token in bright light behind a wall read as plainly visible though nobody could see it, and a token in the dark that every player had darkvision on read as unseen though everyone could. Both wrong, in opposite directions. Now computed against the party's eyes, named by the application through `set_party_eyes` — the engine is given ids because "whose token is this" is a question about accounts and world membership it has never known
- [X] T039a [US3] The early return had to move too: a lit scene with no lights skipped the whole pass, so a Game Master in daylight got no marks at all though walls still hid tokens from the table. Caught by the new engine test, which is why that test uses a bright scene
- [X] T039b [US3] `lighting.rs` went past the 1000-line limit; the viewer token, the party's eyes and the two probe mirrors moved to `systems/lighting_vision.rs` — they are the inputs and outputs of the pass rather than part of it
- [X] T039c [US3] Six engine tests, and a spec note: FR-033 says *at least one* player, so a **split party marks nearly everything**. Built as written and the test states the case plainly; whether a Game Master wants that much marking is a question for play, recorded in the spec rather than decided here
- [X] T040 [US3] Proved: 12 checks green, `tsc --noEmit` clean, 219 engine tests pass, and `pnpm playtest --only=dungeon-crawl` passes both game systems with **0 wall crossings and no soft checks left in the scenario** — every assertion in the crawl is now hard

## Phase 7 (US6): A game system says how a hero sees

**Goal**: D&D 5e's darkvision and carried-light reach come from the character,
declared by the pack (owner decision 2).

**Independent test**: In a dark 5e scene, a character with 60 feet of
darkvision is shown a token 50 feet away, dimly, and not one 70 feet away; a
character without it is shown neither.

- [ ] T041 [US6] Add the `vision` block to the manifest contract: `packs/systems/README.md` and the amendment note in `docs/adrs/20260504-027-game_system_packaging_and_manifest_contract.md`, per contracts/movement-and-vision.md §3
- [ ] T042 [P] [US6] Declare it in `packs/systems/dnd5e/system.json`: darkvision, and a carried light's bright and dim reach
- [ ] T043 [P] [US6] Validate the block in the pack's server crate, as its other data types are validated
- [ ] T044 [US6] Resolve a per-token vision profile server-side from the actor's system data and the pack's declaration, and expose it where the web reads tokens
- [ ] T045 [US6] Convert the system's units through the scene's grid (one cell is one of the system's squares), in one place, with a unit test
- [ ] T046 [US6] Call the existing `set_token_vision` from `apps/web/src/engine/bevy/index.ts` and the token sync, whenever a token's vision changes
- [ ] T047 [P] [US6] Add a playtest step to `apps/web/playtest/combat-5e.playtest.ts` for the independent test above
- [ ] T048 [US6] Verify and prove: `cargo check -p thunderforge`, engine check, `tsc --noEmit`, `pnpm playtest --only=combat-5e`

## Phase 8 (US7): A map that remembers

**Goal**: With exploration on, what a player's token has seen stays on their
map, in their browser, until they clear it or a Game Master resets it (owner
decision 3).

**Independent test**: A player walks through two of three rooms and reloads:
both rooms are shown faded, the third is not. The Game Master resets; the fog
goes, without the player doing anything.

- [ ] T049 [US7] Migration: `scenes.exploration_enabled` (default false) and `scenes.exploration_epoch` (default 0), plus `scene_exploration_resets(scene_id, user_id, epoch)` per data-model.md
- [ ] T050 [US7] Add `setSceneExploration` and `resetSceneExploration` (Game-Master-only) in the scenes mutations, and expose `explorationEnabled`, `explorationEpoch` and `myExplorationEpoch` on the scene query
- [ ] T051 [P] [US7] Server tests: a reset for everyone bumps the scene epoch; a reset for one player writes only their row; a player reads their own greater epoch
- [ ] T052 [US7] Create `src/engine/src/plugins/exploration.rs`: accumulate what the viewer's token can see into a coarse cell grid, draw the remembered area into `CanvasLayer::Fog`, and expose `explored_cells()`
- [ ] T053 [US7] Add `set_exploration` and `clear_exploration` to `src/engine/src/sdk.rs` and `payloads.rs`, per the contract
- [ ] T054 [P] [US7] Engine tests: a seen cell stays remembered after the token moves away; a cleared exploration draws nothing; disabled exploration accumulates nothing
- [ ] T055 [US7] Persist explored cells per `(user, world, scene)` in `apps/web/src/services/worldCacheStorage.ts`, with the epoch they were accumulated under
- [ ] T056 [US7] Load them on scene entry and hand them to the engine; drop them when the scene's (or the player's) epoch is newer
- [ ] T057 [P] [US7] Include explored areas in the storage panel's figures, so a player can see and clear what they hold
- [ ] T058 [US7] Give the Game Master the controls: turn exploration on or off for a scene, and reset it for one player or everyone
- [ ] T059 [P] [US7] Add the playtest step for the independent test above, in `apps/web/playtest/dungeon-crawl.playtest.ts`
- [ ] T060 [US7] Verify and prove: server check, engine check, `tsc --noEmit`, `pnpm playtest`

## Phase 9: Polish and cross-cutting

- [ ] T061 [P] Update `specs/045-token-movement-and-vision/spec.md`'s status to what shipped, naming the phases that landed
- [ ] T062 [P] Update the playtest's own comments where a FINDING became a hard check, so the file says what is true
- [ ] T063 [P] Re-measure: run a full `node scripts/e2e-parallel.mjs` and record the suite's time and result beside the 29.7 minutes of 2026-09-11
- [ ] T064 Run `pnpm verify` and `pnpm -F @thunderforge/web exec tsc --noEmit` before the final commit of the feature

## Dependencies

```text
Phase 1 (setup)
  └─ Phase 2 (crossing test, ADR) ──┐
Phase 3 (doors) ─────────────────── independent, ships first
Phase 4 (keyboard) ──────────────── independent of 3
Phase 5 (walls) ◀──────────────────┘ needs Phase 2; better after Phase 4
Phase 6 (sight and light held) ──── needs Phase 3 (doors reach boards)
Phase 7 (pack vision) ───────────── needs Phase 6's checks to be meaningful
Phase 8 (explored areas) ────────── needs Phase 6; heaviest, last
Phase 9 (polish)
```

## Parallel opportunities

- **Phase 2**: T004 and T005 alongside T003.
- **Phase 3**: T008 and T009 alongside T007.
- **Phase 4**: T018 and T022 alongside the engine work.
- **Phase 5**: T025, T028, T033 and T035 are separate files.
- **Phase 7**: T042 and T043 (the pack) alongside T044 (the server).
- **Phase 8**: T051, T054, T057 and T059 are separate files.

## Implementation strategy

**MVP is Phase 3.** A door that reaches the table is one server change, clears
five findings, and is worth shipping by itself.

Then Phase 4 (a player can walk), then Phase 5 (a wall means something) — after
which the crawl's every movement finding is a passing check, and the spec's
first three user stories are real.

Phases 6 to 8 add what the spec promises beyond the defects: the rules held
rather than assumed, a game system's own sight, and a map that remembers.
