---

description: "Task list for 040-instance-setup"
---

# Tasks: Instance Setup and Configuration

**Input**: Design documents from `/specs/040-instance-setup/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/)

**Tests**: Included, and several are deliverables rather than verification.
FR-023, FR-027 and SC-007 ("demonstrated by attempting to find one") are
requirements *about* tests; so is FR-024, whose only honest form is a test
asserting a `SYNC_GITHUB_APP_*`-only deployment behaves exactly as it did.
Rust unit tests accompany each server surface in the module, as this codebase
already does.

**Read first**: research.md **§ D**. Two entries there — § D1 (FR-002a makes
US1 depend on spec 041) and § D2 (FR-005 cannot be satisfied by collecting a
name and an email) — want the owner's eye before this list is scheduled.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: US1–US6 per spec.md
- Every task names the file it touches

## Path Conventions

Web application, per plan.md § Structure Decision: Rust server under
`src/server/src/`, the composition root in `src/app/src/`, Diesel migrations in
`src/server/migrations/`, React shell in `apps/web/src/`, Playwright suite in
`apps/web/e2e/`, reviewable legal prose in `legal/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the five decisions this feature is not allowed to make silently.
Constitution Principle IV requires them in the same change set, and research.md
already contains their substance.

- [ ] T001 [P] Write ADR-088 (one precedence rule for every instance setting) in `docs/adrs/20260907-088-one_precedence_rule_for_every_setting.md`, extending ADR-041 from OAuth to the product and recording why the OAuth materialisation is kept rather than rewritten (research.md § R5)
- [ ] T002 [P] Write ADR-089 (mail is a transport seam with a durable outbox) in `docs/adrs/20260907-089-mail_transport_seam_and_outbox.md`, recording the `lettre`/rustls choice, the three levels of proof, and why `blocked` is a separate state from `failed` (research.md § R7–R9)
- [ ] T003 [P] Write ADR-090 (credential resolution: scope outside, source inside) in `docs/adrs/20260907-090-credential_scope_before_source.md`, deciding what the spec left open and recording that an application resolves whole rather than field by field (research.md § R10)
- [ ] T004 [P] Write ADR-091 (instance configuration is rows) in `docs/adrs/20260907-091-instance_configuration_is_rows.md`, recording why not the manifest file, why emphatically not `instance_identity`, and why the six manifest keys are presented rather than migrated (research.md § R1, § R2, § R6)
- [ ] T005 [P] Write ADR-092 (operator values are substituted into compiled-in legal prose at render time) in `docs/adrs/20260907-092-operator_values_in_legal_prose.md`, recording the closed token set, the values-as-text rule, and the trust boundary `apps/web/src/legal/legalDocuments.ts` documents
- [ ] T006 Add the five ADRs to the index table in `docs/adrs/README.md` and re-confirm 088–092 are still free before writing them — specs 036, 037, 039 and 041 are being planned in parallel and several of their reservations already collide with each other (036: 073–075; 037: 078 and 084–087; 039: 080–083; 041: 081–084), so this block was chosen to sit clear of all of them

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the declaration list, the resolver and the harness every story
below reads. Nothing here is user-visible on its own.

**⚠️ CRITICAL**: No user story work begins until this phase is complete

### Schema

