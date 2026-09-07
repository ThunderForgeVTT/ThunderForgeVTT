---

description: "Task list for 036-concurrent-client-sessions"
---

# Tasks: Concurrent Client Sessions

**Input**: Design documents from `/specs/036-concurrent-client-sessions/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/)

**Tests**: Included, and not optional here. FR-016 through FR-024 *are*
requirements about tests: the multi-client fixture, the eviction regression
guard, the combat coverage and the OAuth coverage are deliverables, not
verification of deliverables. Rust unit tests accompany each server surface as
the codebase already does.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: US1, US2, US3, US3a, US3b, US3c, US4, US5, US6 per spec.md
- Every task names the file it touches

## Path Conventions

Web application, per plan.md § Structure Decision: Rust server under
`src/server/src/`, shared types in `crates/thunderforge-canvas-core/`, React
shell in `apps/web/src/`, Playwright suite in `apps/web/e2e/`, system packs in
`packs/systems/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the decisions this feature is not allowed to make silently.
Constitution Principle IV requires these in the same change set, and research.md
already contains their substance.

- [X] T001 [P] Write ADR-073 (concurrent sessions and the single play-field claim) in `docs/adrs/20260907-073-concurrent-sessions-and-play-field-claim.md`, superseding the revoke-on-login policy at `src/server/src/auth/sessions.rs:370` and recording what replaces it
- [ ] T002 [P] Write ADR-074 (system-declared checks and sheet-initiated rolls) in `docs/adrs/20260907-074-system-declared-checks.md`, extending ADR-044 and recording why the six existing per-pack roll keys are left alone
- [ ] T003 [P] Write ADR-075 (the peer boundary belongs to the play field) in `docs/adrs/20260907-075-peer-boundary-play-field.md`, amending ADR-052 and naming the fourth separation it adds to hold/continue/distribute
- [ ] T004 Confirm the harness seeds an instance that admits new accounts by checking `src/server/seeds/demo_accounts.sql` sets `access_policy = 'open'`, and record in this file if it does not (the spec assumes it)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: schema and shared types every story below reads. Nothing here is
user-visible on its own.

**⚠️ CRITICAL**: No user story work begins until this phase is complete

- [X] T005 Create the Diesel migration `src/server/migrations/2026-09-07-000000-0000_session_description/up.sql` and `down.sql` adding `last_seen_at`, `client_description` and `ended_reason` to `user_sessions` per data-model.md § 1
- [X] T006 Regenerate `src/server/src/schema.rs` for the new columns and extend `UserSession` / `NewUserSession` in `src/server/src/models.rs`
- [ ] T007 [P] Add the check declaration types (`CheckDeclaration`, binding source, placeholder binding) to `crates/thunderforge-canvas-core/src/system_rules.rs` with native unit tests for parsing and rejection
- [X] T008 [P] Create the play-field claim registry in `src/server/src/play_field.rs` — an account-keyed registry whose entry is owned by a guard, modelled on `PeerRegistry::register` in `src/server/src/peer_signaling.rs`, with unit tests for takeover, drop-on-guard-release and concurrent claims
- [X] T009 Wire `play_field.rs` into `src/server/src/lib.rs` and hold the registry on `AppState` in `src/server/src/state.rs`
- [X] T010 Update `last_seen_at` from `resolve_authenticated_user` in `src/server/src/auth_middleware.rs`, coarsely (at most once per minute per session) so a live session does not write on every request

**Checkpoint**: schema, shared types and the registry exist; stories can start

---

## Phase 3: User Story 1 — A second window signs in without evicting the first (Priority: P1) 🎯 MVP

**Goal**: an account holds several live sessions; signing in ends none of them.

**Independent Test**: sign in as one account in two browser contexts, act in
each, and confirm both stay authenticated and both see each other's changes.

### Tests for User Story 1

- [X] T011 [P] [US1] Add a server test in `src/server/src/auth/sessions.rs` asserting that creating a second session leaves the first live, and that both resolve through `resolve_authenticated_user`
- [X] T012 [P] [US1] Add a server test in `src/server/src/auth/sessions.rs` asserting the 11th live session ends the least recently used and only that one (FR-004)

### Implementation for User Story 1

