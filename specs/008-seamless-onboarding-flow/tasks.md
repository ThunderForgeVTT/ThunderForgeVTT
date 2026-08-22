---

description: "Task list for Seamless Sign-Up-to-Canvas Onboarding Flow"
---

# Tasks: Seamless Sign-Up-to-Canvas Onboarding Flow

**Input**: Design documents from `/specs/008-seamless-onboarding-flow/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/create-world-mutation.md, quickstart.md

**Tests**: Included — this feature changes the primary account-creation and world-entry path every user goes through, and this repo's established per-spec precedent (specs 001-007) treats that as requiring live verification, not just code review.

**Organization**: Tasks are grouped by user story (US1-US3 from spec.md).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1-US3)

## Path Conventions

`src/server/` (Rust/Axum/Diesel backend), `apps/web/` (React frontend). No `src/engine` (Bevy/WASM) changes.

---

## Phase 1: Foundational (Blocking Prerequisites)

**Purpose**: The one piece of shared infrastructure both US1 and US3 build on — knowing, at landing time, whether the current user has any existing worlds.

**⚠️ CRITICAL**: No user story work in Phase 3 (US1) or Phase 5 (US3) can begin until this phase is complete. (US2, Phase 4, does not depend on this phase and could be built in parallel by a different contributor.)

- [X] T001 In `apps/web/src/pages/user/WelcomePage.tsx`, call `getMyWorlds()` (`apps/web/src/api/world.ts`, already used by `WorldListPage`) on mount, holding the result (and a loading flag) in component state — no rendering/redirect decision yet, just the fetch this phase's dependents both need.

**Checkpoint**: `WelcomePage` knows the user's world list before either US1's redirect logic or US3's hub-shortcut rendering needs it.

---

## Phase 2: User Story 1 - A new user reaches their world's canvas quickly, with honest feedback the whole way (Priority: P1)

**Goal**: A zero-world user goes register → create-world form → canvas in one continuous path (2 forms, 0 modals, 0 dashboard stop), with a real loading indicator during the engine's startup and a real error state if it fails.

**Independent Test**: quickstart.md Scenarios 1 and 3 — register a fresh account and confirm the funnel matches the pinned target; confirm the engine-load indicator is visible continuously and an error state renders on failure.

### Tests for User Story 1

- [X] T002 [P] [US1] Playwright e2e in a new `apps/web/e2e/onboarding-flow.spec.ts`: register a brand-new account, confirm the very next screen is the create-world form (`/worlds/create`) with no `/welcome` hub content ever rendered (quickstart Scenario 1 step 1-2; FR-001; SC-001).
- [X] T003 [P] [US1] Playwright e2e in `apps/web/e2e/onboarding-flow.spec.ts`: submit the create-world form, confirm the next screen is `/world/:id/play` directly (never the dashboard), and that the canvas renders the world's auto-created default scene with no "New scene" modal appearing first (quickstart Scenario 1 steps 3-6; FR-004, FR-006; SC-001).
- [X] T004 [P] [US1] Playwright e2e in `apps/web/e2e/onboarding-flow.spec.ts`: entering any world's canvas shows a continuous loading indicator from render until engine-ready, with no gap; simulate an engine load failure (e.g. block the WASM request) and confirm a clear error state renders in its place (quickstart Scenario 3; FR-002, FR-003; SC-002).

### Implementation for User Story 1

- [X] T005 [P] [US1] In `src/server/src/graphql.rs`'s `create_world` resolver, wrap the existing `worlds` insert and a new `scenes` insert (reusing `create_scene`'s exact default values per data-model.md: `type: "battlemap"`, `grid_size: 5`, `grid_type: "square"`, `width`/`height: 100`, scene name = world name, `owner_id` = the same authenticated user) in a single DB transaction — both succeed or both fail. No change to `GraphQLCreateWorldInput`/`GraphQLWorld`'s shape (contracts/create-world-mutation.md).
- [X] T006 [US1] In `apps/web/src/pages/user/WelcomePage.tsx`, using T001's fetched world list: if empty, `navigate("/worlds/create", { replace: true })` immediately, rendering no hub content first.
- [X] T007 [P] [US1] In `apps/web/src/pages/world/CreateWorldPage.tsx`, change the post-success navigation from `` `/world/${world.id}` `` to `` `/world/${world.id}/play` ``.
- [X] T008 [P] [US1] In `apps/web/src/engine/bevy/index.ts`, add an optional `onStageChange?: (stage: "downloading" | "starting") => void` parameter to `mountEngine`/`getWasmModule`, firing `"downloading"` before `await import("@thunderforge/engine/engine")` and `"starting"` after it resolves but before `module.start(...)`.
- [X] T009 [US1] In `apps/web/src/engine/bevy/useCanvasEngine.ts`, pass an `onStageChange` callback through to `mountEngine` and expose the current stage in the hook's returned object alongside the existing `engineReady`/`error`. Depends on T008.
- [X] T010 [US1] In `apps/web/src/pages/world/WorldPage.tsx`, add one new conditional render block for `!engineReady && !engineError`, showing the current load stage as status text — styled identically to the existing `data-testid="scene-load-indicator"` block it sits alongside (e.g. `data-testid="engine-load-indicator"`). Depends on T009.

**Checkpoint**: User Story 1 fully functional and independently verified — SC-001/SC-002 confirmed by T002-T004.

---

## Phase 3: User Story 2 - Nothing in the flow looks configurable or actionable when it isn't (Priority: P1)

**Goal**: Every control shown during account creation and world setup does something real; the invite-code path is reachable and functional from wherever a user lands, for both existing and brand-new accounts.

**Independent Test**: quickstart.md Scenarios 2 and 5 — walk the account-creation/world-setup path confirming no dead controls remain; confirm invite-code redemption works whether or not the user already has an account.

### Tests for User Story 2

- [X] T011 [P] [US2] Playwright e2e in `apps/web/e2e/onboarding-flow.spec.ts`: confirm the create-world form shows only name and description fields — no game-system or interface-pack selector present (quickstart Scenario 2 step 1; FR-005; SC-003).
- [X] T012 [P] [US2] Playwright e2e in `apps/web/e2e/onboarding-flow.spec.ts`: open an existing world's dashboard and confirm every panel shown reflects real data — no unfilled placeholder panel (quickstart Scenario 2 steps 2-3; FR-006; SC-003).
- [X] T013 [P] [US2] Playwright e2e in `apps/web/e2e/onboarding-flow.spec.ts`: as a logged-in user, use the hub's invite-code entry to join a world by code and confirm direct entry; separately, follow a `/join/:code` link while logged out, confirm redirect-to-login preserves the code, click through to registration, and confirm the code is still redeemed after account creation (quickstart Scenario 5; FR-007, FR-012; SC-004).

### Implementation for User Story 2

- [X] T014 [P] [US2] In `apps/web/src/pages/world/CreateWorldPage.tsx`, remove the `gameSystemId`/`interfacePackId` state, their two `Select` components, and the now-unused `GAME_SYSTEM_OPTIONS`/`INTERFACE_PACK_OPTIONS` constants — the `createWorld()` call site stops passing those two fields (already optional server-side, no backend change needed here).
- [X] T015 [P] [US2] In `apps/web/src/pages/world/WorldDashboardPage.tsx`, remove the placeholder panels with no real backing data (Actors, Tokens, Events, Game system, Interface pack per research.md §5); keep the Scenes panel (now always showing ≥1 real scene) and the world's own real metadata.
- [X] T016 [US2] In `apps/web/src/pages/user/WelcomePage.tsx`, replace the dead "Join via Invite Code" → `/counter` CTA with a real code-entry field that submits via `navigate(`/join/${code}`)`.
- [X] T017 [P] [US2] In `apps/web/src/pages/auth/LoginView.tsx`, fix the "Register" link (currently a bare `to="/register"`) to preserve the current `location.search` (e.g. `` to={`/register${location.search}`} ``) so a `?returnTo=` query param survives the Login→Register hop — both pages already honor it independently (research.md §6).

**Checkpoint**: User Story 2 fully functional and independently verified — SC-003/SC-004 confirmed by T011-T013.

---

## Phase 4: User Story 3 - Returning users get a landing experience distinct from first-time users (Priority: P2)

**Goal**: Any user with at least one existing world always sees the hub with direct, one-click shortcuts into their world(s); a user with zero accessible worlds (new or returning-but-emptied) never sees the hub at all.

**Independent Test**: quickstart.md Scenario 4 — confirm the hub is always shown (with correct shortcuts) for any nonzero world count, and never shown for zero, regardless of account age.

### Tests for User Story 3

- [X] T018 [P] [US3] Playwright e2e in `apps/web/e2e/onboarding-flow.spec.ts`: a user with exactly one existing world lands on the hub (not auto-entered into that world) and sees it as a one-click shortcut (quickstart Scenario 4 steps 1-2; FR-001a; SC-005).
- [X] T019 [P] [US3] Playwright e2e in `apps/web/e2e/onboarding-flow.spec.ts`: a user with multiple worlds sees all of them as shortcuts on the same hub (quickstart Scenario 4 step 3; FR-009; SC-005).
- [X] T020 [P] [US3] Playwright e2e in `apps/web/e2e/onboarding-flow.spec.ts`: a returning user whose worlds were all since deleted is routed through the same zero-worlds path as a brand-new user (T006) — no hub, no empty "your worlds" section (quickstart Scenario 4 step 4; FR-010; SC-005).

### Implementation for User Story 3

- [X] T021 [US3] In `apps/web/src/pages/user/WelcomePage.tsx`, when T001's fetched world list is non-empty, render each world as a direct, one-click shortcut card (using the already-fetched data — no new query), replacing the generic "Enter a World" → `/worlds` card with real per-world entries.

**Checkpoint**: User Story 3 fully functional and independently verified — SC-005 confirmed by T018-T020.

---

## Phase 5: Polish & Cross-Cutting Concerns

- [X] T022 [P] Run `cargo check` and `cargo test` on `src/server` (Constitution Principle V), including a new test confirming `create_world` always yields exactly one scene and that a failure on either insert leaves neither committed.
- [X] T023 [P] Run `tsc --noEmit` and `vite build` on `apps/web` (Constitution Principle V).
- [X] T024 [P] Playwright e2e in `apps/web/e2e/onboarding-flow.spec.ts`: submit the create-world form with an input that triggers a validation error, confirm the form re-renders with the entered name/description intact and a specific error message (quickstart Scenario 6; FR-011; SC-006).
- [X] T025 Run the complete quickstart.md walkthrough (all 6 scenarios) as one connected pass against a live dev stack, confirming SC-001 through SC-006 all hold together.

---

## Dependencies & Execution Order

- **Phase 1 (Foundational)**: No dependencies — blocks Phase 2 (US1) and Phase 4 (US3), both of which read T001's fetched world list.
- **Phase 2 (US1)**: Depends on Phase 1. T005 (backend) and T007-T010 (engine loading chain) are independent of T006 and of each other except T008→T009→T010's internal chain — all can proceed in parallel once Phase 1 is done.
- **Phase 3 (US2)**: Independent of Phase 1 and Phase 2 — touches entirely different files (`CreateWorldPage.tsx`'s dropdown removal, `WorldDashboardPage.tsx`, `WelcomePage.tsx`'s invite CTA, `LoginView.tsx`). Could be implemented in parallel with Phase 2 by a different contributor. (Note: T016 touches `WelcomePage.tsx`, the same file as T006/T021 — sequence those three within that file even though they're logically independent edits.)
- **Phase 4 (US3)**: Depends on Phase 1 (T001's fetch) and, in practice, lands after T006 (US1's redirect logic) since both edit `WelcomePage.tsx`'s same render branch.
- **Phase 5 (Polish)**: Depends on all prior phases being complete.

## Parallel Execution Examples

- Phase 1 must finish first, but once done: Phase 2's T005 (backend) and T008 (engine index.ts) can start immediately alongside each other.
- Phase 3 (US2) can be worked entirely in parallel with Phase 2 (US1) by a different contributor — no shared files except the sequencing note on `WelcomePage.tsx` above.
- All Playwright test tasks (T002-T004, T011-T013, T018-T020, T024) target the same new spec file (`apps/web/e2e/onboarding-flow.spec.ts`) — parallel-safe to *write* (independent scenarios, no shared mutable state), but they land as one file, so the final merge step is sequential even though drafting isn't.

## Implementation Strategy

**MVP scope**: User Story 1 alone (Phases 1-2) delivers the feature's core value — the collapsed funnel and honest engine-load feedback for a brand-new user. User Story 2 (Phase 3) is independently valuable and can ship alongside or after it. User Story 3 (Phase 4) is a smaller polish item, naturally sequenced after US1 since it extends the same `WelcomePage.tsx` routing logic.

**Suggested delivery order**: Phase 1 → Phase 2 (US1) as the MVP checkpoint, with Phase 3 (US2) in parallel → Phase 4 (US3) → Phase 5.
