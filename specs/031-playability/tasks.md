---

description: "Task list for Playability 001"
---

# Tasks: Playability 001 — From Demonstrable to Playable

**Input**: Design documents from `/specs/031-playability/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included selectively. Not TDD-by-default — these are the specific
regression tests `quickstart.md` names as "properties whose absence let the
current defects ship", plus native rule tests the engine crate cannot run
itself (its `#[cfg(test)]` modules compile under wasm32 and never execute).

**Organization**: Grouped by user story. Phase order follows plan.md's delivery
order rather than raw priority — `bevy_state` is a prerequisite, and the P1
defects are small and unblock trust in everything else.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to

## Path Conventions

Three surfaces, per plan.md: `src/engine/` (Bevy, wasm32 only),
`crates/thunderforge-canvas-core/` (native-testable rules), `src/server/`
(Axum + Diesel), `apps/web/` (React chrome + Playwright).

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Prerequisites and decisions that unblock everything else.

- [X] T001 Enable the `bevy_state` feature in `src/engine/Cargo.toml`, then verify with `cargo check --target wasm32-unknown-unknown -p thunderforge_engine` and record the release bundle size delta (research R11)
- [X] T002 [P] Write ADR for how a token survives a scene change (candidates A/B in research R2) in `docs/adrs/` and add its row to `docs/adrs/README.md`
- [X] T003 [P] Write ADR for the actor imagery model — rows keyed by role, not two columns (research R4) in `docs/adrs/` and index it in `docs/adrs/README.md`
- [X] T004 [P] Write ADR for presentational item price versus a system-owned economy (research R5) in `docs/adrs/` and index it in `docs/adrs/README.md`
- [X] T005 [P] Correct the RxDB reference under Technology Constraints in `.specify/memory/constitution.md` — the world cache plus the engine/GraphQL bridge is now the sole sync mechanism
- [X] T006 [P] Add a shared `createNpc` fixture in `apps/web/e2e/fixtures/world-cache.ts` (or a new `fixtures/content.ts`) and repoint `apps/web/e2e/world-compendium.spec.ts`, `apps/web/e2e/players-section.spec.ts` and `apps/web/e2e/actor-claim.spec.ts` at it, before any UI moves
- [X] T007 Decide and record the supported browser matrix in `docs/` — a prerequisite for FR-042 and a product decision, not an engineering one (research R7)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The engine mode machine and the persistence every later story reads.

**⚠️ CRITICAL**: T008 and T009 block US9, US1 and US6.

- [X] T008 Introduce the authoring-mode state machine as a Bevy state in `src/engine/src/plugins/` (select | walls | lights | shapes | tokens | interactions), registered in `src/engine/src/lib.rs` — the engine becomes the single authority for the active mode (FR-040a, contracts/engine-events.md)
- [X] T009 Convert `apps/web/src/components/world/GmToolRail/GmToolRail.tsx` to *request* a mode change through the engine boundary rather than holding the active mode in React chrome (Constitution I)
- [X] T010 [P] Add snapping rules — square and hex — as pure functions with unit tests in `crates/thunderforge-canvas-core/src/` (research R8; needed by both Place and wall drawing)
- [X] T011 [P] Create the `world_actor_images` migration in `src/server/migrations/` with paired `up.sql`/`down.sql`, unique on (`actor_id`, `role`), `created_by`/`updated_by` provenance (data-model.md)
- [X] T012 [P] Create the `world_item_prices` migration in `src/server/migrations/` with paired up/down, at most one price per item (data-model.md)
- [X] T013 [P] Create the lore organisation migration in `src/server/migrations/` — `world_lore_entries.parent_id` self-FK plus a `world_lore_tags` table unique on (`lore_entry_id`, `tag`) (data-model.md)
- [X] T014 Regenerate `src/server/src/schema.rs` from the new migrations and confirm the server builds with `cargo check -p thunderforge`

