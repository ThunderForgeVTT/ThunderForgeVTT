# Tasks: GM Staging Page and Full-Screen Play Canvas

**Input**: Design documents from `/specs/009-gm-staging-page/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/world-actors-query.md, quickstart.md (all present)

**Tests**: Not explicitly requested as TDD in the spec; e2e coverage is still included per this repo's established convention (every prior spec 002-008 shipped Playwright coverage alongside implementation, per constitution Principle V).

**Organization**: Grouped by user story (US1/US2/US3 from spec.md), in priority order (US1 P1, US2 P1, US3 P2).

## Phase 1: Foundational (Blocking Prerequisites)

**Purpose**: The one real backend gap (NPC read API) and the one real existing frontend bug (GM-role detection) that every user story depends on. No user story work can start until this phase is complete.

- [X] T001 Add `GraphQLWorldActor` type and `ActorQuery::world_actors(worldId)` resolver in new `src/server/src/graphql/queries/actor.rs`, filtered to `world_actors.world_id = $worldId`, authorized via `require_visible_world` (same helper `graphql/queries/scene.rs`'s `scenes` query uses) — see `contracts/world-actors-query.md` for the exact shape.
- [X] T002 Register the new query module: add `pub mod actor;` + re-export `ActorQuery` in `src/server/src/graphql/queries/mod.rs`, and merge `ActorQuery` into `QueryRoot` in `src/server/src/graphql.rs`.
- [X] T003 [P] Add unit tests for `world_actors` in `src/server/src/graphql/queries/actor.rs`'s `#[cfg(test)] mod tests` (using `test_support::test_app_state`/`insert_test_user`/`insert_test_world`): a member sees the world's actors (including both NPC and non-NPC rows), and a non-member/non-owner is rejected — following the existing pattern in `src/server/src/graphql/helpers.rs`'s `load_visible_world_by_id_rejects_non_member_non_owner` test.
- [X] T004 [P] Add `WorldActorRecord` type in new `apps/web/src/types/actor.ts`, field-for-field matching `GraphQLWorldActor` (see `data-model.md`).
- [X] T005 [P] Add `getWorldActors(worldId)` GraphQL client function in new `apps/web/src/api/actors.ts`, mirroring the existing structure/error-handling of `apps/web/src/api/scenes.ts`.
- [X] T006 Add `useWorldRole(worldId)` hook in new `apps/web/src/hooks/useWorldRole.ts`: derives the current user's role for a world from `useWorldMembers(worldId)` (match `user_id` to the authenticated user), falling back to `"Owner"` when no `world_members` row exists but `world.createdBy === user.id` (research.md §3 — mirrors the server's own `require_world_member` fallback). Returns `{ role: "Owner" | "GM" | "Player" | null, isGm: boolean }` where `isGm` is true for `"Owner"` or `"GM"`.
- [X] T007 Replace `WorldPage.tsx`'s existing `isSceneOwner = Boolean(world && user && world.createdBy === user.id)` with `useWorldRole`'s `isGm` in `apps/web/src/pages/world/WorldPage.tsx` — this is a real bug fix (research.md §3: today's check doesn't recognize an invited co-GM), not just a rename; verify every existing `isSceneOwner`-gated control (scene creation, wall/lighting/shape/token tool panels, `setIsGameMaster` call) still behaves correctly for a true Owner and now also correctly for an invited `GM`-role member.

**Checkpoint**: Backend `worldActors` query is live and tested; frontend has a correct, reusable GM/Owner check. Ready for user story work.

---

## Phase 2: User Story 1 - GM configures a session before handing the screen to the game (Priority: P1) 🎯 MVP

**Goal**: Replace the placeholder `WorldLayout.tsx` shell with a real staging page (real scenes/players/NPCs, no placeholder text, no dead links) and a one-click "Play" transition into a full-screen canvas that preserves engine state on the way back.

**Independent Test**: As a world's GM/owner, navigate to `/world/:id/play`; confirm the staging page (not the canvas) shows real data with no placeholders or dead links; click "Play" and confirm full-screen canvas; click back-to-setup and Play again and confirm no repeated engine-load sequence.

### Implementation for User Story 1

