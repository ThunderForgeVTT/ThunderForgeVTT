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

- [ ] T001 Enable the `bevy_state` feature in `src/engine/Cargo.toml`, then verify with `cargo check --target wasm32-unknown-unknown -p thunderforge_engine` and record the release bundle size delta (research R11)
- [ ] T002 [P] Write ADR for how a token survives a scene change (candidates A/B in research R2) in `docs/adrs/` and add its row to `docs/adrs/README.md`
- [ ] T003 [P] Write ADR for the actor imagery model — rows keyed by role, not two columns (research R4) in `docs/adrs/` and index it in `docs/adrs/README.md`
- [ ] T004 [P] Write ADR for presentational item price versus a system-owned economy (research R5) in `docs/adrs/` and index it in `docs/adrs/README.md`
- [ ] T005 [P] Correct the RxDB reference under Technology Constraints in `.specify/memory/constitution.md` — the world cache plus the engine/GraphQL bridge is now the sole sync mechanism
- [ ] T006 [P] Add a shared `createNpc` fixture in `apps/web/e2e/fixtures/world-cache.ts` (or a new `fixtures/content.ts`) and repoint `apps/web/e2e/world-compendium.spec.ts`, `apps/web/e2e/players-section.spec.ts` and `apps/web/e2e/actor-claim.spec.ts` at it, before any UI moves
- [ ] T007 Decide and record the supported browser matrix in `docs/` — a prerequisite for FR-042 and a product decision, not an engineering one (research R7)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The engine mode machine and the persistence every later story reads.

**⚠️ CRITICAL**: T008 and T009 block US9, US1 and US6.

- [ ] T008 Introduce the authoring-mode state machine as a Bevy state in `src/engine/src/plugins/` (select | walls | lights | shapes | tokens | interactions), registered in `src/engine/src/lib.rs` — the engine becomes the single authority for the active mode (FR-040a, contracts/engine-events.md)
- [ ] T009 Convert `apps/web/src/components/world/GmToolRail/GmToolRail.tsx` to *request* a mode change through the engine boundary rather than holding the active mode in React chrome (Constitution I)
- [ ] T010 [P] Add snapping rules — square and hex — as pure functions with unit tests in `crates/thunderforge-canvas-core/src/` (research R8; needed by both Place and wall drawing)
- [ ] T011 [P] Create the `world_actor_images` migration in `src/server/migrations/` with paired `up.sql`/`down.sql`, unique on (`actor_id`, `role`), `created_by`/`updated_by` provenance (data-model.md)
- [ ] T012 [P] Create the `world_item_prices` migration in `src/server/migrations/` with paired up/down, at most one price per item (data-model.md)
- [ ] T013 [P] Create the lore organisation migration in `src/server/migrations/` — `world_lore_entries.parent_id` self-FK plus a `world_lore_tags` table unique on (`lore_entry_id`, `tag`) (data-model.md)
- [ ] T014 Regenerate `src/server/src/schema.rs` from the new migrations and confirm the server builds with `cargo check -p thunderforge`

**Checkpoint**: Mode is engine-owned; schema is in place; snapping is testable natively.

---

## Phase 3: User Story 9 — The interface does not lie or misfire (Priority: P1)

**Goal**: Remove the four defects the playtest found.

**Independent Test**: Each defect has a direct reproduction; fixed when the reproduction no longer occurs.

- [ ] T015 [US9] Confirm the stray-marker cause before fixing — check whether the marker lands at the tool *button's* screen position rather than the pointer's last map position (research R6), and record the finding in `specs/031-playability/research.md`
- [ ] T016 [US9] Make entering an authoring mode inert on top of T008 so no tool switch places content, in `src/engine/src/plugins/` (FR-040, FR-040a)
- [ ] T017 [US9] Ensure a gesture in flight — a drag or a carried placement — cannot complete under a newly entered mode's rules, in `src/engine/src/plugins/` (spec edge case)
- [ ] T018 [P] [US9] Remove the overlapping loader on the play route so exactly one indicator shows: reconcile the route Suspense fallback in `apps/web/src/routes/AppRoutes.tsx` with `engine-load-indicator` in `apps/web/src/pages/world/WorldPage.tsx` (FR-041, research R9)
- [ ] T019 [P] [US9] Guard `apps/web/src/hooks/useActorSystemData.ts` so no query is issued without an identifier, removing the repeated `Failed to parse "UUID": invalid length: found 0` (FR-043)
- [ ] T020 [US9] Report unsupported client storage plainly instead of an empty cache — surface `CacheError::Unsupported` from `crates/thunderforge-cache-browser/` through to the diagnostics panel in `apps/web/src/components/` (FR-042, research R7, depends on T007)
- [ ] T021 [P] [US9] Add an e2e assertion that exactly one loading indicator is visible at any moment during world load, in `apps/web/e2e/engine-loading.spec.ts` (SC-007)
- [ ] T022 [P] [US9] Add an e2e assertion that switching between every *ordered* pair of tools places nothing, including text as the control case, in `apps/web/e2e/canvas-authoring.spec.ts` (SC-008)