**Checkpoint**: Mode is engine-owned; schema is in place; snapping is testable natively.

---

## Phase 3: User Story 9 — The interface does not lie or misfire (Priority: P1)

**Goal**: Remove the four defects the playtest found.

**Independent Test**: Each defect has a direct reproduction; fixed when the reproduction no longer occurs.

- [X] T015 [US9] Confirm the stray-marker cause before fixing — check whether the marker lands at the tool *button's* screen position rather than the pointer's last map position (research R6), and record the finding in `specs/031-playability/research.md`
- [X] T016 [US9] Make entering an authoring mode inert on top of T008 so no tool switch places content, in `src/engine/src/plugins/` (FR-040, FR-040a)
- [X] T017 [US9] Ensure a gesture in flight — a drag or a carried placement — cannot complete under a newly entered mode's rules, in `src/engine/src/plugins/` (spec edge case)
- [X] T018 [P] [US9] Remove the overlapping loader on the play route so exactly one indicator shows: reconcile the route Suspense fallback in `apps/web/src/routes/AppRoutes.tsx` with `engine-load-indicator` in `apps/web/src/pages/world/WorldPage.tsx` (FR-041, research R9)
- [X] T019 [P] [US9] Guard `apps/web/src/hooks/useActorSystemData.ts` so no query is issued without an identifier, removing the repeated `Failed to parse "UUID": invalid length: found 0` (FR-043)
- [X] T020 [US9] Report unsupported client storage plainly instead of an empty cache — surface `CacheError::Unsupported` from `crates/thunderforge-cache-browser/` through to the diagnostics panel in `apps/web/src/components/` (FR-042, research R7, depends on T007)
- [X] T021 [P] [US9] Add an e2e assertion that exactly one loading indicator is visible at any moment during world load, in `apps/web/e2e/engine-loading.spec.ts` (SC-007)
- [X] T022 [P] [US9] Add an e2e assertion that switching between every *ordered* pair of tools places nothing, including text as the control case, in `apps/web/e2e/canvas-authoring.spec.ts` (SC-008)

**Checkpoint**: The four defects are gone and two of them have regression tests.

---

## Phase 4: User Story 1 — A GM runs a scene without leaving the table (Priority: P1) 🎯 MVP

**Goal**: View and Place from the play screen; selection that can be narrowed.

**Independent Test**: With a connected player, view a character, place a token and move it — the play view is never navigated away from.

