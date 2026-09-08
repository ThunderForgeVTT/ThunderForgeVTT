---

description: "Task list for 041-two-factor-enrolment"
---

# Tasks: Two-Factor Authentication People Can Turn On, Keep, and Recover

**Input**: Design documents from `/specs/041-two-factor-enrolment/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/)

**Tests**: Included, and not optional. Three of this feature's requirements
are claims that can only be *demonstrated* — FR-013 ("starting must not
disable"), FR-016 ("a code must not be accepted twice") and FR-009 ("the
instance cannot display them again") are each an assertion about something
that must **not** happen, and an untested one is indistinguishable from a
wish. `apps/web/e2e/two-factor.spec.ts` already contains FR-013's assertion,
marked `test.fail()`; this feature's job is to make the product satisfy it.
Rust unit tests accompany each server module, as the codebase already does.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: US1–US7 per spec.md
- Every task names the file it touches

## Path Conventions

Web application, per plan.md § Structure Decision: Rust server under
`src/server/src/` (auth is **REST**, not GraphQL), shared auth rules in
`crates/thunderforge-axum-auth-core/`, React shell in `apps/web/src/`,
Playwright suite in `apps/web/e2e/`, Diesel migrations in
`src/server/migrations/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the decisions this feature is not allowed to make silently, and
the split that keeps every later task in a small file. Constitution
Principle IV requires the ADRs in the same change set; research.md already
holds their substance.

**Check the ADR numbers before writing them.** Specs 036–040 were planned in
the same pass: 036 claims 073–075, and 037, 039 and 040 each independently
claim 076–080. 041 takes **081–084** as the first unclaimed block. If those
five plans are reconciled before this phase runs, renumber to match and change
the four filenames below — the decisions are what matter, not the digits.

