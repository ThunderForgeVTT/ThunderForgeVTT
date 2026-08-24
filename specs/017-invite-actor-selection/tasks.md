---

description: "Task list for feature implementation"
---

# Tasks: Player Onboarding — Invite-to-Actor Selection

**Input**: Design documents from `/specs/017-invite-actor-selection/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/graphql-actor-claim.md, quickstart.md (all present)

**Tests**: Inline `#[tokio::test]` resolver tests colocated with mutations (matching `mutations_actors.rs`/`mutations_moderation.rs` convention), plus a Playwright e2e for the concurrent-claim race (FR-006) and the full onboarding flow.

## Format: `[ID] [P?] [Story] Description`

## Path Conventions

Existing `src/server` (Rust/Axum/Diesel/async-graphql) and `apps/web` (React/TS) structure. No new crate, no engine/wasm surface.

---

## Phase 1: Setup

- [X] T001 Create Diesel migration `add_actor_claiming` in `src/server/migrations/<ts>_add_actor_claiming/{up,down}.sql`: `ALTER TABLE world_actors ADD COLUMN available_for_claim BOOLEAN NOT NULL DEFAULT false;`, `ALTER TABLE worlds ADD COLUMN allow_player_created_actors BOOLEAN NOT NULL DEFAULT false;`, `CREATE TABLE world_actor_claims (id UUID PRIMARY KEY DEFAULT gen_random_uuid(), actor_id UUID NOT NULL UNIQUE REFERENCES world_actors(id) ON DELETE CASCADE, world_member_id UUID NOT NULL UNIQUE REFERENCES world_members(id) ON DELETE CASCADE, claimed_at TIMESTAMPTZ NOT NULL DEFAULT now());` (data-model.md) — run `diesel migration run` and confirm `schema.rs` auto-regenerates with the new table/columns
- [X] T002 [P] Add `ActorClaim`/`NewActorClaim` Diesel `Queryable`/`Insertable` structs to `src/server/src/models.rs`; add `available_for_claim: bool` to `WorldActor`/`NewWorldActor`; add `allow_player_created_actors: bool` to the `World`/`NewWorld` structs (wherever the existing `session_notes`/`game_system_id` fields live) (depends on T001)

**Checkpoint**: Schema exists, Diesel models compile.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared GraphQL types and the authorization/atomicity primitives every user story's mutations depend on.