- [X] T023 [US1] Add the placement state machine (`idle → carrying → placed | cancelled`) as a Bevy plugin in `src/engine/src/plugins/placement.rs` and register it in `src/engine/src/lib.rs` (FR-004, research R11)
- [X] T024 [US1] Render the carried token as provisional and follow the cursor, snapping via T010's rules, in `src/engine/src/plugins/placement.rs` (FR-006)
- [X] T025 [US1] Implement cancel — Escape and the chosen pointer gesture — with `OnExit` guaranteeing no trace, in `src/engine/src/plugins/placement.rs` (FR-005)
- [X] T026 [US1] Emit placement-confirmed and placement-cancelled reports across the engine boundary per `contracts/engine-events.md`, in `src/engine/src/`
- [X] T027 [P] [US1] Add the selection-filter plugin in `src/engine/src/plugins/selection_filter.rs`, defaulting to every kind enabled, registered in `src/engine/src/lib.rs` (FR-008)
- [X] T028 [P] [US1] Build the collapsible Select filter menu in `apps/web/src/components/world/GmToolRail/GmToolRail.tsx`, occupying no map space when collapsed (FR-010)
- [X] T029 [US1] Persist filter choices and collapsed state per user per device in `apps/web/src/` (FR-009, research R10)
- [X] T030 [US1] Add View and Place actions to `apps/web/src/components/world/PlayDock/ActorsPanel.tsx`, replacing the link that navigates away from play (FR-001)
- [X] T031 [US1] Open the actor in a new browser tab for a GM from `apps/web/src/components/world/PlayDock/ActorsPanel.tsx` (FR-002, GM half)
- [X] T032 [US1] Wire Place to begin an engine placement and, on confirmation, call the token-create mutation subject to existing ownership rules, in `apps/web/src/` (FR-004, FR-007, contracts/graphql-mutations.md)
- [X] T032a [US1] Declare tool use as a permission in the world's single permission declaration, defaulting to Game-Master-only so existing worlds are unchanged, in `src/server/src/` (FR-044, FR-045, ADR-050)
- [X] T032b [US1] Let a Game Master grant specific tools to a specific player from the world's settings, in `apps/web/src/pages/world/settings/` and `src/server/src/graphql/` (FR-046)
- [X] T032c [US1] Resolve tool permission on the client so an unavailable tool never appears in the rail, in `apps/web/src/pages/world/WorldPage.tsx` and `apps/web/src/components/world/GmToolRail/GmToolRail.tsx` (FR-047)
- [X] T032d [US1] Enforce tool permission engine-side so a directly issued mode request or canvas input is refused regardless of what chrome shows, in `src/engine/src/plugins/authoring_mode.rs` and the input systems (FR-047, SC-012)
- [X] T032e [US1] End a gesture in flight when the acting person's permission for that tool is revoked, and make the loss legible, in `src/engine/src/` (spec edge case)
- [X] T033 [US1] Make an inert selection state legible when every kind is filtered out, in `apps/web/src/components/world/GmToolRail/GmToolRail.tsx` (spec edge case)
- [X] T034 [P] [US1] Add e2e coverage for place-then-cancel leaving nothing, and for a filtered Select ignoring walls, in `apps/web/e2e/canvas-authoring.spec.ts`

**Checkpoint**: A GM can place and select without leaving the table. **MVP.**

---

## Phase 5: User Story 2 — A player uses their character during play (Priority: P1)

**Goal**: The player's own character, in the pane, able to roll.

**Independent Test**: As a connected non-GM, open your character and roll — the play view stays mounted.

- [X] T035 [US2] Render the active game system's character sheet in a compact in-pane presentation in `apps/web/src/components/world/PlayDock/`, reusing `SYSTEM_ACTOR_SHEETS` rather than adding a second sheet (FR-002 player half)
- [X] T036 [US2] Trigger stat and ability rolls from the in-pane view through the existing dice path so results reach the table identically, in `apps/web/src/components/world/PlayDock/` (FR-003)
- [X] T037 [US2] Dismiss the in-pane view back to the pane's previous content with the map still live, in `apps/web/src/components/world/PlayDock/`
- [X] T038 [US2] Handle a game system that supplies no sheet, per the spec edge case, in `apps/web/src/pages/world/actor/systemActorSheets.ts`

**Checkpoint**: A player can act from the table. Note: generality across systems is bounded by spec 032.

---

## Phase 6: User Story 3 — Things on the map can be interacted with (Priority: P2)

**Goal**: The first two placeable interactions — lore and items.

**Independent Test**: Place one lore marker and one item; open the lore, pick up the item, and confirm it leaves the map for exactly one inventory.