- [ ] T007 Create the Diesel migration `src/server/migrations/2026-09-07-000000-0000_instance_settings/up.sql` and `down.sql` for `instance_settings` and `instance_setting_changes` per data-model.md § 1–2, following the commenting style of `src/server/migrations/2026-09-06-000000-0000_instance_access/up.sql`
- [ ] T008 Regenerate `src/server/src/schema.rs` and add `InstanceSetting` / `NewInstanceSetting` / `InstanceSettingChange` structs to `src/server/src/models.rs`
- [ ] T009 Add a migration test asserting `up.sql` → `down.sql` → `up.sql` leaves the schema clean, in `src/server/src/settings/settings_migration_tests.rs` (FR-028's upgrade promise starts with a reversible migration)

### The declaration list and the resolver

- [ ] T010 Create the declaration types and `declarations()` in `src/server/src/settings/registry.rs` per `contracts/settings.md`, with every setting in data-model.md § 0, and a compile-time-checked shape so a declaration cannot omit `env_var`, `secret` or `requirement`
- [ ] T011 [P] Add a registry test in `src/server/src/settings/registry.rs` asserting every declaration has a unique key, that every non-`Prose` declaration has an `env_var`, and that no two declarations share an `env_var` (FR-010, FR-012)
- [ ] T012 Implement `Source`, `Resolved` and `resolve_all` / `resolve` in `src/server/src/settings/resolver.rs`, reading `Backing::Row` from `instance_settings`, `Backing::ManifestFile` through `admin::read_system_manifest`, and `Backing::AccessPolicy` through `auth::instance_access::load_policy` — one row-set load per request, memoised, not a query per setting
- [ ] T013 [P] Add resolver tests in `src/server/src/settings/resolver.rs` asserting environment beats row beats default **for every declared setting** rather than for a sample, and that a `secret` whose ciphertext will not decrypt resolves as unset rather than panicking (data-model.md § 1)
- [ ] T014 Implement the FR-004 validators in `src/server/src/settings/validate.rs` — blank-after-trim, email syntax, reserved TLDs (`.local`, `.example`, `.invalid`, `.test`), and the shipped defaults `stewards@thunderforge.local` and `dmca@thunderforge.example` by name (research.md § R15)
- [ ] T015 [P] Add validator tests in `src/server/src/settings/validate.rs` including that a refusal message never echoes the submitted value
- [ ] T016 Implement the append-only change record in `src/server/src/settings/changes.rs`, redacting per the declaration and storing `redacted` rather than recomputing it (data-model.md § 2)
- [ ] T017 Wire secret encryption and decryption through the existing `src/server/src/crypto.rs` — no second implementation, for the reason that module's own docs give
- [ ] T018 Add `pub mod settings;` to `src/server/src/lib.rs` and hold the memoised resolution on `AppState` in `src/server/src/state.rs`

### Harness

- [ ] T019 Add the `mailpit` service to `compose.yml` beside `postgres` and `rustfs` per `contracts/e2e-fixtures.md`, and wait for it in the `services-up` target in `Makefile` the way `rustfs` already is
- [x] T020 Add the migrated-but-unseeded template database and its per-shard clone to `scripts/e2e-parallel.mjs`, plus a `first-run` Playwright project in `apps/web/playwright.config.ts` pinned to it — US1 is unobservable against the seeded template and that is why no `setup.spec.ts` exists today

**Checkpoint**: a setting can be declared, resolved and changed with a record; the harness can start an instance that has never been set up

---

## Phase 3: User Story 3 — One precedence rule, everywhere (Priority: P1)

**Goal**: an operator setting a value in the environment and an operator
setting it in the admin screens get the same answer to "which one wins?" for
every setting in the product, and can always see the source of a live value.

**Independent Test**: set the same value in both places for several different
settings, confirm the same source wins each time, and that the interface says
so and refuses the edit that would not take.

**Built first** because every other story reads a resolved setting, and
because a precedence rule added afterwards is a precedence rule some surface
already worked around.

### Tests for User Story 3

- [ ] T021 [P] [US3] Add a server test in `src/server/src/graphql/queries/instance_settings.rs` asserting `instanceSettings` reports `ENVIRONMENT` with `fixedBy` for an env-set key and `editable: false` for it (FR-009, FR-011)
- [ ] T022 [P] [US3] Add a server test in `src/server/src/graphql/mutations_instance_settings.rs` asserting `updateInstanceSetting` on an env-fixed key is **refused naming the variable**, not a silent no-op (FR-009, and what ADR-041 records the silent no-op cost the OAuth surface)
- [ ] T023 [P] [US3] Add a redaction test asserting no `secret` declaration is reachable from any read surface except as `SET` / `NOT_SET` — no masked preview, no length — in `src/server/src/graphql/queries/instance_settings.rs` (FR-023, FR-027, SC-007)

### Implementation for User Story 3

- [ ] T024 [US3] Implement the `instanceSettings` and `instanceSettingChanges` queries in `src/server/src/graphql/queries/instance_settings.rs` per `contracts/settings.md`, behind `admin_user(ctx)?`
- [ ] T025 [US3] Implement `updateInstanceSetting` in `src/server/src/graphql/mutations_instance_settings.rs`, writing the change record in the same transaction as the value
- [ ] T026 [US3] Register both surfaces on the roots in `src/server/src/graphql/mod.rs` with an SDL guard test asserting the field names the client uses, in the style of the existing `the_access_surface_is_registered_under_the_names_the_client_uses`
- [ ] T027 [US3] Map `oauth_providers.config_source` onto the same `SettingSource` enum in `src/server/src/graphql/admin_types.rs`, and add `source` / `fixedBy` to `GraphQLSystemManifest`'s `entries` beside the `editable` flag it already carries
- [ ] T028 [P] [US3] Create `apps/web/src/api/instanceSettings.ts` with the GraphQL operations, and `apps/web/src/pages/admin/components/InstanceSettingsPanel.tsx` rendering each setting, its source, and "fixed by `<VAR>`" instead of a disabled field with no explanation
- [ ] T029 [US3] Update `apps/web/src/pages/admin/components/ManifestEditor.tsx` to render `fixedBy` rather than greying a key out, and add the panel to `ADMIN_SECTIONS` in `apps/web/src/pages/admin/components/adminSections.ts`

**Checkpoint**: every setting in the product resolves by one rule and says where its value came from

---

## Phase 4: User Story 2 — Everything set at setup can be changed afterwards (Priority: P1)

**Goal**: an operator changes the support address, the notice contact, the mail
server or a credential months later, from the admin surface, without
redeploying.

**Independent Test**: change each value from the admin surface and confirm the
instance uses the new one without a restart, and that the change is recorded
with who, when and what it was before.

### Tests for User Story 2

- [ ] T030 [P] [US2] Add a server test in `src/server/src/settings/changes.rs` asserting a change to a **secret** records the transition `set` → `set` with `redacted: true` and never either value (FR-008 against FR-023)
- [ ] T031 [P] [US2] Add a server test asserting a changed setting is observed by the next request with no restart, in `src/server/src/settings/resolver.rs` (FR-007)

### Implementation for User Story 2

- [ ] T032 [US2] Add the change-history view to `apps/web/src/pages/admin/components/InstanceSettingsPanel.tsx` — who, when, and what it was before, with redacted rows plainly marked as such
- [ ] T033 [US2] Add the operator identity and notice-contact fields to the panel, grouped so a person editing "who to serve a copyright notice on" is not doing it in a list of thirty keys
- [ ] T034 [P] [US2] Add vitest coverage for the panel's fixed/editable/redacted renderings in `apps/web/src/pages/admin/components/__tests__/InstanceSettingsPanel.test.tsx`
- [ ] T035 [US2] Add `apps/web/e2e/instance-settings.spec.ts` covering quickstart Scenarios B and C: change a value and see it take effect, read its history, and confirm an env-fixed value is shown as fixed and cannot be edited
- [ ] T036 [P] [US2] Document the new environment variable families in `.env.example`, in the same style `SYNC_GITHUB_APP_*` is documented — what each does, and what happens when it is absent

**Checkpoint**: a wrong value entered once is not permanent, and changing it leaves a trail

---

## Phase 5: User Story 6 — An instance says what it is not ready for (Priority: P2)

**Goal**: an administrator sees, in one place, what this instance can and
cannot do given how it is configured — and, for publishing beyond a world,
a refusal rather than a warning.

**Independent Test**: bring up an instance with several things unset and
confirm each is listed with what it disables and how to fix it; confirm a
share beyond a world is refused and a world stays fully playable.

**Built before US1** because setup's final screen is the readiness report:
FR-003's "setup completes and the instance says what is still unset" is this
surface, reached from a different place.

### Tests for User Story 6

- [ ] T037 [P] [US6] Add a server test in `src/server/src/readiness.rs` asserting no gap contains a value, a fragment or a length, for settings that are set as well as unset (FR-027)
- [ ] T038 [P] [US6] Add a server test asserting a fully configured instance reports `fullyConfigured: true` positively rather than by an empty gap list (FR-025 scenario 3)
- [ ] T039 [P] [US6] Add a server test asserting the server starts and serves with **every** setting unset (FR-028, SC-009)
- [ ] T040 [P] [US6] Add a server test asserting an **existing** share link still resolves while the notice contact is unset — only creation is gated, because removing a setting must not break links already issued

### Implementation for User Story 6

- [ ] T041 [US6] Implement `InstanceReadiness` in `src/server/src/readiness.rs`, derived from `settings::registry` per `contracts/readiness.md`, with no stored flag and no cache
- [ ] T042 [US6] Fold `instanceRepositoryIntegration` into the report as one capability in `src/server/src/graphql/queries/lore_sync.rs`, reusing `RegistrationProblem::guidance()` verbatim rather than writing a second vocabulary
- [ ] T043 [US6] Record source flips at startup and expose them, in `src/server/src/readiness.rs` — the spec's "a container is redeployed with a fresh environment and an existing database" edge case
- [ ] T044 [US6] Implement `readiness::may_publish_beyond_world(state)` in `src/server/src/readiness.rs` as the single predicate the gate calls
- [ ] T045 [US6] Apply the gate in `src/server/src/graphql/mutations_collection_shares.rs`, refusing creation with a message naming the missing setting (FR-026, spec 039 FR-053)
- [ ] T046 [P] [US6] Apply the same gate to singleton share creation in `src/server/src/graphql/mutations_actor_shares.rs`, `mutations_item_shares.rs` and `mutations_ability_shares.rs` — the family ADR-069/070/071 already treat as one
- [ ] T047 [US6] Add the `instanceReadiness` query to `src/server/src/graphql/queries/instance_settings.rs` and register it in `src/server/src/graphql/mod.rs` with an SDL guard
- [ ] T048 [US6] Create `apps/web/src/pages/admin/components/ReadinessPanel.tsx` and add its entry to `apps/web/src/pages/admin/components/adminSections.ts`
- [ ] T049 [US6] Add `apps/web/e2e/instance-readiness.spec.ts` covering quickstart Scenario F, including that a world stays fully playable with no notice contact
- [ ] T050 [US6] Close spec 039's dependency by noting in `specs/039-sharing-attestation/spec.md` (FR-053's pointer) that the gate is implemented here, so the two do not drift