- [X] T001 [P] Write ADR-081 (a confirmed second factor is replaced, never disarmed) in `docs/adrs/20260907-081-second_factor_replaced_never_disarmed.md`, superseding the `setup/start` behaviour at `src/server/src/auth/two_factor.rs:91` and recording why the pending secret gets its own row
- [ ] T002 [P] Write ADR-082 (one enrolment flow, three authorisations) in `docs/adrs/20260907-082-one_enrolment_flow_three_authorisations.md`, recording the `purpose` extension to `login_two_factor_challenges` and why an enrolment-scoped session was rejected
- [ ] T003 [P] Write ADR-083 (recovery codes are credentials, not links) in `docs/adrs/20260907-083-recovery_codes_are_credentials.md`, citing the admin bootstrap code as the precedent followed and the raw-stored share codes as the one rejected
- [ ] T004 [P] Write ADR-097 (an administrator's second factor is a property of the role) in `docs/adrs/20260907-084-administrator_second_factor_is_a_role_property.md`, recording the computed-not-stored rule, the first-run gate, and the boundary with spec 040's FR-002a
- [ ] T005 Split `src/server/src/auth/two_factor.rs` (491 lines, against a 1000-line gate) into the module directory `src/server/src/auth/two_factor/` — `mod.rs`, `enrolment.rs`, `verification.rs`, `policy.rs` — as pure movement with no behaviour change, so every task below names a small file. Confirm `./scripts/check-file-length.sh` and `cargo test --workspace` are green before anything else lands
- [ ] T006 Promote `hash_password` out of `src/server/src/auth/sessions.rs:409` into a shared helper with a matching `verify_password`, and move the five inline `Argon2::default().verify_password(...)` sites (`sessions.rs:460`, `two_factor.rs:37`, `two_factor.rs:143`, `oauth.rs:666`, `admin_bootstrap.rs:145`) onto it — a sixth inline copy is how parameters drift

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: schema, shared rules and the QR encoder that every story below
reads. Nothing here is user-visible on its own.

**⚠️ CRITICAL**: No user story work begins until this phase is complete

- [ ] T007 Create the Diesel migration `src/server/migrations/2026-09-07-100000-0000_two_factor_enrolment/up.sql` and `down.sql` creating `two_factor_enrolments` (with `UNIQUE (user_id)`) and adding `users.two_factor_last_used_step` and `users.two_factor_required_by` per data-model.md §§ 1–2. The `100000` timestamp is deliberate: spec 036's work already holds `2026-09-07-000000-0000_session_description`, and Diesel keys a migration on that leading version, so two directories sharing it collide
- [X] T008 Create the Diesel migration `src/server/migrations/2026-09-07-100001-0000_recovery_codes/up.sql` and `down.sql` creating `user_recovery_codes` with the partial index on `(user_id) WHERE used_at IS NULL`, per data-model.md § 3
- [ ] T009 Create the Diesel migration `src/server/migrations/2026-09-07-100002-0000_two_factor_events/up.sql` and `down.sql` creating `two_factor_events` with its `event_type` CHECK and `actor_user_id … ON DELETE SET NULL`, and adding `purpose` and `failed_attempts` to `login_two_factor_challenges`, per data-model.md §§ 4, 6. Carry spec 035's comment forward: no identifier column, and none may be added
- [ ] T010 Regenerate `src/server/src/schema.rs` and add the `TwoFactorEnrolment`, `UserRecoveryCode` and `TwoFactorEvent` `Queryable`/`Insertable` pairs to `src/server/src/models.rs`
- [ ] T011 [P] Add `matched_step_at` (and the internal step-returning form the existing `verify_totp_code` / `verify_totp_code_at` are re-expressed in terms of) to `crates/thunderforge-axum-auth-core/src/totp.rs`, with a proptest asserting the returned step is the one the code was generated for across the skew window. **Change no parameter**: SHA1, 6 digits, 30s, skew 1, issuer `ThunderForge`
- [ ] T012 [P] Create `crates/thunderforge-axum-auth-core/src/recovery_codes.rs` — generation over `random_setup_code()`'s alphabet, normalisation (case-fold, strip hyphens and whitespace), and the constant-work match rule — with a proptest that the number of hash comparisons does not depend on whether or where a code matched
- [ ] T013 [P] Create `src/server/src/qr.rs` — `otpauth://` URI to a module matrix (`size` plus one `0`/`1` string per row) and nothing else — adding the pure-Rust QR crate to `src/server/Cargo.toml`, with a unit test asserting a fixed URI produces a fixed matrix. No SVG, no PNG, no markup (research.md § R10)
- [X] T014 Collapse `required(user)` into one function in `src/server/src/auth/two_factor/policy.rs` per data-model.md § 5 — adding the `is_admin` term, dropping the `two_factor_enabled` term — and route both `is_two_factor_required_for_user` and the inline duplicate at `src/server/src/auth/sessions.rs:477` through it, with unit tests for each of the three terms independently

**Checkpoint**: schema, shared rules, QR and the requirement rule exist

---

## Phase 3: User Story 1 — The enrolment flow (Priority: P1) 🎯 MVP

**Goal**: a person can add a second factor from a screen, from any of the
three entrances, and abandoning it changes nothing.

**Independent Test**: enrol from /settings/security, sign out and back in, and
be challenged.

### Tests for User Story 1

- [ ] T015 [P] [US1] Add server tests in `src/server/src/auth/two_factor/enrolment.rs` asserting `setup/start` writes **no** column on `users` and that an abandoned enrolment leaves a confirmed factor in force (FR-004, FR-013)
- [ ] T016 [P] [US1] Add a server test in `src/server/src/auth/two_factor/enrolment.rs` asserting a wrong confirmation code leaves the pending row intact and the same ticket retryable (FR-001c)
- [ ] T017 [P] [US1] Add a server test in `src/server/src/auth/two_factor/enrolment.rs` asserting `setup/start` and `setup/confirm` refuse a request carrying neither a session nor a ticket, and one carrying both (contracts/enrolment.md)

### Implementation for User Story 1

- [X] T018 [US1] Rewrite `two_factor_setup_start` in `src/server/src/auth/two_factor/enrolment.rs` per `contracts/enrolment.md`: take an authorisation instead of `{username, password}`, write the pending secret to `two_factor_enrolments` only, and return the URI, the grouped secret, the QR matrix and the current `two_factor_enabled`
- [X] T019 [US1] Rewrite `two_factor_setup_confirm` in `src/server/src/auth/two_factor/enrolment.rs` — one transaction: verify against the pending secret, write `users`, issue codes (US2's T026), delete the pending row, record the event
- [X] T020 [US1] Add `purpose` handling and ticket issue/consume to `src/server/src/auth/two_factor/verification.rs`, and the `2fa/status` route, wiring all of it in `src/server/src/auth/two_factor/mod.rs` and `src/server/src/auth/mod.rs`
- [ ] T021 [US1] Remove `TwoFactorSetupStartRequest`/`TwoFactorSetupConfirmRequest`'s `username` and `password` fields in `src/server/src/auth/types.rs` and add the ticket and QR-matrix response shapes
- [ ] T022 [P] [US1] Create `apps/web/src/components/two-factor/QrMatrix.tsx` — inline SVG rectangles from the matrix, with a vitest case in `apps/web/src/components/two-factor/__tests__/QrMatrix.test.tsx`. No `dangerouslySetInnerHTML`, no dependency
- [ ] T023 [US1] Create `apps/web/src/components/two-factor/EnrolmentFlow.tsx` — the one flow: QR beside the grouped copyable secret, the confirming code, retry without re-scan, then the codes. It takes where-to-go-afterwards as a prop and nothing else differs between entrances (FR-001a)
- [X] T024 [US1] Create `apps/web/src/api/twoFactor.ts` and `apps/web/src/pages/user/SecuritySettingsPage.tsx`, and register `/settings/security` in `apps/web/src/routes/AppRoutes.tsx` and `apps/web/src/routes/pageLoaders.ts` beside `/settings/storage`
- [X] T025 [US1] Rewrite `apps/web/e2e/two-factor.spec.ts`'s enrolment cases to drive the **interface** rather than `page.request`, and delete the file-header paragraph that says the UI does not exist — it is the reason the file was written that way

**Checkpoint**: a person can enrol from a screen. This is the feature existing at all.

---

## Phase 4: User Story 2 — Getting back in without the phone (Priority: P1)

**Goal**: ten single-use recovery codes, shown once, unreadable afterwards.

**Independent Test**: enrol, take the codes, sign in with one, and confirm the
same one is refused the second time.

**US1 and US2 ship together or neither ships** — spec.md's checklist says so,
and shipping enrolment without recovery creates a new way to permanently lose
an account.

### Tests for User Story 2

- [X] T026 [P] [US2] Add server tests in `src/server/src/auth/two_factor/recovery.rs` for single use via the conditional `UPDATE` (zero rows means refuse), for whole-set replacement on regeneration, and for the low-water count (FR-008, FR-010, FR-011)
- [ ] T027 [P] [US2] Add a server test asserting **no route anywhere returns an issued code** after the body that created it — assert over the registered route table, not by inspection (FR-009)
- [ ] T028 [P] [US2] Add a server test asserting a recovery code does not advance `users.two_factor_last_used_step` (contracts/recovery-codes.md rule 7)

### Implementation for User Story 2

- [X] T029 [US2] Implement issue, spend, regenerate and count-remaining in `src/server/src/auth/two_factor/recovery.rs` per `contracts/recovery-codes.md`, Argon2 via T006's shared helper, full-scan matching with no early exit
- [X] T030 [US2] Accept `recovery_code` at `POST /authentication/2fa/verify` in `src/server/src/auth/two_factor/verification.rs`, refusing a request carrying both `code` and `recovery_code` before evaluating either
- [ ] T031 [US2] Add `POST /authentication/2fa/recovery-codes` (session + possession) in `src/server/src/auth/two_factor/recovery.rs` and wire it in `src/server/src/auth/two_factor/mod.rs`
- [ ] T032 [US2] Report `recovery_codes_remaining` and `recovery_codes_low` on the session response in `src/server/src/auth/sessions.rs`, so a person is told at sign-in and not only if they visit a settings page (FR-011)
- [ ] T033 [P] [US2] Create `apps/web/src/components/two-factor/RecoveryCodeSheet.tsx` — shown once, copyable, downloadable, with an explicit acknowledgement before it can be dismissed
- [ ] T034 [US2] Add the recovery-code field to the challenge card in `apps/web/src/pages/auth/LoginView.tsx`, and the low-codes notice plus "generate a new set" to `apps/web/src/pages/user/SecuritySettingsPage.tsx`
- [ ] T035 [US2] Add the recovery-code cases to `apps/web/e2e/two-factor.spec.ts`: sign in with one, refuse the same one twice, regenerate and confirm every earlier code is dead

**Checkpoint**: enrolling can no longer cost somebody their account

---

## Phase 5: User Story 4 — Turning it off, on purpose (Priority: P1)

**Goal**: the security hole closed, and a front door that costs what the front
door should cost.

**Independent Test**: attempt removal with the password only and be refused;
remove with password plus a code and succeed.

**T036 is the task that makes the existing `test.fail()` pass.** It is already
satisfied by T018 if T018 is done correctly; this phase is the deliberate
removal path FR-014 asks for, plus the step guard.

### Tests for User Story 4

- [ ] T036 [P] [US4] Add a server test in `src/server/src/auth/two_factor/enrolment.rs` that is the assertion `apps/web/e2e/two-factor.spec.ts`'s `test.fail()` case makes, at the unit level: a started-and-abandoned enrolment leaves the confirmed factor in force (FR-013)
- [ ] T037 [P] [US4] Add server tests in `src/server/src/auth/two_factor/verification.rs` for the step guard — an accepted step is refused a second time, a lower step is refused, and two concurrent requests with the same step yield exactly one success — using `matched_step_at`'s explicit clock rather than sleeping (FR-016)
- [ ] T038 [P] [US4] Add server tests in `src/server/src/auth/two_factor/enrolment.rs` for `disable`: refused on the password alone, accepted with password plus code, accepted with password plus recovery code, refused when `required(user)` (FR-012, FR-027)

### Implementation for User Story 4

- [ ] T039 [US4] Implement the step high-water guard in `src/server/src/auth/two_factor/verification.rs` as the conditional `UPDATE` of data-model.md § 6, applied on every path that accepts a TOTP code, with zero rows updated as the refusal
- [ ] T040 [US4] Implement `POST /authentication/2fa/disable` in `src/server/src/auth/two_factor/enrolment.rs` per `contracts/removal-and-reset.md`, clearing the factor, the codes, the pending row and the step mark in one transaction
- [ ] T041 [US4] Implement the five-attempt challenge budget on `login_two_factor_challenges.failed_attempts` in `src/server/src/auth/two_factor/verification.rs`, and confirm the existing e2e assertion that a wrong code does not burn the challenge still holds (FR-017)
- [ ] T042 [US4] Collapse every refusal on the verification path to one message and one status in `src/server/src/auth/two_factor/verification.rs`, per `contracts/verification.md` § Refusal shapes (FR-018)
- [ ] T043 [US4] Add the removal control, behind password plus possession, to `apps/web/src/pages/user/SecuritySettingsPage.tsx`
- [ ] T044 [US4] Rewrite the `test.fail()` block in `apps/web/e2e/two-factor.spec.ts`: **delete the `test.fail()` line**, keep the assertion unchanged, and add the deliberate-removal cases beside it — which is what its own comment says to do when the server is fixed
- [ ] T045 [US4] Add an e2e case to `apps/web/e2e/two-factor.spec.ts` for the same-code-twice refusal inside the window (SC-005)

**Checkpoint**: the hole is closed, with the test that was written for it going green

---

## Phase 6: User Story 3 — An administrator always has one (Priority: P1)

**Goal**: a fresh instance cannot be brought up with an unprotected
administrator, and nobody is locked out by the rule arriving.

**Independent Test**: bring up a fresh instance and confirm setup cannot
complete without the administrator enrolling.

### Tests for User Story 3

- [ ] T046 [P] [US3] Add server tests in `src/server/src/auth/admin_setup.rs` asserting `setup_status` reads incomplete while an administrator exists without a confirmed factor — **both** halves of `admin_exists || setup_completed_at.is_some()` at `admin_setup.rs:37` (FR-028)
- [ ] T047 [P] [US3] Add server tests in `src/server/src/auth/two_factor/policy.rs` asserting `required(user)` is true for every `is_admin` regardless of the instance switch and the per-account flag, and that clearing `is_admin` writes no two-factor column (FR-027, FR-032)

### Implementation for User Story 3

- [ ] T048 [US3] Change `admin_setup_basic` in `src/server/src/auth/admin_setup.rs:65` to return a `setup` ticket instead of a session, and stop calling `mark_admin_setup_complete_sync` there
- [ ] T049 [US3] Move `mark_admin_setup_complete_sync` (`src/server/src/auth/admin_bootstrap.rs:446`) into the enrolment-confirmation transaction, and issue the session there — **after** the recovery codes are in the response body (FR-029)
- [ ] T050 [US3] Change `setup_status` in `src/server/src/auth/admin_setup.rs:37` to "an administrator exists **and** holds a confirmed second factor", and confirm a process death between the two leaves setup resumable rather than locked
- [ ] T051 [US3] Redirect `admin_setup_oauth_callback` (`src/server/src/auth/admin_setup.rs:277`) to the enrolment step rather than `return_to`
- [ ] T052 [US3] Add the enrolment step to `apps/web/src/pages/setup/SetupPage.tsx` using `EnrolmentFlow` unchanged, and add it to the `setupSteps` list the page already renders
- [ ] T053 [US3] Create `apps/web/e2e/two-factor-setup.spec.ts` covering quickstart Scenario G against a fresh instance — including that setup does **not** complete if the administrator stops after creating the account
- [ ] T054 [US3] Assert in the same spec that enrolment completes with no mail configured — which on a fresh instance is the only state there is (FR-001b, SC-010)
- [ ] T055 [US3] Add an e2e case for granting and removing administrator: granted takes the account through enrolment at next sign-in, removed leaves the factor in place (FR-030, FR-032)

**Checkpoint**: an instance cannot come up with a password-only administrator

---

## Phase 7: User Story 5 — Requiring it of everybody, without locking anybody out (Priority: P2)

**Goal**: the switch that exists becomes a switch it is safe to flip.

**Independent Test**: with the requirement on, sign in as an account that never
enrolled and complete enrolment in that same flow.

### Tests for User Story 5

- [ ] T056 [P] [US5] Add a server test in `src/server/src/auth/sessions.rs` asserting a required-but-not-enrolled account receives `two_factor_enrolment_required` and an `enrol` ticket after a correct password — never a refusal (FR-019)
- [ ] T057 [P] [US5] Add a server test asserting turning any requirement off leaves every confirmed factor in force (FR-022)

### Implementation for User Story 5

- [X] T058 [US5] Return `two_factor_enrolment_required` plus an `enrol` ticket from `authenticate_password_login` in `src/server/src/auth/sessions.rs:477-499`, per `contracts/verification.md` § The login path
- [ ] T059 [US5] Apply the same three rows on the OAuth path in `src/server/src/auth/oauth.rs:475-505`, with the instance-wide term not applying to a provider-only account and the administrator term still applying
- [ ] T060 [US5] Add `twoFactorCoverage` to `src/server/src/graphql/queries/admin.rs` beside `auth_security_settings`, returning counts and never a list of accounts (FR-021)
- [ ] T061 [US5] Render the enrolment branch of the challenge step in `apps/web/src/pages/auth/LoginView.tsx` using `EnrolmentFlow` unchanged, preserving the `returnTo` the page already carries (FR-020)
- [ ] T062 [US5] Show the coverage figures beside the switch in `apps/web/src/pages/admin/components/SecurityPanel.tsx`, including how many people the switch is about to ask something of
- [ ] T063 [US5] Update the policy test in `apps/web/e2e/two-factor.spec.ts` to complete enrolment through the challenge and arrive signed in, and **delete the comment saying it "deliberately does not pretend the lockout is fine"** — the lockout is gone

**Checkpoint**: an operator can turn the switch on without stranding anybody

---

## Phase 8: User Story 6 — Requiring it of one person (Priority: P3)

**Goal**: a column and an endpoint that exist get a surface, and gain the
half FR-023 asks for that a boolean cannot answer.

**Independent Test**: require it of one account and confirm that account and no
other is asked to enrol.

- [ ] T064 [P] [US6] Add a server test in `src/server/src/auth/two_factor/policy.rs` asserting setting the per-account requirement affects that account and no other, and records who set it
- [ ] T065 [US6] Write `two_factor_required_by` and a `requirement_set` / `requirement_cleared` event from `set_admin_user_two_factor_required` (`src/server/src/auth/two_factor/policy.rs`, formerly `two_factor.rs:344`)
- [ ] T066 [US6] Create `apps/web/src/pages/admin/components/UserTwoFactorControl.tsx` showing whether it is required and by whom, and wire it into the admin user surface
- [ ] T067 [US6] Add the per-account requirement case to `apps/web/e2e/two-factor-admin.spec.ts`

**Checkpoint**: the per-account switch is reachable without curl

---

## Phase 9: User Story 7 — When somebody has genuinely lost everything (Priority: P3)

**Goal**: a defined, recorded path back that is not a database edit.

**Independent Test**: an operator resets a locked-out account and the reset
appears in the record.

- [ ] T068 [P] [US7] Add server tests in `src/server/src/auth/two_factor/policy.rs` for the reset route: administrator only, idempotent on an account with no factor, and it signs nobody in and issues nothing
- [ ] T069 [US7] Implement `POST /authentication/admin/users/{user_id}/2fa/reset` in `src/server/src/auth/two_factor/policy.rs` per `contracts/removal-and-reset.md`, behind the existing `verify_admin_request` (`src/server/src/auth/admin_setup.rs:394`)
- [ ] T070 [US7] Implement `src/server/src/auth/two_factor/events.rs` — the `two_factor_events` writer with separate `subject_user_id` and `actor_user_id`, written in the transaction it describes, and carry spec 035's rule forward: the record describes the act, never the person
- [ ] T071 [US7] Add the best-effort notification seam in `src/server/src/auth/two_factor/events.rs` — called after commit, unable to fail the request, and a no-op until spec 040 provides mail (FR-001b, FR-015)
- [ ] T072 [US7] Show the account's own second-factor events on `apps/web/src/pages/user/SecuritySettingsPage.tsx`, which is how a person is told on an instance that cannot send mail
- [ ] T073 [US7] Name who can help on the challenge screen in `apps/web/src/pages/auth/LoginView.tsx`, from the realm manifest's `support_email`, and say so plainly where no administrator can act (FR-026)
- [ ] T074 [US7] Add the reset control to the admin user surface and create `apps/web/e2e/two-factor-admin.spec.ts` covering quickstart Scenario I end to end, including that no step needs `psql`

**Checkpoint**: the answer to "I have lost everything" is a route, not a query

---

## Phase 10: Polish & Cross-Cutting Concerns

- [ ] T075 [P] Document the second-factor lifecycle in `docs/` — the three entrances, what a recovery code is and is not, and where the administrator rule comes from
- [ ] T076 [P] Update `MVP.md`'s two-factor line: it lists 2FA as shipped, which was true of the verifier and of nothing a person could reach
- [ ] T077 Make the six guards fail on purpose per quickstart.md § "Making the guards fail on purpose" and record in the commit that each was seen to bite — especially the constant-work one, which is the only guard here whose absence is invisible from outside
- [ ] T078 Run quickstart scenarios A–I by hand against `make dev`, with a real authenticator app, and note anything the suite does not catch
- [ ] T079 Run `cargo test --workspace -j 4`, `make lint` (lint-host + lint-wasm + file length) and `pnpm --filter @thunderforge/web test`
- [ ] T080 Run the full suite via `node scripts/e2e-parallel.mjs --shards=2` with `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1`, and record the figures in the commit body
- [ ] T081 Run `pnpm verify` and fix what it reports **in the code this feature added** — keep it to that; wide lint passes get their own commit. `pnpm verify:fix` rewrites what is mechanical

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies. **T005 (the module split) must land before any Phase 3+ task**, or every later task edits a file that is about to move. The four ADRs may be written alongside, and must land in the same change set (Principle IV)
- **Foundational (Phase 2)**: blocks every story. T007→T008→T009→T010 are ordered (migrations then schema regeneration); T011, T012 and T013 are independent of them and of each other; T014 needs T010
- **US1 (Phase 3)**: needs Phase 2. Nothing else works without it — every other story is entered through this flow
- **US2 (Phase 4)**: needs US1's confirmation transaction (T019) to issue into. **Ships with US1**, not after
- **US4 (Phase 5)**: T036 and T040 need US1; T037 and T039 need only T011. The step guard can be built in parallel with the enrolment UI
- **US3 (Phase 6)**: needs US1 and US2 — first-run setup issues recovery codes, so it cannot precede the thing that issues them
- **US5 (Phase 7)**: needs US1 and T014's `required(user)`
- **US6 (Phase 8)**: needs T014 and T070's event writer
- **US7 (Phase 9)**: needs T070; T070 itself is needed by US6, so build the event writer early if both are in scope
- **Polish (Phase 10)**: after the stories being shipped

### Within Each User Story

- Server tests accompany the surface they test, in the module, as this codebase does
- Migration → schema → model → handler → route → web → e2e
- E2E last within a story, because it needs both halves

### Parallel Opportunities

- T001–T004 — four ADRs, four files
- T011, T012, T013 — three crates/modules, no shared state
- T015/T016/T017, T026/T027/T028, T036/T037/T038, T046/T047 — test tasks within a story
- T022 (QrMatrix) and T033 (RecoveryCodeSheet) — two components, two files
- **Not parallel, despite appearances**: T034, T061 and T073 all land in `apps/web/src/pages/auth/LoginView.tsx`; T023, T043 and T072 all land in `SecuritySettingsPage.tsx`. Write each group together or accept a merge

## Parallel Example: Setup

```bash
Task: "Write ADR-081 in docs/adrs/20260907-081-second_factor_replaced_never_disarmed.md"
Task: "Write ADR-082 in docs/adrs/20260907-082-one_enrolment_flow_three_authorisations.md"
Task: "Write ADR-083 in docs/adrs/20260907-083-recovery_codes_are_credentials.md"
Task: "Write ADR-097 in docs/adrs/20260907-084-administrator_second_factor_is_a_role_property.md"
```

## Parallel Example: Foundational

```bash
# After T010, these three share nothing:
Task: "Add matched_step_at to crates/thunderforge-axum-auth-core/src/totp.rs"
Task: "Create crates/thunderforge-axum-auth-core/src/recovery_codes.rs"
Task: "Create src/server/src/qr.rs"
```

---

## Implementation Strategy

### MVP (US1 + US2 + US4)

Phases 1, 2, 3, 4 and 5. Enrolment somebody can reach, recovery codes so
reaching it is safe, and the security hole closed.

1. Phase 1 — the four ADRs and the module split
2. Phase 2 — migrations, the step-returning verifier, the recovery-code rules, the QR matrix, `required(user)`
3. Phase 3 — the enrolment flow, reachable from account settings
4. Phase 4 — recovery codes, issued and spendable
5. Phase 5 — the step guard and the deliberate removal path
6. **Stop and validate**: quickstart scenarios A, B, C, D and E by hand, plus the suite
7. Demonstrable: a person enrols in three minutes, gets back in with a code, and cannot be disarmed by anybody holding only the password

**US1 and US2 are one shipment, not two.** spec.md's checklist says it plainly:
shipping enrolment without recovery creates a new way for people to
permanently lose their accounts, and the person it happens to did nothing
wrong. **US4 joins them** because it is the security defect, and because
`apps/web/e2e/two-factor.spec.ts` already carries its assertion in the
repository, marked `test.fail()` — shipping the enrolment UI while leaving
that red would mean adding a front door to a building whose back door is
still open.

### Incremental delivery after MVP

1. **US3** — the administrator rule and the first-run gate. Next, because it is
   the only story that changes what a fresh deployment does, and it is the
   one that gets harder the more instances exist
2. **US5** — the instance-wide switch made safe. Small once US1 exists: it is
   one branch in the login path and one component reused
3. **US6** then **US7** — the per-account switch and the reset path. In that
   order: US6's event writer (T070) is US7's as well
4. **Polish** — the guard-breaking pass in T077 is not optional; the
   constant-work guard is the only thing in this feature whose absence
   nothing else would reveal

### The test that is already in the repository

`apps/web/e2e/two-factor.spec.ts` was written today and its final case is
`test.fail()`-marked with a comment ending: "When the server is fixed,
Playwright will report this as 'expected to fail but passed'; drop the
`test.fail()` line then, and add the disable surface's own coverage beside
it." T044 is that instruction, carried out. **The plan is to make the product
satisfy the existing assertion — the assertion does not move.**

---

## Notes

- [P] tasks touch different files and have no incomplete dependency
- Every story is independently testable; the checkpoints say what "done" looks
  like without the later phases
- Verify per target, per Principle V: native `cargo` for the server and the
  auth crate, `tsc`/`vitest` for the web, e2e for the story. **No wasm target
  is involved in this feature** — the engine is not touched
- Commit per task or coherent group; the four ADRs land with the code, not
  after
- Nothing in this feature changes the TOTP algorithm, digits, period, skew or
  issuer. If a task appears to, it has gone wrong