- [X] T039 [US3] Add authoring for a placed lore marker using the existing `lore.open` effect, in `src/engine/src/plugins/interaction.rs` and the interactions authoring surface in `apps/web/src/` (FR-011)
- [X] T040 [US3] Render the lore marker with `lucide-react`'s book icon and open the entry in a new browser tab on activation, in `apps/web/src/` (FR-012)
- [X] T041 [US3] Contribute an `item.pickup` effect **from the item subsystem**, not the interaction core, in `src/engine/src/` — `scripts/verify.mjs` enforces this seam (FR-013, ADR-054, research R3)
- [X] T042 [US3] Offer Pickup and View on activation of a placed item, in `apps/web/src/` (FR-014)
- [X] T043 [US3] Implement the pickup mutation in `src/server/src/graphql/` — remove the scene token and create one inventory entry, all-or-nothing (FR-015, contracts/graphql-mutations.md)
- [X] T044 [US3] Resolve concurrent pickup at the database boundary so exactly one player wins, reusing spec 017's claim-race pattern, in `src/server/src/graphql/` (FR-016)
- [X] T045 [US3] Restore the token on a refused pickup, leaving map and inventories unchanged, in `apps/web/src/` and `src/engine/src/` (FR-017)
- [X] T046 [US3] Report an interactive whose subsystem is absent as unavailable rather than dispatching into nothing, in `src/engine/src/plugins/interaction.rs` (ADR-054)
- [X] T047 [P] [US3] Add e2e coverage that two simultaneous pickups yield exactly one inventory entry, in `apps/web/e2e/` (SC-006)

**Checkpoint**: A map is a place, not a picture.

---

## Phase 7: User Story 5 — Preparing a scene without revealing it (Priority: P2)

**Goal**: Separate Launch from Preload, and say which is which.

**Independent Test**: Preload changes nothing any player can see and leaves the GM on the list; Launch changes both.

- [X] T048 [US5] Add Preload as a client-side warm of the chosen scene's content, changing no server state, in `apps/web/src/pages/world/scenes/` (FR-020, research R1)
- [X] T049 [US5] Keep Launch as the only action that sets the table's scene and navigates into play, in `apps/web/src/pages/world/scenes/SceneDetailPage.tsx` (FR-021)
- [X] T050 [US5] Build the scene action table showing each scene's description and its render from `scene_preview_images`, in `apps/web/src/pages/world/scenes/ScenesPage.tsx` (FR-023)
- [X] T051 [US5] State the difference between Launch and Preload in the interface, in `apps/web/src/pages/world/scenes/` (FR-022)
- [X] T052 [P] [US5] Add e2e coverage that a connected player observes no change when a GM Preloads, in `apps/web/e2e/` (SC-004)

**Checkpoint**: Preparation no longer reveals itself. **Validate research R1's assumption here** — it is the one most likely to be wrong.

---

## Phase 8: User Story 4 — Moving the party to a new scene (Priority: P2)

**Goal**: Scene change clears and loads, and the party can come along.

**Independent Test**: Change scene bringing the party; old content gone, new content present, party tokens carried.

**Depends on**: T002's ADR.

- [X] T053 [US4] Add the scene-transition state machine (`ready → unloading → loading → ready`) as a Bevy plugin in `src/engine/src/plugins/scene_transition.rs`, registered in `src/engine/src/lib.rs` (FR-018, research R11)
- [X] T054 [US4] Unload the previous scene's tokens, walls and lights on `OnEnter(unloading)` and load the new scene's on `OnEnter(loading)`, in `src/engine/src/plugins/scene_transition.rs` (FR-018)
- [X] T054a [US4] Drive the scene-transition machine from chrome — nothing calls `begin_scene_transition`/`complete_scene_transition`, so T053/T054 are registered and inert and FR-018 is not yet delivered, in `apps/web/src/pages/world/WorldPage.tsx` and `apps/web/src/engine/bevy/index.ts` (FR-018)
- [X] T055 [US4] Implement party retention per T002's ADR in `src/server/src/graphql/`, ensuring a character that already has a token in the destination gains no second one (FR-019, contracts/graphql-mutations.md)
- [X] T056 [US4] Add the GM's bring-the-party choice to the scene-change surface in `apps/web/src/` (FR-019)
- [X] T057 [P] [US4] Add the retention predicate with unit tests in `crates/thunderforge-canvas-core/src/`

**Checkpoint**: A session can move rooms without re-placing the party by hand.

---

## Phase 9: User Story 6 — Authoring a map quickly (Priority: P3)

**Goal**: Snapping, room and door primitives, canvas right-click.

**Independent Test**: Draw a four-walled room with a door in under thirty seconds; walls follow the grid, including hex.