- [X] T008 [P] [US1] Create `NpcRoster` component in new `apps/web/src/components/world/NpcRoster/NpcRoster.tsx`: calls `getWorldActors(worldId)` (T005), filters to `isNpc === true`, renders a list (label, actorType) or a genuine empty state ("No NPCs yet" — not placeholder/lorem-ipsum text) per spec FR-004.
- [X] T009 [US1] Create `WorldStagingPage` component in new `apps/web/src/layouts/world-layout/WorldStagingPage.tsx`: renders the existing `SceneSwitcher` (scenes section), `useWorldMembers`-backed player list (reuse existing member-roster rendering pattern from today's `WorldLayout.tsx`'s "Party roster" panel), the new `NpcRoster` (T008), a clearly-labeled "Lore — coming soon" extension-point placeholder section (spec FR-005 — a real labeled placeholder for a future section is explicitly allowed; this is different from the placeholder *content* FR-004 forbids elsewhere), and a single prominent "Play" button/action.
- [X] T010 [US1] In `apps/web/src/pages/world/WorldPage.tsx`, add local state `const [playView, setPlayView] = useState<"staging" | "playing">("staging")`. Render `WorldStagingPage` (passing a callback that sets `playView` to `"playing"`) when `playView === "staging"`. The canvas container (`#game-canvas-container` and its existing children: engine/scene loading indicators, canvas tool panels, token panel) MUST remain mounted in the DOM regardless of `playView` — gate its **visibility/layout** with CSS (e.g. `display: playView === "playing" ? "block" : "none"`, or an off-screen position), never with a conditional `{playView === "playing" && <div id="game-canvas-container">...}` (research.md §1 — Bevy's `module.start()` only binds once and will not re-attach to a replacement DOM node).
- [X] T011 [US1] Rewrite `apps/web/src/layouts/world-layout/WorldLayout.tsx`: remove the dead "Return to dashboard" `/counter` link and the two placeholder sidebar panels ("Future world metadata...", "Actor sheets, permissions, and compendium panels...", "Player roles and world governance..."). Add a small on-screen "back to setup" control (calls a callback prop that sets `WorldPage.tsx`'s `playView` back to `"staging"`) and restructure the layout so the canvas can occupy the full viewport (remove the `lg:grid-cols-[minmax(260px,0.9fr)_minmax(0,2.2fr)]` column split — canvas gets the full area, sidebar becomes the on-screen toggleable overlay built in Phase 3/US2).
- [X] T012 [US1] Wire `WorldPage.tsx` to pass `playView`-aware props into the (now full-screen) `WorldLayout` only when `playView === "playing"`, and into `WorldStagingPage` only when `playView === "staging"` — both wrapping the same permanently-mounted canvas container from T010.
- [X] T013 [US1] Create `apps/web/e2e/gm-staging-page.spec.ts`: cover quickstart.md Scenarios 1, 2, and 4 — GM sees real staging data with no placeholder/dead-link content, clicking "Play" shows the full-screen canvas, and clicking back-to-setup then "Play" again does **not** repeat the "Downloading engine…" sequence (assert the engine-load indicator does not reappear on the second entry).

**Checkpoint**: A GM can go from `/world/:id/play` through a real staging page into a working full-screen canvas and back, with the engine never re-initializing. This alone is a complete, demonstrable MVP.

---

## Phase 3: User Story 2 - GM keeps essential tools available without losing screen space to the canvas (Priority: P1)

**Goal**: A toggleable on-screen sidebar in full-screen canvas mode exposing scenes, NPC/combat, and trackers/settings, collapsible back to a fully unobstructed canvas.

**Independent Test**: In full-screen canvas mode, toggle the sidebar; confirm it shows real scene/NPC data and collapses back to a full-viewport canvas; confirm an existing canvas tool (e.g. wall tool) still works while the sidebar is open or closed.

### Implementation for User Story 2

- [X] T014 [US2] In `apps/web/src/layouts/world-layout/WorldLayout.tsx`, add a toggleable sidebar (default collapsed) with an on-screen toggle control, containing: a "Scenes" section (reuse `SceneSwitcher`), an "NPC / Combat" section (reuse `NpcRoster` from T008), a "Trackers / Settings" section (a minimal real panel — e.g. current scene name/grid info pulled from already-loaded scene data, not placeholder text), and a clearly-labeled "Lore — coming soon" extension-point placeholder (same treatment as the staging page's, per spec FR-010).
- [X] T015 [US2] Verify/adjust the canvas container's CSS in `WorldLayout.tsx` so opening/closing the sidebar changes layout (canvas width) without breaking `useCanvasEngine.ts`'s existing `ResizeObserver`-based canvas-size sync — confirm the Bevy canvas actually resizes correctly when the sidebar opens/closes (manual check against a running dev instance, per constitution Principle V).
- [X] T016 [US2] Extend `apps/web/e2e/gm-staging-page.spec.ts` to cover quickstart.md Scenario 3: open the sidebar, confirm scene/NPC sections show real data, collapse it, and confirm at least one existing canvas tool (wall tool, reusing the existing wall-drawing helper pattern from `apps/web/e2e/canvas-authoring.spec.ts`) still works correctly in full-screen mode.

**Checkpoint**: US1 + US2 together deliver the full GM experience described in the spec.

---

## Phase 4: User Story 3 - Players get a matching, read-only staging experience (Priority: P2)

**Goal**: Non-GM world members get the same staging-page-first and full-screen-canvas flow, with GM-only editing controls disabled/hidden, and per-user independence (no player's navigation affects another's).

**Independent Test**: As a non-GM member, navigate to `/world/:id/play`; confirm the staging page appears with GM-only controls absent/disabled; confirm clicking "Play" doesn't affect what other connected users see.

### Implementation for User Story 3

- [X] T017 [US3] In `apps/web/src/layouts/world-layout/WorldStagingPage.tsx`, gate scene-creation (`SceneSwitcher`'s existing `canCreateScene` prop) and any future NPC-roster-editing affordance behind `useWorldRole`'s `isGm` (T006) — non-GM members see the same real scene/player/NPC data but without editing controls, per spec FR-012.
- [X] T018 [US3] In `apps/web/src/layouts/world-layout/WorldLayout.tsx`'s sidebar (T014), apply the same `isGm` gating to any editing affordance exposed there (none are added by US1/US2 beyond `SceneSwitcher`'s existing GM-gated "New scene" control, which already respects `canCreateScene`).
- [X] T019 [US3] Confirm (no code change expected — verify via T003's server-side test and existing `require_visible_world` enforcement) that a genuinely non-member session cannot see staging-page data at all, satisfying spec FR-013.
- [X] T020 [US3] Extend `apps/web/e2e/gm-staging-page.spec.ts` to cover quickstart.md Scenario 5: an invited Player sees the staging page with GM controls hidden/disabled, can enter/exit full-screen independently of the GM's own session (use two separate browser contexts, following the existing `secondSessionSameLogin`-style multi-account pattern from `apps/web/e2e/invite-membership.spec.ts`), and a non-member session is denied real data.

**Checkpoint**: All three user stories are independently functional and tested.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Fix the real regression risk this feature introduces for existing e2e coverage, then validate the whole feature end-to-end.

- [X] T021 Update every existing e2e helper/spec that navigates directly to `/world/:id/play` and then immediately assumes the canvas is interactable (`waitForEngineReady`/`canvasBox`/tool-panel interaction with no staging step) to first click the staging page's "Play" action. Confirmed affected files (via `grep -rln "waitForEngineReady\|/play" apps/web/e2e/*.spec.ts`): `apps/web/e2e/token-authoring.spec.ts`, `apps/web/e2e/canvas-authoring.spec.ts`, `apps/web/e2e/map-editor-tooling.spec.ts`, `apps/web/e2e/canvas-asset-paste.spec.ts`, `apps/web/e2e/invite-membership.spec.ts`, `apps/web/e2e/onboarding-flow.spec.ts` — re-check this list at implementation time in case other specs were added since this task was written.
- [X] T022 [P] `cargo check` (native, server) for `src/server`, and `tsc`/`vite build` for `apps/web` — per constitution Principle V, resolve or justify any new warnings.
- [X] T023 Run the full `quickstart.md` walkthrough (all 5 scenarios) against a live dev stack, confirming SC-001 through SC-005 all hold together in one connected pass.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Foundational (Phase 1)**: No dependencies — start immediately. BLOCKS all user stories (US1's staging page needs the `worldActors` query and `useWorldRole`; US2/US3 build directly on US1's files).
- **User Story 1 (Phase 2)**: Depends on Foundational (T001-T007) only. This is the MVP — deliverable and demonstrable on its own.
- **User Story 2 (Phase 3)**: Depends on US1's `WorldLayout.tsx` rewrite (T011) and `NpcRoster` (T008) existing — builds the sidebar into the same file US1 already restructured.
- **User Story 3 (Phase 4)**: Depends on US1's `WorldStagingPage.tsx` (T009) and US2's sidebar (T014) existing — adds gating on top of both.
- **Polish (Phase 5)**: Depends on all user stories being complete (T021 specifically requires US1's staging page to exist, since it changes what those e2e tests must click through).

### Parallel Opportunities

- T003, T004, T005 can run in parallel with each other (different files, all depend only on T001/T002 respectively where applicable — T003 depends on T001/T002; T004/T005 are independent of the backend entirely).
- T008 (`NpcRoster`) can be built in parallel with T006/T007 (role hook) — different files, no shared dependency until T009 needs both.
- T022 (`cargo check`/`tsc`/`vite build`) can run in parallel with T021/T023 — it's a verification pass, not a code change.

## Implementation Strategy

**MVP first**: Phase 1 (Foundational) + Phase 2 (US1) alone already replace the broken placeholder shell with a working staging page and full-screen canvas — this is independently shippable and directly resolves the original bug report ("huge white space blocking scene configuration"). US2 (sidebar) and US3 (player read-only view) are additive increments on top of that same shell, not prerequisites for it.

**Incremental delivery**: Foundational → US1 (MVP, demo-able) → US2 (GM tooling access) → US3 (player parity) → Polish (fix the e2e regression risk, full quickstart validation).
