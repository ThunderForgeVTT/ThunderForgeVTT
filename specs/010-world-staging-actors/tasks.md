---

description: "Task list for World Staging Route and Actor Ownership"
---

# Tasks: World Staging Route and Actor Ownership

**Input**: Design documents from `/specs/010-world-staging-actors/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md (all present)

**Tests**: Not explicitly requested in the spec, but Constitution Principle V commits every feature to `cargo check`/`tsc`/live exercise plus e2e coverage — this task list includes a lean set of backend `cargo test` and Playwright e2e tasks (not exhaustive per-mutation contract tests) to satisfy that commitment without over-testing.

**Organization**: Tasks are grouped by user story (spec.md's US1–US5) to enable independent implementation and testing of each.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1–US5)
- File paths are exact, relative to repo root

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Get the two new tables into the schema before any Rust code can compile against them.

- [X] T001 Create Diesel migration `add_world_actor_permissions` (`up.sql`/`down.sql`) under `src/server/migrations/<timestamp>_add_world_actor_permissions/` per data-model.md: `id`, `actor_id` (FK → `world_actors(id)` ON DELETE CASCADE), `user_id` (FK → `users(id)` ON DELETE CASCADE), `level` VARCHAR(16) with a CHECK constraint for `Viewer`/`Editor`/`Owner`, `created_at`/`updated_at`, `UNIQUE (actor_id, user_id)`
- [X] T002 [P] Create Diesel migration `add_world_actor_shares` (`up.sql`/`down.sql`) under `src/server/migrations/<timestamp>_add_world_actor_shares/` per data-model.md: `id`, `actor_id` (FK → `world_actors(id)` ON DELETE CASCADE), `share_code` VARCHAR(32) UNIQUE, `created_by` (FK → `users(id)`), `revoked` BOOLEAN NOT NULL DEFAULT false, `created_at`/`updated_at`
- [X] T003 Run both migrations locally (`diesel migration run`) and add the resulting `world_actor_permissions` and `world_actor_shares` `table!` blocks plus their `joinable!`/`allow_tables_to_appear_in_same_query!` entries to `src/server/src/schema.rs`
- [X] T004 [P] Add `ActorPermission`/`NewActorPermission` and `ActorShare`/`NewActorShare` structs to `src/server/src/models.rs`, mirroring the existing `WorldInvite`/`NewWorldInvite` pattern

**Checkpoint**: `cargo check` passes with the two new tables and models in place, unused for now.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared authorization logic, GraphQL scaffolding, and the staging-route split every user story below builds on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T005 Create `src/server/src/auth/actor_permissions.rs` with `is_dm_of_world(state, user_id, world_id) -> bool` (wraps `require_world_member`'s existing Owner/GM role check, research.md §3) and `require_actor_permission(state, user_id, actor_id, minimum: ActorPermissionLevel) -> GraphQLResult<()>` (resolves DM → `Owner`, else explicit `world_actor_permissions` row, else default `Viewer`, per data-model.md); register the module in `src/server/src/main.rs`'s `mod auth;` tree
- [X] T006 [P] Add the `ActorPermissionLevel` async-graphql enum (`VIEWER`/`EDITOR`/`OWNER`) and `GraphQLActorPermission`, `GraphQLActorShareLink`, `SharedActorPreview` types to `src/server/src/graphql/types.rs` per contracts/actor-permissions.md and contracts/actor-share.md
- [X] T007 Extend `GraphQLWorldActor` (`src/server/src/graphql/queries/actor.rs` / its `From<WorldActor>` conversion) with a server-resolved `myPermissionLevel: ActorPermissionLevel!` field using `require_actor_permission`'s resolution logic, per contracts/actor-crud.md
- [X] T008 Create empty module stubs `src/server/src/graphql/mutations_actors.rs`, `mutations_actor_permissions.rs`, `mutations_actor_shares.rs` (each with a `#[derive(Default)] pub struct ...Mutation;` and an empty `#[async_graphql::Object] impl` block) and merge all three into the `MutationRoot` tuple in `src/server/src/graphql.rs`
- [X] T009 [P] Add `ActorPermissionLevel` type and `myPermissionLevel` field to `WorldActorRecord` in `apps/web/src/types/actor.ts`; create `apps/web/src/types/actorShare.ts` with `SharedActorPreview` and `ActorShareLinkRecord` types
- [X] T010 [P] Add `worldStaging`, `actorView`, `actorEdit`, `sharedActor` entries to `apps/web/src/routes/pageLoaders.ts`, each pointing at a new (initially minimal) page module
- [X] T011 Create `apps/web/src/pages/world/WorldStagingRoutePage.tsx` — a routed page that loads `world`/`scenes`/`sceneId` state (extracted from `WorldPage.tsx`'s existing data-loading) and renders the existing `layouts/world-layout/WorldStagingPage.tsx` presentational component, with `onPlay` calling `navigate(\`/world/${worldId}/play\`)`; register `/world/:id/staging` in `apps/web/src/routes/AppRoutes.tsx` nested inside `MainLayout`, wrapped in `RequireAuthenticated`
- [X] T012 Simplify `apps/web/src/pages/world/WorldPage.tsx`: remove the `playView: "staging" | "playing"` state and the conditional `display: "block"/"none"` wrapper divs — always render the full-screen `WorldLayout` shell directly (the `/world/:id/play` route itself is unchanged in `AppRoutes.tsx`)
- [X] T013 Update `apps/web/src/pages/user/WelcomePage.tsx`'s "Enter {world.name}" link target from `` `/world/${world.id}/play` `` to `` `/world/${world.id}/staging` ``

**Checkpoint**: `/world/:id/staging` is reachable and shows the existing (unchanged) roster/scene/player panels; `/world/:id/play` goes straight to full-screen canvas with no staging step. Backend compiles with the new permission-resolution helper and empty mutation stubs wired in. User story implementation can now begin.

---

## Phase 3: User Story 1 - DM lands on a real world-manager staging screen (Priority: P1) 🎯 MVP

**Goal**: A DM can land on `/world/:id/staging`, see the real actor roster, create a new NPC, and click "Play" into the full-screen canvas.

**Independent Test**: As a world's DM/owner, click "Enter" on that world from `/welcome`; confirm landing on `/world/:id/staging` showing the world's real actor roster; use "add NPC," confirm it appears; click "Play" and confirm arrival at the full-screen canvas.

### Implementation for User Story 1

- [X] T014 [US1] Implement `createActor(input: CreateActorInput!)` mutation in `src/server/src/graphql/mutations_actors.rs` per contracts/actor-crud.md — DM-only via `is_dm_of_world` (FR-019), `scene_id` defaults to the target world's earliest-created scene (research.md §6), `owned_by`/`created_by` set to the caller, no `world_actor_permissions` rows created
- [X] T015 [P] [US1] Add `cargo test` coverage in `mutations_actors.rs` for `createActor`: DM succeeds and the actor lands on the world's default scene; a non-DM (Player role) caller is rejected
- [X] T016 [US1] Add `createActor` to `apps/web/src/api/actors.ts` (GraphQL client function, mirroring `getWorldActors`'s `postGraphQL` pattern)
- [X] T017 [US1] Wire the staging page's "add NPC" control (in `layouts/world-layout/WorldStagingPage.tsx`, surfaced only when `isGm`) to call `createActor` and refresh the roster (re-fetch `worldActors` or optimistically prepend) — no page reload (FR-004, SC-003)
- [X] T018 [US1] Confirm the existing "Play" button in `WorldStagingPage.tsx` invokes `WorldStagingRoutePage.tsx`'s `onPlay` (from T011), landing on `/world/:id/play`'s full-screen canvas (FR-006)
- [X] T019 [US1] Create `apps/web/e2e/world-staging-route.spec.ts`: DM enters world → lands on `/staging` with app header visible → roster shows real data (or empty state) → add NPC appears without reload → Play navigates to full-screen canvas

**Checkpoint**: User Story 1 is fully functional and independently testable — the MVP.

---

## Phase 4: User Story 2 - Player lands on a landing page for the world (Priority: P1)

**Goal**: A non-DM member reaches the same `/staging` route with DM-only controls hidden, and can independently reach the same full-screen canvas.

**Independent Test**: As a non-DM world member, click "Enter" for a world; confirm `/world/:id/staging` with no NPC-catalog-editing or actor-creation controls; click "Play" and confirm arrival at the same full-screen canvas, independent of the DM's own navigation.

### Implementation for User Story 2

- [X] T020 [US2] In `WorldStagingRoutePage.tsx`, compute the current user's DM status (Owner/GM role via `useWorldMembers`, with the `world.createdBy` fallback — same pattern as spec 009's `isSceneOwner`/`isGm` resolution) and pass it as the existing `isGm` prop into `WorldStagingPage.tsx`
- [X] T021 [US2] Verify (and adjust if needed) that `WorldStagingPage.tsx` already hides "add NPC" and scene-creation controls when `isGm` is `false` (FR-005) while still rendering the roster/scene/player panels read-only
- [X] T022 [US2] Extend `apps/web/e2e/world-staging-route.spec.ts` with a Player-role scenario: no DM-only controls visible, Play still works and is independent of the DM's own view state

**Checkpoint**: User Stories 1 and 2 both work independently — the full P1 staging-route slice is done.

---

## Phase 5: User Story 3 - DM manages an actor's ownership/permissions (Priority: P1)

**Goal**: A DM can open any actor's ownership block, assign Viewer/Editor/Owner to any world member (including multiple simultaneous Owners), and have that control live-play token control and sheet-editing rights; non-DM members (including an actor's own Owner) cannot touch the block.

