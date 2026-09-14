---
description: "Task list for spec 051, pausing a world's play"
---

# Tasks: Pausing a World's Play

**Input**: Design documents from `/specs/051-operator-playfield-shutdown/`

**Prerequisites**: [plan.md](plan.md), [spec.md](spec.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/graphql.md](contracts/graphql.md), [contracts/live-play-lock.md](contracts/live-play-lock.md), [quickstart.md](quickstart.md)

**Tests**: Required. FR-060 to FR-064 make end-to-end proof in a real browser part of the feature, and unit tests alone are not accepted as proof. Every story ends with an e2e that is **run**, not only written.

**Organization**: By user story, in the spec's priority order. The plan's six phases map onto them: plan phase 1 is Setup and Foundational here, 2 is US1, 3 is US2, 4 is US3, 5 is US4 and US5, and 6 is Polish.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1 … US5, from spec.md

## Standing rules for every task

- **Stage explicit paths.** Never `git add -A`, because parallel agents are running. Commits are signed, so check `git config commit.gpgsign` and never pass `--no-gpg-sign`.
- **Verify per crate:**
  - `cd src/server && cargo test -q --lib <module>` for server changes.
  - `cd apps/web && pnpm exec tsc --noEmit` for web changes. `pnpm verify` does not type-check the web app.
  - `cargo check --target wasm32-unknown-unknown -p thunderforge_engine`, only if an engine file changed.
- **The e2e external stack** needs `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1` and `--workers=1`.
- **Offline in e2e:** never use `context.setOffline`. Use `severableLink`, `waitForOffline` and `waitForOnline` from `apps/web/e2e/fixtures/offline.ts`.
- **Process-global flakes:** `settings::resolver` and `settings_migration_tests` can fail under parallel threads. Re-run at `--test-threads=1` before blaming a change.
- **Error codes and names are fixed by the contracts.** The code is `WORLD_PLAY_PAUSED`, the event code is 28, and the rejection reason is `PlayPaused`.

---

## Phase 1: Setup

**Purpose**: The authority decision, and the one new dependency.

- [x] T001 Get ADR-100 accepted by the owner in `docs/adrs/20260913-100-an_operator_can_pause_a_worlds_play.md`. Update its **Status** line and the row in `docs/adrs/README.md`. **Phase 3 (US1) must not start until this is accepted.** Phases 1–2 are reversible and may proceed meanwhile.
- [x] T002 [P] Add `@axe-core/playwright` as a dev dependency in `apps/web/package.json` (pin an exact version, `pnpm install`). Add a helper `expectNoAxeViolations(page, selector?)` in `apps/web/e2e/fixtures/axe.ts`.
- [x] T003 [P] Remove the unused, membership-less world socket (research R7):
  1. Confirm nothing in `apps/` or `packages/` connects to `/api/events`: `grep -rn "api/events" apps packages --include=*.ts --include=*.tsx`.
  2. Drop the `events_router` from `src/app/src/main.rs` (lines ~646-668) and `websocket_handler` from `src/server/src/network/ws.rs` and `src/server/src/network/mod.rs`.
  3. Delete `session::connect_player` and `disconnect_player` in `src/server/src/session.rs` only if `grep -rn` shows no other caller. Leave `touch_last_seen` and the cleanup task if anything still uses them.
  4. Record what was kept and why in the commit message.

---

## Phase 2: Foundational

**Purpose**: The tables, the gate, pausing and lifting at the service layer, and the shared signals. Every story depends on these.

**⚠️ No user story work begins until this phase's checkpoint passes.**

- [x] T004 Create the migration `src/server/migrations/<timestamp>_pausing_a_worlds_play/up.sql` and `down.sql` exactly per `data-model.md`. Take the timestamp after the newest directory in `src/server/migrations/`.
  - **Enums:** `"PauseRequestState"` (`Pending`, `Approved`, `Declined`) and `"PauseTriggerKind"` (`Takedown`, `Operator`, `AbuseReport`).
  - **Tables:** `world_play_pauses`, `world_play_pause_requests`, `world_play_pause_triggers` and `world_live_play`, with every CHECK and partial unique index listed.
  - **Guard triggers:** a pause may only be lifted, once; a decided request may not change; neither may be deleted.
  - No FK to `worlds`/`users` except `world_live_play.world_id … ON DELETE CASCADE`.
  - `down.sql` drops exactly what `up.sql` created.
  - Run `diesel migration run` and `diesel migration redo` in `src/server`.