- [X] T013 [US1] Remove the revoke-on-login statement from `create_session` in `src/server/src/auth/sessions.rs` and capture `client_description` from the request instead (coarse; no address — follow spec 035's "record the act, never the person")
- [X] T014 [US1] Enforce the 10-session bound with LRU eviction in `src/server/src/auth/sessions.rs`, writing `ended_reason = 'bound_exceeded'`
- [ ] T015 [US1] Confirm every remaining caller of session creation (`auth/oauth.rs`, `auth/two_factor.rs`, `auth/admin_setup.rs`) still behaves correctly with no eviction, and add a test for the two-factor path in `src/server/src/auth/two_factor.rs` (a second sign-in must still challenge — spec.md Edge Cases)

**Checkpoint**: two browsers, one account, both alive — the reported defect is gone

---

## Phase 4: User Story 2 — One account, several clients, in the test suite (Priority: P1)

**Goal**: a test can open another client for an existing account without
registering a second one.

**Independent Test**: one spec opens two contexts as one account and asserts an
action in one is visible in the other, under the sharded harness.

### Implementation for User Story 2

- [X] T016 [US2] Create `apps/web/e2e/fixtures/clients.ts` exporting `openAnotherClient(browser, creds, kind, from?)` per `contracts/e2e-fixtures.md`, signing in for real rather than copying `storageState`
- [X] T017 [US2] Add `apps/web/e2e/concurrent-sessions.spec.ts` covering US1's acceptance scenarios through the fixture
- [X] T018 [US2] Add the eviction regression guard to `apps/web/e2e/concurrent-sessions.spec.ts` — sign in in a second context, then act in the first (FR-020)
- [ ] T019 [P] [US2] Move specs that register a second account only to obtain a second window onto the fixture; audit `apps/web/e2e/` for them first and list what moved in the commit body
- [ ] T020 [P] [US2] Leave `inviteAndJoinAsPlayer` in `apps/web/e2e/fixtures/helpers.ts` as it is, and add a comment naming which fixture to use when the two clients are one person (FR-019)
- [X] T021 [US2] Verify a two-client spec passes under `node scripts/e2e-parallel.mjs --shards=2` with no serialisation (FR-018)

**Checkpoint**: the suite can express "one person, two windows"

---

## Phase 5: User Story 3a — The sheet on the second screen (Priority: P1)

**Goal**: one play field per account, everything else a companion; the
character sheet is the first companion.

**Independent Test**: play field in one window, sheet in another, one account;
a stat change agrees in both and only one holds the canvas.

### Tests for User Story 3a

- [X] T022 [P] [US3a] Add server tests for `claimPlayField` / `releasePlayField` in `src/server/src/graphql/mutations_play_field.rs` covering takeover, idempotent re-claim, non-member refusal and concurrent claims resolving to exactly one holder
- [X] T023 [P] [US3a] Add a server test in `src/server/src/play_field.rs` asserting a dropped guard releases the claim, so a killed client does not lock the account out (FR-030)

### Implementation for User Story 3a

- [X] T024 [US3a] Implement `claimPlayField` and `releasePlayField` in `src/server/src/graphql/mutations_play_field.rs` per `contracts/play-field-claim.md`, enforcing world membership at the boundary
- [X] T025 [US3a] Add the `playFieldClaimChanged` subscription to `src/server/src/graphql/subscriptions.rs`, account-scoped, carrying the demotion notice
- [X] T026 [US3a] Register both surfaces on the roots in `src/server/src/graphql/mod.rs` and add an SDL guard test asserting the field names the client uses, in the style of `the_access_surface_is_registered_under_the_names_the_client_uses`
- [ ] T027 [US3a] Create `apps/web/src/services/playFieldClaim.ts` — claim on engine mount, release on unmount, per-page-load client id, and the demotion state
- [ ] T028 [US3a] Claim from the play-field shell in `apps/web/src/pages/world/` immediately before the engine is created, and decline to mount without a held claim (research.md § R4: the canvas defines the play field, not the URL)
- [ ] T029 [US3a] Add the demotion notice and a "take the table back" control to the play-field shell in `apps/web/src/pages/world/`
- [ ] T030 [P] [US3a] Add vitest coverage for `playFieldClaim.ts` in `apps/web/src/services/__tests__/playFieldClaim.test.ts`
- [ ] T031 [US3a] Add `apps/web/e2e/companion-sheet.spec.ts` covering US3a's four acceptance scenarios via the US2 fixture

**Checkpoint**: sheet on one screen, table on the other, one account

---

## Phase 6: User Story 4 — Ending a session, deliberately (Priority: P2)

**Goal**: the protection that login-time eviction was providing, as controls a
person can operate.

**Independent Test**: with several clients live, sign out of one and assert the
others survive; end all and assert every client is signed out next request.

### Tests for User Story 4

- [X] T032 [P] [US4] Add server tests in `src/server/src/graphql/mutations_sessions.rs` for `mySessions`, `endSession` (own only; another account's id is indistinguishable from a missing one) and `endAllSessions`
- [ ] T033 [P] [US4] Add a server test asserting a rendered session row carries no address, mirroring spec 035's `access_events_record_the_act_and_never_the_person`

### Implementation for User Story 4

- [X] T034 [US4] Implement the session registry reads in `src/server/src/auth/session_registry.rs` (live sessions, last seen, current, holds-play-field)
- [X] T035 [US4] Implement `mySessions`, `endSession` and `endAllSessions` in `src/server/src/graphql/mutations_sessions.rs` per `contracts/sessions.md`, writing `ended_reason`
- [ ] T036 [US4] Close the live streams and release any play-field claim held by an ended session, in `src/server/src/graphql/subscriptions.rs` and `src/server/src/play_field.rs` (FR-010)
- [ ] T037 [US4] End every other session on password change in `src/server/src/auth/sessions.rs`, with `ended_reason = 'password_changed'` (FR-008)
- [ ] T038 [US4] Add the session list and its controls to the account page in `apps/web/src/pages/user/`
- [X] T039 [US4] Extend `apps/web/e2e/concurrent-sessions.spec.ts` with US4's five acceptance scenarios

**Checkpoint**: sessions are visible and endable — the eviction's job is covered

---

## Phase 7: User Story 3b — A check rolled from the sheet is the system's check (Priority: P2)

**Goal**: the sheet names a check; the system declares it and the server
resolves it.

**Independent Test**: roll a named check from the sheet in two systems and
assert each produced its own, visible to the table, with the sheet deciding
neither dice nor outcome.

### Tests for User Story 3b

- [ ] T040 [P] [US3b] Add parsing tests for the `checks` declaration in `crates/thunderforge-canvas-core/src/system_rules.rs`, including a pack that declares none
- [ ] T041 [P] [US3b] Add server tests for `rollCheck` in `src/server/src/graphql/mutations_roll_check.rs`: unknown `checkId` refused and never treated as a formula, actor permission enforced, result recorded on the same path as `rollDice`

### Implementation for User Story 3b

- [ ] T042 [US3b] Parse `checks` off the manifest in `src/server/src/systems.rs` and expose it on the system a world is using
- [ ] T043 [US3b] Add the `checks` block to `packs/systems/dnd5e/system.json`, generated from its existing `abilities` and `skills` (each skill already names its governing ability)
- [ ] T044 [US3b] Implement `rollCheck` in `src/server/src/graphql/mutations_roll_check.rs` per `contracts/system-checks.md` — resolve bindings against the actor, hand the finished formula to the existing authoritative path in `src/server/src/graphql/mutations_roll.rs`
- [ ] T045 [US3b] Register `rollCheck` on the mutation root in `src/server/src/graphql/mod.rs` with an SDL guard asserting it takes no formula argument
- [ ] T046 [US3b] Add the check control to the sheet in `apps/web/src/components/sheet/`, rendering only what the system declares and offering nothing when it declares none (FR-037)
- [ ] T047 [P] [US3b] Update `packs/systems/README.md` with the `checks` declaration as part of the published author contract
- [ ] T048 [US3b] Extend `apps/web/e2e/companion-sheet.spec.ts` to roll a check from the sheet in two different systems and assert each is its own system's check, visible to the table

**Checkpoint**: a 5e player rolls Strength from the sheet and the table sees it

---

## Phase 8: User Story 3c — Offline, the sheet sends you back to the table (Priority: P2)

**Goal**: peer capability belongs to the play field; a companion that has lost
the server refuses and says where to retry.

**Independent Test**: cut a sheet window off from the server with its peer path
intact, roll, and assert the refusal names the play field and leaves no record.

### Tests for User Story 3c

- [ ] T049 [P] [US3c] Add a server test in `src/server/src/peer_signaling.rs` asserting registration is refused for a caller holding no play-field claim (FR-038)

### Implementation for User Story 3c

- [ ] T050 [US3c] Gate `peerSignals` registration on a held claim in `src/server/src/peer_signaling.rs` and `src/server/src/graphql/subscriptions.rs`, refusing at registration rather than inspecting relayed payloads (which the server deliberately never interprets)
- [ ] T051 [US3c] Ensure a companion surface opens no peer connection at all in `apps/web/src/services/peerTransfer.ts` — the engine asks, and a companion has no engine
- [ ] T052 [US3c] Refuse adjudicated actions in companion surfaces while the server is unreachable, naming the play field, in `apps/web/src/components/sheet/` — record nothing and queue nothing (FR-040)
- [ ] T053 [US3c] Add `apps/web/e2e/companion-offline.spec.ts` using `apps/web/e2e/fixtures/offline.ts`, asserting the refusal, the absence of any record, and that the play field's own ADR-052 continuation is unchanged (FR-041)

**Checkpoint**: the hard line is a behaviour with a test, not a paragraph

---

## Phase 9: User Story 3 — Shared truth, private view (Priority: P2)

**Goal**: state that must agree across clients agrees; state that must not,
does not.

**Independent Test**: change a shared thing in one client and a private thing in
another; exactly one crosses.

- [ ] T054 [P] [US3] Add `apps/web/e2e/companion-shared-state.spec.ts` asserting the active scene, membership, role and per-object permission changes reach every client of the account without a reload (FR-011, FR-012)
- [ ] T055 [P] [US3] Assert in the same spec that selection and camera do not cross between clients (FR-013)
- [ ] T056 [US3] Assert presence counts a person once however many clients they hold, and shows them present while at least one is live, in `apps/web/e2e/companion-shared-state.spec.ts` (FR-014) — `players_online` is already keyed by `(player_id, world_id)`, so this is a guard on existing behaviour
- [ ] T057 [US3] Assert a member removed mid-session loses access in every one of their clients, not only the one that acted (spec.md Edge Cases)

**Checkpoint**: "the same experience across clients" is defined and enforced

---

## Phase 10: User Story 5 — Combat, driven through the UI (Priority: P3)

**Goal**: the turn structure every system shares, proven above the UI for the
first time.

**Independent Test**: a GM drives the combat panel while a second client
follows the round and active combatant.

- [ ] T058 [US5] Add `apps/web/e2e/combat-panel.spec.ts` driving the existing test ids in `apps/web/src/components/.../CombatPanel.tsx` — `combat-panel`, `start-combat-button`, `start-combat-with-selection-button`, `combat-round-counter`, `advance-turn-button`, `end-combat-button`, `combatant-list`
- [ ] T059 [US5] Cover start, add/update/remove combatant, advance turn and end combat in that spec, against `src/server/src/graphql/mutations_combat.rs`'s behaviour
- [ ] T060 [US5] Assert a second client of the same account follows the round counter and active combatant without a reload, via the US2 fixture
- [ ] T061 [P] [US5] Update `MVP.md`'s "the gap worth closing: combat has no e2e" note to say what now covers it

**Checkpoint**: turn order is proven in a browser

---

## Phase 11: User Story 6 — The OAuth surface, observed (Priority: P3)

**Goal**: four documented behaviours, none of which has ever been exercised in
a browser.

**Independent Test**: the four scenarios against a stub provider, driven
through the real redirect flow.

- [ ] T062 [US6] Add the stub provider service (authorize, token, userinfo) under `apps/web/e2e/fixtures/` or `scripts/`, per `contracts/e2e-fixtures.md`
- [ ] T063 [US6] Give each shard a stub-provider port in `scripts/e2e-parallel.mjs`, exactly as backends and vite servers already get one
- [ ] T064 [US6] Seed an `oauth_providers` row pointing at the stub in `src/server/seeds/e2e_demo.sql` — no new column is needed, the table already carries `authorization_url`, `token_url` and `userinfo_url`
- [ ] T065 [US6] Add `apps/web/e2e/oauth-provider.spec.ts` covering first-login provisioning (ADR-042), linking behind password confirmation (ADR-006), refusal with no verified email, and refusal by a closed instance (spec 035 FR-006)
- [ ] T066 [US6] Confirm no product file changed to make T062–T065 work; if any did, the stub is wrong (FR-023)
- [ ] T067 [US6] Close spec 035's T056 in `specs/035-instance-access/tasks.md` and strike the row from spec 032's deferred-manual-pass table in `specs/032-pack-architecture/tasks.md` (FR-024)

**Checkpoint**: three ADRs' worth of untested auth is tested

---

## Phase 12: Polish & Cross-Cutting Concerns

- [ ] T068 [P] Document concurrent sessions and the play-field claim in `docs/` — what a companion is, what it may do, and where the peer line falls
- [ ] T069 [P] Update `MVP.md` Phase 9 (Multiplayer) and Phase 10 (Permissions) with what this feature changed
- [ ] T070 Make four guards fail on purpose per quickstart.md § "Making the guards fail on purpose" and record in the commit that each was seen to bite
- [ ] T071 Run the quickstart scenarios A–G by hand against `make dev` and note anything the suite does not catch
- [ ] T072 Run `cargo test --workspace -j 4`, `make lint` (lint-host + lint-wasm + file length) and `pnpm --filter @thunderforge/web test`
- [ ] T073 Run the full suite via `node scripts/e2e-parallel.mjs --shards=2` and record the figures in the commit body
- [ ] T074 Run `pnpm verify` and fix what it reports **in the code this feature added** — keep it to that; wide lint passes get their own commit. `pnpm verify:fix` rewrites what is mechanical

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies; the three ADRs may be written while Phase 2 proceeds, but must land in the same change set (Principle IV)
- **Foundational (Phase 2)**: blocks every story. T005→T006 are ordered; T007 and T008 are independent of both
- **US1 (Phase 3)**: needs Phase 2. This alone closes the reported defect
- **US2 (Phase 4)**: needs US1 to be meaningful — the fixture's second context can only stay signed in once eviction is gone
- **US3a (Phase 5)**: needs Phase 2's registry (T008)
- **US4 (Phase 6)**: needs US1; independent of US3a except T036's claim release
- **US3b (Phase 7)**: needs US3a for a sheet window to exist as a companion
- **US3c (Phase 8)**: needs US3a's claim (the gate is "holds the claim")
- **US3 (Phase 9)**: needs US3a; asserts behaviour rather than adding it
- **US5 (Phase 10)**: needs US2's fixture only
- **US6 (Phase 11)**: independent of everything else here — can be built any time after Phase 1
- **Polish (Phase 12)**: after the stories being shipped

### Within Each User Story

- Server tests accompany the surface they test, in the module, as this codebase does
- Registry and types before the GraphQL surfaces that use them
- Server surface before the web client that calls it
- E2E last within a story, because it needs both halves

### Parallel Opportunities

- T001, T002, T003 — three ADRs, three files
- T007 and T008 — different crates, no shared state
- T011/T012, T022/T023, T032/T033, T040/T041 — test tasks within a story
- US6 (Phase 11) can run beside any other phase; it shares no file with them
- T054 and T055 — same story, different assertions, but note both land in one spec file, so write them together or accept a merge

---

## Parallel Example: Foundational

```bash
# After T005 and T006 (migration then schema), these two are independent:
Task: "Add check declaration types in crates/thunderforge-canvas-core/src/system_rules.rs"
Task: "Create the play-field claim registry in src/server/src/play_field.rs"
```

## Parallel Example: Setup

```bash
Task: "Write ADR-073 in docs/adrs/20260907-073-concurrent-sessions-and-play-field-claim.md"
Task: "Write ADR-074 in docs/adrs/20260907-074-system-declared-checks.md"
Task: "Write ADR-075 in docs/adrs/20260907-075-peer-boundary-play-field.md"
```

---

## Implementation Strategy

### MVP (US1 + US2)

Phases 1, 2, 3 and 4. That is the reported defect fixed **and** a test that
says so. Shipping US1 without US2 would leave the eviction free to return
unnoticed, which is why FR-020 exists and why both are P1.

1. Phase 1 (ADRs) and Phase 2 (schema, types, registry)
2. Phase 3 — eviction removed, bound enforced
3. Phase 4 — the fixture, the spec, the regression guard
4. **Stop and validate**: quickstart Scenario A by hand, plus the suite
5. Demonstrable: two browsers, one account, both alive

### Incremental delivery after MVP

1. **US3a** — the play-field claim and the companion sheet. This is the thing
   the person actually asked for, and it is a demo on its own
2. **US4** — session visibility and revocation. Ship with or immediately after
   US3a: it is the security half of Phase 3 and must not drift
3. **US3b** then **US3c** — the sheet gets a check, then the peer line is
   drawn and tested. In that order: US3c's refusal needs something to refuse
4. **US3** — the shared/private assertions, cheap once US3a exists
5. **US5**, **US6** — coverage the fixture unlocked; US6 needs no other phase
   and can be given to whoever is free

### Note on US6

It is P3 by value to this feature and P1 by value to the project — it retires
spec 035's deferred manual pass and covers three ADRs nothing exercises. If
anyone is working in parallel, this is the phase to hand them: it shares no
file with the rest.

---

## Notes

- [P] tasks touch different files and have no incomplete dependency
- Every story is independently testable; the checkpoints say what "done" looks
  like without the later phases
- Verify per target, per Principle V: native `cargo` for the server, wasm for
  the engine, `tsc`/`vitest` for the web, e2e for the story
- Commit per task or coherent group; the ADRs land with the code, not after