**Independent Test**: As a DM, open an actor's ownership block, assign a player Owner/Editor/Viewer, confirm that player's access changes accordingly; confirm a non-DM member (including one holding Owner on that same actor) cannot open or change the block.

### Implementation for User Story 3

- [X] T023 [US3] Implement `actorPermissions(actorId: ID!)` query in `src/server/src/graphql/queries/actor.rs` per contracts/actor-permissions.md — DM-only, returns only explicit rows (no synthesized default-Viewer entries)
- [X] T024 [US3] Implement `setActorPermission`/`removeActorPermission` mutations in `src/server/src/graphql/mutations_actor_permissions.rs` per contracts/actor-permissions.md — DM-only (FR-014), UPSERT on `(actor_id, user_id)` for set, idempotent delete for remove
- [X] T025 [US3] Change `update_actor_system_data`'s authorization check in `src/server/src/graphql.rs` from `world_actors::owned_by.eq(user_id)` to `require_actor_permission(state, user_id, actor_id, minimum: Editor)` (research.md §4)
- [X] T026 [US3] Extend the token-drag/move authorization in `src/server/src/graphql/mutations_tokens.rs` with an OR-condition: when `tokens.actor_id IS NOT NULL`, also allow a caller holding effective `Owner` (via `require_actor_permission`) on that actor, alongside the existing `tokens.owner_user_id == user_id` check (research.md §5, FR-018)
- [X] T027 [US3] Extend the member-removal path in `src/server/src/graphql/mutations_invites.rs` to delete every `world_actor_permissions` row for `(removed user_id, any actor in that world)` in the same transaction (research.md §7, FR-022)
- [X] T028 [P] [US3] Add `cargo test` coverage for `require_actor_permission`: DM resolves to `Owner` regardless of explicit rows; a member with no row defaults to `Viewer`; multiple simultaneous `Owner` rows on one actor are all accepted; a non-DM `Owner`-level caller is rejected from `setActorPermission`/`removeActorPermission`; removing a world member deletes their permission rows across all of that world's actors
- [X] T029 [P] [US3] Add `getActorPermissions`, `setActorPermission`, `removeActorPermission` to `apps/web/src/api/actors.ts`
- [X] T030 [US3] Create `apps/web/src/pages/world/actor/ActorOwnershipBlock.tsx` — lists every world member plus the DM (via `useWorldMembers`), shows each one's explicit level or "default (Viewer)," lets a DM caller change any row via `setActorPermission`/`removeActorPermission`; renders nothing/disabled for a non-DM viewer regardless of their own permission level on the actor
- [X] T031 [US3] Create `apps/web/e2e/actor-ownership.spec.ts`: DM assigns Owner to a player → player gains edit + live-play token control; DM assigns a second simultaneous Owner → both can act on the token; the Owner player cannot see/reach ownership-block controls on their own actor