- [x] T005 Regenerate `src/server/src/schema.rs` (`diesel print-schema`). Check that print-schema did not re-add removed pack tables (see commit `a9c7542`), and hand-correct if it did.
- [x] T006 [P] Add the migration test `src/server/src/play_pause/migration_tests.rs`. It must prove:
  - the second active pause for a world is refused;
  - a second pending request is refused;
  - blank grounds are refused;
  - a lifted pause cannot be re-lifted or un-lifted;
  - a decided request cannot change;
  - deleting from any of the three record tables is refused;
  - a trigger with both or neither owner is refused;
  - deleting a world leaves its pause rows and removes its `world_live_play` row;
  - `down.sql` leaves nothing behind.
- [x] T007 Create the module `src/server/src/play_pause/mod.rs` and register it in `src/server/src/lib.rs`. Add Diesel models for the four tables in `src/server/src/play_pause/models.rs`, with `PauseRequestState` and `PauseTriggerKind` as Diesel enums.
- [x] T008 Implement the gate in `src/server/src/play_pause/gate.rs`:
  - `refuse_if_paused(conn, world_id)`, which reads only the active-pause partial index.
  - `refuse_scene_if_paused(conn, scene_id)`, which resolves the scene's world first.
  - A `PlayPaused { world_id, paused_at }` error with `impl From<PlayPaused> for async_graphql::Error`. It sets extensions `code = "WORLD_PLAY_PAUSED"`, `worldId` and `pausedAt`, and the message "Play in this world has been paused by an operator." It never includes grounds or who paused it.
  - It fails closed when the database cannot be read.
- [x] T009 [P] Add gate tests in `src/server/src/play_pause/gate_tests.rs`:
  - it refuses on an active pause and passes on a lifted one or none;
  - the scene variant resolves correctly;
  - it refuses a site admin who is a member (it is blind to `is_admin`, per ADR-100 decision 2);
  - the error's extensions carry no grounds.
- [x] T010 Implement `pause_world(conn, operator, world_id, grounds, trigger)` in `src/server/src/play_pause/mod.rs`. In one transaction it:
  - validates non-blank grounds (`GROUNDS_REQUIRED`) and that the world exists (`WORLD_NOT_FOUND`);
  - snapshots the world name and the operator's display name;
  - inserts the pause with `created_by`/`updated_by`;
  - attaches the trigger (`Operator` by default);
  - records the world event with code 28 carrying `pausedAt` only.

  If the partial unique index refuses because a pause already exists, it records an `Operator` trigger, with the grounds as its `note`, on the existing pause and returns `alreadyPaused: true` (FR-036).
- [x] T011 Implement `lift_pause(conn, operator, pause_id, grounds)` in `src/server/src/play_pause/mod.rs`. It is a conditional update `WHERE id = $1 AND lifted_at IS NULL RETURNING`. Zero rows returns `PAUSE_ALREADY_LIFTED`, carrying `liftedBy`/`liftedAt` read back. It touches no content, membership or moderation table (FR-041, FR-042).
- [x] T012 [P] Add `EVENT_CODE_WORLD_PLAY_PAUSED: i32 = 28` with a doc comment in `src/server/src/world_events.rs`. Mirror it wherever the web app lists event codes (find with `grep -rn "EXPLORATION_RESET\|= 27" apps/web/src`). *Done server-side. The web app keeps no list: each consumer declares its own code beside its handler (`exploration.ts`, `sceneLighting.ts`), so the constant lands with its consumer, T023's event-28 handler in `WorldPage.tsx`, not as an unused export now.*
- [x] T013 [P] Add `RejectionReason::PlayPaused` with a doc comment in `crates/thunderforge-cache-core/src/queue.rs`. Add the matching variant to the server's reason enum in `src/server/src/graphql/mutations_reconcile.rs`, and to the web mirror (find with `grep -rn "GoneAway" apps/web/src`). Regenerate ts-rs bindings if the enum is exported.
- [x] T014 Add `until_stream_must_end(state, session_id, world_id, inner)` in `src/server/src/graphql/session_lifetime.rs`, per `contracts/live-play-lock.md`.
  - One query per `LIVENESS_POLL` tick: session live AND no active pause for the world.
  - Session gone: the stream completes.
  - Paused: it yields exactly one `Err` with code `WORLD_PLAY_PAUSED`, then completes.
  - Unreadable: it carries on.

  Keep `until_session_ends` for streams that are not world-scoped. Extend the module doc to say why the pause shares the session's poll (research R1).
