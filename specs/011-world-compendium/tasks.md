---

description: "Task list for World Compendium (011-world-compendium)"
---

# Tasks: World Compendium

**Input**: Design documents from `/specs/011-world-compendium/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Backend `cargo test` coverage and Playwright e2e coverage are included per this repo's established convention (every prior spec, 001-010, ships both) — not because tests were explicitly requested in this spec's own text.

**Organization**: Tasks are grouped by user story (US1, US2, US3) per spec.md's priorities, after a Foundational phase that builds the shared Compendium shell both US1 and US2 sit on top of.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)

## Phase 1: Setup

No new project/dependency setup is needed — this feature reuses the existing `apps/web`/`src/server` structure, existing UI primitives (`Tabs`, `Card`, `Panel`, `Input`, `Button`), and spec 010's existing `getWorldActors`/`createActor`/`updateActor` GraphQL operations and `@/search/actorSearch` module as-is (research.md).

- [X] T001 Confirm dev stack runs (`cargo build` in `src/server`, `vite build` in `apps/web`) before starting, per Constitution Principle V's "verify before claiming done" baseline.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The routed Compendium page shell — tabs, the NPCs tab's read-only table+search+preview split view, and the placeholder tabs. Both US1 (DM) and US2 (Player) browse this exact same shell; nothing DM-specific or Player-specific lives here yet.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T002 [P] Add `worldCompendium` lazy-loader entry in `apps/web/src/routes/pageLoaders.ts`
- [X] T003 [P] Create `apps/web/src/pages/world/compendium/ComingSoonTab.tsx` — presentational, takes a `label: string` prop, renders a static "{label} — coming soon" message (contracts/compendium-npcs.md)
- [X] T004 [P] Create `apps/web/src/pages/world/compendium/ActorPreviewPanel.tsx` — presentational, takes `actor: WorldActorRecord | null` and `onClose(): void`; renders name/description/classification/type, a "View" link, and an "Edit" link only when `actor.myPermissionLevel !== "VIEWER"` (contracts/compendium-npcs.md); renders nothing/a "Select an NPC" placeholder when `actor` is `null`
- [X] T005 Create `apps/web/src/pages/world/compendium/NpcCompendiumTab.tsx` — adapted from `apps/web/src/components/world/NpcCatalog/NpcCatalog.tsx` (spec 010): same `getWorldActors`/FlexSearch (`@/search/actorSearch`) data flow and search input, but table rows call a new `onSelect(actorId: string)` prop instead of navigating via `<Link>`; accepts `selectedActorId` for row-highlight styling; keep the existing inline View/Edit `<Link>` buttons per row as direct-navigation shortcuts (contracts/compendium-npcs.md)
- [X] T006 Create `apps/web/src/pages/world/compendium/WorldCompendiumPage.tsx` — owns `selectedActorId: string | null` state; renders `<Tabs>` with a `CompendiumTabDef[]` array (data-model.md) of NPCs (`<NpcCompendiumTab>` + `<ActorPreviewPanel>` side by side), Items (`<ComingSoonTab label="Items" />`), Abilities (`<ComingSoonTab label="Abilities" />`) (depends on T003, T004, T005)
- [X] T007 Create `apps/web/src/pages/world/WorldCompendiumRoutePage.tsx` — routed wrapper following the exact precedent of `WorldStagingRoutePage.tsx` (spec 010): reads `:id` from the route params, passes `worldId` to `WorldCompendiumPage` (depends on T006)
- [X] T008 Add the `/world/:id/compendium` route in `apps/web/src/routes/AppRoutes.tsx`, nested in `MainLayout`/`RequireAuthenticated` exactly like the existing `/world/:id/staging` route (depends on T002, T007)
- [X] T009 [P] Add a new Playwright e2e file `apps/web/e2e/world-compendium.spec.ts` covering: page loads inside app chrome with NPCs tab selected by default; table shows real NPC data; search narrows results instantly; selecting a row opens the preview panel with correct detail; Items/Abilities tabs show "coming soon" with no table/search; a non-member of the world is rejected from the route (spec.md Edge Cases, quickstart.md)

**Checkpoint**: `/world/:id/compendium` is reachable, shows real NPC data, search works, row-select opens the preview panel, and placeholder tabs render — for any world member, DM or Player, indistinguishably. User story work can now begin.

---

## Phase 3: User Story 1 - DM browses and manages NPCs in a dedicated Compendium (Priority: P1) 🎯 MVP

**Goal**: A DM can search, preview, view, edit, and add NPCs entirely from the Compendium, without entering `/play`.

**Independent Test**: As a world's DM, open `/world/:id/compendium`, search for an NPC, select it to see its preview, follow "Edit" into the full edit screen, return, and add a brand-new NPC that appears in the table without a reload (quickstart.md US1).

### Implementation for User Story 1

- [X] T010 [US1] Add an "Add NPC" control (name + optional description inputs + submit) to `NpcCompendiumTab.tsx`, gated on an `isGm: boolean` prop, calling `createActor` (existing spec 010 mutation) and refreshing the tab's roster/FlexSearch index on success — mirrors `WorldStagingPage.tsx`'s current add-NPC form (`apps/web/src/pages/world/compendium/NpcCompendiumTab.tsx`)
- [X] T011 [US1] Thread `isGm` from `WorldCompendiumPage.tsx` down to `NpcCompendiumTab` (reuse the existing `useWorldRole` hook, same pattern as `WorldStagingPage`/`WorldLayout`) (`apps/web/src/pages/world/compendium/WorldCompendiumPage.tsx`) (depends on T006, T010)
- [X] T012 [US1] Verify/confirm `ActorPreviewPanel`'s "Edit" link (built in T004) correctly navigates to `/world/:id/actor/:actorId/edit` and only appears for Editor/Owner permission — add this assertion to `apps/web/e2e/world-compendium.spec.ts` (extends T009): DM adds an NPC via the Compendium, searches for it, selects it, follows Edit, changes its description, returns, and confirms the updated description appears in the table/preview

**Checkpoint**: User Story 1 is fully functional and independently testable — a DM can do everything they could previously do via Session Setup's NPC catalog, now from the Compendium.

---

## Phase 4: User Story 2 - Any world member can browse the Compendium read-only (Priority: P2)

**Goal**: A Player gets the identical browse/search/preview experience as the DM, with create/edit affordances correctly absent based on their own permission level.

**Independent Test**: As a Player (non-DM) world member, open `/world/:id/compendium`, confirm full browse/search/preview parity with the DM's view, and confirm "Add NPC" is absent and "Edit" only appears on NPCs where their `myPermissionLevel` is Editor or Owner (quickstart.md US2).

### Implementation for User Story 2

- [X] T013 [US2] Add a Playwright e2e test to `apps/web/e2e/world-compendium.spec.ts` (extends T009/T012): as a Player world member, load the Compendium, confirm the "Add NPC" control is absent (verifies T010/T011's `isGm` gate correctly excludes non-DM/GM roles), confirm search/select/preview work identically to the DM's experience, and confirm the preview panel omits "Edit" for an NPC where the Player's `myPermissionLevel` is `VIEWER` (verifies T004's gate)

**Checkpoint**: User Stories 1 AND 2 both work independently — the same shell and gating logic correctly serves both roles with no story-specific implementation beyond the two verification tests above (the gates themselves were built once, in US1, per research.md's "no new permission concept" decision).

---

## Phase 5: User Story 3 - Session Setup is simplified to launch-only concerns (Priority: P2)

**Goal**: Session Setup (`/world/:id/staging`) shows only Play, Players, and Last Session Notes; NPC management and the Lore placeholder are gone, replaced by a link to the Compendium.

**Independent Test**: As a DM, load Session Setup, confirm only the three sections remain, edit and save Last Session Notes, reload, and confirm persistence; as a Player, confirm the notes are visible but read-only (quickstart.md US3).

### Tests for User Story 3 ⚠️

- [X] T014 [P] [US3] Add backend tests to `src/server/src/graphql.rs`'s existing `WorldMutation` test module (or a new `#[cfg(test)] mod tests` block colocated with the new mutation): DM/GM can call `updateWorldSessionNotes` and the new text is immediately visible via `world.sessionNotes`; saving `notes: ""` succeeds and does not error; a Player-role member is rejected; a non-member is rejected (contracts/session-notes.md)