- [X] T058 [US6] Add a GM-facing grid-snapping setting defaulting to on, applying to walls and lights, in `apps/web/src/` and `src/engine/src/plugins/grid.rs` (FR-024)
- [X] T059 [US6] Apply T010's snapping rules to wall and light placement, honouring the scene's grid type including hex, in `src/engine/src/plugins/` (FR-025)
- [X] T060 [US6] Add room and door primitives selectable while drawing, in `src/engine/src/plugins/wall.rs` (FR-026)
- [X] T061 [US6] Ensure doors created by the primitive are functional — open, close, lock — reusing the existing door effects, in `src/engine/src/plugins/wall.rs` (FR-027)
- [X] T062 [US6] Add placement helper controls to the interactions authoring surface for the supported effect kinds, in `apps/web/src/` (FR-028)
- [X] T063 [US6] Support canvas right-click and suppress the browser context menu on the canvas surface only, in `src/engine/src/` (FR-029, research R6 caution)

**Checkpoint**: A GM can build a map at speed.

---

## Phase 10: User Story 8 — Managing world content comfortably (Priority: P3)

**Goal**: The administration and compendium surfaces the playtest asked for.

**Independent Test**: Find a player among fifty and change their character binding in under fifteen seconds; create an NPC through the full editor; give an actor both images.

**Depends on**: T006 (fixture) must land before T068.

- [X] T064 [P] [US8] Add persistent sidebar navigation between admin sections in `apps/web/src/pages/admin/` (FR-032)
- [X] T065 [P] [US8] Convert the players table to searchable cards showing each player's bound character in `apps/web/src/pages/world/players/PlayersRoutePage.tsx` (FR-033)
- [X] T066 [US8] Let a GM set a player's character binding from the players section in `apps/web/src/pages/world/players/PlayersRoutePage.tsx` and `src/server/src/graphql/` (FR-034)
- [X] T067 [US8] Make the three writers of the claim relation agree — players section, `apps/web/src/pages/world/actor/ActorDetailPage.tsx`, and the player's own claim — so concurrent binding cannot double-claim (FR-034, contracts/graphql-mutations.md)
- [X] T068 [US8] Move NPC and item creation to a dedicated editing page with explicit save and remove the inline forms from `apps/web/src/pages/world/compendium/NpcCompendiumTab.tsx` and `ItemCompendiumTab.tsx` (FR-035)
- [X] T069 [US8] Implement actor imagery upload for portrait and token roles through the existing transcode/storage path in `src/server/src/graphql/`, mirroring `mutations_lore_images.rs` (FR-036, research R4)
- [X] T070 [US8] Add the imagery upload UI to the actor edit page in `apps/web/src/pages/world/actor/` and display each role where it belongs (FR-036)
- [X] T071 [P] [US8] Add item price recording and display, presentational only, in `src/server/src/graphql/` and `apps/web/src/pages/world/compendium/ItemCompendiumTab.tsx` (FR-037, research R5)
- [X] T072 [P] [US8] Add the lore tree and tags — move, tag, and find by either — in `apps/web/src/pages/world/lore/` and `src/server/src/graphql/`, rejecting cycles at the data boundary, and re-parenting a deleted entry's children to their grandparent rather than orphaning them (FR-038, data-model.md)
- [X] T073 [US8] Allow creating or attaching an item or lore entry from an actor's screen without leaving it, in `apps/web/src/pages/world/actor/` (FR-039)

**Checkpoint**: Preparation and administration are comfortable at real world sizes.

---

## Phase 11: User Story 7 — Running combat the way this ruleset runs it (Priority: P3)

**Goal**: Selected tokens feed the roster. Turn structure by system.

**Independent Test**: Select three tokens and start combat; the roster is exactly those three.

**⚠️ FR-031 is blocked on spec 032** — the static `SYSTEM_ACTOR_SHEETS` registry cannot express system-supplied combat structure. FR-030 ships independently.