- [x] T015 [P] Add stream tests in `src/server/src/graphql/session_lifetime.rs` `mod tests`, modelled on the revoked-session test:
  - a wrapped stream yields one pause error and completes within one tick after `pause_world`;
  - a lifted pause does not end a stream;
  - a revoked session still completes without an error item.
- [x] T016 [P] Add service tests in `src/server/src/play_pause/pause_tests.rs`:
  - pause writes the event and trigger;
  - blank grounds are refused;
  - pausing a paused world returns `alreadyPaused` and adds a trigger rather than a pause;
  - lift sets the columns once, and a second lift reports who lifted;
  - lift leaves a taken-down scene's moderation status unchanged.

**Checkpoint**:
- `cd src/server && cargo test -q --lib play_pause` passes.
- `cargo test -q --lib session_lifetime` passes.
- `cargo check` for the server is clean with no new warnings.

Commit.

---

## Phase 3: User Story 1 — Pause a world's play at once (Priority: P1) 🎯 MVP

**Goal**: An operator pauses a world from the admin portal. Every browser playing it, on any scene, lands on a calm notice within 5 seconds, without reloading. Other worlds are untouched.

**Independent Test**: quickstart scenario 1 (`play-pause.spec.ts`).

**Requires**: T001 accepted.

### Tests for User Story 1

- [ ] T017 [P] [US1] Write the shared e2e fixture `apps/web/e2e/fixtures/playPause.ts`:
  - `pauseWorldAsOperator(adminPage, worldId, grounds)` through GraphQL on the admin session;
  - `liftPauseAsOperator`, `decideRequestAsOperator` and `pendingRequestFor(worldId)`;
  - `expectPausedNotice(page, worldName)`, which asserts the heading, that no grounds text is present, and that no `reload` navigation happened.

  Reuse `openAdminPage` from `fixtures/admin.ts`.
- [ ] T018 [US1] Write `apps/web/e2e/play-pause.spec.ts`:
  1. A Game Master and an invited player join one world in two browser contexts on **different scenes**, using `inviteAndJoinAsPlayer`, `clickPlay` and `launchSceneByName` from `fixtures/helpers.ts`.
  2. A third context plays a second world.
  3. The operator pauses the first world from the `/admin/play-pauses` UI.
  4. Assert both contexts show the notice within 5 s, measured and logged, with no reload.
  5. Assert the third context is still on its playfield.
  6. Assert a non-operator's direct `pauseWorldPlay` GraphQL call is refused.
  7. Run a second case where the pausing operator is also a member of the world. They are removed too.

### Implementation for User Story 1

- [x] T019 [US1] Add the operator mutation `pauseWorldPlay(worldId, grounds): PauseOutcome!` in the new file `src/server/src/graphql/mutations_play_pause.rs`. Guard with `helpers::admin_user`, call `play_pause::pause_world`, and use types from `contracts/graphql.md`. Merge it into the root mutation in `src/server/src/graphql.rs`, and add it to `ADMIN_ONLY` in `src/server/src/graphql/admin_surface_tests.rs`.
- [x] T020 [P] [US1] Add the operator queries `playPauseCandidates(search, first)` and `playPauses(active, worldId, first, after)` in the new file `src/server/src/graphql/queries/play_pause.rs`. `playedNow` may return `false` until T047 lands; leave a `// T047` marker. Merge them into the root query and add them to `ADMIN_ONLY`.
- [x] T021 [US1] Wrap the world-scoped subscriptions in `until_stream_must_end` and call `play_pause::refuse_if_paused` before opening each:
  - `worldEventsCreated` and `playersOnline` in `src/server/src/graphql/subscriptions.rs`;
  - `playField` in `src/server/src/graphql/mutations_play_field.rs`;
  - `peerSignals` in `src/server/src/peer_signaling/surface.rs`, which is newly wrapped since it had no lifetime check before.

  Add one test per subscription asserting refusal to open on a paused world.