### Implementation for User Story 3

- [X] T015 [P] [US3] Create migration `src/server/migrations/<timestamp>_add_world_session_notes/{up,down}.sql` adding nullable `worlds.session_notes TEXT` (data-model.md), run `diesel migration run` to regenerate `src/server/src/schema.rs`
- [X] T016 [US3] Add `session_notes: Option<String>` to the `World` struct in `src/server/src/models.rs` (depends on T015)
- [X] T017 [US3] Add `session_notes: Option<String>` (as `sessionNotes` via async-graphql's default camelCase) to `GraphQLWorld` in `src/server/src/graphql/types.rs`, and map it in `impl From<World> for GraphQLWorld` (depends on T016)
- [X] T018 [US3] Add `UpdateWorldSessionNotesInput` and an `update_world_session_notes` mutation to `WorldMutation` in `src/server/src/graphql.rs`, enforcing DM/GM-only write via the existing `is_dm_of_world`-equivalent check (research.md §2), returning the updated `GraphQLWorld` (depends on T017; satisfies T014's tests)
- [X] T019 [P] [US3] Add `sessionNotes` to the `world(id)` query's field selection in `apps/web/src/api/world.ts`, and add an `updateWorldSessionNotes(worldId, notes)` function (contracts/session-notes.md)
- [X] T020 [P] [US3] Add `sessionNotes: string | null` to `WorldRecord` in `apps/web/src/types/world.ts`
- [X] T021 [US3] Create `apps/web/src/components/world/SessionNotesPanel/SessionNotesPanel.tsx` — takes `worldId`, `notes: string | null`, `isGm: boolean`, `onSaved(notes: string): void`; renders read-only text for non-DM, an editable textarea + Save button for DM/GM, calling `updateWorldSessionNotes` (depends on T019, T020)
- [X] T022 [US3] In `apps/web/src/layouts/world-layout/WorldStagingPage.tsx`: remove the NPC panel (the `NpcCatalog`/NPC-add-form block) and the "Lore — coming soon" `Card`; add `<SessionNotesPanel>`; add a clearly-labeled link/button to `/world/:id/compendium` (depends on T008 for the route to exist, T021)
- [X] T023 [P] [US3] Update `apps/web/e2e/gm-staging-page.spec.ts` and `apps/web/e2e/world-staging-route.spec.ts`: remove/replace assertions that reference the now-removed staging NPC panel testids (`npc-catalog-*`, `new-npc-name-input`, `add-npc-button`, staging's NPC-roster checks) with equivalent assertions against the Compendium page instead, per quickstart.md's regression check
- [X] T024 [P] [US3] Add `apps/web/e2e/session-notes.spec.ts`: DM edits and saves Last Session Notes, reloads, confirms persistence; a Player sees the same text read-only; DM saves an empty value and confirms the "No notes yet" empty state (not an error) on reload; confirms Session Setup shows exactly Play/Players/Last Session Notes and a Compendium link, with no NPC list or Lore placeholder (quickstart.md US3)

**Checkpoint**: All three user stories are independently functional. Session Setup is simplified and the Compendium is the new home for world-artifact management.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T025 [P] Run `cargo test` (server, native target) and confirm the full suite passes, including the new T014 tests
- [X] T026 [P] Run `tsc`/`vite build` (web) and confirm no new type errors or build warnings
- [X] T027 Run the full quickstart.md validation pass end-to-end in a live dev instance (US1, US2, US3, placeholder tabs, full regression check)
- [X] T028 Mark all tasks in this file `[X]` as completed and confirm no `[ ]` remain

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on Setup — BLOCKS User Stories 1 and 2 (both browse the same shell built here). User Story 3 does **not** depend on Phase 2 for its own backend tasks (T014-T018, T019-T020 are independent of the Compendium shell) but its final integration task (T022) depends on T008 (the Compendium route existing) so the "link to Compendium" has somewhere to point.
- **User Stories (Phase 3-5)**: US1 and US2 depend on Phase 2. US3's backend/data-model tasks (T014-T021) can start immediately after Setup, in parallel with Phase 2/US1/US2; only T022 (staging page edit) and T023 (e2e updates referencing the Compendium) are gated on the Compendium route existing.
- **Polish (Phase 6)**: Depends on all three user stories being complete.

### User Story Dependencies

- **User Story 1 (P1)**: Depends on Phase 2. No dependency on US2 or US3.
- **User Story 2 (P2)**: Depends on Phase 2 and on US1's gating logic (T010/T011, T004) already existing to verify against — but adds no new implementation of its own, only tests, so it does not block or get blocked by US1's own completion in practice (both land together).
- **User Story 3 (P2)**: Its backend/data-model half (T014-T021) is fully independent of US1/US2. Its frontend-integration half (T022-T024) depends on Phase 2 (T008) for the Compendium link target, but not on US1/US2's specific tasks.

### Parallel Opportunities

- T002, T003, T004 (different files) can run in parallel once Phase 1 is done.
- T005 depends on nothing from T002-T004 and can run in parallel with them.
- T009 (e2e scaffold) can be written in parallel with T002-T008, though it won't pass until T008 lands.
- T014-T021 (US3's backend + types, all different files) can be built entirely in parallel with Phase 2/US1/US2 work, by a different contributor if staffed.
- T025, T026 can run in parallel with each other at the end.

---

## Parallel Example: Foundational Phase

```bash
# Launch independent Foundational file creation together:
Task: "Add worldCompendium lazy-loader entry in apps/web/src/routes/pageLoaders.ts"
Task: "Create apps/web/src/pages/world/compendium/ComingSoonTab.tsx"
Task: "Create apps/web/src/pages/world/compendium/ActorPreviewPanel.tsx"
Task: "Create apps/web/src/pages/world/compendium/NpcCompendiumTab.tsx"
```

## Parallel Example: User Story 3 backend half

```bash
# Can run entirely independently of the Compendium shell work:
Task: "Create migration adding nullable worlds.session_notes TEXT"
Task: "Add sessionNotes to GraphQLWorld and updateWorldSessionNotes mutation"
Task: "Add sessionNotes/updateWorldSessionNotes to apps/web/src/api/world.ts"
Task: "Add sessionNotes to WorldRecord in apps/web/src/types/world.ts"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (the Compendium shell)
3. Complete Phase 3: User Story 1 (DM add/edit/browse NPCs)
4. **STOP and VALIDATE**: Run quickstart.md's US1 steps against a live dev instance
5. Demo: the Compendium fully replaces Session Setup's NPC catalog for a DM

### Incremental Delivery

1. Setup + Foundational → the Compendium shell is browsable (read-only parity for everyone already, since no role-gating has diverged yet)
2. Add User Story 1 → DM gets add/edit → validate → demo (MVP)
3. Add User Story 2 (tests only) → confirm Player parity/gating → validate
4. Add User Story 3 → Session Setup simplifies, Last Session Notes ships → validate → demo
5. Polish → full regression pass

### Suggested Assignment if Staffed in Parallel

- Contributor A: Phase 2 (Foundational shell) → Phase 3 (US1) → Phase 4 (US2 tests)
- Contributor B: Phase 5's backend half (T014-T021) starting immediately after Phase 1, independent of Contributor A's progress, then joining for T022-T024 once Phase 2 lands