- [X] T074 [US7] Offer the currently selected tokens as the combat roster in `apps/web/src/components/world/PlayDock/CombatPanel.tsx` (FR-030)
- [X] T075 [US7] Keep round and turn presentation for systems that use rounds, unchanged, in `apps/web/src/components/world/PlayDock/CombatPanel.tsx`
- [X] T076 [US7] Turn structure is system-supplied: a ruleset without rounds presents no round counter (FR-031, SC-011). Never actually needed pack code — `turnStructure` is a manifest block in the shape `abilities`/`resources`/`movement` already use, read server-side by `src/server/src/turn_structure.rs` and published as `combat.roundLabel`. Each bundled system declares from its own research digest's `action_economy`: rounds for 5e, Pathfinder, Cypher and Year Zero; **Exchange** for Fate, which is why the label travels with the flag; **none** for Blades in the Dark, whose digest records "no strict turn order or initiative". Verified live across four systems. No combat e2e exists in the suite to extend — recorded as a gap rather than papered over

**Checkpoint**: Combat starts from what the GM selected.

---

## Phase 12: Polish & Cross-Cutting Concerns

- [~] T077 [P] Run the full quickstart manual validation in `specs/031-playability/quickstart.md` on a running dev instance (Constitution V — this feature came from a playtest, so hand-verification is not optional) — **Deferred to the playtest pass (2026-09-03).** Constitution V still wants a person here; the decision is that hand-verification happens once, across the whole product, after the current spec list is wrapped — rather than gating each spec separately. Not done, not dropped, not blocking. See spec 032's tasks.md § *Manual passes, deferred to the playtest*.
- [X] T078 [P] Run `node scripts/e2e-parallel.mjs --shards=4` and confirm no regression against the pre-feature baseline
- [X] T079 [P] Update `docs/adrs/README.md` index rows for every ADR added in Phase 1
- [X] T080 Run `pnpm verify` (rustfmt, clippy, prettier, eslint) and fix what it reports **in the code this feature added**. Keep it to that: a repo-wide lint remediation folded into a feature phase buries the feature work, and every line then has to be read to be sure nothing behavioural slipped in. Wide passes get their own commit. `pnpm verify:fix` rewrites what can be rewritten mechanically
- [X] T081 Confirm the release engine bundle size delta from T001 is acceptable and record it

---

## Engine batches — pay the wasm rebuild once, not twenty times

A Rust change costs a release wasm rebuild (~7 minutes) before any end-to-end
run can see it; a TypeScript change costs nothing. Twenty of the remaining
tasks touch `src/engine/` or `crates/thunderforge-canvas-core/`, and verifying
them one at a time would spend over two hours rebuilding.

So the engine work is grouped into four batches. Each is a coherent unit that
can be written against `cargo check`, then verified in **one** run of the specs
named against it. The web, server and e2e tasks around them carry no rebuild
cost and can be done in the gaps while a batch builds.

**This is a scheduling structure, not a change to dependencies** — the phase
dependencies below still hold, and a batch must not start before its
prerequisites.

### Batch A — modes and gestures (after Phase 3's gating)

T017, T032d, T032e. Everything about a mode transition interrupting or refusing
work in progress. Small, and it completes the story T016 began.

**Verify with**: `canvas-authoring`, `map-editor-tooling`, `token-authoring`.

### Batch B — placement and selection (US1, the MVP)

T023, T024, T025, T026, T027, plus T059 (apply snapping to wall and light
placement) since it is the same input path and the same rebuild.

The largest batch and the one with the most new surface. T063 (canvas
right-click) joins it because research R6 warns that context-menu suppression
must not deepen the input-routing problem — it wants the same eyes and the same
verification run as placement.

**Verify with**: `canvas-authoring`, `token-authoring`, plus the new placement
coverage from T034.

### Batch C — interactions on the map (US3)