- [ ] T022 [P] [US1] Add `apps/web/src/api/playPause.ts`, with GraphQL documents and typed calls for every field in `contracts/graphql.md`, and an `isPlayPaused(error)` predicate on the extension code.
- [ ] T023 [US1] Route the pause signal centrally so each source sends the page to `/world/:id/paused`:
  - `apps/web/src/api/graphqlClient.ts`: any error with `WORLD_PLAY_PAUSED` dispatches a single `play-paused` event carrying `worldId`, deduplicated per world.
  - `apps/web/src/engine/world/sync/subscriptionClient.ts`: a stream error item with that code is surfaced as a pause, not as a thrown failure or a silent `done`.
  - `apps/web/src/pages/world/WorldPage.tsx`: world event code 28 does the same.
  - On receipt, tear down the play session through the existing leave-play path: close peer connections, dispose world subscriptions, and stop the heartbeat. Then navigate with `replace`.
- [ ] T024 [P] [US1] Build the notice page `apps/web/src/pages/world/PlayPausedPage.tsx` and its route in `apps/web/src/routes/AppRoutes.tsx`, per the contract's *The notice*:
  - **Wording:** "Play in {world} has been paused by an operator of this instance, since {time}." Give the actions "Go to your worlds" and a statement that this page will notice when play resumes. Never mention a reason, takedown, report, violation or blame.
  - **Accessibility:** heading focus on arrival, and a polite live region.
  - **Styling:** the existing fantasy design system components in `apps/web/src/components/ui/`, sized to read across a room.
  - **Polling:** `worldPlayState` every 30 s and on focus, stubbed to `paused: true` until T057 lands, with a `// T057` marker.
- [ ] T025 [US1] Build the operator page `apps/web/src/pages/admin/PlayPausesPage.tsx` with a route under `RequireAdmin` in `AppRoutes.tsx`, and add an entry to `apps/web/src/pages/admin/components/adminSections.ts`.
  - **This phase:** a world search (`playPauseCandidates`), a "Pause play" action opening a confirm dialog with required grounds, and an *Active pauses* list (`playPauses(active: true)`).
  - **Later phases** add requests (US3), lift (US4) and the record (US5) as sections of this page.
- [ ] T026 [US1] Run `play-pause.spec.ts` against the stack until it passes. Record the measured removal times in the commit message. Then run `cargo test -q --lib` for the server (re-run flakes at `--test-threads=1`) and `pnpm exec tsc --noEmit`. Commit.

**Checkpoint**: US1 works on its own. An operator can stop a live table.

---

## Phase 4: User Story 2 — A pause holds (Priority: P2)

**Goal**: No path back into a paused world: not the page, not a direct server call, not a reconnect, and not queued offline changes. The world's Owner cannot lift it.

**Independent Test**: quickstart scenario 2 (`play-pause-holds.spec.ts`), plus the surface test.

### Tests for User Story 2

- [ ] T027 [US2] Write the closed-list test `src/server/src/graphql/play_pause_surface_tests.rs`, following `admin_surface_tests.rs`.
  - **Tables:** `GATED`, `NOT_WORLD_SCOPED` and `OPERATOR`, keyed by root field name.
  - **Half one:** introspect the schema's root `Mutation` and `Subscription` fields and fail on any field in no table or in two. Also require `worldSyncPlan` and `worldEventsSince` in `GATED`.
  - **Half two:** a `FIXTURES` map from each `GATED` name to a minimal valid request against a seeded paused world. Fail if any `GATED` name lacks a fixture. Run each as the world's Game Master and as a member site admin, and assert `WORLD_PLAY_PAUSED`.

  Register it in `src/server/src/graphql.rs` `#[cfg(test)]`. It is expected to fail until T029–T035 land.
- [ ] T028 [US2] Write `apps/web/e2e/play-pause-holds.spec.ts`:
  1. A player's context is severed with `severableLink`, and a token move is queued offline while severed.
  2. The operator pauses the world.
  3. A connected context opens `/world/:id/play`: expect the notice.
  4. With the player's session, call `heartbeat`, `worldSyncPlan` and `moveOwnToken` directly, and open `worldEventsCreated`: expect `WORLD_PLAY_PAUSED` from each.
  5. Restore the link: expect the notice, `PlayPaused` rejections in the reconcile report, "1 change you made while offline wasn't kept", and the server token position unchanged (`serverTokenPosition`).
  6. The world's Owner calls `liftWorldPlayPause`: expect refusal.
  7. `expectNoAxeViolations` on the notice.

### Implementation for User Story 2

- [ ] T029 [US2] Gate the play entry points, adding the `play_pause::refuse_if_paused` call after the existing membership check in each:
  - `heartbeat` in `src/server/src/graphql/mutations_heartbeat.rs`;
  - `worldEventsSince` in `src/server/src/graphql/queries/world_events_since.rs`;
  - `worldSyncPlan` in `src/server/src/graphql/queries/world_sync_plan.rs`;
  - `launchScene` in `src/server/src/graphql/mutations_scenes.rs`.