- [X] T003 [P] Add `GraphQLActorClaim` type to `src/server/src/graphql/types.rs` (contracts/graphql-actor-claim.md's shape: `actorId`, `actor` (resolved via `#[ComplexObject]`, loads the `GraphQLActor`), `worldMemberId`, `claimedByUserId`, `claimedAt`)
- [X] T004 Extend `GraphQLActor` in `types.rs` with `#[graphql(complex)]` fields `availableForClaim: bool` (flat passthrough) and `claimedBy: Option<GraphQLWorldMember>` (queries `world_actor_claims` joined to `world_members`/`users` for this actor) (depends on T002, T003)
- [X] T005 [P] Extend `GraphQLWorld` in `types.rs` with `allowPlayerCreatedActors: bool` flat field (depends on T002)
- [X] T006 Create `src/server/src/graphql/mutations_actor_claims.rs` with the module skeleton and a shared internal helper `require_no_existing_claim(conn, world_id, user_id) -> GraphQLResult<WorldMember>` (looks up the caller's `world_members` row for `world_id`, errors if a non-GM role AND a claim already exists in `world_actor_claims` for that member) — used by both `claimActor` and `createAndClaimActor` (depends on T002)

**Checkpoint**: Shared types and authorization helper exist — user-story mutations can be implemented.

---

## Phase 3: User Story 1 - A joining player picks a GM-designated character (Priority: P1) 🎯 MVP

**Goal**: A non-GM member with no claim is routed to Actor Selection, sees exactly the GM-designated available characters, and can claim one atomically.

**Independent Test**: Per spec.md's US1 Independent Test — mark two actors available, join as a new player, confirm the list, claim one, confirm it disappears for the next joiner.

### Implementation for User Story 1

- [X] T007 [US1] Implement `availableActors(worldId)` query in `src/server/src/graphql/queries/actor.rs` (extend existing file, follow its `_impl` free-function + thin resolver pattern): `world_actors` filtered by `world_id`, `is_npc = false`, `available_for_claim = true`, NOT EXISTS in `world_actor_claims`, passed through `crate::moderation::filter_visible` (contracts/graphql-actor-claim.md)
- [X] T008 [US1] Implement `myActorClaim(worldId)` query in `queries/actor.rs`: returns `None` for the GM/Owner role or a member with no claim row; otherwise the `GraphQLActorClaim` (contracts/graphql-actor-claim.md)
- [X] T009 [US1] Implement `claimActor(worldId, actorId)` mutation in `mutations_actor_claims.rs`: transaction — verify actor belongs to `worldId` and is currently available (T007's filter, re-checked server-side), verify caller via T006's helper, `INSERT` the claim row, map a unique-constraint violation on `actor_id` to the "just claimed by someone else" error (research.md §4) (depends on T006, T007)
- [X] T010 [US1] Wire `mutations_actor_claims` and the two new queries into the GraphQL schema root in `src/server/src/graphql.rs` (depends on T008, T009)
- [X] T011 [US1] Resolver tests in `mutations_actor_claims.rs`/`queries/actor.rs`: non-member rejected; GM never gated (myActorClaim returns None regardless of claim state); claiming an unavailable/already-claimed actor errors; claiming succeeds and the actor disappears from a subsequent `availableActors` call; a member who already has a claim cannot claim a second actor (FR-014) (depends on T009, T008, T007)
- [ ] T012 [US1] Create `apps/web/src/api/actorClaims.ts`: `getMyActorClaim(worldId)`, `getAvailableActors(worldId)`, `claimActor(worldId, actorId)` — fetch-based GraphQL calls mirroring `api/items.ts`'s pattern (depends on T010)
- [ ] T013 [P] [US1] Create `apps/web/src/types/actorClaim.ts`: `ActorClaimRecord`, `AvailableActorRecord` TS types (depends on T012)
- [ ] T014 [US1] Create `apps/web/src/pages/world/ActorSelectionPage.tsx` (`/world/:id/actor-select`): fetches `myActorClaim`+`availableActors` on mount; if already claimed, redirects to `/world/:id`; renders the list of available characters with a "Select" action per character; renders the "ask your GM" wait state (FR-010) when the list is empty and (for now, before T0xx in US2) create-your-own isn't shown; on claim success, redirects to `/world/:id` (depends on T012, T013)
- [ ] T015 [US1] Add the `/world/:id/actor-select` route to `apps/web/src/routes/AppRoutes.tsx` (depends on T014)
- [ ] T016 [US1] Add the routing gate: wherever a non-GM member currently lands on `/world/:id` after `joinWorld` (in `JoinWorldPage.tsx`'s `handleJoin`) and on direct visits to the world-entry route, check `myActorClaim` (skip entirely for GM/Owner role, FR-003) and redirect to `/world/:id/actor-select` when null — implement as a shared check (e.g. a small hook `useActorClaimGate` or inline in the world-entry page component) so it applies both right after joining AND on any later revisit while unclaimed (research.md §5) (depends on T012)
- [ ] T017 [US1] Playwright e2e in `apps/web/e2e/actor-claim.spec.ts` (US1 scenarios): GM marks two actors available, new player joins, lands on Actor Selection, sees exactly those two, claims one, revisits later and goes straight to the world dashboard (not Actor Selection again) (depends on T016)

**Checkpoint**: User Story 1 fully functional and independently testable.

---

## Phase 4: User Story 2 - A joining player creates their own character (Priority: P2)

**Goal**: When `allow_player_created_actors` is on, the Actor Selection screen offers "create your own," auto-claimed on creation.

**Independent Test**: Per spec.md's US2 Independent Test.

### Implementation for User Story 2

- [X] T018 [US2] Implement `createAndClaimActor(worldId, name, description)` mutation in `mutations_actor_claims.rs`: re-checks `worlds.allow_player_created_actors` server-side (FR-008, not just client-hidden), re-checks caller has no existing claim via T006's helper, inserts a new `world_actors` row (`is_npc = false`, `available_for_claim = true`, ownership fields set to the creating user per the existing `world_actors` insert convention) and its claim row in one transaction (contracts/graphql-actor-claim.md) (depends on T006)
- [ ] T019 [US2] Add `allowPlayerCreatedActors` to the GM's world-settings surface — extend whichever page already exposes `session_notes`/game-system settings (check `WorldStagingPage.tsx` or a dedicated world-settings page) with a toggle wired to a `setWorldAllowPlayerCreatedActors(worldId, value)` mutation (add this small mutation alongside `mutations_actor_claims.rs` or wherever world-level settings mutations already live) (depends on T005)
- [ ] T020 [P] [US2] Add `createAndClaimActor` to `apps/web/src/api/actorClaims.ts` (depends on T018)
- [ ] T021 [US2] Extend `ActorSelectionPage.tsx` to show a "create your own character" form/option when `world.allowPlayerCreatedActors` is true (re-fetched live, not cached from join time — FR-008), calling `createAndClaimActor` on submit (depends on T020, T014)
- [X] T022 [US2] Resolver tests: `createAndClaimActor` rejected when the setting is off (even if called directly, bypassing the UI); succeeds and auto-claims when on; a member with an existing claim cannot call it again (depends on T018)
- [ ] T023 [US2] [P] Playwright e2e addition in `actor-claim.spec.ts`: GM turns setting on, new player creates a character, auto-claimed; GM turns setting off, a different new player sees no create option and a direct API call is rejected (depends on T021, T022)

**Checkpoint**: User Stories 1-2 both work independently.

---

## Phase 5: User Story 3 - The GM manages availability and claims (Priority: P2)

**Goal**: GM can mark/unmark availability and un-claim, from existing Actor management surfaces.

**Independent Test**: Per spec.md's US3 Independent Test.

### Implementation for User Story 3

- [X] T024 [US3] Implement `setActorAvailability(actorId, available)` mutation in `mutations_actor_claims.rs`: requires Owner-level Actor permission via the existing `auth::actor_permissions::require_actor_permission` (reused verbatim, no new authority per research.md §6); rejects if `is_npc = true` and `available = true` (contracts/graphql-actor-claim.md); idempotent (depends on T002)
- [X] T025 [US3] Implement `unclaimActor(actorId)` mutation in `mutations_actor_claims.rs`: same Owner-level check; `DELETE FROM world_actor_claims WHERE actor_id = ...`, no-op if none exists; does NOT touch `available_for_claim` (data-model.md's validation rules) (depends on T002)
- [X] T026 [US3] Wire `setActorAvailability`/`unclaimActor` into the schema root in `graphql.rs` (depends on T024, T025)
- [X] T027 [US3] Resolver tests: non-Owner caller rejected for both mutations; `setActorAvailability(true)` on an NPC-classified actor rejected; un-claim makes the actor reappear in `availableActors` (if still flagged available) without needing to re-flag it; un-claim leaves the previous claimant as a world member (their `world_members` row untouched) (depends on T024, T025)
- [ ] T028 [US3] Add "Available for claiming" toggle and "claimed by" display (using T004's `availableForClaim`/`claimedBy` fields) to the actor detail view a DM sees (wherever `ActorDetailPage.tsx`'s ownership-block section lives, following the same DM-only-visible pattern used for `ItemOwnershipBlock`), plus an "Un-claim" action when `claimedBy` is set (depends on T026)
- [ ] T029 [US3] [P] Playwright e2e addition: GM toggles availability on an actor detail page, confirms it appears on Actor Selection for a joining player; after a claim, GM sees who claimed it and un-claims; confirms it reappears as available and the prior claimant is routed back to Actor Selection on next visit (depends on T028, T016)

**Checkpoint**: All three user stories independently functional.

---

## Phase 6: Cross-Cutting (Session Setup invite link, FR-015)

- [ ] T030 [P] Add the invite-URL display to `apps/web/src/layouts/world-layout/WorldStagingPage.tsx`, reusing the existing invite-generation mutation/hook (`useWorldInvites`/`generateInviteCode.ts`) already wired to the world dashboard's "Generate Join Link" control — no new server mutation needed (FR-015/SC-005)

---

## Phase 7: Polish & Cross-Cutting Concerns

- [ ] T031 Concurrency test: two simulated simultaneous `claimActor` calls for the same actor (via two DB connections/transactions in a resolver test, or a Playwright two-context race) — confirm exactly one succeeds and the other gets the "just claimed" error (FR-006/SC-003)
- [ ] T032 [P] Run `cargo check`/`cargo test` in `src/server` (native — no wasm surface for this feature)
- [ ] T033 [P] Run `cargo clippy --all-targets` on `src/server`, fix any new warnings (workspace is currently clippy-clean)
- [ ] T034 [P] Run `pnpm --filter @thunderforge/web build` and eslint on new/touched frontend files
- [ ] T035 Execute every scenario in `specs/017-invite-actor-selection/quickstart.md` against a running local dev stack in a real browser, including the two-browser-context concurrency check and the "GM's own invite link never routes to Actor Selection" check
- [ ] T036 [P] Confirm `./scripts/check-file-length.sh` shows no new failures introduced by this feature's files

---

## Dependencies & Execution Order

- **Setup (Phase 1)**: No dependencies.
- **Foundational (Phase 2)**: Depends on Setup — blocks all user stories.
- **US1 (Phase 3)**: Depends on Foundational. Independent MVP once done.
- **US2 (Phase 4)**: Depends on Foundational + T006/T014 from US1 (extends the same Actor Selection page and reuses the claim-check helper).
- **US3 (Phase 5)**: Depends on Foundational only (T002/T004) — independent of US1/US2's mutations, though its Playwright test (T029) reuses US1's routing gate (T016) to observe the "returns to Actor Selection" behavior.
- **Cross-cutting (Phase 6)**: Independent of all user stories — can run any time after Setup.
- **Polish (Phase 7)**: Depends on all desired user stories being complete.

### Parallel Opportunities

- T002 alongside nothing (single task, blocking).
- T003, T005 (Phase 2) in parallel once T002 lands.
- T013, T020 (Phase 3/4, different files) parallelizable with their phase's sequential tasks once their dependencies land.
- T023, T029 (Playwright additions in different describe blocks of the same file) can be written in parallel then merged.
- T032-T034, T036 (Phase 7) in parallel.

---

## Implementation Strategy

### MVP First

1. Phase 1 (Setup) + Phase 2 (Foundational).
2. Phase 3 (US1) — GM-designated claiming, the core value.
3. **STOP and VALIDATE**: quickstart.md's US1 scenarios against a running server.
4. Phase 5 (US3) — GM management surfaces (needed to actually mark actors available in the first place, so validate this early even though it's P2).
5. Phase 4 (US2) — player-created characters.
6. Phase 6 + Polish.

Note: although US3 is P2 in spec.md's own prioritization (it's the "how a GM makes US1 possible at all" surface, explicitly called out as P2 only because a GM could out-of-band coordinate for a first release per spec.md's own reasoning), implementing it directly after US1 rather than strictly last is recommended — without it there is no way to actually mark an actor available to manually test US1 beyond direct DB/GraphQL manipulation.
