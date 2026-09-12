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

- [ ] T003 Add a movement-blocking crossing test to `crates/thunderforge-canvas-core/src/wall.rs`, beside `is_visible`: given two points and a `WallSet`, report whether the segment properly crosses a wall with `blocks_movement` or a closed door
- [ ] T004 [P] Unit-test that test in `crates/thunderforge-canvas-core/src/wall_tests.rs`: a crossing, a move alongside a wall, a move through an open door, a move through a `blocks_movement: false` wall, and a move through the point where two walls meet (which counts as crossing)
- [ ] T005 [P] Write `docs/adrs/<date>-0XX-server_side_movement_adjudication.md`: the server judges a player's move, the engine shows the stop, the geometry lives in one crate, and a Game Master is never judged (spec decision 1)
- [ ] T006 Verify the crate: `cargo test -p thunderforge-canvas-core`

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

- [ ] T024 [US2] Create `src/server/src/movement/mod.rs`: judge a move — `from`, `to`, optional path — against the scene's walls using the Phase 2 crossing test; return `allowed` or `refused` with the wall
- [ ] T025 [P] [US2] Unit-test that module in `src/server/src/movement/tests.rs`, including a path longer than the 64-segment bound being refused as malformed
- [ ] T026 [US2] Add the optional `path` argument to `moveOwnToken` in `src/server/src/graphql/mutations_tokens.rs` and judge every player move with it; refuse with "A wall is in the way"
- [ ] T027 [US2] Leave `updateToken` unjudged for a Game Master, and say so in a comment naming spec decision 1
- [ ] T028 [P] [US2] Send the path from `apps/web/src/engine/world/sync/tokens.ts`: the engine's committed route for a route move, and nothing for a drag, whose straight line the server infers
- [ ] T029 [US2] Apply a refusal in the web sync: put the token back where the server says it is, and surface the reason to the player who moved
- [ ] T030 [US2] Stop a keyboard step in `src/engine/src/systems/token_move.rs` before it is sent, using the same crossing test, and show the stop
- [ ] T031 [US2] Stop a drag the same way in `src/engine/src/systems/token.rs`, returning the token to where the drag began
- [ ] T032 [US2] Refuse to extend a planned route through a blocking wall in `src/engine/src/systems/token_move.rs`
- [ ] T033 [P] [US2] Engine tests for all three: step, drag and route
- [ ] T034 [US2] Turn the playtest's wall checks hard in `apps/web/playtest/dungeon-crawl.playtest.ts` (the aimed attempt, and the wander's crossings)
- [ ] T035 [P] [US2] Add `apps/web/e2e/token-movement-walls.spec.ts`: a crossing move sent straight to the server from a player's session is refused, naming a wall
- [ ] T036 [US2] Verify and prove: `cargo check -p thunderforge`, `cargo check --target wasm32-unknown-unknown -p thunderforge_engine`, `pnpm -F @thunderforge/web exec tsc --noEmit`, then `pnpm playtest --only=dungeon-crawl` clears `a wall that blocks movement should stop Aria at it` and `heroes went through walls that block movement`

## Phase 6 (US3, US4): The rules of sight and light, held

**Goal**: What the spec says about sight and light is checked, not assumed —
including the cases the crawl does not cover today.

**Independent test**: A player with no token sees the lit board; a token in
darkness with no wall between is hidden; the Game Master sees every token, and
one no player can see is marked for them.

- [ ] T037 [P] [US3] Add playtest steps for the uncovered cases in `apps/web/playtest/dungeon-crawl.playtest.ts`: a player with no token (FR-035), and a token hidden by darkness alone (FR-031)
- [ ] T038 [P] [US4] Add a playtest step for a carried light following its token as it moves (FR-042)
- [ ] T039 [US3] Mark for the Game Master a token that at least one player cannot see (FR-033), in `src/engine/src/systems/lighting.rs`, if it is not already what `Perceived::Dim` means for a Game Master — read first, then change only if needed
- [ ] T040 [US3] Verify and prove: engine check, `tsc --noEmit`, `pnpm playtest --only=dungeon-crawl` still green with the new hard checks

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