- [ ] T030 [US2] Gate `reconcileQueuedChanges` in `src/server/src/graphql/mutations_reconcile.rs`. On a paused world, return a report rejecting every change with `PlayPaused`, never an error, and apply nothing. Add a test beside the existing reconcile tests.
- [ ] T031 [P] [US2] Gate the token, wall, light and door mutations with `refuse_if_paused` or `refuse_scene_if_paused`, called beside, not inside, `actor_in_world`/`is_dm_*`. Find them under `src/server/src/graphql/` with `grep -ln "fn .*token\|wall\|light" src/server/src/graphql/mutations_*.rs`.
- [ ] T032 [P] [US2] Gate the interactives mutations in `src/server/src/graphql/mutations_interactives.rs`: create, update, delete, reset, approve and refuse a request, set door designation/lock/secret, and `activateInteractive`.
- [ ] T033 [P] [US2] Gate the combat, chat, roll and scene mutations. Locate them with `grep -ln "is_dm_of_world\|is_dm_of_scene\|actor_in_world\|require_world_member" src/server/src/graphql/mutations_*.rs`, excluding files T031, T032 and T034 own.
- [ ] T034 [P] [US2] Gate world-content writes:
  - compendium: `src/server/src/graphql/mutations_compendium.rs`;
  - the library's per-world book list and deltas: `src/server/src/graphql/mutations_library.rs`;
  - lore, staging and world settings;
  - invitations and membership changes.

  The spec's default is *readable, not editable*. Leave read-only queries ungated.
- [ ] T035 [P] [US2] Gate the world-scoped REST writes: scene and asset uploads, and map import. Find them in `src/app/src/main.rs` routes and their handlers under `src/server/src/`. Answer `423 Locked` with body `{"code":"WORLD_PLAY_PAUSED"}`, and add a handler test per route.
- [ ] T036 [US2] Fill the three tables and every `FIXTURES` entry in `play_pause_surface_tests.rs` until both halves pass. Every root field not gated must be justified by its table.
- [ ] T037 [P] [US2] Tell a pause apart from being offline in `apps/web/src/engine/world/sync/heartbeat.ts`. A refusal carrying `WORLD_PLAY_PAUSED` dispatches the pause signal. It never counts toward the three failures that switch to offline queueing (`offlineQueue.ts` `shouldQueue`). Add a vitest case beside the existing heartbeat tests.
- [ ] T038 [P] [US2] Handle `PlayPaused` in `apps/web/src/engine/world/sync/offlineQueue.ts` `reconcileWorld`. Revert the rejected changes through the existing `revert` path, keep the count, and hand it to the notice (for example via navigation state) so `PlayPausedPage.tsx` shows "{n} change(s) you made while offline weren't kept". Add a vitest case.
- [ ] T039 [US2] Make sure the page refuses at entry: opening `/world/:id/play` on a paused world surfaces the `WORLD_PLAY_PAUSED` from `worldSyncPlan` as the notice, not a generic load error, in `apps/web/src/pages/world/WorldPage.tsx`.
- [ ] T040 [US2] Run `play-pause-holds.spec.ts` and `play-pause.spec.ts` until both pass. Run `cargo test -q --lib` (surface test included) and `tsc --noEmit`. Re-run the existing e2e that touch the changed paths:
  - `live-sync.spec.ts`, `world-event-catchup.spec.ts`, `world-cache-offline.spec.ts`, `world-cache-isolated.spec.ts`, `companion-offline.spec.ts`;
  - the interactives and library e2e.

  Ungated worlds must behave as before. Commit.

**Checkpoint**: US1 and US2 together are a pause that means something.

---

## Phase 5: User Story 3 — A takedown asks for a pause (Priority: P3)

**Goal**: A takedown on content of a world in live play raises one pending request per world. An operator approves it, which pauses the world as in US1, or declines it, which leaves no visible trace.

**Independent Test**: quickstart scenario 3 (`play-pause-request.spec.ts`).

### Tests for User Story 3

