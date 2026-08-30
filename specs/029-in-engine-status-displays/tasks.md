---
description: "Task list for 029-in-engine-status-displays"
---

# Tasks: In-Engine Status Displays and the Engine UI SDK

**Input**: Design documents from `/specs/029-in-engine-status-displays/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/)

**Tests**: Included, and not as a preference. SC-004 and SC-005a are
**wire-level** assertions — they require inspecting the payload that reaches a
non-GM client, because checking the rendered screen would pass against a
client that received a value and chose not to draw it. That is the bug class
this feature guards, so the tests that prove it are part of the feature.

**Organization**: Grouped by user story, each independently testable.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no incomplete dependencies)
- **[Story]**: Which user story the task serves

## Path Conventions

Paths follow [plan.md](./plan.md)'s structure decision: pure rules in
`crates/thunderforge-canvas-core/`, rendering in `src/engine/`, authority in
`src/server/`, chrome in `apps/web/`.

**The engine crate's tests compile and never run.** Any rule that must be
verified by an executing test belongs in `thunderforge-canvas-core`. This is
why the model, the depletion order and the banding arithmetic are not beside
the renderer.

---

## Phase 1: Setup

**Purpose**: Establish the type-generation pipeline before anything depends on it

- [ ] T001 Add `ts-rs` as a dependency of `crates/thunderforge-canvas-core/Cargo.toml`
- [ ] T002 Verify `ts-rs` output for `Option<T>` and `f32` payload fields against the shapes in `specs/029-in-engine-status-displays/contracts/engine-sdk.md`, and record the finding in `specs/029-in-engine-status-displays/research.md` §2 — this is the open follow-up that decides whether `ts-rs` survives as the choice
- [ ] T003 [P] Add a `pnpm` script that regenerates SDK types and fails on a non-empty `git diff`, per research §3
- [ ] T004 [P] Create the generated-output directory `apps/web/src/engine/sdk/` with a README stating the file is generated, which command regenerates it, and that hand-edits will be overwritten

**Checkpoint**: Types can be generated and drift is detectable

---

## Phase 2: Foundational (Blocking Prerequisites)

**⚠️ CRITICAL**: No user story can begin until this phase is complete

**Purpose**: The resource model, the wire types, the dead-pipeline fix, and the ADR the constitution requires

> T005–T009 all live in `resource_display.rs` and are therefore sequential.
> T011–T012 share `systems/token.rs`. T013 and T014 touch nothing else and
> can run alongside any of them.

- [ ] T005 [P] Create `crates/thunderforge-canvas-core/src/resource_display.rs` with `ResourceDefinition`, `ResourceEntry`, `ResourceKind` and `DisclosureState` per [data-model.md](./data-model.md)
- [ ] T006 Write executing tests in `crates/thunderforge-canvas-core/src/resource_display.rs` for the entry rules: ordering, depletion consuming the highest index first, a spent entry remaining in the list, and rejection of a second entry when `allowStacking` is false
- [ ] T007 Implement quarter-band arithmetic in `crates/thunderforge-canvas-core/src/resource_display.rs` with tests covering exactly 25%, exactly zero, a spent top entry, and a multi-entry pool
- [ ] T008 Implement disclosure application (entries → the one field each state permits) in `crates/thunderforge-canvas-core/src/resource_display.rs`, with a test asserting each state yields exactly one of `entries`/`proportion`/`quarter` and never two
- [ ] T009 Register `resource_display` in `crates/thunderforge-canvas-core/src/lib.rs` and re-export the public types
- [ ] T010 Derive `ts-rs` bindings on the wire types in `crates/thunderforge-canvas-core/src/resource_display.rs` and commit the generated output to `apps/web/src/engine/sdk/` (depends on T002, T009)
- [ ] T011 Attach the `Token` component to spawned token entities in `src/engine/src/systems/token.rs`, so `calculate_derived_stats` has input for the first time — see research §6
- [ ] T012 Add a `TokenStatus` component to `src/engine/src/components.rs` and attach it alongside `Token` in `src/engine/src/systems/token.rs` (depends on T011)
- [ ] T013 [P] Create migration `src/server/migrations/<timestamp>_create_token_resource_disclosure/{up,down}.sql` for the sparse per-token table with `created_by`/`updated_by` provenance, per data-model.md
- [ ] T014 [P] Write ADR in `docs/adrs/` covering the generated versioned SDK boundary replacing `apply_world_command`, and the ECS/React split for status presentation — Constitution IV gate, currently the one open item from [plan.md](./plan.md)'s re-check
- [ ] T015 Confirm `cargo check --target wasm32-unknown-unknown -p thunderforge_engine` passes after T011–T012; a native check on this crate is not a signal (Constitution V)

**Checkpoint**: Rules are tested, types are generated, the derived-stat systems finally have input, and the ADR gate is closed

---

## Phase 3: User Story 1 - Reading my own character at a glance (Priority: P1) 🎯 MVP

**Goal**: A player sees their character's resources as a bar above the token and a fuller panel in a screen corner, updating live.

**Independent Test**: Open a world as a player whose character has hit points. The token carries a bar reflecting current against maximum. Reduce the value from another session; the bar shortens without a reload.

### Tests for User Story 1

- [ ] T016 [P] [US1] Vitest for the SDK command wrappers in `apps/web/src/engine/sdk/__tests__/commands.test.ts` — a well-formed `setTokenStatus` is accepted, a malformed one fails to type-check (compile-fail fixture)
- [ ] T017 [P] [US1] Playwright scenario in `apps/web/e2e/status-display.spec.ts`: a player's token shows a bar on load, and it updates without a reload when the value changes elsewhere

### Implementation for User Story 1

- [ ] T018 [P] [US1] Create the Bevy plugin `src/engine/src/plugins/status_display.rs` drawing bar and counter geometry attached to token entities, addable/removable from the `App` builder without touching `TokenPlugin` internals (Constitution II)
- [ ] T019 [US1] Register `StatusDisplayPlugin` in `src/engine/src/lib.rs` (depends on T018)
- [ ] T020 [US1] Implement `setTokenStatus` and `clearTokenStatus` handling in `src/engine/src/lib.rs`'s command dispatch, per `contracts/engine-sdk.md`
- [ ] T021 [US1] Implement the read surface `getTokenStatus` / `listTokenStatus` in `src/engine/src/lib.rs` — FR-021's testing surface, and how React observes state without becoming a second source of truth
- [ ] T022 [P] [US1] Add a `tokenStatus(sceneId)` GraphQL query in `src/server/src/graphql/queries/` returning resolved status, `VISIBLE` only at this stage
- [ ] T023 [US1] Carry status through the existing world-event path in `apps/web/src/engine/world/sync/tokens.ts` so changes reach the engine live (FR-009)
- [ ] T024 [P] [US1] Build the corner panel component in `apps/web/src/components/StatusPanel/StatusPanel.tsx`, reading through the SDK read surface and computing nothing itself (Constitution I)
- [ ] T025 [US1] Mount the panel in `apps/web/src/pages/world/WorldPage.tsx` and bind it to the current selection (depends on T024)
- [ ] T026 [US1] Render a resource with no maximum as a labelled counter rather than a partially filled bar, in `src/engine/src/plugins/status_display.rs` (FR-002, US1 scenario 4)

**Checkpoint**: A player sees their own character's vitals on the token and in the corner, live. This is the MVP.

---

## Phase 4: User Story 2 - Running a table without opening every sheet (Priority: P1)

**Goal**: A GM reads every entitled token's state across the board at once.

**Independent Test**: Place several NPC tokens with differing health. As GM, every one shows its bar simultaneously with no click.

### Tests for User Story 2

- [ ] T027 [P] [US2] Playwright scenario in `apps/web/e2e/status-display.spec.ts`: several tokens display concurrently for a GM, and a token the viewer may not inspect shows nothing rather than an empty bar

### Implementation for User Story 2

- [ ] T028 [US2] Support concurrent per-token displays across a scene in `src/engine/src/plugins/status_display.rs`, with per-entity state rather than a single active token
- [ ] T029 [US2] Render "not disclosed" distinguishably from "at zero" in `src/engine/src/plugins/status_display.rs` (FR-008) — absence of knowledge and knowledge of absence are different facts and must not look alike
- [ ] T030 [US2] Extend `tokenStatus(sceneId)` in `src/server/src/graphql/queries/` to resolve every token in a scene for the requesting viewer

**Checkpoint**: A GM reads the whole board at a glance

---

## Phase 5: User Story 3 - Not learning what I was not told (Priority: P1)

**Goal**: A player cannot discover withheld information by reading a bar.

**Independent Test**: As a player, view an NPC whose exact values are not disclosed. Confirm no exact figure appears in the rendered bar, the panel, **or any payload reaching the client**.

### Tests for User Story 3

- [ ] T031 [P] [US3] Wire-level test in `src/server/src/graphql/` asserting that for `GREYED`, `PERCENTAGE` and `CHUNKED` the exact figure is absent from the payload reaching a non-GM client — SC-004/SC-005a. Assert on the payload, never the screen
- [ ] T032 [P] [US3] Test in `src/server/src/graphql/` that a GM and a player subscribed to the same scene receive **different** payloads for the same token
- [ ] T033 [P] [US3] Test in `src/server/src/graphql/` that `PERCENTAGE` carries no maximum and `CHUNKED` carries a quarter index rather than a proportion (FR-013b)

### Implementation for User Story 3

- [ ] T034 [US3] Apply disclosure server-side in the `tokenStatus` resolver in `src/server/src/graphql/queries/`, using the canvas-core helpers from T008 (depends on T030)
- [ ] T035 [US3] Emit only the field the state permits in `src/server/src/graphql/` — the GraphQL type carries `entries`/`proportion`/`quarter` as mutually exclusive optionals, per `contracts/graphql-disclosure.md`
- [ ] T036 [US3] Render a coarse disclosure visibly differently from an exact one in `src/engine/src/plugins/status_display.rs` (FR-014), so nobody mistakes an estimate for a reading
- [ ] T037 [US3] Ensure the panel in `apps/web/src/components/StatusPanel/StatusPanel.tsx` states what is unavailable rather than showing blanks (US3 scenario 3)
- [ ] T038 [US3] Audit that no animation, ordering or sizing differs by withheld value in `src/engine/src/plugins/status_display.rs` (FR-016)

**Checkpoint**: Disclosure holds at the wire, not just on screen. All P1 stories complete.

---

## Phase 6: User Story 3a - Choosing what the table gets to know (Priority: P2)

**Goal**: A GM sets disclosure per token per resource and changes it mid-encounter.

**Independent Test**: Set a boss token to chunked; a player sees only a quarter band and no exact figure reaches their client. Change to visible mid-session; the player's view updates without a reload.

### Tests for User Story 3a

- [ ] T039 [P] [US3a] Test in `src/server/src/graphql/mutations_tokens.rs` that `setTokenDisclosure` requires `runs_the_world()` and refuses a Player
- [ ] T040 [US3a] Test in `src/server/src/graphql/mutations_tokens.rs` that two tokens of the same actor can hold different states (FR-013d)
- [ ] T041 [P] [US3a] Playwright scenario in `apps/web/e2e/status-display.spec.ts`: a GM changes state mid-session and a connected player's view updates without a reload

### Implementation for User Story 3a

- [ ] T042 [US3a] Add the `TokenResourceDisclosure` model and schema entries in `src/server/src/models.rs` and `src/server/src/schema.rs` (depends on T013)
- [ ] T043 [US3a] Implement `setTokenDisclosure` in `src/server/src/graphql/mutations_tokens.rs`, gated on `thunderforge_authz::Actor::runs_the_world()` — reuse, do not add a parallel check (Constitution III)
- [ ] T044 [US3a] Add a disclosure-changed event code in `src/server/src/world_events.rs` and emit on change — a value change and a change in what may be _known_ are different facts a client may react to differently
- [ ] T045 [US3a] Handle the new event code in `apps/web/src/engine/world/sync/tokens.ts` so displays appear and vanish live
- [ ] T046 [US3a] Add the GM-facing disclosure control to `apps/web/src/components/TokenPanel.tsx`, presenting the four states with their differing safety made visible rather than as four interchangeable appearances (FR-013c)

**Checkpoint**: A GM controls disclosure without stopping play

---

## Phase 7: User Story 4 - A system that tracks more than hit points (Priority: P2)

**Goal**: A game system declares its own resources; tokens render them with no engine change.

**Independent Test**: A system declaring health, stamina and mana shows three bars; one declaring health and energy shows two. No engine change between them.

### Tests for User Story 4

- [ ] T047 [P] [US4] Test in `crates/pack_system_spec/src/lib.rs` that a manifest declaring resources validates, and that a `counter` with `allowStacking: true` is rejected
- [ ] T048 [P] [US4] Playwright scenario in `apps/web/e2e/status-display.spec.ts` covering a three-resource system and a two-resource system with no engine rebuild between them

### Implementation for User Story 4

- [ ] T049 [P] [US4] Extend the system manifest schema in `crates/pack_system_spec/src/lib.rs` with `ResourceDefinition[]`
- [ ] T050 [P] [US4] Declare resources for the bundled systems in `packs/systems/*/server/`, starting with `dnd5e`
- [ ] T051 [US4] Serve declarations through the existing manifest pipeline in `src/server/src/systems.rs` (depends on T049)
- [ ] T052 [US4] Implement `setResourceDefinitions` handling in `src/engine/src/lib.rs`, rejecting duplicate ids and counters that allow stacking, reported through the event callback
- [ ] T053 [US4] Draw nothing at all — not an empty container — for a system declaring no resources, in `src/engine/src/plugins/status_display.rs` (FR-007)

**Checkpoint**: The engine is system-agnostic in fact, not just in intent

---

## Phase 8: User Story 5 - Putting the panel where it does not cover the map (Priority: P2)

**Goal**: The viewer chooses the panel's corner and the choice persists.

**Independent Test**: Move the panel to another corner, reload, find it where it was left.

### Tests for User Story 5

- [ ] T054 [P] [US5] Vitest in `apps/web/src/components/StatusPanel/__tests__/placement.test.ts` for persistence and for the no-selection case showing no stale values

### Implementation for User Story 5

- [ ] T055 [US5] Add corner selection and persistence to `apps/web/src/components/StatusPanel/StatusPanel.tsx` (FR-011)
- [ ] T056 [US5] Clear the panel on deselection in `apps/web/src/components/StatusPanel/StatusPanel.tsx` so a previous token's values never linger (FR-012)

**Checkpoint**: The panel sits where the viewer wants it

---

## Phase 9: User Story 6 - Building on the engine without guessing (Priority: P2)

**Goal**: Every display is driven through a typed SDK; a mistake is a compile error, and a rejection is reported rather than discarded.

**Independent Test**: Send a declaration with a misspelled field — the build fails. Send an incompatible `sdkVersion` — the engine reports it and applies nothing.

### Tests for User Story 6

- [ ] T057 [P] [US6] Compile-fail fixture in `apps/web/src/engine/sdk/__tests__/` proving a wrong field name or type is rejected by `tsc`
- [ ] T058 [P] [US6] Test in `src/engine/src/lib.rs` that a version mismatch applies nothing and reports through the event callback
- [ ] T059 [P] [US6] Test in `src/engine/src/lib.rs` that a rejected command leaves prior display state intact — a bad update must not blank a display that was correct

### Implementation for User Story 6

- [ ] T060 [US6] Add the `sdkVersion` envelope and mismatch rejection in `src/engine/src/lib.rs` per research §4
- [ ] T061 [US6] Implement the `EngineSdkError` reporting path in `src/engine/src/lib.rs` for every code in `contracts/engine-sdk.md`; silent discard is not acceptable (FR-020)
- [ ] T062 [US6] Write typed command wrappers in `apps/web/src/engine/sdk/commands.ts` over the generated types, so no caller hand-builds JSON
- [ ] T063 [US6] Migrate the existing `apply_world_command` call sites for status commands in `apps/web/src/engine/world/sync/` onto the wrappers (depends on T062)
- [ ] T064 [US6] Wire the generation-diff check from T003 into CI

**Checkpoint**: The boundary is typed and its failures are loud

---

## Phase 10: User Story 7 - Appearance that can later be themed (Priority: P3)

**Goal**: Appearance is data supplied by the application, so a later theming feature has something to configure.

**Independent Test**: Change supplied appearance values; rendering changes with no engine rebuild.

### Tests for User Story 7

- [ ] T065 [P] [US7] Test in `crates/thunderforge-canvas-core/src/resource_display.rs` that the default palette is distinguishable in perceived lightness as well as hue (FR-024, SC-007) — mirroring the existing token-kind palette test, because a red bar and a green bar are the same bar to many viewers

### Implementation for User Story 7

- [ ] T066 [US7] Implement `setDisplayAppearance` with partial override semantics in `src/engine/src/lib.rs`
- [ ] T067 [US7] Define the documented default appearance in exactly one place in `crates/thunderforge-canvas-core/src/resource_display.rs` (FR-023)
- [ ] T068 [US7] Consume appearance values in `src/engine/src/plugins/status_display.rs` rather than compiled-in constants (FR-022)

**Checkpoint**: Theming has a surface to attach to

---

## Phase 11: Polish & Cross-Cutting Concerns

- [ ] T069 Measure engine capacity with status displays enabled via `apps/web/e2e/engine-limits.spec.ts` (`--workers=1`) and record the figure against the 3,200-sprite baseline — SC-006 requires a **stated** number; an unmeasured cost is the failure, a measured reduction is not
- [ ] T070 [P] Optimise off-screen tokens to skip full display cost in `src/engine/src/plugins/status_display.rs` (FR-026)
- [ ] T071 [P] Document the feature in `docs/` including the four disclosure states and the note that percentage discloses more than it appears to
- [ ] T072 [P] Update `MVP.md` Phase 5 with the verified outcome, replacing the stale "unverified" note now that stats demonstrably reach the screen
- [ ] T073 Run `specs/029-in-engine-status-displays/quickstart.md` end to end, including the manual step comparing `GREYED` against a resource genuinely at zero — that one is worth doing by eye, because a design that renders them alike passes every automated check while misleading every player
- [ ] T074 Run `pnpm verify` (rustfmt, clippy, prettier, eslint) and fix what it reports **in the code this feature added**. Keep it to that: a repo-wide lint pass folded into a feature phase buries the feature work. `pnpm verify:fix` rewrites what can be rewritten mechanically

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies
- **Foundational (Phase 2)**: Depends on Setup — **blocks every user story**
- **US1 (Phase 3)**: Depends on Foundational. The MVP.
- **US2 (Phase 4)**: Depends on US1's plugin and query existing
- **US3 (Phase 5)**: Depends on US2's multi-token resolution (T030)
- **US3a (Phase 6)**: Depends on US3's disclosure application and the T013 migration
- **US4 (Phase 7)**: Depends on Foundational only — can run parallel to US2/US3
- **US5 (Phase 8)**: Depends on US1's panel (T024)
- **US6 (Phase 9)**: Depends on Foundational; hardens what US1 uses
- **US7 (Phase 10)**: Depends on US1's plugin
- **Polish (Phase 11)**: After the stories being shipped

### Critical path

`T001 → T002 → T010` (type generation) and `T005 → T008` (the rules) both
gate Phase 2's completion. `T011` (attaching `Token`) is independent of both
and can start immediately — it is also the single task that turns the existing
derived-stat systems from decorative into live.

### Parallel Opportunities

- Phase 1: T003, T004 together
- Phase 2: T005–T008 are one file and must be sequential; T013 and T014 run parallel to everything
- **US4 (Phase 7) is independent of US2/US3** and can be staffed alongside them
- All test tasks marked [P] within a story run together
- Phase 11: T070, T071, T072 together

---

## Parallel Example: Phase 2

```bash
# The migration and the ADR need nobody else:
Task: "T013 Create token_resource_disclosure migration"
Task: "T014 Write the ADR for the SDK boundary and the ECS/React split"

# Meanwhile, the rules go in one file and stay sequential:
Task: "T005 → T006 → T007 → T008 in resource_display.rs"
```

---

## Implementation Strategy

### MVP: Phases 1–3

Setup, Foundational, then User Story 1. That yields a player who can see their
own character's vitals on the token and in the corner, updating live — and it
makes the derived-stat subsystem execute for the first time.

**Stop and validate there.** US1 is demonstrable on its own.

### Then, in order of what it buys

1. **US2** (Phase 4) — the GM reads the whole board. Small delta over US1.
2. **US3** (Phase 5) — disclosure holds at the wire. **Do not ship publicly
   before this**: US1 and US2 alone display exact values to everyone, which is
   a regression in what a GM can keep hidden compared to today, where nothing
   is displayed at all.
3. **US3a** (Phase 6) — the GM control surface.
4. **US4** (Phase 7) — other systems. Parallelisable earlier if staffed.
5. **US5, US6, US7** — placement, SDK hardening, themeable appearance.

### The ordering risk worth naming

US1 and US2 are both P1 and both ship _display_. US3 is the one that makes
display safe. Shipping the first two without the third would put exact NPC
values in front of every player — so US3 is not a refinement of the P1 work,
it is the condition on releasing it.

---

## Notes

- [P] = different files, no incomplete dependencies
- Commit after each task or logical group
- **Assert disclosure from the payload, never the screen** — a screen test
  passes against a client that received the value and chose not to draw it
- The engine crate's `#[cfg(test)]` modules never execute; a green
  `cargo check` there means it compiles, not that it works
- Restart the stack after engine or server changes; a stale `dist/engine` or
  `target/debug/thunderforge` has repeatedly produced failures that looked
  like logic bugs