**Checkpoint**: The four defects are gone and two of them have regression tests.

---

## Phase 4: User Story 1 — A GM runs a scene without leaving the table (Priority: P1) 🎯 MVP

**Goal**: View and Place from the play screen; selection that can be narrowed.

**Independent Test**: With a connected player, view a character, place a token and move it — the play view is never navigated away from.

- [ ] T023 [US1] Add the placement state machine (`idle → carrying → placed | cancelled`) as a Bevy plugin in `src/engine/src/plugins/placement.rs` and register it in `src/engine/src/lib.rs` (FR-004, research R11)
- [ ] T024 [US1] Render the carried token as provisional and follow the cursor, snapping via T010's rules, in `src/engine/src/plugins/placement.rs` (FR-006)
- [ ] T025 [US1] Implement cancel — Escape and the chosen pointer gesture — with `OnExit` guaranteeing no trace, in `src/engine/src/plugins/placement.rs` (FR-005)
- [ ] T026 [US1] Emit placement-confirmed and placement-cancelled reports across the engine boundary per `contracts/engine-events.md`, in `src/engine/src/`
- [ ] T027 [P] [US1] Add the selection-filter plugin in `src/engine/src/plugins/selection_filter.rs`, defaulting to every kind enabled, registered in `src/engine/src/lib.rs` (FR-008)
- [ ] T028 [P] [US1] Build the collapsible Select filter menu in `apps/web/src/components/world/GmToolRail/GmToolRail.tsx`, occupying no map space when collapsed (FR-010)
- [ ] T029 [US1] Persist filter choices and collapsed state per user per device in `apps/web/src/` (FR-009, research R10)
- [ ] T030 [US1] Add View and Place actions to `apps/web/src/components/world/PlayDock/ActorsPanel.tsx`, replacing the link that navigates away from play (FR-001)
- [ ] T031 [US1] Open the actor in a new browser tab for a GM from `apps/web/src/components/world/PlayDock/ActorsPanel.tsx` (FR-002, GM half)
- [ ] T032 [US1] Wire Place to begin an engine placement and, on confirmation, call the token-create mutation subject to existing ownership rules, in `apps/web/src/` (FR-004, FR-007, contracts/graphql-mutations.md)
- [ ] T033 [US1] Make an inert selection state legible when every kind is filtered out, in `apps/web/src/components/world/GmToolRail/GmToolRail.tsx` (spec edge case)
- [ ] T034 [P] [US1] Add e2e coverage for place-then-cancel leaving nothing, and for a filtered Select ignoring walls, in `apps/web/e2e/canvas-authoring.spec.ts`

**Checkpoint**: A GM can place and select without leaving the table. **MVP.**

---

## Phase 5: User Story 2 — A player uses their character during play (Priority: P1)

**Goal**: The player's own character, in the pane, able to roll.

**Independent Test**: As a connected non-GM, open your character and roll — the play view stays mounted.

- [ ] T035 [US2] Render the active game system's character sheet in a compact in-pane presentation in `apps/web/src/components/world/PlayDock/`, reusing `SYSTEM_ACTOR_SHEETS` rather than adding a second sheet (FR-002 player half)
- [ ] T036 [US2] Trigger stat and ability rolls from the in-pane view through the existing dice path so results reach the table identically, in `apps/web/src/components/world/PlayDock/` (FR-003)
- [ ] T037 [US2] Dismiss the in-pane view back to the pane's previous content with the map still live, in `apps/web/src/components/world/PlayDock/`
- [ ] T038 [US2] Handle a game system that supplies no sheet, per the spec edge case, in `apps/web/src/pages/world/actor/systemActorSheets.ts`

**Checkpoint**: A player can act from the table. Note: generality across systems is bounded by spec 032.

---

## Phase 6: User Story 3 — Things on the map can be interacted with (Priority: P2)

**Goal**: The first two placeable interactions — lore and items.

**Independent Test**: Place one lore marker and one item; open the lore, pick up the item, and confirm it leaves the map for exactly one inventory.

