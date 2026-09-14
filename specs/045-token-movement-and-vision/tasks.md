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

- [X] T001 Record the baseline: run `pnpm playtest --only=dungeon-crawl` and keep its report, so each phase can be compared against what the product did before it. **The baseline was not captured before the work**, and cannot be now. The closest before-state is the playtest suite's first commit, `adb02d0` (2026-09-11), whose crawl recorded seven FINDINGs per game system, and the owner's manual playtest of 2026-09-10 (`specs/031-playability/playtest-2026-09-10.md`, P9 and P11 on shading and imported walls, fixed by spec 031 before this spec began). **After-state, run 2026-09-14:** `pnpm playtest --only=dungeon-crawl` passes both game systems (genie 1.4 min, dnd5e 1.9 min), no `✘` in the log, `0 wall crossing(s)` in each twelve-round wander. Compared: *keyboard* — dead before (only the startup placeholder was `PlayerControlled`), a hard pass now; *walls* — the aimed drag went through in both systems, now refused with the token left where it began, and the wander crosses nothing; *doors* — the opened door stayed shut on Aria's, the Game Master's and Brom's boards, now open on all three, hard; *vision* — the wall already hid the goblin from players then, and daylight and a brazier through the door were blocked by the door defect and now pass, as do the steps added since (darkness alone hides a token, a player with no token sees the lit board, a carried lantern travels, the Game Master is marked what the party cannot see). **Not shown fixed by this run:** the crawl does not exercise a gridless keyboard step, a game system's carried light (T065), or darkvision's reach on a board (T066)
- [X] T002 [P] Read `specs/045-token-movement-and-vision/contracts/movement-and-vision.md` into the working set — it is the contract every phase below is measured against. **Checked against the shipped code on 2026-09-14, after the work rather than before it.** Met: `moveOwnToken`'s optional `path`, judged and anchored, refused with "A wall is in the way" before anything is written, over 64 points refused as malformed (`src/server/src/movement/mod.rs:29`, `graphql/mutations_tokens.rs:438`); `updateToken` unjudged, and the web sends a Game Master's moves there (`apps/web/src/engine/world/sync/tokens.ts:462-487`); every door change — designation, lock, secret, activation, approval — announces code 10 through `announce_door`; `set_controlled_token`; `set_token_vision` called by the product; the engine refusing first and the player told why, locally (`WorldPage.tsx:1091`) and on a server refusal (`tokens.ts:191`); explored areas never sent to the server. **Three shape changes** — exploration as its own query with `forUser` and non-`Scene` returns, exploration through wasm exports with no `clear_exploration` and no epoch, and the manifest's `slot`/`source` pair plus `unitsPerCell` — behave as contracted and are now written into the contract's §5. **One gap:** a carried light's reach is resolved by the server and read by the web, then dropped — only `darkvision` reaches the engine (`apps/web/src/engine/world/sync/tokenVision.ts:69-77`), so FR-061 and FR-064's carried light are not met. Opened as T065

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

- [X] T041 [US6] `packs/systems/README.md` gains a `vision` section, and ADR-027 a 2026-09-11 amendment. The amendment says plainly what this block only half closes: `unitsPerCell` lives inside `vision` because **no manifest has ever recorded what a grid square is worth** — `movement` declares "30" with the unit implicit, which works only because nothing converts it. When `movement` needs the same answer, lift it to the top level rather than declaring it twice
- [X] T042 [P] [US6] `packs/systems/dnd5e/system.json` declares darkvision and both carried-light reaches, in five-foot squares. A test reads the shipped manifest rather than a copy
- [X] T043 [P] [US6] Validated in `pack_system_spec`, which is where manifest validation lives — the pack server crates validate *actor data*, a different thing. Refuses what is silent at runtime: a square of no size, and a distance naming no field (a `source` of `""` reads nothing, for ever, without complaining)
- [X] T043a [P] The comment on `SystemResource` has claimed since spec 029 that "the two are kept honest by a test asserting the field names match". **No such test existed anywhere.** There are two now, one per mirror
- [X] T044 [US6] `src/server/src/vision_profiles.rs` resolves it, and `tokenVision(sceneId)` exposes it — a sibling query keyed by token id like `tokenAttributes`, because the inputs are one manifest and one read of every sheet in the scene. A token seeing by the default rules is omitted rather than returned as zeroes
- [X] T045 [US6] `GridUnits::cells` is the one place, with 16 crate tests. The scene's `grid_size` turns out to be **pixels** per cell, which is the only thing it has ever been despite reading like a measurement; `cells_to_world` is where cells become drawable
- [X] T046 [US6] `set_token_vision` reached from the token sync on scene load and from the event fan-out on a sheet change. Zero is sent for a token the server omitted, or a character who *lost* their darkvision would keep it until a reload
- [X] T046a [US6] **A sheet edit announced nothing at all**, so FR-067 had no channel to arrive on. Sheet changes now carry world-event code 26 — not the token code, for the reason phase 1 learned with doors: announcing a change on a channel describing something else reaches the wrong listeners. A sheet is not a token; a character may have no token, or several
- [X] T047 [P] [US6] `a dwarf's darkvision reaches further than a human's` in `combat-5e.playtest.ts`. Asked of the **engine**, not the server: a server answering correctly proves only that it can read its own manifest, and the point of the step is the chain — manifest, sheet, server, web, engine. Needed a new probe, `token_vision`, mirrored from the lighting pass itself so it cannot report a profile that arrived and is never consulted
- [X] T048 [US6] Proved: 12 checks green, `tsc --noEmit` clean, and the step passes — Aria's 60 feet arrive as 12 cells of world units, Brom's silent sheet gives him zero rather than anyone else's sight, and granting him darkvision mid-session reaches the board with no reload (FR-067). The scenario still reports its 8 pre-existing FINDINGs, which is what it is for and unchanged by this

