---

description: "Task list for feature implementation"
---

# Tasks: Players Section

**Input**: Design documents from `/specs/023-players-section/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/players-section.graphql.md, quickstart.md

**Tests**: Included — matches this project's established convention (every prior spec pairs backend `cargo test` resolver coverage with a Playwright e2e spec per user story).

**Organization**: Tasks are grouped by user story (P1/P2 from spec.md) so each can be implemented, tested, and shipped independently.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1, US2)

## Path Conventions

Existing monorepo layout: `src/server/` (Rust/GraphQL backend), `apps/web/` (React frontend + Playwright e2e under `apps/web/e2e/`). No migrations, no engine-crate changes — see plan.md.

---

## Phase 1: Setup

No setup tasks — no new migration, no new project, no ADR required (Constitution Check in plan.md: nothing here is architecturally significant). Proceed directly to Foundational.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The one backend query extension, the one backend authorization fix, and the frontend route/nav shell every user story's page hangs off.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T001 Add a `claimedActor` field to the type `worldMembers(worldId)` returns in `src/server/src/graphql/queries/invite.rs` (`world_members_impl`/`WorldMembershipPayload`), resolved by joining `world_actor_claims` on `world_member_id = member.id` then `world_actors` for the claimed actor's id/label; `null` when no claim exists (data-model.md, contracts/players-section.graphql.md)
- [X] T002 [P] In `src/server/src/graphql/mutations_invites.rs`, change `update_member_role` and `remove_member`'s caller-identity lookup from a raw `world_members` row fetch to `require_world_member` (the same Owner-fallback helper `is_dm_of_world` already uses) — authorization logic itself (`can_change_roles`/`can_manage`, self-removal rejection) stays unchanged (research.md §3)
- [X] T003 [P] `cargo test` coverage in `src/server/src/graphql/queries/invite.rs` (or its test module) for: `claimedActor` populated when a claim exists, `null` when it doesn't; and in `mutations_invites.rs` for: a world's Owner with no `world_members` row of their own can now successfully call `update_member_role`/`remove_member` (the bug this phase fixes)
- [X] T004 [P] Add `claimedActor: { id, label } | null` to `WorldMemberRecord`/`WorldMemberDoc` and the `worldMembers` query's field list in `apps/web/src/api/worldMembers.ts` / `apps/web/src/hooks/useWorldMembers.ts`; add `updateMemberRole(worldId, userId, role)` and `removeMember(worldId, userId)` wrapper functions to `apps/web/src/api/worldMembers.ts` (currently only inlined ad hoc in `CampaignSettingsPanel.tsx`)
- [X] T005 Add a "Players" entry to `WorldSidebarNav.tsx`'s `categories` array (visible to every member, alongside Scenes/NPCs/Lore/Items/Abilities), linking to `/world/:id/players`
- [X] T006 Add route `/world/:id/players` in `apps/web/src/routes/AppRoutes.tsx` + a `worldPlayers` loader in `apps/web/src/routes/pageLoaders.ts`, pointing at new `apps/web/src/pages/world/players/PlayersRoutePage.tsx` (mirrors `ScenesRoutePage.tsx`: fetches `world`+`isGm`, wraps content in `WorldSectionShell`) rendering a new `apps/web/src/pages/world/players/PlayersPage.tsx` skeleton (branches on `isGm`, both branches fleshed out in US1/US2 below)

**Checkpoint**: Schema/query extension exists, the mutation fix is in place, and the Players section is reachable (even if minimal) — user story phases can now proceed independently.

---

## Phase 3: User Story 1 - Every member browses the roster as characters (Priority: P1) 🎯 MVP

**Goal**: Every world member sees every member paired with their claimed character (or a clear "no character claimed" state), reachable from the sidebar; the roster is gone from Overview.

**Independent Test**: Per spec.md — a non-GM world member opens the Players section and sees every member listed alongside the character each has claimed (or "no character claimed").

- [X] T007 [US1] Build `PlayersPage.tsx`'s base roster: every member from `getWorldMembers(worldId)`, each row showing the member and, when `claimedActor` is non-null, a link to that character's existing detail view (`/world/:id/actor/:actorId/view`) — when null, a clear "No character claimed" label instead of an empty/broken row
- [X] T008 [US1] Remove the player roster list (and its member-fetch) from `apps/web/src/layouts/world-layout/WorldStagingPage.tsx` (Overview); keep `SessionSetupInviteLink` on Overview in its own small panel rather than nested inside the now-removed roster panel
- [X] T009 [P] [US1] Playwright e2e `apps/web/e2e/players-section.spec.ts`: register a GM + two invited members (one claims a character, one doesn't), confirm the Players section shows all three correctly paired/marked, confirm Overview no longer shows a roster (quickstart.md §1)

**Checkpoint**: User Story 1 is independently functional — every member has a real, character-aware roster view.

---

## Phase 4: User Story 2 - GM moderation view with role/removal controls (Priority: P2)

**Goal**: The same Players section gives GM/Owner members role-change and remove-member controls, not shown to anyone else; the dashboard's Campaign Settings panel no longer duplicates this (FR-011).

**Independent Test**: Per spec.md — a GM opens the Players section and successfully changes a member's role and removes a member, while a non-GM member in the same world sees neither control.

- [X] T010 [US2] Add the GM-only branch to `PlayersPage.tsx`: a role `<select>` and a Remove button per row (reusing the authorization/effect semantics of `CampaignSettingsPanel.tsx`'s existing controls, not reinventing them), calling the `updateMemberRole`/`removeMember` wrappers from T004; hidden entirely for non-GM callers
- [X] T011 [US2] Remove the "Player Roster" list block and its embedded role-change/Remove controls from `apps/web/src/components/campaign/CampaignSettingsPanel.tsx`; update its header copy (was "Manage invites and player roster") to reflect its narrower remaining scope (invite generation + the "Allow players to create their own actors" toggle) (FR-011)
- [X] T012 [P] [US2] Playwright e2e `apps/web/e2e/players-section.spec.ts` (extend, or a second file): GM changes a member's role and removes another member from the Players section, confirms both take effect; a non-GM member sees no role/remove controls anywhere on the page; confirms the world dashboard's Campaign Settings panel no longer shows roster/role/remove controls (quickstart.md §2)

**Checkpoint**: Both user stories complete and independently verified — Players section is the sole place to browse-as-characters and to moderate.

---

## Phase 5: Polish & Cross-Cutting Concerns

- [X] T013 Run `cargo check` and `cargo test` for `src/server`, and `tsc`/build for `apps/web`, resolving any new warnings introduced by this feature (Constitution Principle V) — pre-existing warnings/failures unrelated to this feature are not blocking
- [X] T014 Full quickstart.md walkthrough (both sections) against a running dev instance before calling the feature done

---

## Dependencies & Execution Order

- **Foundational (Phase 2)** blocks both user stories — the `claimedActor` field and the mutation fix are shared prerequisites; the route/nav shell is what both stories' pages render into.
- **User Story 1 (Phase 3)**: depends only on Foundational. This is the MVP — deliverable and demoable alone (a read-only, character-aware roster).
- **User Story 2 (Phase 4)**: depends on Foundational (T004's mutation wrappers) and on US1's `PlayersPage.tsx` existing (T007) to add its GM-only branch onto — not independent of US1's file, but independently *testable* once both are done (a non-GM user exercises only US1's surface).
- **Polish (Phase 5)**: depends on both stories being complete.

## Parallel Execution Examples

- Within Foundational: T002, T003, T004 are `[P]` once T001 lands (different files/concerns).
- Within US1: T009 (e2e) is `[P]` with nothing else in its phase (only one other task, T007→T008 sequential since T008 depends on T007's page existing... actually T008 only touches WorldStagingPage.tsx, independent of T007's file — could also run in parallel; marked sequential here only because both are small and easiest to verify in order).
- Across stories: nothing in US2 can start before T007 (US1's page) exists, so full parallelism between US1 and US2 isn't available — expect them sequential in practice despite the formal dependency graph allowing Foundational's own tasks to parallelize.

## Implementation Strategy

**MVP = User Story 1 alone** (Phase 1 → 2 → 3): every member gets a real, character-aware roster, and Overview stops hosting a bare list. Recommended incremental delivery: Foundational → US1 (MVP checkpoint) → US2 (GM moderation, supersedes the dashboard's equivalent controls) → Polish.