**Checkpoint**: All three P1 stories (staging route for DM and Player, plus ownership management) are complete and independently testable.

---

## Phase 6: User Story 4 - Dedicated actor view/edit routes (Priority: P2)

**Goal**: Any world member can reach `/world/:id/actor/:actorId/view`; a member with Editor/Owner access can reach `/edit` and save changes; a Viewer-only member is redirected from `/edit` to `/view`.

**Independent Test**: As a member with at least Viewer access, navigate to `/view`; confirm it renders. Attempt `/edit` as a Viewer-only member; confirm redirect to `/view`.

### Implementation for User Story 4

- [X] T032 [US4] Implement `updateActor(input: UpdateActorInput!)` mutation in `src/server/src/graphql/mutations_actors.rs` per contracts/actor-crud.md — requires `require_actor_permission(minimum: Editor)` (FR-010, FR-011)
- [X] T033 [P] [US4] Add `cargo test` coverage for `updateActor`: Editor and Owner succeed; Viewer (default and explicit) is rejected
- [X] T034 [US4] Add `updateActor` and a single-actor fetch helper (e.g. `getActor(actorId)`, filtering an existing `worldActors` fetch or adding a narrow query if needed) to `apps/web/src/api/actors.ts`
- [X] T035 [US4] Create `apps/web/src/pages/world/actor/ActorDetailPage.tsx` (shared `mode: "view" | "edit"` component): view mode renders read-only actor data for any member with ≥ `myPermissionLevel: VIEWER`; edit mode is only reachable when `myPermissionLevel !== "VIEWER"` (else redirect to `/view`, FR-011) and embeds `ActorOwnershipBlock.tsx` (from T030), itself gated DM-only inside that component
- [X] T036 [US4] Register `/world/:id/actor/:actorId/view` and `/world/:id/actor/:actorId/edit` in `apps/web/src/routes/AppRoutes.tsx`, nested inside `MainLayout`, wrapped in `RequireAuthenticated`, denying non-members consistent with existing world-visibility rules (FR-012)
- [X] T037 [US4] Link each actor in the staging roster (`NpcRoster.tsx` / the players/NPC panels) to its `/view` route
- [X] T038 [US4] Create `apps/web/e2e/actor-detail-routes.spec.ts`: Viewer can view but is redirected away from `/edit`; Editor/Owner can save from `/edit`; a non-member is denied both routes