T039, T041, T045, T046. The lore marker, the contributed `item.pickup` effect,
the removal of the picked-up token, and reporting an interactive whose
subsystem is absent.

**Divergence from the plan, as built**: no optimistic removal, and therefore no
restore path. The plan assumed the engine would delete the token immediately
and put it back on refusal. It does not: the token stays until the server's
answer arrives through the ordinary sync. FR-017 is then satisfied by
construction — a refused pickup leaves the map and every inventory untouched
because nothing was changed in anticipation — and the failure mode the restore
path would have had (an undo that runs during a scene change or a disconnect,
losing the token for good) does not exist. The cost is that a pickup takes a
round trip to look like it worked.

**Verify with**: the new US3 coverage (T047) and `interactive-approval`.

### Batch D — scene transition and wall primitives (US4, US6)

T053, T054, T057, T060, T061. The scene-transition state machine with its
unload/load hooks, the native retention predicate, and room/door primitives.

**Verify with**: `canvas-authoring`, `world-cache-isolated`, and the US4
coverage.

### Not in a batch

T058 (the snapping *setting*) is web-side and only needs the engine to already
honour snapping, which Batch B delivers. The remaining 48 tasks touch web,
server or e2e only and can be verified without a rebuild at all.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies. T007 gates T020; T002 gates Phase 8; T006 should precede T068.
- **Foundational (Phase 2)**: Depends on T001. **Blocks US9, US1, US6.**
- **Phase 3 (US9)**: Depends on T008/T009 for T016–T017; T018/T019 are independent and can start immediately.
- **Phase 4 (US1)**: Depends on T008 (mode) and T010 (snapping).
- **Phase 5 (US2)**: Depends only on Setup. Can run parallel to Phase 4.
- **Phase 6 (US3)**: Depends on Setup. Independent of Phases 4–5.
- **Phase 7 (US5)**: Independent.
- **Phase 8 (US4)**: Depends on T002's ADR and T053.
- **Phase 9 (US6)**: Depends on T010 and T008.
- **Phase 10 (US8)**: Depends on T011–T014 for T069–T072; T006 before T068.
- **Phase 11 (US7)**: T074–T075 independent; T076 unblocked 2026-09-03 (see its entry).

### Parallel Opportunities

- Phase 1: T002, T003, T004, T005, T006 all parallel (different files).
- Phase 2: T010, T011, T012, T013 all parallel; T014 after the migrations.
- Phase 3: T018, T019, T021, T022 parallel with each other and with T015–T017.
- Phase 4: T027 and T028 parallel; T034 parallel once behaviour exists.
- Phases 5, 6, 7 can proceed concurrently with different people.
- Phase 10: T064, T065, T071, T072 parallel.

---

## Implementation Strategy

### MVP scope

**Phases 1, 2 and 4** — Setup, Foundational, and User Story 1. That delivers a
GM who can place tokens and narrow selection without leaving the table, which is
the session itself.

Phase 3 (the defects) is small and worth folding into the MVP: two of the four
are single-file fixes, and the stray-marker fix falls out of the mode machine
the MVP already needs.

### Incremental delivery

1. Setup + Foundational → mode is engine-owned, schema in place
2. + US9 → the interface stops misfiring
3. + US1 → **MVP**, demo it
4. + US2 → players can act from the table
5. + US3 → the map becomes interactive
6. + US5, US4 → scene preparation and party movement
7. + US6, US8 → authoring speed and administration
8. + US7 (FR-030) → combat roster; FR-031 when spec 032 lands

### Notes

- Verify per crate, per Constitution V: `cargo check --target wasm32-unknown-unknown -p thunderforge_engine`, `cargo check -p thunderforge`, `tsc`, `vitest`.
- Rules that can be tested natively belong in `crates/thunderforge-canvas-core/` — the engine crate's own tests never execute.
- Commit per task or coherent group.
- Three ADRs must land **with** the feature, not after it (Constitution IV).
