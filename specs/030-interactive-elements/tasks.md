---
description: "Task list for 030-interactive-elements"
---

# Tasks: Interactive Elements — Props, Doors and Triggers

**Input**: Design documents from `/specs/030-interactive-elements/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included. The spec gives every story an Independent Test, and
quickstart.md defines seven layers. This project's standing preference is that
an end-to-end run is what moves a claim from theory to proven, so each story
ends in a Playwright spec rather than beginning and ending in unit tests.

**Organization**: Grouped by user story so each is independently implementable
and testable.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story the task serves
- Exact file paths are given in every task

## Path Conventions

Existing repository layout, per plan.md: shared rules in
`crates/thunderforge-canvas-core/`, engine plugins in `src/engine/src/plugins/`,
server in `src/server/src/`, web app in `apps/web/src/`, end-to-end specs in
`apps/web/e2e/`.

---

## Phase 1: Setup

**Purpose**: The decision record and the storage this feature adds.

- [X] T001 Write `docs/adrs/ADR-0NN-interaction-effect-contribution.md` recording the contribution seam — declarations in `thunderforge-canvas-core`, dispatch by Bevy event, and the three rejected alternatives from research §1 and §2. Constitution Principle IV requires this to land with the feature, not after it
- [X] T002 Create the Diesel migration for `interactives` in `src/server/migrations/` with paired `up.sql`/`down.sql`, per data-model.md, including `created_by`/`updated_by` provenance
- [X] T003 [P] Create the Diesel migration for `interaction_requests` in `src/server/migrations/` with paired `up.sql`/`down.sql`
- [X] T004 [P] Create the Diesel migration adding `locked` and `secret` to `walls` in `src/server/migrations/`, both `NOT NULL DEFAULT false` so every existing wall keeps its current behaviour
- [X] T005 Regenerate `src/server/src/schema.rs` from the migrations and confirm no unrelated table drifted

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The seam itself, plus the generic authoring and activation path.
Every story plugs into this; none can start before it.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

### The rules, where tests execute

- [X] T006 Create `crates/thunderforge-canvas-core/src/interaction.rs` with `EffectDeclaration` (id, label, description, subject_kinds, config), `SubjectKind`, `Trigger`, `Activation` and `FireMode`, all deriving ts-rs export to `apps/web/src/engine/sdk/`
- [X] T007 [P] Implement `EffectRegistry` assembly in `crates/thunderforge-canvas-core/src/interaction.rs`, taking contributed declaration sets and returning either a registry or a collision error
- [X] T008 [P] Test in `crates/thunderforge-canvas-core/src/interaction.rs` that two contributors declaring the same id fail at assembly rather than at first use — a collision found when a GM happens to author one of them is a collision found at the table (FR-042)
- [X] T009 [P] Test in `crates/thunderforge-canvas-core/src/interaction.rs` that an empty contribution set assembles successfully into an empty registry. A build with no subsystems must offer nothing, not fail (FR-039)
- [X] T010 Implement authoring validation in `crates/thunderforge-canvas-core/src/interaction.rs`: subject and geometry must agree with `subject_kind`, `Trigger::Enter` is valid only for a region, and effect config must validate against its declaration
- [X] T011 [P] Test the validation rules in `crates/thunderforge-canvas-core/src/interaction.rs`, including that a region carrying a subject reference and a door carrying none are both rejected rather than tolerated
- [X] T012 Implement activation resolution in `crates/thunderforge-canvas-core/src/interaction.rs` returning the tagged outcome from contracts/graphql.md — performed, requested, refused, unavailable, no-effect
- [X] T013 [P] Test activation resolution in `crates/thunderforge-canvas-core/src/interaction.rs` across every combination of activation mode, lock state, fire mode and viewer role. This is the truth table the server enforces; getting it right here is what makes the server a thin caller

### Doors as a shared rule

- [X] T014 Extend `crates/thunderforge-canvas-core/src/wall.rs` with `locked` and `secret`, and a function giving what a segment blocks from its door state and the wall's own blocking profile
- [X] T015 [P] Test the blocking rule in `crates/thunderforge-canvas-core/src/wall.rs`: open blocks neither; closed blocks exactly what the wall blocks, so a closed window stays see-through and a closed stone door does not (FR-008, FR-009)
- [X] T016 [P] Test in `crates/thunderforge-canvas-core/src/wall.rs` that lock is independent of state, so "open and not closeable by players" is expressible — the case a three-state model cannot represent (FR-010)

### Server: persistence, authorization, dispatch

- [X] T017 Create `src/server/src/interaction.rs` assembling the registry from contributing modules and exposing it for validation
- [X] T018 Create `src/server/src/graphql/queries/interactives.rs` with `effectRegistry` and `interactives(sceneId)`, returning the GM authoring view and the reduced player view described in contracts/graphql.md
- [X] T019 Create `src/server/src/graphql/mutations_interactives.rs` with `createInteractive`, `updateInteractive`, `deleteInteractive` and `resetInteractive`, all refused for non-GMs at the data boundary (FR-005, Principle III)
- [X] T020 Implement `activateInteractive` in `src/server/src/graphql/mutations_interactives.rs` returning the tagged outcome, enforcing lock, GM-only and fire-mode server-side
- [X] T021 [P] Test in `src/server/src/graphql/mutations_interactives.rs` that a player cannot create, edit, delete or reset an interactive
- [X] T022 [P] Test in `src/server/src/interaction.rs` that an interactive whose `effect_id` is absent from the registry resolves to unavailable rather than being dispatched or deleted (FR-041)
- [X] T023 Register the new query and mutation roots in `src/server/src/graphql.rs`
- [X] T024 Emit interactive and door changes on the existing `worldEventsCreated` subscription with their own event codes in `src/server/src/graphql/mutations_interactives.rs`, reusing the transport walls and token status already use (FR-020)

### Engine: the plugin that names no effect

- [X] T025 Create `src/engine/src/plugins/interaction.rs` with `InteractionPlugin` owning placement, hit-testing, permission resolution and `once` bookkeeping, and writing an activation event per contracts/engine-events.md
- [X] T026 Register `InteractionPlugin` in `src/engine/src/lib.rs` so it is independently addable and removable per Principle II
- [X] T027 Add the activation command and read surface to `src/engine/src/lib.rs`, exported through the typed SDK rather than hand-built JSON, following the pattern spec 029 established

### Web: authoring driven by the registry

- [X] T028 [P] Create `apps/web/src/api/interactives.ts` with the registry, list, authoring, activation and approval calls
- [X] T029 [P] Create `apps/web/src/engine/world/sync/interactives.ts` applying interactive and door events into the world store
- [X] T030 Create `apps/web/src/components/InteractionAuthor/InteractionAuthor.tsx` building its effect list and its configuration form from the registry, never from a hard-coded list (FR-038)

**Checkpoint**: The seam exists and offers nothing, because nothing has been contributed yet. That is the correct state and worth confirming before going further.

---

## Phase 3: User Story 1 — A prop that opens something (Priority: P1) 🎯 MVP

**Goal**: A GM places a book, attaches a lore entry, and a player clicking it opens that page without disturbing the scene.

**Independent Test**: Place a prop, attach a link, click as a player, observe the page open and the scene unchanged.

- [X] T031 [P] [US1] Contribute the `lore.open` declaration in `crates/thunderforge-canvas-core/src/interaction.rs` as the first contributor, referencing a lore entry by id rather than a URL (research §5)
- [X] T032 [P] [US1] Test in `crates/thunderforge-canvas-core/src/interaction.rs` that a link effect cannot be configured with a free-text address — the field does not accept one, which is what dissolves the hostile-destination edge case without an allowlist
- [X] T033 [US1] Handle `lore.open` in `src/engine/src/plugins/interaction.rs`'s contributor module or a dedicated `src/engine/src/plugins/lore_link.rs`, opening the target in a new tab without navigating the canvas away
- [X] T034 [US1] Support placing a prop in `apps/web/src/components/InteractionAuthor/InteractionAuthor.tsx` as a token of the existing `object` kind with no actor, reusing the placement pipeline rather than adding one (research §8)
- [X] T035 [US1] Verify every consumer of tokens tolerates a null actor, starting with `src/server/src/graphql/queries/token_status.rs`, and fix any that assume otherwise
- [X] T036 [US1] Ensure an interactive with no effect is silently inert rather than an error, in `src/server/src/graphql/mutations_interactives.rs` — scenery is legitimate (US1 scenario 3)
- [X] T037 [US1] End-to-end spec `apps/web/e2e/interactive-prop.spec.ts`: a GM places a prop and links a lore entry; a player clicks it and the page opens; a prop with no link does nothing; a non-member is offered no interactive at all

**Checkpoint**: The spine works end to end with exactly one contributor. Stop and validate here — this is the MVP.

---

## Phase 4: User Story 2 — Doors that open, close and lock (Priority: P2)

**Goal**: Designate a door on an existing wall; players open and close it; the GM locks it.

**Independent Test**: Designate a door, open and close it as a player, watch vision and movement change, lock it as GM and confirm the player's click stops working.

- [X] T038 [P] [US2] Contribute `door.set_state` and `door.set_lock` declarations from `crates/thunderforge-canvas-core/src/wall.rs`, keeping door knowledge with doors rather than in the interaction core (FR-039)
- [X] T039 [US2] Add `setDoorDesignation` and `setDoorLock` to `src/server/src/graphql/mutations_interactives.rs`, GM-only
- [X] T040 [US2] Route player door-state changes through `activateInteractive` in `src/server/src/graphql/mutations_interactives.rs` so there is one authorization path rather than two
- [X] T041 [P] [US2] Test in `src/server/src/graphql/mutations_interactives.rs` that a player cannot open a locked door **at the server**. This is the rule most likely to be implemented by hiding a button, and a screen test would pass against a server that performs it when asked directly
- [X] T042 [P] [US2] Test in `src/server/src/graphql/mutations_interactives.rs` that a GM can change a locked door's state (FR-013)
- [X] T043 [US2] Handle the door effects in `src/engine/src/plugins/wall.rs`, setting state through the wall plugin's existing systems rather than reaching into geometry
- [X] T044 [US2] Draw a door distinguishably from a plain wall in `src/engine/src/plugins/wall.rs`, and re-resolve vision and movement when its state changes
- [X] T045 [US2] Add the GM secondary-interaction menu offering shut and lock in `apps/web/src/components/InteractionAuthor/InteractionAuthor.tsx`, following the canvas's existing right-click convention (FR-023)
- [X] T046 [US2] Tell a player their click failed because the door is locked, in `apps/web/src/components/InteractionAuthor/InteractionAuthor.tsx`. Silence is indistinguishable from the product being broken (FR-014)
- [X] T047 [US2] Resolve concurrent activation to a single state in `src/server/src/graphql/mutations_interactives.rs`, so two players clicking one door do not diverge (SC-005)
- [X] T048 [US2] End-to-end spec `apps/web/e2e/interactive-doors.spec.ts` across two browsers: designate, open, close, watch vision change without a reload, lock as GM, confirm the player is refused and told, confirm the GM is not

**Checkpoint**: Doors work, and were built as a contributor rather than as part of the core.

---

## Phase 5: User Story 3 — A switch that changes the lighting (Priority: P2)

**Goal**: A lever toggles named lights for every viewer.

**Independent Test**: Place a switch, associate lights, click as a player, see the lights change for everybody.

- [X] T049 [P] [US3] Contribute the `light.toggle` declaration from `crates/thunderforge-canvas-core/src/lighting.rs`
- [X] T050 [US3] Handle `light.toggle` in `src/engine/src/plugins/lighting.rs`, toggling through that plugin's existing systems so shadows re-resolve
- [X] T051 [US3] Show the light association while editing and hide it from players, in `apps/web/src/components/InteractionAuthor/InteractionAuthor.tsx` (US3 scenario 1)
- [X] T052 [US3] Report a deleted associated light to the GM while still toggling the rest, in `src/server/src/interaction.rs` — a broken association must not silently make a switch dead (US3 scenario 3, FR-019)
- [X] T053 [US3] End-to-end spec `apps/web/e2e/interactive-lighting.spec.ts`: a player activates a switch and both viewers see the lighting change

**Checkpoint**: A second independent contributor exists, which is the first real evidence the seam is a seam.

---

## Phase 6: User Story 4 — A secret the GM chooses to reveal (Priority: P3)

**Goal**: A prepared secret door is not presented to players until an interactive reveals it.

**Independent Test**: Prepare a secret door, confirm players are not shown a door, trigger the reveal, confirm it becomes usable and stays revealed.

- [ ] T054 [P] [US4] Contribute the `door.reveal` declaration from `crates/thunderforge-canvas-core/src/wall.rs`
- [ ] T055 [US4] Draw a secret door distinguishably for the GM and not at all for players, in `src/engine/src/plugins/wall.rs`. Per the spec's decision the geometry still reaches every client; it is the drawing that differs
- [ ] T056 [US4] Handle `door.reveal` in `src/engine/src/plugins/wall.rs`, persisting the revelation so it survives a reload (US4 scenario 4)
- [ ] T057 [US4] Add `setDoorSecret` to `src/server/src/graphql/mutations_interactives.rs`, GM-only
- [ ] T058 [US4] End-to-end spec `apps/web/e2e/interactive-secrets.spec.ts`: a player sees no door, a GM does, a reveal makes it a normal door for both, and it is still revealed after a reload

---

## Phase 7: User Story 5 — Something that happens when players arrive (Priority: P3)

**Goal**: A region fires an effect when a token crosses into it.

**Independent Test**: Define a region, move a token across its boundary, observe the effect fire exactly once.

- [ ] T059 [P] [US5] Implement region containment and entry detection in `crates/thunderforge-canvas-core/src/interaction.rs`, comparing previous against current containment
- [ ] T060 [P] [US5] Test in `crates/thunderforge-canvas-core/src/interaction.rs` that entry fires once per crossing and never while moving _within_ a region (FR-030)
- [ ] T061 [P] [US5] Test in `crates/thunderforge-canvas-core/src/interaction.rs` that overlapping regions both fire in a stable order, so a double-region crossing is reproducible rather than arbitrary
- [ ] T062 [US5] Wire entry detection into token movement in `src/engine/src/plugins/interaction.rs`
- [ ] T063 [US5] Add an explicit scene-mode signal in `src/engine/src/lib.rs` so preparation movement does not fire regions. A GM dragging a token in preparation and in play is the same gesture, so this cannot be inferred (FR-032, research §6)
- [ ] T064 [US5] Implement `fire_mode` once-tracking and `resetInteractive` in `src/server/src/graphql/mutations_interactives.rs` (FR-031)
- [ ] T065 [US5] Add region drawing to `apps/web/src/components/InteractionAuthor/InteractionAuthor.tsx`, visible to the GM while editing and never to players
- [ ] T066 [US5] End-to-end spec `apps/web/e2e/interactive-regions.spec.ts`: crossing fires once, moving within does not re-fire, a once-region does not fire for a second token, and GM preparation movement fires nothing

---

## Phase 8: User Story 6 — A player asks, the GM decides (Priority: P3)

**Goal**: A gated interactive raises a request; nothing happens until the GM approves.

**Independent Test**: Trigger a request as a player, approve and refuse it, confirm neither outcome moves anybody until the GM acts.

- [ ] T067 [P] [US6] Contribute the `nav.request_scene` declaration in `crates/thunderforge-canvas-core/src/interaction.rs`, whose approved effect does nothing further until multi-scene navigation exists — the request and the decision are the parts this feature owns
- [ ] T068 [US6] Implement the request lifecycle in `src/server/src/interaction.rs`: raise, approve, refuse, and cancel when the requester leaves or the interactive is deleted
- [ ] T069 [US6] Add `approveRequest` and `refuseRequest` to `src/server/src/graphql/mutations_interactives.rs`, GM-only and never callable by the requester
- [ ] T070 [P] [US6] Test in `src/server/src/interaction.rs` that approval re-checks permission at decision time, so a door locked after the request was raised stays locked
- [ ] T071 [P] [US6] Test in `src/server/src/interaction.rs` that nothing expires a request into approval (FR-027). Silence is not consent
- [ ] T072 [US6] Create `apps/web/src/components/ApprovalQueue/ApprovalQueue.tsx` showing pending requests with requester, interactive and proposed outcome, reachable from a second device
- [ ] T073 [US6] Tell the requesting player the outcome, in `apps/web/src/components/ApprovalQueue/ApprovalQueue.tsx` and its player-side counterpart (FR-028)
- [ ] T074 [US6] End-to-end spec `apps/web/e2e/interactive-approval.spec.ts` across two browsers: a request reaches the GM, refusing changes nothing, approving runs the effect, and doing nothing leaves it pending

---

## Phase 9: User Story 7 — A new subsystem becomes triggerable (Priority: P3)

**Goal**: Prove the seam is a seam, rather than a shape that happens to have three users.

**Independent Test**: Add a trivial contributor, confirm it becomes authorable and runs, and that removing it leaves everything else working.

- [ ] T075 [US7] Add a deliberately trivial contributor behind a feature flag in `crates/thunderforge-canvas-core/src/interaction.rs` that does one observable thing and nothing else, existing only to be added and removed
- [ ] T076 [P] [US7] Add a textual check to `scripts/verify.mjs` asserting that `src/engine/src/plugins/interaction.rs` does not contain "light", "door" or "sound". FR-039 written as a grep is a violation anyone can see, rather than a matter of judgement
- [ ] T077 [P] [US7] Test in `crates/thunderforge-canvas-core/src/interaction.rs` that the registry assembled without the door and lighting contributors offers neither, and that authoring still works
- [ ] T078 [US7] Confirm an interactive authored against an absent contributor is reported unavailable to the GM, is not deleted, and reaches players as nothing rather than as an error, in `src/server/src/interaction.rs` (FR-041)
- [ ] T079 [US7] End-to-end spec `apps/web/e2e/interactive-contribution.spec.ts`: the trivial contributor appears as authorable and runs; with it absent nothing else breaks; and a scene authored against it loses no data

**Checkpoint**: All stories independently functional, and the architecture claim is tested rather than asserted.

---

## Phase 10: Polish & Cross-Cutting Concerns

- [ ] T080 [P] Document the feature in `docs/interactive-elements.md`, including what open, closed and locked mean, and how a subsystem contributes an effect
- [ ] T081 [P] Update `./MVP.md` with what this delivers and what it deliberately does not — party tokens and multi-scene navigation remain unbuilt
- [ ] T082 Measure a scene with 50 interactives against the documented baseline via `apps/web/e2e/engine-limits.spec.ts` and record the figure. The expected answer is "no measurable change"; an expected result that was never checked is an assumption (SC-007)
- [ ] T083 Run `specs/030-interactive-elements/quickstart.md` end to end, including the manual walkthrough — steps 5 and 8 are the ones worth doing by eye
- [ ] T084 Run `pnpm verify` (`./scripts/verify.mjs` — rustfmt, clippy, prettier, eslint) and fix what it reports **in the code this feature added**. Keep it to that: a repo-wide lint remediation folded into a feature phase buries the feature work. `pnpm verify:fix` rewrites what can be rewritten mechanically

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)**: no dependencies
- **Foundational (Phase 2)**: depends on Setup — **blocks every user story**
- **US1 (Phase 3)**: depends on Foundational. The MVP
- **US2, US3 (Phases 4–5)**: depend on Foundational. Independent of each other and of US1
- **US4 (Phase 6)**: depends on US2 — a secret door is a door
- **US5, US6 (Phases 7–8)**: depend on Foundational only
- **US7 (Phase 9)**: needs at least one contributor to exist, so in practice follows US1. Strongest once two exist
- **Polish (Phase 10)**: after the stories being shipped

### Within each story

- Rules in `canvas-core` before the server that enforces them
- Server before the engine and web that call it
- The end-to-end spec last, because it is the thing that proves the story rather than a step toward it

### Parallel opportunities

- T003 and T004 (migrations, different directories)
- T007–T009, T011, T013 (independent tests in one module, written together)
- T015 and T016 (wall rule tests)
- T021 and T022 (server authorization tests)
- T028 and T029 (separate web modules)
- US2, US3, US5 and US6 can proceed in parallel once Foundational is done
- Within a story, every task marked [P] touches a different file

---

## Parallel Example: Foundational

```bash
# The rules and their tests, written together:
Task: "Implement EffectRegistry assembly in crates/thunderforge-canvas-core/src/interaction.rs"
Task: "Test duplicate-id collision fails at assembly"
Task: "Test an empty contribution set assembles to an empty registry"
Task: "Test the blocking rule in crates/thunderforge-canvas-core/src/wall.rs"
```

---

## Implementation Strategy

### MVP first (User Story 1 only)

1. Phase 1: Setup — including the ADR, which Principle IV wants landing with the feature rather than after it
2. Phase 2: Foundational — the seam. Confirm at its checkpoint that it works and offers nothing
3. Phase 3: US1 — one contributor, end to end
4. **Stop and validate.** A GM can place a book that opens a lore page, and the architecture underneath it is the one every later effect will use

### Incremental delivery

Each story after US1 adds a contributor without reopening the core. If the core ever needs editing to add one, FR-039 has been violated and T076's check should be failing.

Doors (US2) are the highest table value and the most likely place the architecture slips, so they are worth doing second and reviewing carefully.

### Parallel team strategy

After Foundational: one developer on doors (US2 + US4), one on lighting and regions (US3 + US5), one on approval (US6). US7 is best done by whoever did _not_ build the core, since it tests a claim the author is least likely to doubt.

---

## Notes

- [P] tasks touch different files and have no incomplete dependencies
- The engine crate's `#[cfg(test)]` modules compile but never execute (Constitution V), which is why every rule worth testing lives in `canvas-core`
- Verify per target: `cargo check --target wasm32-unknown-unknown` for the engine, native `cargo check` for the server, `tsc` for the web app
- Restart the dev stack after engine or server changes — `dist/engine` and `target/debug/thunderforge` are what the browser actually gets
- `--workers=1` for any Playwright run involving the engine
