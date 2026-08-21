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

- [X] T001 [P] [US1] Update `apps/web/e2e/token-authoring.spec.ts`'s existing resize/rotate test (currently drives `]`/`[`/`,`/`.` keyboard input) to instead drag the new resize/rotate handle sprites, confirming the same whole-grid-cell-increment and independent-rotation behavior (FR-002, FR-003; quickstart.md Scenario 1 steps 3-4) — written and **run live, passing** (see fix note under T006 — a real rotate-handle bug was caught by this test on the first live run).
- [X] T002 [P] [US1] Playwright e2e: confirm resize/rotate handles are visibly rendered on a selected token for the GM (FR-001; quickstart.md Scenario 1 step 2) — written as a behavioral-proxy test (pixel inspection of the Bevy canvas is a documented non-option in this repo's e2e environment — see canvas-authoring.spec.ts's "Wall sync across sessions" comment); **run live, passing**.
- [X] T003 [P] [US1] Playwright e2e (reuses spec 004's now-available second-account pattern): confirm a non-GM player sees no resize/rotate handles on any token, including one they control (FR-005; quickstart.md Scenario 1 step 6) — regression guard, since T015 already covered this for the keyboard-era implementation — written and **run live, passing** (both tests in this describe block; its `test.setTimeout` also raised 120s→180s for the same WASM-reload-headroom reason as the resize/rotate describe block).

### Implementation for User Story 1

- [X] T004 [US1] Relocate `handle_token_drag` and `handle_token_resize_rotate_keyboard` from `src/engine/src/systems/selection.rs` into a new `src/engine/src/systems/token.rs`, per research.md §1 — behavior-preserving move, no logic change
- [X] T005 [US1] Add `TokenResizeHandle`/`TokenRotateHandle` marker components and a `sync_token_visuals` system (spawn/despawn each pass, GM-gated) to `src/engine/src/systems/token.rs`, mirroring `wall.rs`'s `WallHandle` pattern (`wall.rs:47`, `627-643`) — per research.md §2
- [X] T006 [US1] Add `handle_token_resize_drag` and `handle_token_rotate_drag` systems to `src/engine/src/systems/token.rs`, mirroring `handle_wall_input`'s `WallDragMode` state-machine shape (`wall.rs:55-78`, `152+`) — resize drag reuses the existing `MIN_TOKEN_SCALE`/`MAX_TOKEN_SCALE` clamp and whole-grid-cell-increment logic (now driven by drag distance, not key presses); rotate drag computes angle continuously from cursor-to-token-center. **Bug found and fixed during T001's first live run**: `rotate_handle_world_pos` computed its offset from the constant `ROTATE_HANDLE_OFFSET` alone, never multiplying by `transform.scale` the way its sibling `resize_handle_world_pos` does — so after a resize, the rotate handle's actual world position silently stayed at scale-1 distance while the (correctly-derived-from-the-formula) test grabbed where it was supposed to be at the token's real scale, missing the handle entirely and leaving rotation at 0. Fixed by scaling the local offset by `transform.scale.y` in `rotate_handle_world_pos` (`src/engine/src/systems/token.rs`), matching the formula both `resize_handle_world_pos` and the e2e test's own doc comments already documented as intended. Confirmed via `cargo check --target wasm32-unknown-unknown`, a `dist/engine` WASM rebuild, and a live rerun of all three US1 tests passing.
- [X] T007 [US1] Grow `src/engine/src/plugins/token.rs` from its current placeholder into a real `TokenPlugin`, chaining T004-T006's systems in `Update`, mirroring `WallPlugin::build`'s exact shape (`plugins/wall.rs`) — per research.md §1 and Constitution Principle II
- [X] T008 [US1] Decide and implement whether spec 004's keyboard shortcuts remain as a secondary input path or are removed, per spec.md's Assumptions (either is acceptable) — document the choice in this task's own commit — **decision: kept as secondary/power-user path** (`handle_token_resize_rotate_keyboard` still chained in `TokenPlugin`), documented in that function's doc comment in `systems/token.rs`

**Checkpoint**: User Story 1 fully functional and independently verified — SC-001 confirmed by T001-T003.

---

## Phase 4: User Story 2 - Assigning token ownership in TokenPanel never hangs the UI (Priority: P1)

**Goal**: Root-cause and fix the remaining Radix Popover auto-dismissal race in `TokenPanel`'s ownership-assignment UI, un-skipping spec 004's blocked test.

**Independent Test**: As a GM, assign a token's owner and primary status via TokenPanel repeatedly (5+ attempts); confirm 100% reliability, no hang, no unexpected popover close.

### Research for User Story 2 (required before implementation, per research.md §3)

- [X] T009 [US2] Live-instrument `TokenPanel.tsx`'s `Popover.Root`/`Popover.Content` (React DevTools Profiler, or temporary render/effect logging, or a `console.trace` in Radix's `onOpenChange` if exposed) while reproducing the hang, to identify the actual trigger — per research.md §3's two unconfirmed candidates (the `refresh()`-triggered list re-render possibly remounting `Popover.Root`, or a timing interaction with the optimistic-update re-render) — **neither candidate was the actual cause.** Instrumented via a temporary `console.log`+stack trace in `onOpenChange` plus an isolated minimal repro script (register → world → scene → token → fill owner input → Tab → check primary checkbox), run against the live dev stack. Root cause found: the primary checkbox's `disabled={!token.ownerUserId}` only flips after `handleSetOwnership`'s network round trip resolves via `refresh()`, but Tab-driven focus traversal happens synchronously, well before that resolves — the browser's real tab order skips a still-`disabled` element entirely, so focus jumped past the checkbox to whatever came next in the DOM (outside `Popover.Content`), and Radix's outside-focus dismissal read that as "focus left the popover" and closed it before the checkbox was ever actually focused/clicked.
- [X] T010 [US2] Fix the confirmed root cause from T009 in `TokenPanel.tsx` — exact change depends on T009's finding (e.g. stabilizing list-item/`Popover.Root` identity across `refresh()`'s re-render, or sequencing the optimistic-update and refetch-triggered re-renders so they don't race) — implemented: a new `ownerDrafts` local-state map tracks each owner input's live typed value (via a new `onChange`), and the checkbox's `disabled` now reads `!(ownerDrafts[token.tokenId] ?? token.ownerUserId ?? '').trim()` instead of `!token.ownerUserId` — the checkbox enables itself the instant there's a non-empty typed value, fully decoupled from the mutation/refetch cycle, so it's never still-disabled by the time Tab is pressed. Verified against the live dev stack via two isolated repro scripts (single-token and three-token cases) — both pass reliably, Tab now lands correctly on the enabled checkbox every time.
- [X] T011 [US2] Un-skip `apps/web/e2e/token-authoring.spec.ts`'s player-owned-token test (remove `test.skip`, restore to `test`); run it 3 consecutive times against a live dev stack to confirm reliability, not just a single pass (FR-006, FR-007; SC-002, SC-003; quickstart.md Scenario 2) — un-skipped, and **3 of 3 consecutive live runs confirmed passing** at the 480s timeout (1 from the prior session, 2 more this session: 5.3m and 5.4m real runtime). Reliability confirmed.