- [ ] T041 [P] [US3] Add request service tests in `src/server/src/play_pause/requests_tests.rs`:
  - a raise creates one pending request with its trigger;
  - a second raise for the same world attaches to it (FR-033);
  - a raise for a paused world attaches to the pause and creates no request (FR-036);
  - a raise for a world not in live play does nothing;
  - the same moderation action raised twice records once;
  - approving creates a pause with `request_id`, moves nothing else and records event 28;
  - declining records no event;
  - two concurrent decisions on two connections produce exactly one pause, and the loser gets `decidedHere: false` with the winner's name.
- [ ] T042 [P] [US3] Add takedown hook tests in `src/server/src/moderation/` (beside `scene_tests.rs`, new file `play_pause_hook_tests.rs`):
  - a takedown on a scene, an actor and a lore entry of a live world each raise;
  - a takedown on an adopted copy reached by `fan_out_disable` raises for that copy's world when it is live;
  - a forced hook failure leaves the takedown in effect and logs the action id;
  - counter-notice restoration, an upheld appeal (`restore_case_sync`) and the lazy elapse in `effective_status_sync` each leave an active pause active (FR-042).
- [ ] T043 [US3] Write `apps/web/e2e/play-pause-request.spec.ts`:
  1. A table plays a scene. File a takedown on the scene through `/legal/dmca`, then one on an actor of the same world. Move the local `fileSceneTakedown` helper from `dmca-scene-takedown.spec.ts` into `fixtures/playPause.ts` and import it back there.
  2. Expect one pending request at `/admin/play-pauses` with two triggers, marked *played now*.
  3. Approve it: the table lands on the notice.
  4. In a second live world, raise and decline a request: no change on the table's page within 10 s, no event 28, and `worldPlayState.history` empty.
  5. Two operator contexts approve one request at once: one pause, and the second is shown who decided.
  6. A takedown on a world nobody has played for over a minute raises no request.

### Implementation for User Story 3

- [ ] T044 [US3] Implement the live-play mark in `src/server/src/play_pause/live_play.rs`:
  - `mark_live(state, world_id)`, throttled in-process with a `DashMap<Uuid, Instant>` on `AppState` or a module static. It skips when this process marked the world under 30 s ago, otherwise runs the conditional upsert from `data-model.md`.
  - `in_live_play(conn, world_id) -> bool` (a beat within 45 s).

  Call `mark_live` from `heartbeat` in `src/server/src/graphql/mutations_heartbeat.rs` after the membership and pause checks. Amend that resolver's "in memory, not in a row" comment to say what is now written and why (research R3).
- [ ] T045 [US3] Implement `src/server/src/play_pause/requests.rs`:
  - `raise(conn, world_id, trigger)`, per research R4: paused → attach to the pause; else `INSERT … ON CONFLICT DO NOTHING` the pending request, then attach the trigger to the pending one; idempotent per `(moderation_action_id, owner)`.
  - `raise_for_takedown(conn, world_id, action)`, which returns early unless `in_live_play`.
  - `decide(conn, operator, request_id, decision, note)`, a single conditional update `WHERE state = 'Pending'`. On `Approve`, in the same transaction, insert the pause via the internal form of `pause_world`, with the note as grounds, `request_id` set and event 28. If an active pause already exists, attach the request's triggers to it instead. Zero rows reads back the winner.
- [ ] T046 [US3] Call the hook from `submit_takedown_notice_impl` in `src/server/src/graphql/mutations_moderation.rs`. Call it after the blocking takedown work returns, beside the lore hook (~line 280), for the target's world from the `CONTENT_DISABLED` event row. Also call it for every world `reach::fan_out_disable` disabled a copy in: return those world ids from `src/server/src/moderation/reach.rs` if it does not already. It is non-fatal: log at `error` with the moderation action id, and never fail the takedown.
- [ ] T047 [US3] Add the operator fields in `src/server/src/graphql/mutations_play_pause.rs` and `src/server/src/graphql/queries/play_pause.rs`:
  - `decidePlayPauseRequest(requestId, decision, note): PauseDecisionOutcome!`;
  - `playPauseRequests(state, first, after)`;
  - `playedNow` computed with `in_live_play` on `PauseRequest`, `PlayPause` and `PauseCandidateWorld`, replacing the T020 marker.

  Add the new fields to `ADMIN_ONLY` and to `OPERATOR` in `play_pause_surface_tests.rs`.
