---

description: "Task list for Token Authoring Polish — Real Resize/Rotate Handles & Reliable Ownership Assignment"
---

# Tasks: Token Authoring Polish — Real Resize/Rotate Handles & Reliable Ownership Assignment

**Input**: Design documents from `/specs/006-token-authoring-polish/`

**Prerequisites**: plan.md, spec.md, research.md, quickstart.md

**Tests**: Included — this feature exists specifically to make previously-written-but-blocked e2e coverage pass live, per spec 004's own precedent that live verification is required, not optional.

**Organization**: Tasks are grouped by user story (US1-US2 from spec.md).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2)

## Path Conventions

`src/engine/` (WASM Bevy engine), `apps/web/` (React frontend). No `src/server` changes in this feature.

---

## Phase 1: Setup / Phase 2: Foundational

**Not applicable as separate phases.** US1 and US2 are fully independent (different files: engine plugin/systems vs. `TokenPanel.tsx`) — no shared blocking prerequisite exists between them.

---

## Phase 3: User Story 1 - A GM resizes and rotates a token using real canvas handles (Priority: P1)

**Goal**: Replace spec 004's keyboard-shortcut resize/rotate stand-in with real canvas-rendered drag handles, restructuring `token.rs` into a proper plugin along the way.

**Independent Test**: On a scene with an existing token selected, a GM drags a resize handle and a separate rotate handle, confirming grid-cell snapping and independent facing change, with both persisting and gated GM-only.

### Tests for User Story 1

- [ ] T001 [P] [US1] Update `apps/web/e2e/token-authoring.spec.ts`'s existing resize/rotate test (currently drives `]`/`[`/`,`/`.` keyboard input) to instead drag the new resize/rotate handle sprites, confirming the same whole-grid-cell-increment and independent-rotation behavior (FR-002, FR-003; quickstart.md Scenario 1 steps 3-4)
- [ ] T002 [P] [US1] Playwright e2e: confirm resize/rotate handles are visibly rendered on a selected token for the GM (FR-001; quickstart.md Scenario 1 step 2)
- [ ] T003 [P] [US1] Playwright e2e (reuses spec 004's now-available second-account pattern): confirm a non-GM player sees no resize/rotate handles on any token, including one they control (FR-005; quickstart.md Scenario 1 step 6) — regression guard, since T015 already covered this for the keyboard-era implementation

### Implementation for User Story 1

- [ ] T004 [US1] Relocate `handle_token_drag` and `handle_token_resize_rotate_keyboard` from `src/engine/src/systems/selection.rs` into a new `src/engine/src/systems/token.rs`, per research.md §1 — behavior-preserving move, no logic change
- [ ] T005 [US1] Add `TokenResizeHandle`/`TokenRotateHandle` marker components and a `sync_token_visuals` system (spawn/despawn each pass, GM-gated) to `src/engine/src/systems/token.rs`, mirroring `wall.rs`'s `WallHandle` pattern (`wall.rs:47`, `627-643`) — per research.md §2
- [ ] T006 [US1] Add `handle_token_resize_drag` and `handle_token_rotate_drag` systems to `src/engine/src/systems/token.rs`, mirroring `handle_wall_input`'s `WallDragMode` state-machine shape (`wall.rs:55-78`, `152+`) — resize drag reuses the existing `MIN_TOKEN_SCALE`/`MAX_TOKEN_SCALE` clamp and whole-grid-cell-increment logic (now driven by drag distance, not key presses); rotate drag computes angle continuously from cursor-to-token-center
- [ ] T007 [US1] Grow `src/engine/src/plugins/token.rs` from its current placeholder into a real `TokenPlugin`, chaining T004-T006's systems in `Update`, mirroring `WallPlugin::build`'s exact shape (`plugins/wall.rs`) — per research.md §1 and Constitution Principle II
- [ ] T008 [US1] Decide and implement whether spec 004's keyboard shortcuts remain as a secondary input path or are removed, per spec.md's Assumptions (either is acceptable) — document the choice in this task's own commit

**Checkpoint**: User Story 1 fully functional and independently verified — SC-001 confirmed by T001-T003.

---

## Phase 4: User Story 2 - Assigning token ownership in TokenPanel never hangs the UI (Priority: P1)

**Goal**: Root-cause and fix the remaining Radix Popover auto-dismissal race in `TokenPanel`'s ownership-assignment UI, un-skipping spec 004's blocked test.

**Independent Test**: As a GM, assign a token's owner and primary status via TokenPanel repeatedly (5+ attempts); confirm 100% reliability, no hang, no unexpected popover close.

### Research for User Story 2 (required before implementation, per research.md §3)

- [ ] T009 [US2] Live-instrument `TokenPanel.tsx`'s `Popover.Root`/`Popover.Content` (React DevTools Profiler, or temporary render/effect logging, or a `console.trace` in Radix's `onOpenChange` if exposed) while reproducing the hang, to identify the actual trigger — per research.md §3's two unconfirmed candidates (the `refresh()`-triggered list re-render possibly remounting `Popover.Root`, or a timing interaction with the optimistic-update re-render)

