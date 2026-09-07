---

description: "Task list for 037-in-app-feedback"
---

# Tasks: In-App Feedback, Delivered as GitHub Issues

**Input**: Design documents from `/specs/037-in-app-feedback/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/)

**Tests**: Included, and three of them are the deliverable rather than its
verification. SC-004 (a planted secret that does not arrive), FR-019's
ambiguous-failure adoption, and FR-016's "nothing else was deleted" are
*claims this feature makes about safety*, and a claim without a test is a
sentence. Rust unit tests accompany each server surface, as this codebase
already does.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: US1–US6 per spec.md
- Every task names the file it touches

## Path Conventions

Web application, per plan.md § Structure Decision: Rust server under
`src/server/src/`, its binary at `src/app/src/main.rs`, migrations at
`src/server/migrations/`, React shell in `apps/web/src/`, Playwright suite in
`apps/web/e2e/`. Nothing in `src/engine/` or `crates/` changes except where a
task says so.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the decisions this feature is not allowed to make silently.
Constitution Principle IV requires these in the same change set, and
research.md already contains their substance.

- [ ] T001 **Credential resolution's ADR belongs to spec 040** (its reserved ADR-078, "scope outside, source inside"). Check `docs/adrs/` for it: if it exists, reference it from this feature's artifacts and do nothing else; if it does not, write it under 040's reserved number and note in `specs/040-instance-setup/plan.md` that 037 landed it — do **not** write a second ADR for the same decision under an 037 number
- [ ] T002 [P] Write ADR-084 (the submission is the record, the tracker is a destination) in `docs/adrs/20260907-084-feedback_submission_is_the_record.md`, recording FR-018's ordering, the delivery pass modelled on `lore_sync/schedule.rs`, and what "exactly once" can honestly mean against a host with no idempotency key
- [ ] T003 [P] Write ADR-085 (redaction is a capture-time client filter; the server refuses, never rewrites) in `docs/adrs/20260907-085-feedback_redaction_before_review.md`, stating the trust boundary and why a server-side rewrite after approval satisfies FR-012's words and breaks its promise
- [ ] T004 [P] Write ADR-086 (feedback attachments are stored unshared so they can expire) in `docs/adrs/20260907-086-feedback_attachments_are_deletable.md`, answering the warning in `src/server/src/storage/dedupe.rs` — deletion exists now, it is confined to a prefix nothing dedupes, and extending it to canvas assets still requires the reference counting that module describes
- [ ] T005 [P] Write ADR-087 (feedback delivery is not a public repository of user content) in `docs/adrs/20260907-087-feedback_delivery_dmca_determination.md`, the constitution's DMCA checkpoint determination, following ADR-067's reasoning and recording the two obligations it makes requirements: FR-014's pre-submission notice and FR-010's removable review
- [ ] T006 [P] Add the ADRs to the index table in `docs/adrs/README.md`, and **confirm the numbers are still free** — specs 036, 039 and 040 all reserve above 072 and 039/040 already collide on 080 (plan.md § Numbering). If 084–087 have been taken while this was in flight, renumber here and find-and-replace across this spec's artifacts
- [ ] T007 Document `FEEDBACK_GITHUB_APP_*` and `GLOBAL_GITHUB_APP_*` in `.env.example` beside the existing `SYNC_GITHUB_APP_*` block, and **fix the pre-existing error there** — that block currently describes `SYNC_GITHUB_APP_CLIENT_ID` as the "NUMERIC identifier", contradicting `repo_host.rs`'s own documentation and GitHub's recommendation

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: schema, credentials, storage and the version surface every story
below reads. Nothing here is user-visible on its own.

**⚠️ CRITICAL**: No user story work begins until this phase is complete

- [ ] T008 Create the Diesel migration `src/server/migrations/2026-09-07-010000-0000_feedback_submissions/up.sql` and `down.sql` with `feedback_submissions`, `feedback_attachments`, `feedback_delivery_attempts` and `feedback_destination` per data-model.md, including every CHECK constraint and the three partial indexes
- [ ] T009 Regenerate `src/server/src/schema.rs` for the four tables (`diesel migration run`, then the `joinable!` and `allow_tables_to_appear_in_same_query!` entries), and add `FeedbackSubmission`/`NewFeedbackSubmission`, `FeedbackAttachment`/`NewFeedbackAttachment`, `FeedbackDeliveryAttempt`/`NewFeedbackDeliveryAttempt`, `FeedbackDestination` to `src/server/src/models.rs`
- [ ] T010 Add `AppScope`, `Field`, `ScopedApp` and `registration_for(scope)` to `src/server/src/repo_host.rs` per `contracts/github-app-scopes.md`, with per-field fallback and a recorded source per field; keep `registration_from_env()` delegating to `AppScope::Sync`
- [ ] T011 Move the `git_is_available()` check out of the shared resolver into the sync scope only, in `src/server/src/repo_host.rs`, and add a test in `src/server/src/repo_host_tests.rs` asserting a feedback-scope resolution succeeds with no `git` on PATH
- [ ] T012 [P] Add tests in `src/server/src/repo_host_tests.rs` for scoped resolution: specific-wins-per-field, global-fills-the-rest, all three private-key forms under each prefix, and that no `RegistrationProblem` message contains a value, a fragment or a length
- [ ] T013 [P] Add `rustfs::delete_object(cfg, key)` to `src/server/src/storage/rustfs.rs`, refusing any key not under `feedback/` **inside the function**, with unit tests for both the accept and the refuse case
- [ ] T014 [P] Update the module docs in `src/server/src/storage/dedupe.rs` — object deletion now exists, where it is confined, why feedback attachments are safe to delete (they are never deduped and carry no `content_hash`), and that widening it to canvas assets still needs the reference counting that module already describes
- [ ] T015 [P] Add the application version surface: `env!("CARGO_PKG_VERSION")` plus `option_env!("THUNDERFORGE_GIT_SHA")` on an authenticated GraphQL query in `src/server/src/graphql/queries/healthcheck.rs`, and a `define` block in `apps/web/vite.config.mts` exposing the same pair to the client — **not** on `/api/status`, which deliberately reports no build identifier and is unauthenticated
- [ ] T016 Create `config/feedback-redaction.json` with the seven rule kinds from `contracts/attachments.md` § 2, as the single list both the client filter and the server validator read
- [ ] T017 Create `src/server/src/feedback/mod.rs` (record, states, retention constants) and `src/server/src/feedback/redaction.rs` (the validator that refuses and never rewrites), wire `pub mod feedback;` into `src/server/src/lib.rs`, with unit tests per rule kind including one that asserts an approved payload is returned byte-identical when it contains nothing

**Checkpoint**: schema, scoped credentials, deletable storage, a version and the redaction rules exist; stories can start

---

## Phase 3: User Story 1 — Say something from wherever you are (Priority: P1) 🎯 MVP

**Goal**: a person sends one of three kinds from any screen and is returned to
what they were doing.

**Independent Test**: submit one of each kind from three different screens and
confirm each is accepted, acknowledged, and does not navigate the person away.

### Tests for User Story 1

- [ ] T018 [P] [US1] Add server tests in `src/server/src/graphql/mutations_feedback.rs` for `submitFeedback`: each of the three kinds recorded, `game_system_id` resolved from `world_id` and never taken from the input, a submission with no `world_id` accepted, and the row committed before any delivery call exists
- [ ] T019 [P] [US1] Add server tests in `src/server/src/graphql/feedback_rate_limit.rs` for the window and the per-account key, including `the_e2e_auth_bypass_does_not_disable_this_limiter` mirroring the test of that name in `src/server/src/graphql/share_rate_limit.rs`

### Implementation for User Story 1

- [ ] T020 [US1] Create `src/server/src/graphql/feedback_rate_limit.rs` — 5 per 10 minutes, keyed on the account id, on the sliding-window shape of `src/server/src/graphql/share_rate_limit.rs`, returning `extensions.code = "FEEDBACK_RATE_LIMITED"`
- [ ] T021 [US1] Implement `submitFeedback` in `src/server/src/graphql/mutations_feedback.rs` per `contracts/feedback.md` — authorize, rate limit, validate redaction, write submission + attachment rows + objects in one transaction, return
- [ ] T022 [US1] Register the feedback query and mutation objects on `QueryRoot` and `MutationRoot` in `src/server/src/graphql.rs`, with an SDL guard test asserting the field names the client uses
- [ ] T023 [P] [US1] Create `apps/web/src/services/feedbackDraft.ts` — `sessionStorage`, debounced write, cleared on success, never `localStorage` (research.md § R13)
- [ ] T024 [P] [US1] Create `apps/web/src/api/feedback.ts` calling `postGraphQL` from `@/api/graphqlClient`, surfacing `FEEDBACK_RATE_LIMITED` and `FEEDBACK_CONTAINS_SECRET` from `GraphQLRequestError.codes` rather than by matching messages
- [ ] T025 [US1] Create `apps/web/src/components/feedback/FeedbackLauncher.tsx` and `FeedbackDialog.tsx` — the three kinds, fields per kind (FR-003), context assembled from `useLocation()`, `useParams()` and `useAuth()` since there is no `WorldContext` to read
- [ ] T026 [US1] Mount the launcher from `apps/web/src/main.tsx` above the router — FR-001 says "any screen" and `main.tsx` is the only place above every route; this adds the app's first root-level overlay host, and `apps/web/src/components/ui/sonner.tsx`'s `Toaster` (defined but never mounted anywhere today) is mounted here too for FR-004's acknowledgement
- [ ] T027 [P] [US1] Add vitest coverage for `feedbackDraft.ts` in `apps/web/src/services/__tests__/feedbackDraft.test.ts`, including that a cleared storage returns an empty draft rather than throwing
- [ ] T028 [US1] Add `apps/web/e2e/feedback-submit.spec.ts` covering US1's four acceptance scenarios plus FR-006's "a refusal does not discard what the person wrote"

**Checkpoint**: a person can send feedback from anywhere and it is kept — even with nothing configured (FR-030)

---

## Phase 4: User Story 2 — A report that carries its own evidence (Priority: P1)

**Goal**: logs and context arrive without the person collecting anything, a
screenshot is offered and optional, and the review shows exactly what will be
sent.

**Independent Test**: cause a console error, submit an issue, and confirm the
logs and context arrived and that the screenshot was included only because the
person chose it.

### Tests for User Story 2

- [ ] T029 [P] [US2] Add vitest coverage for `apps/web/src/services/feedbackLogBuffer.ts` in `apps/web/src/services/__tests__/feedbackLogBuffer.test.ts`: the entry cap, the byte cap, per-entry truncation, `droppedCount`, that the original console function is still called, and that nothing is written to any storage API
- [ ] T030 [P] [US2] Add vitest coverage for `apps/web/src/services/feedbackRedaction.ts` in `apps/web/src/services/__tests__/feedbackRedaction.test.ts`, one case per rule kind from `config/feedback-redaction.json`, asserting the marker is visible and the value is absent
- [ ] T031 [P] [US2] Add a server test in `src/server/src/feedback/redaction.rs` asserting that a payload containing a secret is **refused** with `FEEDBACK_CONTAINS_SECRET` and that nothing is written — not rewritten, not partially stored

### Implementation for User Story 2

- [ ] T032 [US2] Create `apps/web/src/services/feedbackRedaction.ts` reading `config/feedback-redaction.json`, exporting `redact(line)` returning the text, a count and the kinds found
- [ ] T033 [US2] Create `apps/web/src/services/feedbackLogBuffer.ts` — patch `console.error`/`warn`/`info`, register `window.onerror` and `unhandledrejection`, redact **at push**, bound at 500 entries / 128 KB / 2 KB per entry, expose `snapshot()`; start it from `apps/web/src/main.tsx`
- [ ] T034 [US2] Create `apps/web/src/services/feedbackScreenshot.ts` — `getDisplayMedia`, one frame to an `OffscreenCanvas`, PNG, track stopped in a `finally`; resolve `null` on refusal, dismissal or an absent API so declining costs nothing
- [ ] T035 [US2] Create `apps/web/src/components/feedback/FeedbackReview.tsx` — message, context, the full scrollable log bundle with kept/dropped/redaction counts, the screenshot at inspectable size, a remove control on every part, and FR-014's notice **in this step** rather than after it
- [ ] T036 [US2] Store attachments server-side in `src/server/src/feedback/mod.rs`: screenshot through the existing `transcode::transcode_to_webp` (inheriting `MAX_UPLOAD_BYTES` and `TooLarge`), logs as `text/plain`, both under `feedback/{submission_id}/{attachment_id}` and **never** through `storage::dedupe::object_holding`
- [ ] T037 [US2] Add the authenticated attachment route in `src/server/src/assets_serve/feedback.rs` and register `/feedback-assets/{attachment_id}` in `src/app/src/main.rs` beside the other asset routes, checking that the caller owns the submission or is an administrator
- [ ] T038 [US2] Add the capture flags to `launchOptions.args` in `apps/web/playwright.config.ts` per `contracts/e2e-harness.md` § 1, with a comment saying what they do and do not prove
- [ ] T039 [US2] Add `apps/web/e2e/feedback-evidence.spec.ts` covering US2's four acceptance scenarios and **SC-004**: plant a bearer token and a `?token=` URL in the console, then assert absence in three places — the rendered review, the mutation payload, and the destination — because asserting only the last would pass for a plan that redacted after approval

**Checkpoint**: a report carries its evidence, and the preview is provably the truth

---

## Phase 5: User Story 3 — It becomes an issue somebody can work (Priority: P1)

**Goal**: a configured destination turns submissions into labelled issues
carrying their evidence.

**Independent Test**: configure the app against a repository, submit one of
each kind, and confirm three issues arrive with their attachments intact.

### Tests for User Story 3

- [ ] T040 [P] [US3] Add tests in `src/server/src/feedback/issue_body.rs` for title, labels and body: one kind label per kind, the `delivery_key` comment present, the submitter rendered as a reference and **never** an email, and the inline log block cut at 48 KB with the withheld count stated
- [ ] T041 [P] [US3] Add tests in `src/server/src/feedback/schedule.rs` for `due_now`: a pending submission is due, one with an unfinished attempt is not, backoff is respected, a purged submission is excluded, and ordering is oldest-first

### Implementation for User Story 3

- [ ] T042 [US3] Make `open_issue` check `response.status()` in `src/server/src/repo_host.rs` and classify the outcome (2xx / 4xx / 5xx / transport), add a scope-carrying sibling, and accept `labels` in the create POST
- [ ] T043 [US3] Add `put_file(scope, installation, owner, name, branch, path, bytes)` to `src/server/src/repo_host.rs` using the Contents API, creating the `feedback-attachments` branch from the default head on first use — no new permission needed, `github::REQUESTED_PERMISSIONS` already asks for `contents:write`
- [ ] T044 [US3] Add `repo_host` support for `GITHUB_API_BASE` / `GITHUB_WEB_BASE` via the existing, currently unused `GitHubApp::with_bases` in `crates/thunderforge-repo-host/src/github.rs` — configuration, not a test branch
- [ ] T045 [US3] Implement `src/server/src/feedback/issue_body.rs` per `contracts/delivery.md` § 3, including the public-embed / private-link choice driven by the destination's observed visibility
- [ ] T046 [US3] Implement `src/server/src/feedback/deliver.rs` — one attempt: resolve credentials, upload attachments, create the issue, finish the attempt row, map host errors to `FeedbackFailureReason` **and never carry a host body through**
- [ ] T047 [US3] Implement `src/server/src/feedback/schedule.rs` with `TICK_SECONDS`, `BACKOFF_SECONDS`, `next_attempt_after` and `due_now`, modelled on `src/server/src/lore_sync/schedule.rs`, and spawn it from `src/app/src/main.rs` beside `spawn_lore_sync_task` — unconditionally, for the reason that call site already gives
- [ ] T048 [US3] Refresh and store the destination's visibility (`repo_host::repository_is_public`) on the pass, with `visibility_checked_at`, and surface `feedbackDestinationNotice` in `src/server/src/graphql/queries/feedback.rs`
- [ ] T049 [P] [US3] Create `apps/web/e2e/fixtures/githubStub.ts` per `contracts/e2e-harness.md` § 2, and give each shard a port in `scripts/e2e-parallel.mjs` exactly as backends and vite servers already get one
- [ ] T050 [US3] Seed a `feedback_destination` row pointing at the stub in `src/server/seeds/e2e_demo.sql`, and set `FEEDBACK_GITHUB_APP_*` from `crates/thunderforge-repo-host/tests/fixtures/throwaway-test-app-key.pem` in the harness
- [ ] T051 [US3] Add `apps/web/e2e/feedback-delivery.spec.ts` covering US3's four acceptance scenarios: three kinds arrive, attachments are present, each is distinguishable by label, and an unconfigured instance reports which variables are missing without printing one

**Checkpoint**: feedback becomes work somebody can pick up

---

## Phase 6: User Story 6 — Feedback survives a broken destination (Priority: P2)

**Goal**: an unreachable or unconfigured destination never loses a submission,
and recovery delivers each item exactly once.

**Independent Test**: break the destination, submit, confirm the person is told
it was received rather than shown an error, then restore it and confirm the
item arrives once.

### Tests for User Story 6

- [ ] T052 [P] [US6] Add a server test in `src/server/src/feedback/deliver.rs` asserting that an ambiguous failure (transport, or 5xx, or no status) causes the **next** attempt to search for the `delivery_key` before creating, and that a 4xx does not
- [ ] T053 [P] [US6] Add a server test asserting `outcome = 'adopted'` when the search finds an existing issue, and that the submission moves to `delivered` with that issue's URL
- [ ] T054 [P] [US6] Add a server test in `src/server/src/graphql/queries/feedback.rs` asserting no `FeedbackFailureReason` and no operator-visible field can carry a credential, a fragment or a host response body

### Implementation for User Story 6

- [ ] T055 [US6] Implement the search-before-create adoption path in `src/server/src/feedback/deliver.rs` per `contracts/delivery.md` § 4, using `GET /search/issues` against the `delivery_key`
- [ ] T056 [US6] Implement `undeliveredFeedback`, `abandonFeedbackDelivery` and `resumeFeedbackDelivery` in `src/server/src/graphql/queries/feedback.rs` and `src/server/src/graphql/mutations_feedback.rs`, administrator-only, per `contracts/delivery.md` § 5
- [ ] T057 [US6] Add the undelivered-feedback view to the admin surface in `apps/web/src/pages/admin/`, showing attempt count, next attempt, reason and expiry
- [ ] T058 [US6] Add `/_control/fail-next` handling to `apps/web/e2e/fixtures/githubStub.ts` — record the issue, then drop the connection, which is the only way to produce the failure FR-019 is actually about
- [ ] T059 [US6] Extend `apps/web/e2e/feedback-delivery.spec.ts` with US6's three acceptance scenarios, including the ambiguous-failure case asserting exactly one issue exists and the attempt row says `adopted`

**Checkpoint**: nothing a person took the trouble to write is lost

---

## Phase 7: User Story 4 — One app for every subsystem, said out loud (Priority: P2)

**Goal**: a global application serves every subsystem, a specific one wins for
its own, and the operator is told what they are choosing while choosing it.

**Independent Test**: set only the global variables and confirm both feedback
and lore sync work; then set a feedback-specific value and confirm it wins for
feedback while the global still serves everything else.

- [ ] T060 [P] [US4] Add a server test asserting lore sync resolves through `GLOBAL_GITHUB_APP_*` when no `SYNC_GITHUB_APP_*` is set, in `src/server/src/repo_host_tests.rs` — the half of FR-024 that is about the *other* subsystem
- [ ] T061 [US4] Surface the resolution report — per field, which prefix, for every scope — on the administrator configuration query in `src/server/src/graphql/queries/admin.rs`, rendering `ScopedApp::sources` verbatim
- [ ] T062 [US4] Render the resolution report and the subsystem list in `apps/web/src/pages/admin/`, so that wherever global credentials appear the operator is told **which subsystems** the application will act for (FR-026, SC-007)
- [ ] T063 [US4] Add `apps/web/e2e/feedback-credentials.spec.ts` covering US4's four acceptance scenarios, including the partial-specification case where the report shows a mixed origin
- [ ] T064 [US4] Add a note to `specs/040-instance-setup/spec.md`'s GitHub section pointing at `specs/037-in-app-feedback/contracts/github-app-scopes.md` as the resolver this feature builds and 040's surface consumes, so the two cannot drift apart unnoticed

**Checkpoint**: one application, several subsystems, and nobody surprised by the blast radius

---

## Phase 8: User Story 5 — Knowing what happened to what you sent (Priority: P2)

**Goal**: a submitter sees the state of their own submissions and nobody
else's.

**Independent Test**: submit an item, close the corresponding issue, and
confirm the submitter sees the change.

- [ ] T065 [P] [US5] Add a server test in `src/server/src/graphql/queries/feedback.rs` asserting `mySubmissions` returns only the caller's rows and that there is no argument by which another account's could be reached
- [ ] T066 [US5] Implement `mySubmissions` in `src/server/src/graphql/queries/feedback.rs` per `contracts/feedback.md`
- [ ] T067 [US5] Add the state refresh to the delivery pass in `src/server/src/feedback/schedule.rs` — bounded per tick, oldest-refreshed-first, `open`/`closed` only, recording `issue_state_checked_at`
- [ ] T068 [US5] Create `apps/web/src/components/feedback/MySubmissions.tsx` and route it under the account pages, showing what was sent, its state, when that was observed, and when the instance's copies expire
- [ ] T069 [US5] Add `apps/web/e2e/feedback-status.spec.ts` covering US5's three acceptance scenarios, including that a second account sees none of the first's

**Checkpoint**: the loop closes, which is what makes somebody send a second one

---

## Phase 9: Retention, and the promise about it

**Purpose**: FR-016 is a privacy commitment, not a storage optimisation, and it
is the only place this feature deletes anything.

- [ ] T070 [P] Add server tests in `src/server/src/feedback/mod.rs` asserting the sweep deletes only expired feedback objects, sets `attachments_purged_at`, leaves the rows, and leaves every canvas, lore, actor and scene object untouched
- [ ] T071 Implement the retention sweep on the delivery task's tick in `src/server/src/feedback/schedule.rs` — one schedule, not a second one
- [ ] T072 Show the expiry and the purged state in `apps/web/src/components/feedback/MySubmissions.tsx` and in the pre-submission notice, stating plainly that the destination's copy is unaffected and cannot be recalled by the instance
- [ ] T073 Add the `ATTACHMENTS_EXPIRED` failure path — a submission whose evidence expired before delivery succeeded fails permanently, says so, and does not deliver a report without the thing it was for

**Checkpoint**: the retention that was promised is the retention that happens

---

## Phase 10: Polish & Cross-Cutting Concerns

- [ ] T074 [P] Document the feedback path in `docs/` — what is captured, what is redacted and when, where attachments live, how long they live, and what a maintainer sees
- [ ] T075 [P] Update `MVP.md` with the feedback path as the way MVP learns what is wrong with it, and note that it is MVP-adjacent rather than part of the play loop
- [ ] T076 Confirm no product file changed to make the harness work (`contracts/e2e-harness.md` § "What must NOT appear in product code"); if any did, the stub is wrong
- [ ] T077 Make the five guards fail on purpose per `quickstart.md` § "Making the guards fail on purpose" and record in the commit that each was seen to bite
- [ ] T078 Run the quickstart scenarios A–I by hand against `make dev`, including the two only a person can judge — the screenshot picker and the pre-submission notice — and note anything the suite does not catch
- [ ] T079 Run `cargo test --workspace -j 4`, `make lint` and `pnpm --filter @thunderforge/web test`
- [ ] T080 Run the full suite via `node scripts/e2e-parallel.mjs --shards=2` with `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1` and `--workers=1` for any external-stack run, and record the figures in the commit body
- [ ] T081 Run `pnpm verify` and fix what it reports **in the code this feature added** — keep it to that; wide lint passes get their own commit

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies. The ADRs may be written while Phase 2 proceeds, but must land in the same change set (Principle IV)
- **Foundational (Phase 2)**: blocks every story. T008→T009 are ordered; T010→T011→T012 are ordered; T013/T014, T015, T016 are independent of all of it
- **US1 (Phase 3)**: needs Phase 2. Delivers a working, honest feature on its own — with no destination configured, submissions are collected and kept (FR-030)
- **US2 (Phase 4)**: needs US1's mutation to submit into. Independent of every delivery task
- **US3 (Phase 5)**: needs US1 and, for attachments to arrive, US2. T042–T044 (`repo_host.rs`) are ordered among themselves and touch one file
- **US6 (Phase 6)**: needs US3's delivery path to break. Its stub control (T058) needs T049
- **US4 (Phase 7)**: needs only Phase 2's T010–T012. **Can run beside any other phase** — it shares no file with US1, US2 or US5
- **US5 (Phase 8)**: needs US3, because there is no issue state to read back before delivery exists
- **Retention (Phase 9)**: needs T013 and Phase 5's schedule. Independent of US4, US5 and US6
- **Polish (Phase 10)**: after the stories being shipped

### Within Each User Story

- Server tests accompany the surface they test, in the module, as this codebase does
- Domain module before the GraphQL surface that calls it; GraphQL surface before the web client that calls that
- E2E last within a story, because it needs both halves

### Parallel Opportunities

- T002–T006 — four ADRs and an index, five files (T001 is a check, and may find there is nothing to write)
- T013/T014, T015, T016 — storage, versions and rules, no shared file
- T029, T030, T031 — three test files across two languages
- T040/T041, T052/T053/T054 — test tasks within a story
- **US4 (Phase 7) can run beside any other phase.** If two people are on this, that is the split
- T023/T024 and T027 are independent of each other and of T025

## Parallel Example: Foundational

```bash
# After T008 and T009 (migration then schema), these are independent:
Task: "Add rustfs::delete_object in src/server/src/storage/rustfs.rs"
Task: "Add the application version surface in graphql/queries/healthcheck.rs and apps/web/vite.config.mts"
Task: "Create config/feedback-redaction.json"
```

## Parallel Example: Setup

```bash
Task: "Write ADR-084 in docs/adrs/20260907-084-feedback_submission_is_the_record.md"
Task: "Write ADR-085 in docs/adrs/20260907-085-feedback_redaction_before_review.md"
Task: "Write ADR-086 in docs/adrs/20260907-086-feedback_attachments_are_deletable.md"
Task: "Write ADR-087 in docs/adrs/20260907-087-feedback_delivery_dmca_determination.md"
```

---

## Implementation Strategy

### MVP (US1 + US2 + US3)

Phases 1, 2, 3, 4 and 5. All three are P1 and the reason is worth stating,
because the tempting smaller slice is wrong.

**US1 alone is not the MVP.** A feedback control that collects messages with no
evidence produces exactly what the product already gets today — "it broke",
days later, without the screen, the world or the error. spec.md's Context says
so in its first paragraph, and shipping US1 alone would be shipping the problem
with a button on it.

**US1 + US2 without US3 is defensible and is not recommended.** It is honest —
FR-030 makes it work, submissions are kept, nothing is lost — but nobody reads
them, and the playtest that this feature exists to serve would end with a table
of rows and no issues. Take it only if the destination cannot be configured in
time.

1. Phase 1 (the ADRs) and Phase 2 (schema, scoped credentials, deletable
   storage, versions, redaction rules)
2. Phase 3 — the control, the mutation, the rate limit, the draft
3. Phase 4 — the buffer, the redaction, the screenshot, the review. **This is
   where FR-012 is either kept or quietly broken**, so quickstart Scenario C is
   run by hand before the phase is called done
4. Phase 5 — the delivery pass and the issue
5. **Stop and validate**: quickstart Scenarios A–D by hand, plus the suite
6. Demonstrable: an error happens, somebody reports it in thirty seconds, and
   a maintainer has a reproducible issue the same minute

### Incremental delivery after MVP

1. **US6** — the destination breaks and nothing is lost. Ship close behind the
   MVP: it is the failure mode with the worst consequence and the one most
   likely to occur during a playtest, when the instance is new and the
   credentials are fresh
2. **Phase 9 (retention)** — with or immediately after US6. It is a privacy
   commitment already made in the pre-submission notice, and a notice that
   promises an expiry nothing performs is worse than no notice
3. **US4** — the credential scopes. Genuinely parallel; hand it to whoever is
   free, from Phase 2 onward
4. **US5** — knowing what happened. Last, as spec.md's own checklist says: "the
   first submission is valuable even if nothing comes back", and it is the one
   to cut if the feature has to ship smaller

### Note on the order of US2 and US3

US2 before US3, deliberately. If delivery is built first, the natural next step
is to make the server assemble and redact what it sends — which is the plan
FR-012 exists to reject. Building the client-side capture, redaction and review
first means the server's job is only ever to validate what it was handed, and
the wrong architecture never gets a chance to look convenient.

---

## Notes

- [P] tasks touch different files and have no incomplete dependency
- Every story is independently testable; the checkpoints say what "done" looks
  like without the later phases
- Verify per target, per Principle V: native `cargo` for the server, `tsc`
  and `vitest` for the web, e2e for the story. **No wasm target is involved** —
  this feature touches no engine code, so a green host build is a complete
  check rather than half of one
- Commit per task or coherent group; the ADRs land with the code, not
  after