- [ ] T048 [US3] Add a *Requests* section to `apps/web/src/pages/admin/PlayPausesPage.tsx`. It lists pending requests with world, raised time, *played now* and each trigger (kind, a link to the moderation case for takedowns). Approve and Decline each open a dialog requiring a note. A lost race shows "Already decided by {name} at {time}" and refreshes.
- [ ] T049 [US3] Run `play-pause-request.spec.ts`, the Phase 3–4 e2e, and the existing moderation e2e (`dmca-takedown`, `dmca-scene-takedown`, `dmca-counter-notice`, `takedown-reach`, `collection-moderation`, `account-standing`) until they pass. Run `cargo test -q --lib` and `tsc --noEmit`. Commit.

**Checkpoint**: the gap T042 found is closed for the normal case, through an operator's decision.

---

## Phase 6: User Story 4 — Lift a pause (Priority: P4)

**Goal**: An operator lifts a pause. Play resumes, with nothing else changed, and separately taken-down content stays withheld.

**Independent Test**: quickstart scenario 4 (`play-pause-lift.spec.ts`).

### Tests for User Story 4

- [ ] T050 [US4] Write `apps/web/e2e/play-pause-lift.spec.ts`:
  1. Take down a scene of a world through `/legal/dmca`, pause the world, then lift it from `/admin/play-pauses`.
  2. Expect the notice to offer "Return to the world" within 30 s, play to start, and the taken-down scene to be still withheld.
  3. Pause again, then resolve a takedown by counter-notice using the helpers in `dmca-counter-notice.spec.ts`, moved to `fixtures/playPause.ts` if shared. Expect the pause still active and the notice still shown.
  4. As the world's Owner, attempt the lift through GraphQL: refused.

### Implementation for User Story 4

- [ ] T051 [US4] Add `liftWorldPlayPause(pauseId, grounds): PlayPause!` in `src/server/src/graphql/mutations_play_pause.rs`, calling `play_pause::lift_pause`. Add it to `ADMIN_ONLY` and `OPERATOR`.
- [ ] T052 [US4] Add a "Lift pause" action to each active pause on `apps/web/src/pages/admin/PlayPausesPage.tsx`, with a dialog requiring grounds. An already-lifted refusal shows who lifted it and when.
- [ ] T053 [US4] Add a "Return to the world" action on `apps/web/src/pages/world/PlayPausedPage.tsx`, shown when `worldPlayState.paused` is false. It navigates to the world page, which rejoins through the normal path. Depends on T057.
- [ ] T054 [US4] Run `play-pause-lift.spec.ts` and the earlier pause e2e until they pass, then `tsc --noEmit`. Commit.

---

## Phase 7: User Story 5 — Know what happened, and nothing more (Priority: P5)

**Goal**: The Game Master sees that play was paused and when, never why. Operators see the whole record, declined requests included. The record outlives the world.

**Independent Test**: SC-004 and SC-006, asserted inside the US1, US3 and US4 e2e, plus the checks below.

### Tests for User Story 5

- [ ] T055 [P] [US5] Add server tests in `src/server/src/play_pause/record_tests.rs`:
  - `worldPlayState` returns only `paused`, `pausedAt` and `history` spans for a member, and refuses a non-member;
  - a declined request never affects `worldPlayState`;
  - the operator `playPauses`/`playPauseRequests` return who, when, grounds, triggers and lift;
  - a world deleted while paused leaves both readable with `worldExists: false`;
  - deleting the pausing operator's account leaves `pausedBy.name` intact.
- [ ] T056 [US5] Extend the e2e:
  - `play-pause.spec.ts`: the Game Master's world page and world list show "Play paused by an operator since {time}", with no reason on any Game Master surface. Assert the absence of the grounds string in each page's full text.
  - `play-pause-request.spec.ts`: the declined request appears in the operator record.
  - `play-pause-lift.spec.ts`: the history shows the paused and lifted times.

### Implementation for User Story 5

- [x] T057 [US5] Add `worldPlayState(worldId): WorldPlayState!` in `src/server/src/graphql/queries/play_pause.rs`. Guard with `require_world_member`, not the gate. Select only `paused_at` and `lifted_at` from `world_play_pauses`. Classify it as a read in the surface test notes. Replace the T024 stub in `PlayPausedPage.tsx`.
- [ ] T058 [P] [US5] Add a *Record* section to `apps/web/src/pages/admin/PlayPausesPage.tsx`: all pauses and decided requests, newest first, paginated. Each shows world (marked if it no longer exists), who, when, grounds or note, triggers, and lift.
- [ ] T059 [P] [US5] Show *that and when* to members:
  - a quiet banner on the world page in `apps/web/src/pages/world/WorldPage.tsx`;
  - a status on the world's card in the world list (find the card component with `grep -rn "clickPlay\|Play</" apps/web/src/pages`);
  - the pause history in the world's settings, with times only.
