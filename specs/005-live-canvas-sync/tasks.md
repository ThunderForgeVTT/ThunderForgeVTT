---

description: "Task list for Live Cross-Client Canvas Sync via GraphQL Subscriptions"
---

# Tasks: Live Cross-Client Canvas Sync via GraphQL Subscriptions

**Input**: Design documents from `/specs/005-live-canvas-sync/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/subscription-transport.md, quickstart.md

**Tests**: Included — spec.md's Acceptance Scenarios are explicit runnable validation, and this feature's premise (a change in one browser context appears in another, live) is inherently a two-browser-context test-writing task, per specs 001/003/004's established convention.

**Organization**: Tasks are grouped by user story (US1-US3 from spec.md). Unlike spec 004, Foundational here is small (one authorization fix + the new transport module) but still genuinely blocking — no story can deliver its acceptance scenarios without both.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)

## Path Conventions

Existing two-part layout for this feature: `src/server/` (Rust/Axum/GraphQL backend) and `apps/web/` (React frontend). This feature makes no `src/engine` changes.

---

## Phase 1: Setup

**Not applicable as a separate phase.** This feature's only "setup" is the authorization fix and the new transport module, both genuinely blocking for every user story — folded into Phase 2 Foundational below.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Close the authorization gap research found (Constitution Principle III — required, not optional) and stand up the new client-side subscription transport module every user story wires into.

**⚠️ CRITICAL**: No user story's acceptance scenarios can be verified until this phase is complete — none of them work without both a real client transport and a subscription that's safe to actually use.

- [X] T001 [P] Author ADR recording the new GraphQL subscription client transport decision (docs/adrs/`<date>`-0XX-graphql_subscription_client_transport.md) — library choice, cookie-based auth reuse, full-refetch-on-reconnect strategy, and the T002 authorization fix as a corequisite, per research.md §6 and plan.md's Constitution Check (Principle IV, recommended) — **done 2026-08-25, after the fact**: the transport (T002-T011, T017-T018) shipped without this ADR ever being written, discovered during an unrelated e2e audit; written as `docs/adrs/20260825-048-graphql_subscription_client_transport.md`, which is honest that the full-refetch-on-reconnect half (T012-T016 below) never actually shipped.
- [X] T002 Add a world-membership authorization check to `SubscriptionRoot::world_events_created` (`src/server/src/graphql.rs:~1443-1500`), rejecting/not-streaming to a requester who is not a member of the given `worldId`, reusing whatever existing `world_members` lookup already backs authorization elsewhere in this file/`mutations_*.rs` — per research.md §2, a required correction, not optional hardening (Constitution Principle III) — verified in code (`graphql.rs`'s `world_events_created`, membership check present).
- [X] T003 Server test: a user who is not a member of a world attempting to open `worldEventsCreated(worldId)` for that world receives no events / an authorization error — in `src/server/src/graphql.rs`'s test module or a sibling test file, following the existing scene-owner authorization test convention (depends on T002; quickstart.md Scenario 3) — coverage exists in `graphql.rs`'s test module.
- [X] T004 Add the minimal GraphQL-over-WebSocket client dependency (e.g. `graphql-ws`'s client half) to `apps/web/package.json`, per research.md §3's finding that no GraphQL client library exists in `apps/web` today — `"graphql-ws": "^5.16.0"` present.
- [~] T005 Create `apps/web/src/engine/world/sync/transport.ts`: `openWorldEventSubscription(worldId)` returning `{ events, state, close }` per contracts/subscription-transport.md and the `LiveSyncState` shape in data-model.md — connects to `/api/ws` (existing route, cookie-authenticated automatically, no client-side token passing needed per research.md §1/§3) (depends on T004) — **deviated, still true after T014-T016 landed**: shipped as `apps/web/src/engine/world/sync/subscriptionClient.ts`'s `subscribeToWorldEvents(worldId): AsyncIterable<WorldEventLike>` plus (as of T014-T016, 2026-08-25) `getLiveSyncState()`/`subscribeToLiveSyncState()` exposing the `LiveSyncState` shape — the same module, not a separate `transport.ts`, and not the single `{ events, state, close }` object contracts/subscription-transport.md originally specified (three separate exports instead of one bundled return value; `close` isn't exposed at all — the module's one shared connection is a singleton closed only implicitly). Functionally equivalent for every consumer's purposes; naming/shape diverges from the original contract doc, which was never updated to match.

**Checkpoint**: Foundation ready — a safe, authorized, real subscription transport exists. User story implementation can now begin.

---

## Phase 3: User Story 1 - A connected player sees a GM's wall/light/shape edit without reloading (Priority: P1) 🎯 MVP

**Goal**: Wire the transport from Phase 2 into the three existing (but never-fed) inbound event-consumer functions for walls, lights, and shapes, so a change in one connected client appears in another within a few seconds, with no reload.

**Independent Test**: With two browser contexts on the same scene, make a wall/light/shape change in one and confirm it appears in the other within a few seconds, no reload triggered.

### Tests for User Story 1

- [~] T006 [P] [US1] Playwright e2e (two browser contexts): a wall, light, and shape change made in one context appears in the other within a few seconds, no reload — new `apps/web/e2e/live-sync.spec.ts` (FR-002-004; SC-001; quickstart.md Scenario 1) — **deviated**: no dedicated `live-sync.spec.ts` file exists; cross-session wall coverage instead lives in `apps/web/e2e/map-editor-tooling.spec.ts` ("Live cross-session sync" describe block). Needs a clean re-run to confirm current pass/fail (an audit run on 2026-08-25 was confounded by unrelated concurrent load and produced unreliable "canvas not found" failures — not a real signal either way).
- [ ] T007 [P] [US1] Playwright e2e: a client's own change never visibly double-applies, flickers, or reverts when it round-trips back via the subscription (FR-007; SC-002; quickstart.md Scenario 1 step 5) — no dedicated test found for this specific scenario; not confirmed either built or missing.

### Implementation for User Story 1

- [X] T008 [US1] Wire `apps/web/src/engine/world/sync/walls.ts`'s existing `startWallEventSync` to consume `transport.ts`'s `events` for the active `worldId` — no change to `startWallEventSync`'s internal logic (FR-005) — verified: `WorldPage.tsx` dispatches `applyWallWorldEvent` off `subscribeToWorldEvents`.
- [X] T009 [US1] Same wiring for `apps/web/src/engine/world/sync/lights.ts`'s inbound consumer — verified: `applyLightWorldEvent` wired identically.
- [X] T010 [US1] Same wiring for `apps/web/src/engine/world/sync/shapes.ts`'s inbound consumer — verified: `applyShapeWorldEvent` wired identically.
- [X] T011 [US1] Open the subscription (`openWorldEventSubscription`) once per active world when a scene is mounted, in `apps/web/src/pages/world/WorldPage.tsx`, and close it on unmount/world change — verified: `subscribeToWorldEvents(id)` called in `WorldPage.tsx` (via `subscriptionClient.ts`, not a `transport.ts` `openWorldEventSubscription` — see T005's deviation note).

**Checkpoint**: User Story 1 fully functional and independently verified — SC-001/SC-002 confirmed by T006-T007.

---

## Phase 4: User Story 2 - A connected client's live sync survives a brief disconnect (Priority: P2)

**Goal**: Automatic reconnect with exponential backoff, a persistent "reconnecting" indicator with no dead-end state, and a full scene re-fetch on successful reconnect to guarantee correctness for anything missed during the outage.

**Independent Test**: Simulate a network drop, confirm a persistent "reconnecting" indicator (not silent stale data), make a change on another client during the outage, restore connectivity, and confirm the recovered client's full re-fetch picks up that change without a manual reload.

### Tests for User Story 2

- [X] T012 [P] [US2] Playwright e2e: simulate a network drop (devtools offline mode) on one context while another makes a change; restore connectivity; confirm the dropped context automatically reconnects, performs a full re-fetch, and correctly shows the change made during the outage, with no manual reload (FR-008, FR-008a; SC-003; quickstart.md Scenario 2) — **built and passing, 2026-08-25**: `apps/web/e2e/live-sync.spec.ts`, 4/4 consecutive clean runs against a real dev stack. **Deviated from "devtools offline mode"**: a full `context.setOffline(true)` also breaks Vite dev-server's own dynamic module fetches (this app lazy-loads route chunks), crashing the page before the WebSocket drop is even observed — used `page.routeWebSocket`'s `connectToServer()`/`close()` instead, severing only the `/api/ws` connection while leaving HTTP/module loading intact. Confirms reconnect + a fresh `SceneWalls` GraphQL request after restore, not just the indicator clearing.
- [X] T013 [P] [US2] Playwright/manual test: an extended network drop keeps the client retrying indefinitely with a persistent "reconnecting" indicator — no dead-end state requiring manual action (FR-009, FR-009a; SC-004; quickstart.md Scenario 2 step 5) — covered by the same `live-sync.spec.ts` test: asserts `live-sync-reconnecting-indicator` becomes visible and contains "Reconnecting" while the WebSocket is severed.

### Implementation for User Story 2

**BUILT 2026-08-25** (closing the gap the 2026-08-25 audit found): `apps/web/src/engine/world/sync/subscriptionClient.ts` now registers `graphql-ws`'s `on: { connected, closed }` lifecycle callbacks and a `retryWait` backoff, with `retryAttempts: Infinity` so retry never gives up (FR-009a). `apps/web/src/pages/world/WorldPage.tsx` subscribes to the resulting `LiveSyncState` and triggers a full scene re-fetch on every transition into `live` after having been live before (skips the redundant initial-mount transition, which the existing per-resource loader effects already cover). Docs/adrs/20260825-048-graphql_subscription_client_transport.md's "What did NOT ship" section is now stale and should be revisited/updated to reflect this landing.

- [X] T014 [US2] Implement exponential backoff reconnect logic (research.md §5: 1s/2s/4s/8s... capped at 30s) — **deviated**: shipped inside the existing `apps/web/src/engine/world/sync/subscriptionClient.ts` (not a new `transport.ts` — see T005's earlier deviation note) via `graphql-ws`'s `retryWait`/`retryAttempts: Infinity` options, exposing `LiveSyncState`/`getLiveSyncState()`/`subscribeToLiveSyncState()` transitioning between `connecting`/`live`/`reconnecting` with no terminal/dead-end state.
- [X] T015 [US2] Wire the transport's live-sync state to a persistent "reconnecting" UI indicator in `apps/web/src/pages/world/WorldPage.tsx` — `live-sync-reconnecting-indicator` testid, a small non-blocking corner badge (coordinated with spec 004's own centered loading/error overlay in the same file — different screen position, no visual/logic overlap).
- [X] T016 [US2] On every transition into `live` (including after a reconnect, not just initial load), trigger a full re-fetch of the current scene by reusing the existing `loadWallsIntoStore`/`loadLightsIntoStore`/`loadShapesIntoStore`/`loadTokensIntoStore` functions already in `WorldPage.tsx` (research.md §4) — no new fetch logic; gated on a `wasLiveRef` so the very first `connecting`→`live` transition on mount doesn't redundantly re-trigger loaders the existing per-resource effects already ran.

**Checkpoint**: User Story 2 complete — SC-003/SC-004 confirmed by `live-sync.spec.ts`, 2026-08-25.

---

## Phase 5: User Story 3 - Reused by tokens once spec 004 lands (Priority: P3)

**Goal**: Confirm the same transport delivers token changes to connected clients with zero token-specific transport code, using today's existing `upsert_token` path as the interim verification target (spec 004's own token authoring will ride this same wiring once it lands).

**Independent Test**: With the transport from User Story 1 active, confirm a token position change is delivered to a second connected client the same way a wall change is.

### Tests for User Story 3

- [~] T017 [P] [US3] Playwright e2e: a token position change (via today's existing `upsert_token` GraphQL path) made in one context appears in another within a few seconds, via the same transport as walls/lights/shapes — new test in `apps/web/e2e/live-sync.spec.ts` (FR-002-005 applied to tokens; SC-005; quickstart.md Scenario 4) — **deviated**: coverage lives in `apps/web/e2e/token-authoring.spec.ts`, not a dedicated `live-sync.spec.ts`, same pattern as T006.
- [X] T018 [US3] Wire `apps/web/src/engine/world/sync/tokens.ts`'s existing inbound consumer to `transport.ts`'s `events`, identically to T008-T010 (FR-005) — verified: `applyTokenWorldEvent` wired identically to walls/lights/shapes in `WorldPage.tsx`.

**Checkpoint**: User Story 3 fully functional and independently verified — SC-005 confirmed. Directly unblocks spec 004's own SC-002.

---

## Phase 6: User Story 4 - A GM can invite a genuine second player into their world (Priority: P2, folded in post-planning)

**Goal**: Fix the two concrete, already-diagnosed bugs blocking real invite/membership functionality (research.md §7), so a GM can generate a working invite and a genuinely distinct second account can join their world — closing the project-wide gap where every "second session" test today reuses the GM's own login.

**Independent Test**: As a GM, generate an invite code; as a second, distinct user account, redeem it and confirm membership; confirm that account sees scenes under the existing GM-only authoring gates, nothing new.

**Dependencies**: None on Phases 2-5 — entirely different files (`graphql.rs`'s `create_world`, `mutations_invites.rs`, `CampaignSettingsPanel.tsx`). Can be staffed fully in parallel with any other phase.

### Tests for User Story 4

- [~] T019 [P] [US4] Server test: creating a world results in an immediate `world_members` row for the owner — in `src/server/src/graphql.rs`'s test module or a sibling test file (FR-013) — **deviated (see T023)**: no row is inserted; instead `owner_can_be_authorized_for_invites_immediately_after_world_creation` (`mutations_invites.rs::tests`) confirms the owner is immediately *authorized* via `require_world_member`'s existing fallback, which is the functional equivalent FR-013 actually needs. Marked partial rather than done because it doesn't literally match its own original wording (no physical row).
- [X] T020 [P] [US4] Server test: the world's own owner can successfully call `generate_invite_code` for their own world immediately after creating it, with no separate manual membership step (FR-012, FR-013; regression guard for the exact bug found in spec 003) — covered by the same `owner_can_be_authorized_for_invites_immediately_after_world_creation` test (exercises the exact `require_world_member` call `generate_invite_code` now makes); passes, `cargo test --bin thunderforge` 75/75.
- [X] T021 [P] [US4] Playwright e2e: as a GM, generate an invite code via `CampaignSettingsPanel.tsx`'s UI and confirm it succeeds (no argument-shape error); as a second, genuinely distinct user account, redeem the code and confirm membership; confirm that account sees the world's scenes under existing GM-only authoring gates — new `apps/web/e2e/invite-membership.spec.ts` (FR-012-015; SC-006, SC-007; quickstart.md Scenario 5) — passes live, 4/4 consecutive runs against a real dev stack (docker compose + `cargo run` + vite). Uncovered two additional real bugs beyond the two originally diagnosed (see Implementation notes below) — both fixed, not worked around.

### Implementation for User Story 4

- [X] T022 [US4] Fix `apps/web/src/components/campaign/CampaignSettingsPanel.tsx`'s `handleGenerateInvite` (~line 154-165) to send `input: { worldId, maxUses }` matching `GenerateInviteCodeInput`'s actual shape, instead of flat top-level arguments (FR-012) — done. **Scope expanded**: the same snake_case/camelCase argument-shape bug existed in every other GraphQL call in this file (`worldInvites`, `worldMembers` queries; `updateMemberRole`, `removeMember` mutations) and in `JoinWorldPage.tsx`'s `joinWorld` mutation — all fixed, since `loadInvites()`/`loadMembers()` fire on mount and blocked the panel (and the join flow itself) from working at all otherwise. `updateMemberRole`/`removeMember`'s own *authorization* checks (mirroring generate_invite_code's original bug) were left unfixed — out of this story's tested surface (no Acceptance Scenario exercises role change/removal) but flagged here for a future pass.
- [~] T023 [US4] Fix `src/server/src/graphql.rs`'s `create_world` (~line 1182-1213) to insert a `world_members` row for the world's own owner in the same transaction as world creation (FR-013) — **deliberately not done; deviated from the plan.** Reading `auth/world_membership.rs`'s `require_world_member` doc comment (and `test_support.rs`'s `insert_test_world` doc comment) revealed this exact gap was already known and already deliberately compensated for by a purpose-built fallback (`require_world_member` falls back to `worlds.created_by` when no row exists) — its own comment explicitly frames inserting a row in `create_world` as "a separate, deliberate follow-up, not done as part of this cleanup." The actual bug was that `generate_invite_code`/`world_invites` never adopted that existing fallback, instead keeping raw `world_members`-only lookups from before the fallback existed. Fixed by routing both through `require_world_member` (see T022's file list — `mutations_invites.rs`, `queries/invite.rs`) rather than inserting a row, which avoids a new uniqueness/duplicate-row risk and matches the codebase's own already-established intent. `create_world` itself was not touched.
- [X] T024 [US4] Confirm (via T020/T021) that `generate_invite_code`'s existing authorization check (`mutations_invites.rs`) now succeeds for a world's own owner with no other change needed (FR-015) — confirmed; no new/parallel authorization path was added, `require_world_member` is the same single existing mechanism every other data-boundary check in this codebase already uses.

**Additional bugs found and fixed while making T021 pass live (not in the original two-bug diagnosis)**:
1. `CampaignSettingsPanel.tsx`'s `worldInvites`/`worldMembers` queries and `updateMemberRole`/`removeMember` mutations all had the identical snake_case-vs-camelCase argument-shape bug as `generateInviteCode` — blocking the panel's own mount-time `loadInvites()`/`loadMembers()` calls, independent of the two originally-diagnosed bugs.
2. `JoinWorldPage.tsx`'s `joinWorld` mutation had the same bug (`code: $code` flat instead of `input: JoinWorldInput!`) — this one directly blocked T021's join step; found via live network inspection (server returned a clear GraphQL schema-mismatch error) after the click silently produced no visible error in the UI.
3. `generate_invite_code`'s invite-code generation took the first 8 hex characters of a **v7** (time-ordered) UUID for the human-facing code — colliding on `world_invites_invite_code_key` whenever two invites are generated within the same millisecond (trivially reproducible by this test's own back-to-back runs, and a real latent production risk under any concurrent invite generation). Fixed by deriving the code from an independent `Uuid::new_v4()` instead, keeping the row's own `id` as v7 for index locality.

**Checkpoint**: User Story 4 fully functional and independently verified — SC-006/SC-007 confirmed, live, against a real running dev stack, with a genuinely distinct second account (first such test in the project).

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final verification spanning all four stories, plus confirming no regression to existing outbound behavior.

- [X] T025 [P] Run native `cargo test` on `src/server` (including T003's and T019-T020's new tests) and `cargo check --target wasm32-unknown-unknown` on `src/engine` (unaffected by this feature but must still compile clean), resolving any new warnings (Constitution Principle V) — 2026-08-25: `cargo test --bin thunderforge` 292/292 passing, `cargo check --target wasm32-unknown-unknown -p thunderforge_engine` clean, no new warnings.
- [~] T026 [P] Run `tsc`/build on `apps/web` and execute the full existing `apps/web/e2e/canvas-authoring.spec.ts` suite alongside this feature's new `live-sync.spec.ts` and `invite-membership.spec.ts`, confirming no regression to the existing outbound mutation bridges (FR-006) — `tsc` clean (2026-08-25, only pre-existing unrelated errors). e2e portion NOT re-verified: an audit run of `map-editor-tooling.spec.ts` (which carries the actual wall live-sync coverage — see T006's deviation note) was confounded by unrelated concurrent live-browser load from another in-progress investigation and produced unreliable failures; needs a clean re-run with exclusive dev-server access.
- [~] T027 Execute quickstart.md Scenarios 1-5 end-to-end against a running local dev stack, confirming SC-001 through SC-007 all hold together — Scenario 2 (reconnect/resync) now confirmed via `live-sync.spec.ts`, 2026-08-25. Scenarios 1/3/4/5 still need a clean, uncontended full run to confirm together (blocked on this sandbox's separate, unrelated "headless Chromium can't render the Bevy canvas" limitation for anything requiring canvas interaction — not a gap in this feature).
- [X] T028 [P] Coordinate the final state of `apps/web/src/pages/world/WorldPage.tsx` with spec 004's own changes to the same file (loading/error UI vs. reconnect-triggered resync) — done: the reconnect indicator (`live-sync-reconnecting-indicator`, top-right corner) and spec 004's `sceneLoadState` overlay (centered) render as separate, non-overlapping blocks with no shared state or logic; no merge conflict.
- [X] T029 [P] Re-run the project's e2e and unit-test gap analyses' specific findings: confirm at least one test now exercises a genuine non-owner account (SC-007), and confirm the invite mutations no longer show zero test coverage — confirmed via T021 (`invite-membership.spec.ts`, a genuinely distinct second account) and T020/T024's `mutations_invites.rs` coverage.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Foundational (Phase 2)**: T002 (authorization fix) blocks T003 (its test). T004 blocks T005 (transport module needs the dependency). T001 (ADR) has no hard code dependency but should land alongside T002 conceptually. BLOCKS User Stories 1-3 — none of their acceptance scenarios are achievable without T002/T005 existing. Does NOT block User Story 4 (different files entirely).
- **User Story 1 (Phase 3)**: Depends only on Phase 2.
- **User Story 2 (Phase 4)**: Depends on Phase 2 and, practically, on US1's wiring (T008-T011) existing in the same `transport.ts`/`WorldPage.tsx` files it extends — sequenced after US1 though independently testable once its own tasks land.
- **User Story 3 (Phase 5)**: Depends on Phase 2 only; independent of US2, and can be done in parallel with US2 since it touches a different sync file (`tokens.ts`) than US2's `transport.ts`/`WorldPage.tsx` reconnect work.
- **User Story 4 (Phase 6)**: Depends on nothing else in this spec — fully independent, different files (`graphql.rs`'s `create_world`, `mutations_invites.rs`, `CampaignSettingsPanel.tsx`). Can be done first, last, or fully in parallel with Phases 2-5.
- **Polish (Phase 7)**: Depends on all four user stories being complete.

### Parallel Opportunities

- T001 in parallel with T002-T003 (different files, ADR doc vs. code).
- T006-T007 (US1 tests) in parallel with each other.
- **Phase 6 (US4) can run fully in parallel with Phases 2-5** — zero file overlap with the subscription-transport work.
- T012-T013 (US2 tests) in parallel with each other.
- **US3 (Phase 5) can be staffed in parallel with US2 (Phase 4)** — different files (`tokens.ts` vs. `transport.ts`/`WorldPage.tsx`).
- T019-T021 (US4 tests) in parallel with each other.
- T025, T026, T028, T029 (Phase 7) in parallel.

---

## Parallel Example: Foundational phase

```bash
Task: "T001 Author ADR for subscription client transport"
Task: "T002 Add world-membership check to world_events_created in src/server/src/graphql.rs"
```

## Parallel Example: US3 alongside US2

```bash
# Different files — no conflict:
Task: "T018 Wire tokens.ts to transport.ts"          # US3
Task: "T014 Implement exponential backoff in transport.ts"  # US2
```

## Parallel Example: US4 alongside everything else

```bash
# Zero file overlap with the transport work — safe to run anytime:
Task: "T022 Fix CampaignSettingsPanel.tsx's invite mutation call"   # US4
Task: "T023 Fix create_world to insert an owner world_members row"  # US4
Task: "T008 Wire walls.ts to transport.ts"                          # US1, unrelated files
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 2 (Foundational) — the authorization fix is non-negotiable before any real client traffic flows through this subscription.
2. Complete Phase 3 (US1).
3. **STOP and VALIDATE**: Run T006-T007, confirm SC-001/SC-002.
4. This retroactively makes specs 001-004's "connected client sees the change live" claims actually true for walls/lights/shapes.

### Incremental Delivery

1. Foundational → authorized, working transport ready.
2. US1 → live sync for walls/lights/shapes (MVP).
3. US2 → reconnect resilience.
4. US3 → tokens ride the same transport, unblocking spec 004's SC-002.
5. US4 (invite/membership fix) → independent of 1-4, can land at any point in this sequence, including first.
6. Polish → final cross-story verification.

### Suggested Team Split

- One track: Phase 2 + Phase 3 (Foundational + US1) — the authorization fix and initial wiring are tightly coupled.
- Another track: Phase 4 (US2) — reconnect/backoff logic, once Phase 2 lands.
- Another track: Phase 5 (US3) — a single small wiring task, can be picked up by anyone once Phase 2 lands.
- Another track: Phase 6 (US4) — entirely independent, can be picked up immediately by anyone with zero coordination needed with the other three tracks.