**Checkpoint**: User Story 2 fully functional and independently verified — SC-002/SC-003 confirmed by T009-T011.

---

## Phase 5: Polish & Cross-Cutting Concerns

- [X] T012 [P] Run `cargo check --target wasm32-unknown-unknown` on `src/engine`, resolving any new warnings (Constitution Principle V) — clean, no new warnings (verified three times across the two sessions: plain `cargo check`, via the repo's `wasm-pack`-based `scripts/build.mjs --only-wasm --force`, and again after this session's `rotate_handle_world_pos` scale fix — each rebuilt the actual `dist/engine` bundle the dev server serves)
- [X] T013 [P] Run `tsc`/build on `apps/web` — `vite build` succeeds; `tsc --noEmit` shows only pre-existing errors in unrelated files (RxDB collections, hooks, replication — none touch `TokenPanel.tsx` or `token-authoring.spec.ts`), confirmed via `grep` isolating this feature's touched files from the noise
- [X] T014 Run the complete `token-authoring.spec.ts` suite (all tests, none skipped) as one connected pass, confirming SC-004 — per research.md §4, this satisfies spec 004's original T039 ask with no separate manual walkthrough needed — **all 10 tests passed in one connected 17.0-minute run**, nothing skipped, nothing filtered.
- [X] T015 [P] Update `specs/004-token-canvas-authoring/tasks.md`'s T011/T015/T021-T023/T039 notes to point at this feature's closure, and update `MVP.md` if its Phase 4/6 notes reference the keyboard-shortcut interim or the skipped test — done: all five task notes in spec 004's tasks.md updated to point at this feature's closure; `MVP.md`'s Phase 4 note updated to remove the stale "resize/rotate handles not yet built" line and record spec 006 as complete. Also added a new Post-MVP backlog entry on WASM bundle size (raised mid-session; unrelated to this feature but noted while touching the file).

---

## Closure note (2026-08-21)

All tasks complete. This session (resuming from the 2026-08-21 mid-Phase-5 pause) finished the remaining work:
- T011 (US2): 2 more consecutive live runs of the un-skipped ownership test passed at the 480s timeout (3/3 total across both sessions).
- T001-T003 (US1): run live for the first time. The first run caught a real bug — `rotate_handle_world_pos` wasn't scaling its offset with the token's current `transform.scale` (unlike its sibling `resize_handle_world_pos`), so after a resize the rotate handle's actual position silently diverged from where the test (correctly, per the documented formula) expected it, and rotation never applied. Fixed in `src/engine/src/systems/token.rs`; `cargo check --target wasm32-unknown-unknown` clean; `dist/engine` WASM rebuilt; all three tests then passed live. Two describe-block `test.setTimeout`s (90s→180s, 120s→180s) also needed raising for the same WASM-reload-headroom reason as US2's 480s bump.
- T014: full `token-authoring.spec.ts` suite run as one connected pass — 10/10 tests, 17.0 minutes, nothing skipped.
- T015: spec 004's `tasks.md` (T011/T015/T021-T023/T039) and `MVP.md`'s Phase 4 note updated to point at this feature's closure.

**Nothing was committed to git this session** — all changes remain in the working tree (`src/engine/src/{plugins,systems}/token.rs`, `src/engine/src/systems/selection.rs`, `src/engine/src/plugins/selection.rs`, `src/engine/src/systems/mod.rs`, `apps/web/src/components/TokenPanel.tsx`, `apps/web/e2e/token-authoring.spec.ts`, `specs/004-token-canvas-authoring/tasks.md`, `MVP.md`, `dist/engine/*` rebuilt artifacts). Ready for review/commit.

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