- [ ] T060 [US5] Run the four pause e2e until they pass, then `cargo test -q --lib` and `tsc --noEmit`. Commit.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T061 [P] Run `expectNoAxeViolations` on `/admin/play-pauses` in `apps/web/e2e/play-pause.spec.ts`. Check the notice at 200% zoom and at a 1920×1080 viewport viewed as a room screen: take screenshots and record them in the commit (SC-008).
- [ ] T062 Measure SC-001 across 5 runs of `play-pause.spec.ts`: the event path and, with the world event suppressed in a test-only way, the poll-only path. Record both in `specs/051-operator-playfield-shutdown/quickstart.md` under a *Measured* heading. If the poll-only path exceeds 5 s, correct SC-001 or `LIVENESS_POLL` and say which.
- [ ] T063 [P] Update `apps/web/PRODUCT.md` with the pause as an operator lever, and the notice's tone rule. Update spec 015's tasks and notes (`specs/015-dmca-notice-takedown/`) to point the withdrawn "takedowns reach live tables" item at spec 051.
- [ ] T064 [P] Add "Found in implementation" to ADR-100 in `docs/adrs/20260913-100-an_operator_can_pause_a_worlds_play.md`: anything the surface test caught, any gated field that surprised, and whether the trigger backstop is still unneeded.
- [ ] T065 Run the full e2e suite for regressions (sharded as usual), the full server test suite, clippy for host and wasm (`make lint`) and `pnpm verify`. Fix what this feature broke, and record pre-existing flakes as such. Then tick this ledger and commit.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (T001–T003)**: T002 and T003 have no dependencies. T001 blocks Phase 3 onward, but not Phase 2.
- **Foundational (T004–T016)**: T004 → T005 → T007 → T008, T010, T011, T014. T006, T009, T012, T013, T015 and T016 follow the code they test. **Blocks every story.**
- **US1 (T017–T026)**: needs Foundational and T001.
- **US2 (T027–T040)**: needs US1's signal routing (T023) and notice (T024), which the e2e lands on.
- **US3 (T041–T049)**: needs Foundational, and US1 for the e2e's "approve pauses as in US1". It is independent of US2 at the server level.
- **US4 (T050–T054)**: needs US1. T053 needs T057.
- **US5 (T055–T060)**: T057 can be built right after Foundational, and it unblocks T024's poll and T053. The rest follows US1, US3 and US4 for the e2e assertions.
- **Polish (T061–T065)**: after all stories.

### Within a story

Tests are written first and expected to fail. Then server, then web, then the e2e **run** as the story's last task.

### Parallel opportunities

- Setup: T002 ∥ T003.
- Foundational: after T007, T008 ∥ T010/T011 ∥ T012 ∥ T013 ∥ T014. Tests T009 ∥ T015 ∥ T016.
- US1: T017 ∥ T020 ∥ T022 ∥ T024 once T019 is in.
- US2: T031 ∥ T032 ∥ T033 ∥ T034 ∥ T035 (disjoint files) and T037 ∥ T038 (web). T036 closes them.
- US3: T041 ∥ T042. T044 before T045.
- US5: T055 ∥ T058 ∥ T059.

### Parallel example: User Story 2

```text
Agent A: T031 gate token/wall/light/door mutations
Agent B: T032 gate interactives mutations
Agent C: T034 gate compendium, library, lore, staging, settings, membership
Agent D: T037 + T038 heartbeat and offline queue in apps/web
Then: T036 fills the surface test, T040 runs the e2e
```

---

## Implementation Strategy

### MVP: Setup, Foundational, then US1

A world can be paused by an operator and every live browser is removed within five seconds. This alone closes the gap spec 015 T042 found, for an honest client. **Stop and validate** with `play-pause.spec.ts`.

### Incremental delivery

1. **US2** makes the pause hold against a modified or offline client. This is the difference between a notice and a lock, and should follow the MVP immediately.
2. **US3** puts takedowns through an operator's decision.
3. **US4** brings a paused world back.
4. **US5** gives the table *that and when*, and gives operators the full record.
5. **Polish** measures SC-001, audits accessibility, and records what implementation found.

Each story commits separately with signed commits and explicit paths, and is ticked here only after its e2e has run green.