### Implementation for User Story 2

- [ ] T010 [US2] Fix the confirmed root cause from T009 in `TokenPanel.tsx` — exact change depends on T009's finding (e.g. stabilizing list-item/`Popover.Root` identity across `refresh()`'s re-render, or sequencing the optimistic-update and refetch-triggered re-renders so they don't race)
- [ ] T011 [US2] Un-skip `apps/web/e2e/token-authoring.spec.ts`'s player-owned-token test (remove `test.skip`, restore to `test`); run it 3 consecutive times against a live dev stack to confirm reliability, not just a single pass (FR-006, FR-007; SC-002, SC-003; quickstart.md Scenario 2)

**Checkpoint**: User Story 2 fully functional and independently verified — SC-002/SC-003 confirmed by T009-T011.

---

## Phase 5: Polish & Cross-Cutting Concerns

- [ ] T012 [P] Run `cargo check --target wasm32-unknown-unknown` on `src/engine`, resolving any new warnings (Constitution Principle V)
- [ ] T013 [P] Run `tsc`/build on `apps/web`
- [ ] T014 Run the complete `token-authoring.spec.ts` suite (all tests, none skipped) as one connected pass, confirming SC-004 — per research.md §4, this satisfies spec 004's original T039 ask with no separate manual walkthrough needed
- [ ] T015 [P] Update `specs/004-token-canvas-authoring/tasks.md`'s T011/T015/T021-T023/T039 notes to point at this feature's closure, and update `MVP.md` if its Phase 4/6 notes reference the keyboard-shortcut interim or the skipped test

---

## Dependencies & Execution Order

### Phase Dependencies

- **User Story 1 (Phase 3)** and **User Story 2 (Phase 4)** are fully independent — different files (engine plugin/systems vs. `TokenPanel.tsx`), no shared prerequisite. Either can be done first, or both in parallel.
- **Polish (Phase 5)**: Depends on both user stories being complete (T014 specifically needs US2's test un-skipped to run the "all tests, none skipped" pass).

### Parallel Opportunities

- T001-T003 (US1 tests) in parallel with each other.
- **Phase 3 (US1) and Phase 4 (US2) can be staffed fully in parallel** — zero file overlap.
- T012, T013, T015 (Phase 5) in parallel.

---

## Implementation Strategy

### MVP First

Either user story alone delivers real value and can ship independently:
1. US1 alone closes the interaction-consistency gap (real handles vs. keyboard shortcuts).
2. US2 alone closes the reliability gap (ownership assignment stops hanging) and un-blocks the one remaining spec 004 test.

### Incremental Delivery

1. US1 and US2 in parallel (no dependency between them).
2. Polish once both land — the full suite run (T014) is the natural integration checkpoint.

### Suggested Team Split

- One track: Phase 3 (US1) — Rust/Bevy engine work.
- Another track: Phase 4 (US2) — React/TokenPanel debugging and fix, starting with the required T009 instrumentation step.