- [ ] T039 [US3] Add authoring for a placed lore marker using the existing `lore.open` effect, in `src/engine/src/plugins/interaction.rs` and the interactions authoring surface in `apps/web/src/` (FR-011)
- [ ] T040 [US3] Render the lore marker with `lucide-react`'s book icon and open the entry in a new browser tab on activation, in `apps/web/src/` (FR-012)
- [ ] T041 [US3] Contribute an `item.pickup` effect **from the item subsystem**, not the interaction core, in `src/engine/src/` — `scripts/verify.mjs` enforces this seam (FR-013, ADR-054, research R3)
- [ ] T042 [US3] Offer Pickup and View on activation of a placed item, in `apps/web/src/` (FR-014)
- [ ] T043 [US3] Implement the pickup mutation in `src/server/src/graphql/` — remove the scene token and create one inventory entry, all-or-nothing (FR-015, contracts/graphql-mutations.md)
- [ ] T044 [US3] Resolve concurrent pickup at the database boundary so exactly one player wins, reusing spec 017's claim-race pattern, in `src/server/src/graphql/` (FR-016)
- [ ] T045 [US3] Restore the token on a refused pickup, leaving map and inventories unchanged, in `apps/web/src/` and `src/engine/src/` (FR-017)
- [ ] T046 [US3] Report an interactive whose subsystem is absent as unavailable rather than dispatching into nothing, in `src/engine/src/plugins/interaction.rs` (ADR-054)
- [ ] T047 [P] [US3] Add e2e coverage that two simultaneous pickups yield exactly one inventory entry, in `apps/web/e2e/` (SC-006)

**Checkpoint**: A map is a place, not a picture.

---

## Phase 7: User Story 5 — Preparing a scene without revealing it (Priority: P2)

**Goal**: Separate Launch from Preload, and say which is which.

**Independent Test**: Preload changes nothing any player can see and leaves the GM on the list; Launch changes both.

- [ ] T048 [US5] Add Preload as a client-side warm of the chosen scene's content, changing no server state, in `apps/web/src/pages/world/scenes/` (FR-020, research R1)
- [ ] T049 [US5] Keep Launch as the only action that sets the table's scene and navigates into play, in `apps/web/src/pages/world/scenes/SceneDetailPage.tsx` (FR-021)
- [ ] T050 [US5] Build the scene action table showing each scene's description and its render from `scene_preview_images`, in `apps/web/src/pages/world/scenes/ScenesPage.tsx` (FR-023)
- [ ] T051 [US5] State the difference between Launch and Preload in the interface, in `apps/web/src/pages/world/scenes/` (FR-022)
- [ ] T052 [P] [US5] Add e2e coverage that a connected player observes no change when a GM Preloads, in `apps/web/e2e/` (SC-004)

**Checkpoint**: Preparation no longer reveals itself. **Validate research R1's assumption here** — it is the one most likely to be wrong.

---

## Phase 8: User Story 4 — Moving the party to a new scene (Priority: P2)

**Goal**: Scene change clears and loads, and the party can come along.

**Independent Test**: Change scene bringing the party; old content gone, new content present, party tokens carried.

**Depends on**: T002's ADR.

- [ ] T053 [US4] Add the scene-transition state machine (`ready → unloading → loading → ready`) as a Bevy plugin in `src/engine/src/plugins/scene_transition.rs`, registered in `src/engine/src/lib.rs` (FR-018, research R11)
- [ ] T054 [US4] Unload the previous scene's tokens, walls and lights on `OnEnter(unloading)` and load the new scene's on `OnEnter(loading)`, in `src/engine/src/plugins/scene_transition.rs` (FR-018)
- [ ] T055 [US4] Implement party retention per T002's ADR in `src/server/src/graphql/`, ensuring a character that already has a token in the destination gains no second one (FR-019, contracts/graphql-mutations.md)
- [ ] T056 [US4] Add the GM's bring-the-party choice to the scene-change surface in `apps/web/src/` (FR-019)
- [ ] T057 [P] [US4] Add the retention predicate with unit tests in `crates/thunderforge-canvas-core/src/`

**Checkpoint**: A session can move rooms without re-placing the party by hand.

---

## Phase 9: User Story 6 — Authoring a map quickly (Priority: P3)

**Goal**: Snapping, room and door primitives, canvas right-click.

**Independent Test**: Draw a four-walled room with a door in under thirty seconds; walls follow the grid, including hex.

- [ ] T058 [US6] Add a GM-facing grid-snapping setting defaulting to on, applying to walls and lights, in `apps/web/src/` and `src/engine/src/plugins/grid.rs` (FR-024)
- [ ] T059 [US6] Apply T010's snapping rules to wall and light placement, honouring the scene's grid type including hex, in `src/engine/src/plugins/` (FR-025)
- [ ] T060 [US6] Add room and door primitives selectable while drawing, in `src/engine/src/plugins/wall.rs` (FR-026)
- [ ] T061 [US6] Ensure doors created by the primitive are functional — open, close, lock — reusing the existing door effects, in `src/engine/src/plugins/wall.rs` (FR-027)
- [ ] T062 [US6] Add placement helper controls to the interactions authoring surface for the supported effect kinds, in `apps/web/src/` (FR-028)
- [ ] T063 [US6] Support canvas right-click and suppress the browser context menu on the canvas surface only, in `src/engine/src/` (FR-029, research R6 caution)