**Checkpoint**: Actors are independently linkable/bookmarkable; ownership-block UI from US3 is now reachable through a real route.

---

## Phase 7: User Story 5 - Share an actor and copy it into another world (Priority: P2)

**Goal**: An Owner-level member can generate a share link; anyone logged in can view a read-only, world-identity-scrubbed preview and copy it as a brand-new, fully independent actor into one of their own DM-level worlds.

**Independent Test**: Generate a share link; open it as an unrelated user; confirm a read-only preview with no edit controls; copy it into a destination world; confirm a new, independent actor (with cloned sub-data) appears there and that editing either copy never affects the other.

### Implementation for User Story 5

- [X] T039 [US5] Implement `createActorShareLink`/`revokeActorShareLink` mutations in `src/server/src/graphql/mutations_actor_shares.rs` per contracts/actor-share.md — `createActorShareLink` requires `myPermissionLevel == OWNER` (FR-023); `revokeActorShareLink` allows the link's `created_by` OR DM of the actor's world (FR-029)
- [X] T040 [US5] Implement `sharedActor(shareCode: String!)` query in the same file — authenticated-only (no world-membership check), returns the `SharedActorPreview` projection (label/actorType/isNpc/gameSystemId/systemData only — no world/scene/owner ids, research.md §9), or a clear not-available error if revoked/missing (FR-024)
- [X] T041 [US5] Implement `myDmWorlds` query in `src/server/src/graphql/queries/user.rs` (or a new `queries/world.rs`) per research.md §8 — union of worlds where the caller is `created_by` or holds an accepted `GM` `world_members` row
- [X] T042 [US5] Implement `copySharedActorToWorld(input: CopySharedActorInput!)` mutation in `mutations_actor_shares.rs` — re-validates the share link and the caller's DM-level access on `destinationWorldId` server-side, then in one transaction inserts a new `world_actors` row (fresh id, destination world's default scene per research.md §6, `owned_by`/`created_by` = caller, zero permission rows) and clones every `world_actor_system_data` row from the source actor onto the new actor id (FR-026, FR-027, FR-030)
- [X] T043 [P] [US5] Add `cargo test` coverage: `createActorShareLink` requires Owner-level; `sharedActor` rejects a revoked/missing code and never returns world/scene/owner ids; `copySharedActorToWorld` produces a new actor with empty permissions and cloned system-data rows, and re-rejects a caller who lost DM access on the destination between listing and confirming
- [X] T044 [P] [US5] Create `apps/web/src/api/actorShares.ts` with `createActorShareLink`, `revokeActorShareLink`, `sharedActor`, `myDmWorlds`, `copySharedActorToWorld`
- [X] T045 [US5] Add a "Share" action (Owner-level only, reusing `myPermissionLevel`) to `ActorDetailPage.tsx` that calls `createActorShareLink` and shows/copies the resulting link, following `CampaignSettingsPanel.tsx`'s existing invite-link copy-to-clipboard UX pattern
- [X] T046 [US5] Create `apps/web/src/pages/actor-share/SharedActorPage.tsx` at route `/shared/actor/:code` (registered in `AppRoutes.tsx`, nested in `MainLayout`, login-gated the same way `JoinWorldPage.tsx` redirects an unauthenticated visitor to `/login?returnTo=...`): renders the read-only preview, or a "no longer available" state for a revoked/missing code
- [X] T047 [US5] Implement the "Copy to World" flow in `SharedActorPage.tsx`: fetch `myDmWorlds`, show a world picker, require an explicit confirmation step, call `copySharedActorToWorld`, and show a clear success notification naming the destination world; show an explanatory empty state instead of the picker when `myDmWorlds` is empty
- [X] T048 [US5] Create `apps/web/e2e/actor-share.spec.ts`: share → view as unrelated user (no edit controls, no source-world leakage) → copy to a destination world → verify independence (editing either copy doesn't affect the other) → revoke → "no longer available"; also cover the no-eligible-destination-world state

**Checkpoint**: All five user stories are independently functional and testable.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Final verification per Constitution Principle V.

- [X] T049 [P] Run `cargo check` (native target) for `src/server` and resolve any new warnings introduced by this feature
- [X] T050 [P] Run `tsc`/`vite build` for `apps/web` and resolve any new type errors introduced by this feature
- [X] T051 Walk through every scenario in `specs/010-world-staging-actors/quickstart.md` in a running dev instance, including the "Full regression check" section (direct `/play` visit skips `/staging`; `/world/:id` dashboard unchanged)
- [X] T052 Run the existing canvas-authoring e2e suite (`apps/web/e2e/`: wall, lighting, shape, map-import, asset-paste, token tools) unmodified and confirm no regression from `WorldPage.tsx`'s `playView`-state removal (T012)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately.
- **Foundational (Phase 2)**: Depends on Setup (needs the new tables/models to exist) — BLOCKS all user stories.
- **User Stories (Phase 3–7)**: All depend on Foundational completion.
  - US1 and US2 (Phase 3, 4) share the staging-route infrastructure from Foundational and can be built back-to-back or in parallel by different people.
  - US3 (Phase 5) is independent of US1/US2's route work but shares no files with them — can run in parallel with Phase 3/4.
  - US4 (Phase 6) depends on `ActorOwnershipBlock.tsx` (built in US3, T030) being available to embed — sequence US3 before US4, or stub the embed and wire it up last.
  - US5 (Phase 7) depends on `ActorDetailPage.tsx` (built in US4, T035) as the place its "Share" action lives — sequence US4 before US5.
- **Polish (Phase 8)**: Depends on all desired user stories being complete.

### User Story Dependencies

- **US1 (P1)**: No dependencies on other stories beyond Foundational.
- **US2 (P1)**: Builds on the same staging route as US1 (shares `WorldStagingRoutePage.tsx`) but is independently testable (different role, same page).
- **US3 (P1)**: No dependencies on US1/US2; independently testable via the actor query/mutations directly even before US4's routes exist (e.g. via GraphQL playground or a temporary test harness).
- **US4 (P2)**: Soft dependency on US3 for `ActorOwnershipBlock.tsx` to embed in the edit page — the view/edit routes and `updateActor` mutation themselves have no hard dependency on US3.
- **US5 (P2)**: Soft dependency on US4 for a page to host the "Share" action — the backend share/copy mutations themselves have no hard dependency on US4.

### Parallel Opportunities

- T001/T002 (the two migrations) — different directories, parallel.
- T004, T006, T009, T010 — different files, parallel within Phase 2.
- Within US3: T028 (tests) and T029 (API client) — different files, parallel.
- Within US5: T043 (tests) and T044 (API client) — different files, parallel.
- Across stories: once Foundational is done, US1+US2 (frontend-heavy) and US3 (backend-heavy) can be staffed in parallel by different people, converging before US4 starts.

---

## Parallel Example: Foundational Phase

```bash
# After T001-T003 (migrations + schema.rs) are done:
Task: "Add ActorPermission/NewActorPermission and ActorShare/NewActorShare structs to src/server/src/models.rs"
Task: "Add ActorPermissionLevel enum and GraphQL types to src/server/src/graphql/types.rs"
Task: "Add ActorPermissionLevel type and myPermissionLevel field to apps/web/src/types/actor.ts"
Task: "Add worldStaging/actorView/actorEdit/sharedActor entries to apps/web/src/routes/pageLoaders.ts"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL — blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Test User Story 1 independently per its Independent Test above
5. Deploy/demo if ready — a DM can already catalog and add NPCs and reach the canvas, even before ownership/sharing exist

### Incremental Delivery

1. Setup + Foundational → staging route and canvas-only `/play` work for everyone, roster is read-only
2. Add US1 → DM can add NPCs → demo
3. Add US2 → Player experience matches → demo (both P1 stories done)
4. Add US3 → ownership block real, live-play control follows it → demo
5. Add US4 → actors get real, linkable detail routes hosting the ownership block → demo
6. Add US5 → sharing/cross-world copy, the feature's headline capability → demo
7. Polish → verify no regressions, run full quickstart

### Suggested Team Split

- Developer A: Foundational's backend half (T001–T008) then US3 (backend-heavy, Phase 5)
- Developer B: Foundational's frontend half (T009–T013) then US1+US2 (Phase 3–4)
- Converge for US4 (needs both US3's ownership block and the route shell) and US5 (needs US4's page to host sharing)