## Phase 8 (US7): A map that remembers

**Goal**: With exploration on, what a player's token has seen stays on their
map, in their browser, until they clear it or a Game Master resets it (owner
decision 3).

**Independent test**: A player walks through two of three rooms and reloads:
both rooms are shown faded, the third is not. The Game Master resets; the fog
goes, without the player doing anything.

- [X] T049 [US7] Migration written and run; schema regenerated
- [X] T050 [US7] `setSceneExploration` and `resetSceneExploration`, plus `sceneExploration(sceneId)` as a **query of its own** rather than fields on the scene: the answer depends on who is asking, and a `SimpleObject` built from a row has no viewer. The greater of the two epochs is resolved server-side — a client that took the wrong one would keep a map it had been told to drop
- [X] T051 [P] [US7] Seven tests, one of which **found a real bug**: a reset for everyone reached everyone *except* the player reset most recently. Their row held 1, the scene incremented 0 to 1, they took the greater, and nothing moved. A reset for everyone now clears past the highest row and deletes the rows it subsumes
- [X] T052 [US7] `plugins/exploration.rs`, asking the same `is_visible` the lighting pass uses — a second notion of visibility would drift, and the first thing a player would notice is remembered ground showing through a wall
- [X] T053 [US7] `set_exploration`, `set_explored_cells` and `explored_cells` as wasm exports. A reset arrives as the **empty string**, not an absent call, so that "the Game Master cleared your map" and "the application has not spoken yet" cannot look alike
- [X] T054 [P] [US7] Twelve tests. Two had to be restructured for a reason worth keeping: a reset clears the map and the player's surroundings return **in the same frame**, because they are looking at them. Correct on a board, and it makes a reset unobservable in a test that also accumulates — so those reconcile without accumulating, and a new test states the real behaviour
- [X] T055 [US7] `services/exploredAreas.ts`, in a **sibling IndexedDB database** rather than the world cache's. That schema is opened and versioned from Rust; adding a store means a version bump both sides must agree on, and a disagreement makes the world cache unopenable — which costs a player far more than their fog
- [X] T056 [US7] Loaded on scene entry, saved on a ten-second timer plus once on the way out, and dropped when the server's epoch is greater. A scene whose state cannot be read keeps exploration **on**: turning it off would show a player the whole map, the one outcome a Game Master who enabled it must never get by accident
- [X] T057 [P] [US7] In the storage panel, accounted separately from the world cache and worded to say why: a cached world can always be fetched again, and a map is the only record of where this player walked
- [X] T058 [US7] In the Lighting tool, beside the scene's light — the same question from the player's chair. Per-player resets are offered by token name, because that is the name on the board the Game Master is looking at
- [X] T059 [P] [US7] Proved by e2e instead, in `apps/web/e2e/scene-exploration.spec.ts`, and deliberately: the thing under test **is the browser**. The map has to live in real IndexedDB, survive a real reload, and be dropped when the epoch says so — none of which a playtest's probes reach. Both tests pass
- [X] T060 [US7] Proved: 12 checks green, `tsc --noEmit` clean, 231 engine tests, the exploration server tests, and both e2e tests pass against a real browser — the map survives a reload and a Game Master's reset reaches the player's own storage without the player doing anything