**Checkpoint**: A GM can build a map at speed.

---

## Phase 10: User Story 8 — Managing world content comfortably (Priority: P3)

**Goal**: The administration and compendium surfaces the playtest asked for.

**Independent Test**: Find a player among fifty and change their character binding in under fifteen seconds; create an NPC through the full editor; give an actor both images.

**Depends on**: T006 (fixture) must land before T068.

- [ ] T064 [P] [US8] Add persistent sidebar navigation between admin sections in `apps/web/src/pages/admin/` (FR-032)
- [ ] T065 [P] [US8] Convert the players table to searchable cards showing each player's bound character in `apps/web/src/pages/world/players/PlayersRoutePage.tsx` (FR-033)
- [ ] T066 [US8] Let a GM set a player's character binding from the players section in `apps/web/src/pages/world/players/PlayersRoutePage.tsx` and `src/server/src/graphql/` (FR-034)
- [ ] T067 [US8] Make the three writers of the claim relation agree — players section, `apps/web/src/pages/world/actor/ActorDetailPage.tsx`, and the player's own claim — so concurrent binding cannot double-claim (FR-034, contracts/graphql-mutations.md)
- [ ] T068 [US8] Move NPC and item creation to a dedicated editing page with explicit save and remove the inline forms from `apps/web/src/pages/world/compendium/NpcCompendiumTab.tsx` and `ItemCompendiumTab.tsx` (FR-035)
- [ ] T069 [US8] Implement actor imagery upload for portrait and token roles through the existing transcode/storage path in `src/server/src/graphql/`, mirroring `mutations_lore_images.rs` (FR-036, research R4)
- [ ] T070 [US8] Add the imagery upload UI to the actor edit page in `apps/web/src/pages/world/actor/` and display each role where it belongs (FR-036)
- [ ] T071 [P] [US8] Add item price recording and display, presentational only, in `src/server/src/graphql/` and `apps/web/src/pages/world/compendium/ItemCompendiumTab.tsx` (FR-037, research R5)
- [ ] T072 [P] [US8] Add the lore tree and tags — move, tag, and find by either — in `apps/web/src/pages/world/lore/` and `src/server/src/graphql/`, rejecting cycles at the data boundary (FR-038)
- [ ] T073 [US8] Allow creating or attaching an item or lore entry from an actor's screen without leaving it, in `apps/web/src/pages/world/actor/` (FR-039)

**Checkpoint**: Preparation and administration are comfortable at real world sizes.

---

## Phase 11: User Story 7 — Running combat the way this ruleset runs it (Priority: P3)

**Goal**: Selected tokens feed the roster. Turn structure by system.

**Independent Test**: Select three tokens and start combat; the roster is exactly those three.

**⚠️ FR-031 is blocked on spec 032** — the static `SYSTEM_ACTOR_SHEETS` registry cannot express system-supplied combat structure. FR-030 ships independently.

- [ ] T074 [US7] Offer the currently selected tokens as the combat roster in `apps/web/src/components/world/PlayDock/CombatPanel.tsx` (FR-030)
- [ ] T075 [US7] Keep round and turn presentation for systems that use rounds, unchanged, in `apps/web/src/components/world/PlayDock/CombatPanel.tsx`
- [ ] T076 [US7] **BLOCKED on spec 032** — make turn structure system-supplied so a ruleset without rounds shows no round counter (FR-031, SC-011)

**Checkpoint**: Combat starts from what the GM selected.

---

## Phase 12: Polish & Cross-Cutting Concerns

- [ ] T077 [P] Run the full quickstart manual validation in `specs/031-playability/quickstart.md` on a running dev instance (Constitution V — this feature came from a playtest, so hand-verification is not optional)
- [ ] T078 [P] Run `node scripts/e2e-parallel.mjs --shards=4` and confirm no regression against the pre-feature baseline
- [ ] T079 [P] Update `docs/adrs/README.md` index rows for every ADR added in Phase 1
- [ ] T080 Run `pnpm verify` (rustfmt, clippy, prettier, eslint) and fix what it reports **in the code this feature added**. Keep it to that: a repo-wide lint remediation folded into a feature phase buries the feature work, and every line then has to be read to be sure nothing behavioural slipped in. Wide passes get their own commit. `pnpm verify:fix` rewrites what can be rewritten mechanically
- [ ] T081 Confirm the release engine bundle size delta from T001 is acceptable and record it

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
- **Phase 11 (US7)**: T074–T075 independent; T076 blocked on spec 032.

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
