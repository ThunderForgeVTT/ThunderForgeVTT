---

description: "Task list for 035 — Instance Access: the gate and the invitation"
---

# Tasks: Instance Access — the Gate and the Invitation

**Input**: Design documents from `/specs/035-instance-access/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: Included, and load-bearing. The spec defines an "Independent Test"
per story and twelve measurable success criteria; both contracts carry a test
table; and SC-001 is the assertion that would have caught the silent-admission
bug this feature exists to prevent. Constitution Principle V requires the check
before "done" is claimed.

**Organization**: Grouped by user story so each ships independently.

**Scope**: **US1 and US2 only** — the P1 pair the spec calls "the complete
minimum product". US3 (request-access intake), US4 (delivery durability) and
US5 (retention) are specified but not planned; see [research.md §8](./research.md).
Do not begin them from this file — they need their own plan pass first.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: US1 or US2, mapping to spec.md
- Exact file paths included

## Path Conventions

Web application, per plan.md: Rust API in `src/server/` and `src/app/`, React
front end in `apps/web/`, Playwright specs in `apps/web/e2e/`.

---

## Phase 1: Setup

- [X] T001 Write `docs/adrs/20260906-072-instance_admission_precedes_provisioning.md` recording that the instance access policy is a gate **above** ADR-042: ADR-042 keeps deciding *how* an admitted account is created, and stops deciding *whether* a stranger is admitted; a closed instance restores ADR-007's effective outcome without reverting ADR-042 on an open one. Constitution Principle IV requires it in this change set, not after
- [X] T002 [P] Add ADR-072 to the index table in `docs/adrs/README.md`, after the ADR-071 row
- [X] T003 [P] Fix the stale ADR citations found in research §1: `src/server/src/auth/registration.rs` lines 4, 48 and 93 attribute auto-provisioning to "ADR-011", which is the Export-My-Data Contract. The governing decision is ADR-042. Correct them and grep `src/server/src/auth/` for any other occurrence

---

## Phase 2: Foundational — blocking prerequisites for both stories

**Nothing in US1 or US2 can start until this phase completes.**

- [X] T004 Create the Diesel migration `src/server/migrations/<timestamp>_instance_access/` with paired `up.sql`/`down.sql` creating all four tables per [data-model.md](./data-model.md): `instance_access_settings`, `instance_invitations`, `instance_invitation_redemptions`, `instance_access_events`. Carry `created_by`/`updated_by` provenance per Constitution Principle III
- [X] T005 In the same migration's `up.sql`, seed the settings row conditionally (FR-013, FR-013a): `invite_only` when `NOT EXISTS(SELECT 1 FROM users)`, `open` otherwise. This is the only branch in the migration, and it must be here rather than in a lazy `ensure_` insert — after the fact nothing can tell a fresh instance from an upgraded one
- [X] T006 Run the migration and regenerate `src/server/src/schema.rs`; confirm the four tables and the `joinable!`/`allow_tables_to_appear_in_same_query!` entries appear
- [X] T007 [P] Add `InstanceAccessSetting`, `InstanceInvitation`, `NewInstanceInvitation`, `InstanceInvitationRedemption`, `NewInstanceInvitationRedemption`, `InstanceAccessEvent` and `NewInstanceAccessEvent` to `src/server/src/models.rs`, following the surrounding `Queryable`/`Selectable`/`Insertable` conventions
- [X] T008 Create `src/server/src/auth/instance_access.rs` with the policy enum (`Open`/`InviteOnly`/`Closed`), its `as_db_str`/parse pair, and — per data-model — an unparseable stored value degrading to `Closed` (fail shut, never fail open). Declare the module in `src/server/src/auth/mod.rs`
- [X] T009 Add `ensure_instance_access_settings`, `load_instance_access_settings` and `update_instance_access_policy` to `src/server/src/admin.rs`, mirroring the existing `ensure_/load_/update_two_factor_policy` trio exactly
- [X] T010 [P] Add `record_access_event` to `src/server/src/auth/instance_access.rs` writing one append-only `instance_access_events` row. It MUST accept no email or credential parameter at all — FR-012's prohibition is easier to keep when the function cannot express the violation
- [X] T011 [P] Add fixtures to `src/server/src/test_support.rs`: `set_instance_access_policy` and `insert_test_instance_invitation`, so both stories' tests can arrange a policy state without duplicating inserts

**Checkpoint**: schema, models, policy type, settings accessors and event writer exist. Both stories can now proceed in parallel.

---

## Phase 3: User Story 1 — a closed instance is actually closed (P1)

**Goal**: every path that can create an account consults one gate, so closing the instance closes it on the OAuth path too — not only on the registration form.

**Independent test**: set the instance closed on a live instance with at least one OAuth provider configured, attempt account creation by local form, direct request and provider sign-in with an unmatched verified email; confirm each is refused, that a pre-existing user is unaffected throughout, and that each refusal is recorded.

### The gate

- [X] T012 [US1] Rename `ensure_registration_allowed` to `ensure_admission_allowed` in `src/server/src/auth/registration.rs` and widen it to the contract in [contracts/instance-access.md](./contracts/instance-access.md): it takes the route and an optional invitation code, and returns admit-or-refuse. **Keep the existing first-run check as its first branch** so FR-010's bootstrap exemption stays where it already is rather than being re-derived
- [X] T013 [US1] Implement the policy decision inside the gate per the contract's table: `open` admits; `invite_only` admits only with a valid invitation; `closed` refuses everything **including a valid invitation** (FR-001 — "no account may be created by any means"). Update the re-export in `src/server/src/auth/mod.rs:97`
- [X] T014 [US1] Call the gate from the OAuth path in `src/server/src/auth/oauth.rs`'s `resolve_oauth_login`, placed **after** the "does this verified email match an existing user" lookup and immediately before the auto-provisioning branch at ~line 327. Placing it after the lookup is what makes FR-009 true by construction: an existing account never reaches the gate
- [X] T015 [US1] Update the local call site in `src/server/src/auth/sessions.rs:64` for the widened signature, keeping the gate **before** the username/email uniqueness probes so a closed instance cannot become an account-existence oracle (FR-011)

### Refusals and disclosure

- [X] T016 [US1] Return the contract's local refusal — `409` with `registration_blocked` and "This instance is not accepting new accounts" — from `src/server/src/auth/sessions.rs`, identical whether or not the submitted address belongs to a user
- [X] T017 [US1] Surface the OAuth refusal as a redirect to `/login?error=instance_closed` through `oauth_callback`'s existing error path in `src/server/src/auth/oauth.rs`, setting no session cookie, writing no `users` row and creating no identity link
- [X] T018 [US1] Write an `admission_refused` event on every refusal from T016 and T017, from the refusal sites in `src/server/src/auth/sessions.rs` and `src/server/src/auth/oauth.rs`, via `record_access_event` in `src/server/src/auth/instance_access.rs`: time, route, provider and the policy at the time — and **no email address** (FR-012)

### Reading and changing the policy

- [X] T019 [US1] Extend `setup_status` in `src/server/src/auth/admin_setup.rs` with `accessPolicy` and `acceptingAccessRequests` per the contract. `acceptingAccessRequests` is a constant `false` until US3 exists; it ships now so the front-end shape does not change later. Expose nothing else — no user count, no invitation existence (FR-003)
- [X] T020 [P] [US1] Add the `instanceAccessSettings` and `instanceAccessEvents` admin-only queries to `src/server/src/graphql/queries/admin.rs`, beside the existing `auth_security_settings` query
- [X] T021 [US1] Add the `setInstanceAccessPolicy` mutation in `src/server/src/graphql/mutations_instance_access.rs` (new), writing the settings update and its `policy_changed` event **in one transaction** — an audit row that can be lost independently of the change it records is not an audit row (FR-004). Register it in `src/server/src/graphql.rs`

### Web

- [X] T022 [P] [US1] Add `apps/web/src/api/instanceAccess.ts` with typed reads for the policy and events and a write for the policy
- [X] T023 [P] [US1] Add an `Access` entry to `ADMIN_SECTIONS` in `apps/web/src/pages/admin/components/adminSections.ts` — one entry, which is what that list exists for
- [X] T024 [US1] Create `apps/web/src/pages/admin/components/AccessPanel.tsx` modelled on `SecurityPanel.tsx`: the three-state policy control and a recent-activity list of access events. Mount it in `apps/web/src/pages/admin/SettingsPage.tsx` as the `access` section
- [X] T025 [US1] Implement FR-003a in the signed-out surface (`apps/web/src/pages/auth/`): on `invite_only` or `closed`, offer no registration affordance **and** state that the instance is invite-only or not accepting new accounts. Saying so is required — a page that silently omits sign-up is indistinguishable from a broken one

### Tests

- [X] T026 [P] [US1] Server tests in `src/server/src/auth/instance_access.rs`: the policy decision table (all three states × with/without invitation), and that an unparseable stored policy degrades to `Closed`
- [X] T027 [P] [US1] Server test in `src/server/src/auth/instance_access.rs`: with the policy closed and no administrator present, first-run bootstrap still succeeds (FR-010, SC-003)
- [X] T028 [P] [US1] Server test in `src/server/src/auth/sessions.rs`'s test module: a closed instance returns byte-identical refusal bodies for an email that belongs to a user and one that does not (FR-011)
- [X] T029 [P] [US1] Server tests in `src/server/src/auth/instance_access.rs`: a policy change writes exactly one `policy_changed` event with actor, previous and new state; a refused admission writes exactly one `admission_refused` event; **neither row contains an email address** (FR-004, FR-012)
- [X] T030 [P] [US1] Migration tests in `src/server/src/auth/instance_access.rs`: a fresh database seeds `invite_only`; a database with an existing user seeds `open` (FR-013, FR-013a)
- [X] T031 [US1] E2E `apps/web/e2e/instance-access-gate.spec.ts` covering the story's independent test: closed instance, local registration refused, **provider sign-in with an unmatched verified email refused with no session and no new account**, a pre-existing user signing in and reaching a world throughout, and the refusals visible in the admin panel. The OAuth leg must be e2e — the refusal is a redirect produced mid-callback and no unit test can prove the person lands on it holding no session

**Checkpoint**: US1 is independently shippable. A closed instance is closed on every path, and the operator can see it.

---

## Phase 4: User Story 2 — invite a specific person in (P1)

**Goal**: a closed door with a way through it for one named person.

**Independent test**: issue an invitation on a non-open instance, redeem it end to end as a new person by both the local and a provider route, confirm it cannot be redeemed beyond its limit, then revoke a second and confirm redemption fails.

### Issuing, listing, revoking

- [X] T032 [US2] Create `src/server/src/graphql/mutations_instance_invitations.rs` with `create_instance_invitation_impl` and `revoke_instance_invitation_impl`, admin-only, generating the code with `graphql::share_codes::generate_link_code` — v4-derived, never v7, for the reason ADR-049 gives (FR-019). Register it in `src/server/src/graphql.rs`
- [X] T033 [US2] Add the `instanceInvitations` admin query to `src/server/src/graphql/queries/admin.rs`, returning each invitation's derived state, remaining uses, issuer and redemptions (FR-018). Derive state in `src/server/src/graphql/mutations_instance_invitations.rs` the way `derive_link_state` does for world invites; **do not store it**
- [X] T034 [US2] Confirm by inspection of `src/server/src/graphql/queries/admin.rs` and `src/server/src/graphql/mutations_instance_invitations.rs` that no query resolves an invitation **by code** — only redemption may, and it consumes a use. This mirrors the no-enumeration invariant ADR-049 depends on

### Redemption

- [X] T035 [US2] Implement `redeem_instance_invitation` in `src/server/src/auth/instance_access.rs` as one transaction, in the order [contracts/instance-invitations.md](./contracts/instance-invitations.md) specifies: **(0)** rate limit before the code is looked up, **(1)** already-a-user check, **(2)** the conditional UPDATE, **(3)** account creation, **(4)** the redemption row, **(5)** the event
- [X] T036 [US2] Write step 2 as a single conditional UPDATE carrying the whole validity predicate in its WHERE clause, per data-model. Do not read-then-write: spec 027 lost updates that way and two redeemers raced the last use (`mutations_invites.rs:240`). Zero rows updated means unusable, without distinguishing which of the four reasons (FR-011)
- [X] T037 [US2] Implement step 1, the already-a-user check, **before** any use is consumed (FR-020a): an existing account is authenticated and the invitation is left untouched for its intended recipient. This is spec 027's fix at `mutations_invites.rs:195` restated at instance scope
- [X] T038 [US2] Implement step 0 in `src/server/src/auth/instance_access.rs` using `crate::graphql::share_rate_limit` (FR-019a) — the limiter already guarding the anonymous share reads. Its refusal MUST be distinguishable from `invitation_unusable`, so a legitimate recipient retrying is never told their invitation is bad
- [X] T039 [US2] Ensure a failure at step 3 rolls back step 2 in `src/server/src/auth/instance_access.rs` by wrapping steps 2–5 in one Diesel transaction — a failed signup must not burn a use
- [X] T040 [US2] Accept an optional `invitationCode` on `POST /api/authentication/register` in `src/server/src/auth/sessions.rs` and pass it to the gate
- [X] T041 [US2] Carry the invitation code through the OAuth round trip in `src/server/src/auth/oauth.rs` on the existing authorization session (`load_and_consume_authorization_session`), so it survives the provider redirect without a cookie of its own

### Web

- [X] T042 [P] [US2] Add invitation reads and writes to `apps/web/src/api/instanceAccess.ts`
- [X] T043 [US2] Create `apps/web/src/pages/invite/InstanceInvitePage.tsx` at `/invite/:code`, offering local registration and each configured provider, both carrying the code. Route it in `apps/web/src/routes/AppRoutes.tsx` **outside** `RequireAuthenticated` — a redemption page behind a login wall admits nobody
- [X] T044 [US2] Extend `AccessPanel.tsx` with the invitations list and an issue form (uses, expiry, note), showing each invitation's state, remaining uses and redemptions (FR-018)

### Tests

- [X] T045 [P] [US2] Server test in `src/server/src/auth/instance_access.rs`: an N-use invitation admits exactly N accounts under N+2 **concurrent** redemptions (SC-006). This is the test that would catch a regression to read-then-write
- [X] T046 [P] [US2] Server test in `src/server/src/auth/instance_access.rs`: revoked, expired, exhausted and never-existed produce byte-identical refusals (FR-011, US2 scenarios 4–5)
- [X] T047 [P] [US2] Server test in `src/server/src/auth/instance_access.rs`: redeeming as an existing account consumes no use and creates no redemption row (FR-020a)
- [X] T048 [P] [US2] Server test in `src/server/src/auth/instance_access.rs`: sustained redemption attempts are throttled, with a message distinguishable from `invitation_unusable` (FR-019a)
- [X] T049 [P] [US2] Server test in `src/server/src/auth/instance_access.rs`: an invitation issued while closed, then the policy opened and closed again, is governed only by its own revocation, expiry and use count (FR-020, US2 scenario 6)
- [X] T050 [P] [US2] Server test in `src/server/src/auth/instance_access.rs`: an account created by redemption differs in no column from one created on an open instance, and holds no world membership (FR-017, US2 scenario 7)
- [X] T051 [P] [US2] Server test in `src/server/src/auth/instance_access.rs`: a failed account insert leaves `used_count` unchanged (T039)
- [X] T052 [US2] E2E `apps/web/e2e/instance-invitations.spec.ts`: on an invite-only instance, redeem one invitation by local registration and a second by first-time provider sign-in, confirm a third refuses once revoked with the same wording as an exhausted one, and confirm the admin sees who redeemed what and when

**Checkpoint**: US1 + US2 together are the shippable minimum — closed by default, opened one person at a time.

---

## Phase 5: Polish & cross-cutting

- [X] T053 [P] Run `node scripts/verify.mjs` and resolve anything it flags; `check-file-length.sh` is part of it, and `oauth.rs` and `sessions.rs` are both being grown here
- [X] T054 [P] Run `cargo test --workspace -- --test-threads=1` and `pnpm exec vitest run`. Use `--test-threads=1`: `plugins::authoring_mode` fails intermittently in parallel on process-global state, unrelated to this feature
- [X] T055 Run both new e2e specs against a real stack: `node scripts/e2e-parallel.mjs --shards=2 --only=instance-access-gate,instance-invitations`. Use 2 shards — 4 exhausted memory on a 31GB machine
- [X] T056 [P] **Closed 2026-09-09 by an automated test, not by hand.** The reason this was deferred was that "the e2e harness has no OAuth provider to hand, so no automated test exercises a real provider handshake against a closed instance". Spec 036 US6 gave the harness one: `scripts/oauth-stub.mjs`, a seeded `stub` provider row, and `apps/web/e2e/oauth-provider.spec.ts`, whose fourth scenario closes the instance, walks the whole handshake, and asserts the identity is refused — then re-opens the instance and asserts *the same identity* is admitted, so the refusal is a policy decision and not a broken handshake. FR-006 is now proven by observation on every run rather than by construction. Scenarios A and B are still worth walking in the playtest for everything else they cover; step 5 is no longer the step that matters.
- [X] T057 [P] Update `specs/035-instance-access/spec.md`'s Status line to record what shipped — and note in it that US3–US5 remain unbuilt, so the next reader does not repeat this session's discovery that a "Draft" status says nothing

---

## Dependencies

```text
Phase 1 (Setup: ADR + citation fix)
  └─> Phase 2 (Foundational: migration, models, policy type, settings, events)
        ├─> Phase 3 (US1: the gate)  ──┐
        └─> Phase 4 (US2: invitations) ┘  US2's redemption calls the gate,
                                          so T035 depends on T013
              └─> Phase 5 (Polish)
```

- **US1 is independently shippable.** It needs nothing from US2.
- **US2 depends on US1's gate** (T013) but on nothing else in Phase 3; its web and test tasks are independent of US1's.
- T005 (the conditional seed) blocks T030 (its test) and nothing else.

## Parallel execution examples

**Phase 2** — after T006 regenerates the schema: T007, T010 and T011 touch different files and can run together.

**Phase 3** — T020, T022, T023 are three different files; T026–T030 are five independent test files.

**Phase 4** — T045 through T051 are seven independent server tests and can be written in parallel once T035–T039 exist.

## Implementation strategy

**MVP is US1 alone.** A closed instance that is genuinely closed is shippable
and useful on its own: it is the guarantee, and the spec says nothing else is
worth building first. It leaves the operator with no way to let anyone in,
which is correct for an instance that is meant to be shut.

**US1 + US2 is the demo.** Add invitations and the operator can hand a link to
a named guest — the thing spec 035 exists for.

**Stop there.** US3–US5 are a separate feature sharing the document. They need
their own plan pass before any task in them is written.