## Phase 9: Polish and cross-cutting

- [X] T061 [P] Update `specs/045-token-movement-and-vision/spec.md`'s status to what shipped, naming the phases that landed. Written phase by phase, with what was built and not proven named as such: the gridless step, SC-007 on a board, and exploration outside e2e
- [X] T062 [P] Update the playtest's own comments where a FINDING became a hard check, so the file says what is true. The crawl's header still described soft FINDING checks it no longer has; it now says the seven first-run FINDINGs are hard checks, and why two checks remain `expect.soft`. T040's "no soft checks left in the scenario" was not quite so — those two are soft, without FINDINGs, and a soft miss still fails the run. The inline comments at each former FINDING were already rewritten by the phases that fixed them
- [ ] T063 [P] Re-measure: run a full `node scripts/e2e-parallel.mjs` and record the suite's time and result beside the 29.7 minutes of 2026-09-11. Deliberately not run on 2026-09-14: the next full run on `main` serves this task
- [X] T064 Run `pnpm verify` and `pnpm -F @thunderforge/web exec tsc --noEmit` before the final commit of the feature. 2026-09-14: `pnpm verify` all 14 checks passed, `tsc --noEmit` clean
- [X] T065 [US6] A game system's carried light reaches no board. FR-061 and FR-064 require a system to set the bright and dim reach of a light a character carries; D&D 5e declares both (`packs/systems/dnd5e/system.json`, `vision.carriedLight`), `vision_from` resolves them and `tokenVision` returns `carriedBright`/`carriedDim` in world units (`src/server/src/graphql/queries/token_vision.rs:156-157`), and the web reads both and dispatches only `darkvision` (`apps/web/src/engine/world/sync/tokenVision.ts:69-77`). The engine's profile carries darkvision and nothing else (`crates/thunderforge-canvas-core/src/vision_declaration_tests.rs:171`). Needs a decision on what a carried light *is* in the engine — a light attached to the token, as the crawl's lantern is placed by a Game Master, or part of the token's profile — then a playtest step that watches it light a dark board **Decided and built 2026-09-14** (spec decision 6: a carried light is a light attached to its token). The web sends `set_carried_light { tokenId, bright, dim }` beside `set_token_vision`, zeros included (`apps/web/src/engine/world/sync/tokenVision.ts`); the engine keeps it in `LightSet` as `carried:<tokenId>` with `attached_token_id` set, the mechanism a Game Master's token-attached light already used, so it gets its own shadow-map row, lights every seat and follows its token (`src/engine/src/app.rs` `SetCarriedLight`, `LightSource::carried` in canvas-core). `remove_token` removes it; the Game Master's light tools leave it alone. **Evidence:** `e2e/carried-light.spec.ts` passes (3 of 4 runs; the other stalled at Brom's fifth keyboard step, before the torch was in question, and has not recurred): the torch reaches Aria's engine at 200/400 on Brom's token, lights the rat for her, the wall keeps the goblin dark by hidden tokens and by pixels, the light follows Brom's eight-step keyboard walk on her board and the old spot goes dark, and it goes out when the sheet drops it and when the token is deleted. `pnpm playtest --only=dungeon-crawl` passes both systems, with a hard dnd5e step where Brom's sheet torch lights a bat on Aria's board (Genie declares no carried light). Not exercised: a drag moving a carried light (the keyboard walk and remote moves go through the same token transform)
- [X] T066 [US6] SC-007 on a board: in a dark D&D 5e scene, a character with 60 feet of darkvision is shown a token 50 feet away dimly and not one 70 feet away, and a character without darkvision is shown neither. `combat-5e.playtest.ts` proves the sixty feet reach the engine; the range rule is proven only in `thunderforge-canvas-core` (`vision.rs`, `darkvision_reveals_the_dark_only_dimly_and_only_in_range`). Nothing has watched the two meet **Proven on a board 2026-09-14** by `e2e/darkvision-range.spec.ts`: a dark D&D 5e scene on a 50-unit grid, Aria's sheet says 60 feet and her engine receives 600; her board draws the goblin at 50 feet dimly (new probe `__engineProbe.dimTokens()`) and hides the orc at 70; Brom, with none, hides both. Passes

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