**Checkpoint**: an unconfigured instance is honest about it, and the one gate spec 039 depends on is a refusal

---

## Phase 6: User Story 1 — From empty database to a real instance, in one pass (Priority: P1) 🎯 MVP

**Goal**: somebody deploys ThunderForge, opens it, and is walked through what
this deployment needs to be — finishing with an instance that is usable and
contactable, without editing a file.

**Independent Test**: bring up an empty database, complete setup, and confirm
the instance is usable and that its published pages name a real operator and a
real contact.

**⚠️ Depends on spec 041.** FR-002a forbids completing setup without a
confirmed second factor, and no enrolment interface exists anywhere in the
application (041's own Context says so). T063 implements the *predicate*; the
*flow* is 041's. See research.md § D1 before scheduling this phase.

### Tests for User Story 1

- [x] T051 [P] [US1] Add a server test in `src/server/src/auth/admin_setup.rs` asserting two concurrent `/authentication/setup/complete` calls produce one success and one `409 setup_complete` — never a generic `500` from a unique-constraint violation (spec Edge Case; today the check-then-insert has no transaction)
- [x] T052 [P] [US1] Add a server test in `src/server/src/auth/admin_bootstrap.rs` asserting an unconsumed bootstrap code **survives a restart** rather than being regenerated (FR-006; today `ensure_admin_bootstrap_code` mints a fresh one on every start and silently kills the operator's link)
- [x] T053 [P] [US1] Add a server test asserting `/authentication/setup/complete` is refused while any `RequiredAtSetup` declaration is unset, naming which (FR-002, FR-003)
- [x] T054 [P] [US1] Add a server test asserting `/authentication/setup/complete` is refused while the first administrator's `two_factor_confirmed_at` is null (FR-002a)
- [x] T055 [P] [US1] Add a vitest in `apps/web/src/legal/__tests__/legalDocuments.test.ts` asserting an **unset** token still renders its visible `[OPERATOR — …]` marker, preserving that file's existing invariant

### Implementation for User Story 1 — the server

- [x] T056 [US1] Extend `SetupStatusResponse` with `required_settings` and `second_factor_confirmed` in `src/server/src/auth/admin_setup.rs` per `contracts/setup.md`, so the wizard is driven by the registry and not by a hard-coded list of steps
- [x] T057 [US1] Add `POST /authentication/setup/settings` in `src/server/src/auth/admin_setup.rs`, writing each step's values as it is completed so resumability is a property of the storage (FR-006)
- [x] T058 [US1] Add `POST /authentication/setup/complete` in `src/server/src/auth/admin_setup.rs`, wrapping the admin-exists check, the user insert and `setup_completed_at` in **one** `conn.transaction`, following `instance_identity::instance_id`'s reasoning about check-then-insert windows
- [x] T059 [US1] Reuse an existing unconsumed bootstrap code in `src/server/src/auth/admin_bootstrap.rs` and offer a deliberate regeneration instead of minting one per start
- [x] T060 [US1] Stop hard-coding `http://127.0.0.1:5173/setup/{code}` in the `tracing::warn!` in `src/server/src/auth/admin_bootstrap.rs` — it is the first thing an operator running a container sees and it is wrong for every one of them
- [x] T061 [US1] Register the two new routes in `src/server/src/auth/mod.rs`'s `router()`
- [x] T062 [US1] Add the unauthenticated `publishedOperatorValues` query in `src/server/src/graphql/anonymous.rs` per `contracts/legal-rendering.md` — spec 039's FR-056 requires the notice contact be reachable without an account — exposing those six values and nothing else
- [x] T063 [US1] Implement the completion predicate in `src/server/src/auth/admin_setup.rs`: every `RequiredAtSetup` declaration resolves **and** `users.two_factor_confirmed_at` is non-null for the first administrator. **The enrolment flow is spec 041's and is not designed here** (FR-002a)

### Implementation for User Story 1 — the legal pages

- [x] T064 [US1] Replace the four *value* markers in `legal/terms-of-service.md` and `legal/privacy-policy.md` with the closed tokens from `contracts/legal-rendering.md`, leaving the ten *prose* markers exactly as they are (research.md § D2)
- [x] T065 [US1] Create `apps/web/src/legal/operatorTokens.ts` with the closed token set and the substitution returning literal/value segments, and apply it in `apps/web/src/legal/legalDocuments.ts` — a substituted value is rendered as **text** and never passes through `LegalProse`'s inline parser
- [x] T066 [US1] Render the segments in `apps/web/src/components/legal/LegalProse.tsx` without extending what it parses, keeping its "trusted input only" invariant intact
- [x] T067 [US1] Replace the hard-coded designated-agent literals in `apps/web/src/pages/legal/DmcaCompliancePage.tsx` with the resolved notice-contact values, keeping the "configure before launch" text as the unset rendering
- [x] T068 [P] [US1] Update `legal/README.md`'s "Open items" to record the split between fillable values and operator prose blocks, and what is now filled from settings

### Implementation for User Story 1 — the wizard

- [x] T069 [US1] Break `apps/web/src/pages/setup/SetupPage.tsx` into steps under `apps/web/src/pages/setup/steps/` — account, operator, notices, support, mail, second factor, review — driven by `required_settings` rather than by a hard-coded sequence
- [x] T070 [US1] Add data-testids throughout the setup steps; **there are none on `SetupPage.tsx` today**, which is part of why first-run has never been tested
- [x] T071 [US1] Show a setting the environment has fixed as fixed, and do not ask for it, in `apps/web/src/pages/setup/steps/` (FR-009) — an instance configured wholly by environment reaches the review step in two screens
- [x] T072 [US1] Show the readiness report on the review step so setup ends by saying what is still unset (FR-003, FR-005 scenario 5)
- [x] T073 [US1] State on the notices step that registering a designated agent, where the operator's jurisdiction requires one, is the operator's own obligation and is not performed by this software (spec 039 FR-055)
- [x] T074 [US1] Add `apps/web/e2e/instance-setup.spec.ts` in the `first-run` project, covering quickstart Scenario A end to end — including reading the bootstrap link from the server log as an operator would, and asserting the legal pages name the entered operator afterwards

**Checkpoint**: an empty database becomes a usable, contactable instance in one pass, with no file edited — SC-001

---

## Phase 7: User Story 4 — The instance can send mail, and knows whether it can (Priority: P2)

**Goal**: an operator configures a mail server, sends a test message to
themselves, and it arrives. An operator who configures nothing gets an
instance that says plainly which features are affected rather than one that
silently drops messages.

**Independent Test**: configure a mail server, send a test message, receive it;
then remove the configuration and confirm the affected features say what they
cannot do and that nothing is discarded.

**This is the largest single piece of the feature.** There is no mail
subsystem anywhere in this codebase — confirmed across every `Cargo.toml`,
`Cargo.lock` and source file.

### Tests for User Story 4

- [ ] T075 [P] [US4] Add unit tests for the outbox state machine in `src/server/src/mail/outbox.rs` — `queued`/`blocked`/`sending`/`sent`/`failed`, and that configuring mail moves `blocked` back to `queued` (FR-015)
- [ ] T076 [P] [US4] Add a test in `src/server/src/mail/mod.rs` asserting no `DeliveryFailure::reason` for any `lettre` error class contains a configured value — host, username or password (FR-014, SC-004)
- [ ] T077 [P] [US4] Add a test asserting no read surface returns a message body, in `src/server/src/graphql/mutations_mail.rs` (FR-016)
- [ ] T078 [P] [US4] Add a test asserting a `sent` message is never sent twice by a retry, guarded by `sent_at` written in the same transaction as the state (idempotence)
- [ ] T079 [US4] Add a Rust **integration** test sending through a real `SmtpTransport` to Mailpit in `src/server/src/mail/smtp_integration_tests.rs`, asserting the From address and that the message arrives — the level it is tempting to skip and the one where mail actually fails

### Implementation for User Story 4

- [ ] T080 [US4] Create the Diesel migration `src/server/migrations/2026-09-07-000100-0000_mail_outbox/up.sql` and `down.sql` for `mail_outbox` per data-model.md § 3, and regenerate `src/server/src/schema.rs`
- [ ] T081 [US4] Add `lettre` with `tokio1-rustls-tls` and `smtp-transport`, `default-features = false`, to `src/server/Cargo.toml` — rustls, not native-tls, because the first-party tree is rustls throughout and `openssl` appears only through the legacy `websocket` chain
- [ ] T082 [US4] Define `MailTransport`, `OutgoingMessage`, `Availability` and `DeliveryFailure` in `src/server/src/mail/mod.rs` per `contracts/mail.md`, with `Unconfigured` refusing with the list of missing settings rather than being an `Option` every call site must remember to check
- [ ] T083 [US4] Implement `SmtpTransport` in `src/server/src/mail/smtp.rs`, built from resolved `mail.*` settings and rebuilt when one changes so FR-007 holds with no restart
- [ ] T084 [P] [US4] Implement `CapturingTransport` in `src/server/src/mail/capture.rs` behind `#[cfg(any(test, feature = "test-support"))]`, and expose it from `src/server/src/test_support.rs` the way the adjudicator already is
- [ ] T085 [US4] Implement `enqueue` and the operator-visible projection in `src/server/src/mail/outbox.rs` — **nothing calls `MailTransport::send` directly**, which is what makes FR-015 true by construction
- [ ] T086 [US4] Implement `spawn_mail_task` and `due_now` in `src/server/src/mail/schedule.rs`, modelled on `src/server/src/lore_sync/schedule.rs` — the same nine-step backoff array, and selection extracted **outside** the spawned loop for the reason that file states
- [ ] T087 [US4] Register `mail::schedule::spawn_mail_task(app_state)` in `src/app/src/main.rs` beside the five `spawn_*_task` calls already there, and add `pub mod mail;` to `src/server/src/lib.rs`
- [ ] T088 [US4] Implement `mailAvailability`, `mailOutbox`, `sendTestMail` and `retryOutboxMessage` in `src/server/src/graphql/mutations_mail.rs` per `contracts/mail.md`, with `sendTestMail` behind `admin_user(ctx)?` **and** the existing limiter from `src/server/src/graphql/share_rate_limit.rs` — an unrestricted send-to-any-address mutation is an open relay
- [ ] T089 [US4] Register the mail surface in `src/server/src/graphql/mod.rs` with an SDL guard asserting `OutboxEntry` has no body field
- [ ] T090 [US4] Create `apps/web/src/pages/admin/components/MailPanel.tsx` — settings, the test message, and the outbox with no body anywhere — and add its entry to `apps/web/src/pages/admin/components/adminSections.ts`
- [ ] T091 [US4] Add mail to the readiness capability list in `src/server/src/readiness.rs`, naming the missing settings and what is limited (FR-015, FR-017)
- [ ] T092 [P] [US4] Create `apps/web/e2e/fixtures/mailpit.ts` with `inbox`, `waitForMessage` and `clearInbox` per `contracts/e2e-fixtures.md`
- [ ] T093 [US4] Add a Mailpit SMTP and API port per shard in `scripts/e2e-parallel.mjs`, exactly as backends, vite servers and buckets already get one
- [ ] T094 [US4] Add `apps/web/e2e/mail-delivery.spec.ts` covering quickstart Scenario D — configure, test, receive, break it, confirm nothing prints a password, clear the settings, confirm a message is blocked rather than discarded, reconfigure, confirm it goes

**Checkpoint**: the instance can tell somebody something, and says so honestly when it cannot

---

## Phase 8: User Story 5 — GitHub credentials at two scales (Priority: P2)

**Goal**: an operator registers one application for everything, or one per
subsystem, or a mixture — and understands what they have chosen while they are
choosing it.

**Independent Test**: configure only a global app and confirm every subsystem
uses it; add a subsystem-specific one and confirm it wins for that subsystem
alone.

### Tests for User Story 5

- [ ] T095 [P] [US5] Add the **back-compatibility** test in `src/server/src/github_apps.rs`: a process with only `SYNC_GITHUB_APP_*` set resolves lore sync's application identically to `registration_from_env()` today (FR-024). This is the test that protects somebody's running deployment and it is written first
- [ ] T096 [P] [US5] Add a resolution test asserting the order in `contracts/github-applications.md` — subsystem-env, subsystem-instance, global-env, global-instance — including the case the spec left open: a global env application does **not** beat a deliberately configured subsystem one (research.md § R10)
- [ ] T097 [P] [US5] Add a test asserting a partially specified subsystem application is reported incomplete and falls through to the **whole** global application rather than borrowing its private key (FR-021, US5 scenario 4)
- [ ] T098 [P] [US5] Add a test asserting no field, message or log line carries a key, a fragment or a length, in `src/server/src/graphql/mutations_github_apps.rs` (FR-023, SC-007)

### Implementation for User Story 5

- [ ] T099 [US5] Implement `registration_for(subsystem)` in `src/server/src/github_apps.rs`, with `repo_host::registration_from_env()` unchanged as its first step, and reuse `RegistrationProblem` and `guidance()` rather than writing a second error vocabulary
- [ ] T100 [US5] Add the `github_app.global.*` and `github_app.<subsystem>.*` declarations to `src/server/src/settings/registry.rs`, with the private key marked `secret` and the `_FILE` form storing a path rather than the key so a Docker secret stays one
- [ ] T101 [US5] Parse a private key on save in `src/server/src/graphql/mutations_github_apps.rs` exactly as `registration_from_env` parses it at startup, refusing a value that is not a key rather than storing it as configured (FR-022)
- [ ] T102 [US5] Implement `githubApplications`, `setGithubApplication` and `checkGithubApplication` in `src/server/src/graphql/mutations_github_apps.rs` per `contracts/github-applications.md`, with `actsFor` computed from the current resolution (FR-020)
- [ ] T103 [US5] Register the surface in `src/server/src/graphql/mod.rs` with an SDL guard asserting no field returns a key
- [ ] T104 [US5] Create `apps/web/src/pages/admin/components/GitHubAppsPanel.tsx` showing each scope, its source, its completeness, and — wherever global credentials are set — which subsystems the application will act for
- [ ] T105 [US5] Add the `GLOBAL_GITHUB_APP_*` family to `.env.example`, documented in the same style `SYNC_GITHUB_APP_*` already is, including the three private-key forms and their precedence
- [ ] T106 [US5] Add `apps/web/e2e/github-apps.spec.ts` covering quickstart Scenario E, starting with the FR-024 case: a `SYNC_GITHUB_APP_*`-only deployment behaves exactly as before
- [ ] T107 [P] [US5] Update spec 037's FR-023–FR-026 pointer in `specs/037-in-app-feedback/spec.md` to name this contract, so `FEEDBACK_GITHUB_APP_*` is configured here rather than invented there

**Checkpoint**: one application or several, and the operator knows which acts for what

---

## Phase 9: Polish & Cross-Cutting Concerns

- [ ] T108 [P] Document instance configuration in `docs/` — the precedence rule, how to add a setting, and what the declaration's fields buy you
- [ ] T109 [P] Update `MVP.md` with what this feature changed: first-run setup, the settings surface, mail, and the publish gate
- [ ] T110 Make the six guards fail on purpose per quickstart.md § "Making the guards fail on purpose" and record in the commit body that each was seen to bite
- [ ] T111 Run quickstart Scenario G by hand — an existing deployment upgraded with no reconfiguration and no failure to start (FR-024, FR-028, SC-009) — and record the result; it is not automatable in the current harness
- [ ] T112 Run quickstart Scenarios A–F by hand against `make dev` and note anything the suite does not catch; Scenario A with a stopwatch, because SC-001 is a claim about a human
- [ ] T113 Search the entire product for a rendered credential — screens, GraphQL responses, logs, the audit trail — and record what was searched and found. SC-007 says "demonstrated by attempting to find one", so the attempt is the deliverable
- [ ] T114 Run `cargo test --workspace -j 4`, `make lint` (lint-host + lint-wasm + file length) and `pnpm --filter @thunderforge/web test`
- [ ] T115 Run the full suite via `node scripts/e2e-parallel.mjs --shards=2`, including the `first-run` project, and record the figures in the commit body
- [ ] T116 Run `pnpm verify` and fix what it reports **in the code this feature added** — keep it to that; wide lint passes get their own commit

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies. The five ADRs may be written while Phase 2 proceeds but must land in the same change set (Principle IV)
- **Foundational (Phase 2)**: blocks every story. T007→T008→T009 are ordered; T010→T012→T013 are ordered; T019 and T020 are independent of everything else here
- **US3 (Phase 3)**: needs Phase 2 entire. Built first because every other story reads a resolved setting
- **US2 (Phase 4)**: needs US3's surface. Cheap once it exists
- **US6 (Phase 5)**: needs Phase 2's registry. T044→T045→T046 are ordered; the rest is parallel
- **US1 (Phase 6)**: needs US6 (the review step shows readiness) and **spec 041** (T063's predicate has no flow behind it until 041 lands — research.md § D1)
- **US4 (Phase 7)**: needs Phase 2 and T019/T020. Independent of US1, US5 and US6 apart from T091's readiness entry
- **US5 (Phase 8)**: needs Phase 2 only. Shares no file with US4 and can run beside it
- **Polish (Phase 9)**: after the stories being shipped

### Within Each User Story

- Server tests accompany the surface they test, in the module, as this codebase does
- Registry and resolver before every GraphQL surface that reads them
- Server surface before the web client that calls it
- E2E last within a story, because it needs both halves
- In Phase 8, **T095 is written before T099**. It is the test that says an existing operator's deployment still works, and writing it after the refactor is how it quietly gets written to match the refactor

### Parallel Opportunities

- T001–T005 — five ADRs, five files
- T011, T013, T015 — different modules, no shared state
- T021, T022, T023 — test tasks within US3
- T037–T040 — test tasks within US6
- T075–T078 — test tasks within US4
- T095–T098 — test tasks within US5
- **US4 (Phase 7) and US5 (Phase 8) share no file** and can be given to two people
- T028 and T029 both touch the admin components directory but different files; T029 and T048 and T090 and T104 all edit `adminSections.ts`, so accept a merge or add all four entries in one commit

## Parallel Example: Setup

```bash
Task: "Write ADR-088 in docs/adrs/20260907-088-one_precedence_rule_for_every_setting.md"
Task: "Write ADR-089 in docs/adrs/20260907-089-mail_transport_seam_and_outbox.md"
Task: "Write ADR-090 in docs/adrs/20260907-090-credential_scope_before_source.md"
Task: "Write ADR-091 in docs/adrs/20260907-091-instance_configuration_is_rows.md"
Task: "Write ADR-092 in docs/adrs/20260907-092-operator_values_in_legal_prose.md"
```

## Parallel Example: after Foundational

```bash
# Two people, no shared file:
Task: "Phase 7 — the mail subsystem, src/server/src/mail/*"
Task: "Phase 8 — GitHub applications, src/server/src/github_apps.rs"
```

---

## Implementation Strategy

### MVP — Phases 1, 2, 3, 5 and 6 (US3 + US2 + US6 + US1)

That is **an empty database becoming a usable, contactable instance, with one
precedence rule and an honest account of what is missing** — and it is the
smallest slice that is worth anything, because each of the four is unusable
without the others:

- **US3 without the rest** is plumbing nobody sees.
- **US1 without US3** collects values with no defined precedence, which is the
  problem the spec exists to fix, reproduced one screen further on.
- **US1 without US6** ends setup on a screen that cannot say what is unset,
  which FR-003 requires.
- **US6's gate without US1's collection** refuses publishing on every instance
  with no way to fix it.

Phase 4 (US2) is a small addition on top of Phase 3 and should ship with it: a
setup wizard whose answers are frozen is a worse version of the file it
replaced.

1. Phase 1 (five ADRs) and Phase 2 (schema, registry, resolver, harness)
2. Phase 3 — one rule, one source, visible
3. Phase 4 — editable afterwards, with a record
4. Phase 5 — readiness, and the gate spec 039 depends on
5. Phase 6 — the pass itself, and the legal pages naming a real operator
6. **Stop and validate**: quickstart Scenarios A, B, C, F and G by hand, plus
   the suite
7. Demonstrable: an empty database to a contactable instance in under ten
   minutes, with no file edited

**The scheduling constraint on the MVP**: Phase 6 cannot *complete* before
spec 041's enrolment flow exists (FR-002a). Phases 1–5 have no such
dependency and are worth shipping on their own — they are the configuration
spine every later spec was going to need anyway. If 041 is not close, ship
Phases 1–5 and hold Phase 6.

### Incremental delivery after MVP

1. **US4 (mail)** — the largest piece, and the one three other specs are
   already promising. It is a demo on its own: configure a server, press a
   button, watch a message arrive
2. **US5 (GitHub applications)** — shares no file with US4 and can run beside
   it. It retires the "each subsystem decides again" problem before spec 037
   arrives and decides again

### A note on sequencing with spec 041

041 needs no mail (its FR-001b is explicit: "enrolment MUST NOT require the
instance to be able to send a message"), and 040 needs 041 only for setup's
last step. So the two interlock at exactly one point and nowhere else. The
cheapest order is: 040 Phases 1–5, then 041's enrolment, then 040 Phase 6.

---

## Notes

- [P] tasks touch different files and have no incomplete dependency
- Every story is independently testable; the checkpoints say what "done" looks
  like without the later phases
- Verify per target, per Principle V: native `cargo` for the server, wasm for
  the engine (unchanged here, still linted), `tsc`/`vitest` for the web, e2e
  for the story
- Commit per task or coherent group; the ADRs land with the code, not after
- **Nothing in this feature may render a credential, a fragment of one, or its
  length.** If a task's implementation makes that hard, the task is wrong —
  not the rule
