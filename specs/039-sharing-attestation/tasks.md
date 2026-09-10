---

description: "Task list for 039-sharing-attestation"
---

# Tasks: The Sharing Attestation

**Input**: Design documents from `/specs/039-sharing-attestation/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/)

**Tests**: Included, and not optional. The whole feature is a claim about
evidence, and a control nobody tried to defeat is a decoration. Every user
story carries at least one test whose job is to fail when the guard is removed —
see quickstart.md § "Making the guards fail on purpose". Rust unit tests
accompany each server surface in the same file, as this codebase already does.

**Not legal advice.** Three tasks below change legal prose. They are assigned to
a reviewer, not to a developer, and `legal/README.md` already marks this
directory as needing review before launch.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: US1–US8 per spec.md
- Every task names the file it touches

## Path Conventions

Web application, per plan.md § Structure Decision: Rust server under
`src/server/src/`, binary wiring in `src/app/src/main.rs`, Diesel migrations in
`src/server/migrations/`, reviewable prose in `legal/`, React shell in
`apps/web/src/`, Playwright suite in `apps/web/e2e/`.

---

## Phase 1: Setup — the decisions this feature may not make silently

**Purpose**: Constitution Principle IV, and one determination the constitution's
DMCA guardrail requires **on record before implementation begins**. T004 is a
hard gate, not a documentation chore: if it is declined, Phase 6 does not exist
and US6's requirements come out of the spec.

- [X] T001 [P] Write ADR-076 (the sharing attestation record) in `docs/adrs/20260909-076-the_sharing_attestation_record.md` — **renumbered**: the plan reserved 080–083 and this line said 093, and both blocks were taken by specs 040 and 041 while this spec waited. 076–080 were free, so 039 takes 076–079 — terms compiled into the server with `include_str!` on the precedent at `src/server/src/admin.rs:37`, versioned by content hash, archived in `terms_versions` at startup, recorded with no foreign key to `users` on the precedent `content_moderation_actions` set
- [X] T002 [P] Write ADR-077 (account standing and the termination window) in `docs/adrs/20260909-077-account_standing_and_the_termination_window.md` — standing derived from the existing counting rather than stored, the window as the only row, the sweep as the fifth `spawn_*_task`, and `MODERATION_TERMINATION_REQUIRES_HUMAN` defaulting to true
- [X] T003 [P] Write ADR-078 (the operator acknowledgement) in `docs/adrs/20260909-078-the_operator_acknowledgement.md` — an instance attesting on the same record as a person, why `instance_identity` is not reused, and the boundary with spec 040
- [~] T004 **GATE** ADR-079 written as **PROPOSED** in `docs/adrs/20260909-079-adoption_provenance_and_the_reach_of_a_takedown.md`, carrying the guardrail determination, the three specific costs, and both outcomes spelled out. **Still needs the accountable owner's acceptance as ADR-069 has** — it is a liability decision that amends an accepted ADR, so it is not the implementer's to sign. Phases 1–5 do not wait on it; Phase 6 does. It reverses spec 026 FR-012's "no referential link back to the source" (`src/server/src/collections/copy.rs:10`) and amends ADR-069. If declined: strike FR-022–FR-023d and SC-006/SC-008 from `specs/039-sharing-attestation/spec.md`, delete Phase 6 from this file, and record the decision in the ADR as rejected
- [X] T005 The seed sets a notice contact. `src/server/seeds/e2e_demo.sql` — which `make dev` applies and `global-setup.ts` re-applies per e2e run — now writes three `instance_settings` rows:

  | key | value |
  |---|---|
  | `notice.contact_name` | `ThunderForge E2E Notice Contact` |
  | `notice.contact_email` | `notices@realdomain.org` |
  | `notice.contact_postal_address` | `1 Test Street / Testville, TS 00000 / Testland` |

  All three are `Backing::Row` and `RequiredFor(Capability::PublishBeyondWorld)`, so with FR-053 enforced an instance without them refuses every share path. Written straight into the table rather than through `settings::changes::write_setting`: a seed has no actor, and `instance_setting_changes` records *who* changed a setting — a question a seed cannot answer honestly. **Deliberately not a reserved-TLD address.** Spec 040 already implements this gate as its FR-026 (`graphql/publishing_gate.rs`), and `settings::validate::placeholder_problem` treats `.invalid`/`example.*`/`.test` as *not configured* — a `.invalid` address would have left the gate shut for a reason no test intended, with the row sitting there looking correct. Caught by reading 040's own fixture, which uses `realdomain.org` for exactly this. Applied against the local database and read back.

---

## Phase 2: The prose — assigned to a reviewer, not a developer

**Purpose**: three legal documents change, and research.md § R12 is where the
spec's claim to "write no policy" stops being exactly true. These block Phase 3
because the version identity is the hash of the final words: revising the text
after attestations exist is a version transition (which is fine, and is US4),
but revising it *during* Phase 3 is churn.

- [X] T006 Renamed to `legal/sharing-terms.md` and reworded for four publishing paths, still two short sections. Its comment now also records that the version identity is the hash of its own body, so a reviewer knows a typo fix is free and a meaning change is a version transition. **Draft — still needs a lawyer**, as `legal/README.md` says of everything here
- [~] T007 **Blocked on T004.** In `legal/sharing-terms.md`, revise the second section: "A copy someone takes is theirs, and **cannot be recalled**" becomes untrue for copies disabled by a takedown the day FR-023 ships. **Reviewer decision** — the product must not tell people something it no longer does
- [X] T008 Write `legal/operator-responsibilities.md` — net-new prose saying plainly that the legal obligations of everything in an instance belong to whoever operates it: the content in it, notices filed against it, the law where it runs; and that registering a designated agent, where the jurisdiction requires one, is the operator's own act and is not performed by this software (FR-042, FR-055)
- [X] T009 [P] `legal/README.md`: both documents in the status table, and a new section on version identity — editing a sentence mints a version and every earlier attestation keeps resolving to the earlier words; editing the leading comment does not
- [X] T010 [P] `legalDocuments.test.ts` updated for the rename and for the new document; the `[OPERATOR]`-markers-survive-rendering assertions are untouched. 564 web tests pass

---

## Phase 3: Foundational (Blocking Prerequisites)

**Purpose**: the archive, the record and the gate. Nothing here is user-visible
on its own, and every story below reads it.

**⚠️ CRITICAL**: No user story work begins until this phase is complete

- [X] T011 Create `src/server/src/legal/mod.rs` — `include_str!("../../../../legal/sharing-terms.md")` and the operator statement, normalisation (strip the leading HTML comment, trim) matching `apps/web/src/legal/legalDocuments.ts`'s `sectionsOf`, `sha256` version identity, and section splitting; with unit tests asserting a prose edit changes the identity and a comment edit does not
- [X] T012 Create the Diesel migration `src/server/migrations/2026-09-09-130000-0000_terms_versions_and_attestations/up.sql` and `down.sql` (dated the day it was written) for `terms_versions` and `attestations` per data-model.md §§ 1–2, following the comment style of `2026-09-05-000000-0000_create_world_collections/up.sql` and stating why `attestations` has no FK to `users`. Four constraints beyond the data model, each driven against a scratch database and seen to refuse: a share attestation must name what it published, an operator acknowledgement is unique per instance, a version must be in the archive, and an operator row may carry no world or share
- [X] T013 Migration run and `schema.rs` regenerated (`terms_versions`, `attestations`). `TermsVersion`, `Attestation` and `NewAttestation` added to `models.rs`. **No `NewTermsVersion`**: `TermsVersion` is itself `Insertable` and the only writer is the startup archive, so a second shape would be a second way to write a table nothing else may write. Neither struct has an update shape and `NewAttestation` carries no `attested_at` — a caller that could supply the moment is a caller that could write its own evidence (FR-014)
- [ ] T014 Add `ensure_terms_versions_recorded(state)` to `src/server/src/legal/mod.rs` and call it from `src/app/src/main.rs` beside `ensure_admin_bootstrap_code` and `ensure_instance_identity`, with a test asserting a version is archived before any attestation naming it can be accepted (FR-016)
- [ ] T015 Create `src/server/src/attestation.rs` — record, retrieve by publishable, retrieve by subject, and `redact_for_deleted_account` (nulls `subject_username`, keeps the rest) per data-model.md § 2; unit tests for each
- [ ] T016 Create `src/server/src/publishing.rs` — `PublishableKind`, `AttestationInput`, `require_notice_contact` (reading `THUNDERFORGE_NOTICE_CONTACT` until spec 040 owns it) and `require_attestation` per `contracts/publishing-gate.md`, with unit tests for each of the four refusals and for the order they are checked in
- [ ] T017 [P] Create the migration `src/server/migrations/2026-09-07-134000-0000_account_notices/up.sql` and `down.sql` for `account_notices` per data-model.md § 6, and `src/server/src/notices.rs` to write and read them — the durable half of "the person is told"; delivery is spec 040's and the module doc says so
- [ ] T018 Add `sharingTerms`, `operatorStatement` and `legalDocumentVersion` to `src/server/src/graphql/queries/legal.rs` per `contracts/attestation.md`, register on the query root in `src/server/src/graphql.rs`, and add an SDL guard test for the names the client uses

**Checkpoint**: the terms have an identity, the identity has an archive, the archive has a record, and one function can refuse a publish

---

## Phase 4: User Story 1 + 3 — asked on every path, and the server is the one asking (Priority: P1) 🎯 MVP

**Goal**: all four publishing paths require an attestation, and the requirement
lives in the server rather than in a page.

**Independent Test**: publish by each of the four paths and confirm each asks;
then call each mutation directly with no attestation and confirm it is refused
and no link exists.

US1 and US3 are one phase because they are one change seen from two sides —
shipping the dialogs without the gate is the defect this feature exists to
remove, and the checklist says so.

### Tests for User Stories 1 and 3

- [ ] T019 [P] [US3] Add the SDL guard test in `src/server/src/graphql/mod.rs` asserting **every** mutation matching `create*ShareLink` declares a non-null `attestation: AttestationInput` — this is the mechanism by which FR-002 covers a content type nobody has written yet
- [ ] T020 [P] [US3] Add impl-level tests to `src/server/src/graphql/mutations_collection_shares.rs`: an unknown version id is refused, no share row is created, and the message names no valid version identity (FR-012, FR-013)
- [ ] T021 [P] [US1] Add impl-level tests to `src/server/src/graphql/mutations_actor_shares.rs`, `mutations_item_shares.rs` and `mutations_ability_shares.rs` mirroring T020 — three files, one shape

### Implementation for User Stories 1 and 3

- [ ] T022 [US3] Add `attestation: AttestationInput!` to `create_collection_share_link_impl` in `src/server/src/graphql/mutations_collection_shares.rs`, call `publishing::require_attestation` before the code is minted, and write the attestation **inside the same transaction** as the share row
- [ ] T023 [P] [US1] Same change to `create_actor_share_link_impl` in `src/server/src/graphql/mutations_actor_shares.rs`
- [ ] T024 [P] [US1] Same change to `create_item_share_link_impl` in `src/server/src/graphql/mutations_item_shares.rs`
- [ ] T025 [P] [US1] Same change to `create_ability_share_link_impl` in `src/server/src/graphql/mutations_ability_shares.rs`
- [ ] T026 [US1] Create `apps/web/src/components/legal/AttestationDialog.tsx` — one dialog, rendering `sharingTerms` from the server (not the Vite glob), returning the version identity to its caller, and short enough that sharing five things in a row stays a task somebody finishes (SC-007)
- [ ] T027 [US1] Replace the inline terms block in `apps/web/src/pages/world-collections/WorldCollectionsPage.tsx` with the shared dialog, keeping the `share-terms` test id so existing coverage still points at something
- [ ] T028 [P] [US1] Add the dialog to `apps/web/src/pages/world/actor/ActorDetailPage.tsx` — the first time this path has asked anything
- [ ] T029 [P] [US1] Add the dialog to `apps/web/src/pages/world/item/ItemDetailPage.tsx`
- [ ] T030 [P] [US1] Add the dialog to `apps/web/src/pages/world/ability/AbilityDetailPage.tsx`
- [ ] T031 [P] [US1] Add the `attestation` argument to `apps/web/src/api/collections.ts`, `actorShares.ts`, `itemShares.ts` and `abilityShares.ts`
- [ ] T032 [US1] Add `apps/web/e2e/sharing-attestation.spec.ts` covering US1's five acceptance scenarios and US3's three, including a direct API call with no attestation and one with a fabricated version id

**Checkpoint**: the position is enforceable — four paths ask, and the server, not the page, is what requires it

---

## Phase 5: User Story 2 + 4 — the record, and what happens when the words change (Priority: P1 / P2)

**Goal**: an attestation answers a question asked years later, and revising the
terms does not rewrite what anybody agreed to.

**Independent Test**: share, look the attestation up, revise the terms, restart,
and confirm the old attestation still resolves to the old words while new shares
use the new ones.

### Tests for User Stories 2 and 4

- [ ] T033 [P] [US2] Add a test in `src/server/src/attestation.rs` asserting an attestation survives its share being revoked and deleted (FR-007)
- [ ] T034 [P] [US4] Add a test asserting `Attestation.terms` resolves through `terms_versions` and **not** through the compiled-in constant — remove the join and it must fail (FR-008, quickstart § "Making the guards fail on purpose")
- [ ] T035 [P] [US2] Add a test asserting `redact_for_deleted_account` nulls `subject_username` and changes nothing else (FR-010, FR-037)

### Implementation for User Stories 2 and 4

- [ ] T036 [US2] Add `attestationsFor` (admin) and `myAttestations` to `src/server/src/graphql/queries/legal.rs` per `contracts/attestation.md`, and register them with an SDL guard test
- [ ] T037 [US2] Surface the attestation on the admin moderation view in `apps/web/src/pages/admin/ModerationReviewPage.tsx` — SC-003 is "in under a minute and without a developer", which means a link from a case to the agreement, not a query somebody writes
- [ ] T038 [US2] Call `attestation::redact_for_deleted_account` from `delete_user_data_sync` in `src/server/src/users/mod.rs`, and add a test that the row survives the account
- [ ] T039 [US4] Extend `apps/web/e2e/sharing-attestation.spec.ts` with the revision scenario: record, revise `legal/sharing-terms.md`, restart, confirm both versions resolve to their own words and no earlier attestation moved

**Checkpoint**: an agreement is evidence rather than a moment

---

## Phase 6: User Story 6 — a takedown reaches everywhere the work went (Priority: P2)

**⚠️ Blocked on T004.** If ADR-094 is declined, this phase does not exist.

**Goal**: content taken down stops being served by every path the instance
controls, and adopted copies are disabled rather than deleted.

**Independent Test**: publish, have it adopted, take it down, and confirm every
path the instance controls stops serving it while the adopter's own work is
untouched; then withdraw and confirm both come back.

### Tests for User Story 6

- [ ] T040 [P] [US6] Add a test in `src/server/src/moderation/reach.rs` asserting a child case carries `account_id = NULL`, so an adopter accrues **no strike** for somebody else's upload (FR-023b) — give it the adopter's id and the test must fail
- [ ] T041 [P] [US6] Add a test asserting the walk is transitive: a copy of a copy is disabled (data-model.md § 3)
- [ ] T042 [P] [US6] Add an SDL test asserting **no** field, query, subscription or route reads `content_adoptions` — the assertion that keeps ADR-069's determination true
- [ ] T043 [P] [US6] Add a failing-first test in `src/server/src/lore_sync/plan.rs` proving taken-down lore reaches the Git mirror today, because `filter_visible` is called with `"lore_entry"` while every moderation row carries `"world_lore_entry"`

### Implementation for User Story 6

- [ ] T044 [US6] Create the migration `src/server/migrations/2026-09-07-131000-0000_content_adoptions/up.sql` and `down.sql` per data-model.md § 3, with a comment stating the no-reader rule and recording that `"scene"` maps to no moderation entity so scenes record no adoption
- [ ] T045 [US6] Create the migration `src/server/migrations/2026-09-07-132000-0000_moderation_parent_case/up.sql` and `down.sql` adding nullable `parent_case_id` to `content_moderation_actions`; run `make schema` and extend the models
- [ ] T046 [US6] Record an adoption per copied entity in `src/server/src/collections/copy.rs`, inside the existing transaction, and update the module header — its current text says the copies carry no source id and that is about to stop being true
- [ ] T047 [P] [US6] Record adoptions in the three singleton copy impls in `mutations_actor_shares.rs`, `mutations_item_shares.rs` and `mutations_ability_shares.rs`
- [ ] T048 [US6] Create `src/server/src/moderation/reach.rs` with `fan_out_disable` and `fan_out_forward` per `contracts/takedown-reach.md`, and add `content_disabled_as_copy` to the action types and to `is_disabled_status` in `src/server/src/moderation/mod.rs`
- [ ] T049 [US6] Call `fan_out_disable` from `submit_takedown_notice_impl` and `fan_out_forward` from `submit_counter_notice_impl` in `src/server/src/graphql/mutations_moderation.rs`, and fan `resolveModerationCase` to child cases — the source coming back while its copies stay dark is the failure FR-023d exists for
- [ ] T050 [US6] Write `account_notices` rows on fan-out: `adopted_copy_disabled` to each adopter, `share_taken_down` to the sharer (FR-023b, FR-024). The adopter's wording must make clear they are not accused of anything — a copy-writing requirement with a test
- [ ] T051 [US6] Write `adopted_copy_restored` when a lazy restoration materialises in `src/server/src/moderation/mod.rs`'s `effective_status` path
- [ ] T052 [US6] Fix the entity-type string in `src/server/src/lore_sync/plan.rs:109` and `src/server/src/lore_sync/incoming.rs:242` (`"lore_entry"` → `"world_lore_entry"`), closing T043
- [ ] T053 [US6] Refuse a share link whose target is disabled, on the existing one-message rule `UNAVAILABLE` already uses, in all four `mutations_*_shares.rs` read paths (FR-022)
- [ ] T054 [US6] Add `apps/web/e2e/takedown-reach.spec.ts` covering quickstart Scenario F end to end, including the adopter's untouched neighbouring work (FR-023c) and restoration without asking (FR-023d)

**Checkpoint**: a takedown takes something down, and does not take an adopter's world with it

---

## Phase 7: User Story 5 — repeat infringement costs the ability to publish (Priority: P2)

**Goal**: the counting that already exists acquires a consequence, on a ladder,
before the final rung.

**Independent Test**: drive an account past the suspension rung and confirm
publishing is refused while playing, editing and reading are not.

### Tests for User Story 5

- [ ] T055 [P] [US5] Add tests in `src/server/src/moderation/standing.rs` for the ladder at 0, 1, 2 and 3 strikes, and for a strike ageing past the lookback restoring `mayPublish` with nothing else happening (FR-035)
- [ ] T056 [P] [US5] Add a test asserting a suspended account retains every content permission it had (FR-019, FR-038)

### Implementation for User Story 5

- [ ] T057 [US5] Extract `strike_count(conn, account_id)` from `repeat_infringer_flags_impl` in `src/server/src/graphql/queries/moderation.rs` into `src/server/src/moderation/mod.rs`, and have the query call it — one definition of a strike, so FR-027 is true by construction
- [ ] T058 [US5] Create `src/server/src/moderation/standing.rs` with the four ladder settings (`MODERATION_STRIKE_WARN_AT`, `MODERATION_STRIKE_SUSPEND_PUBLISHING_AT`, `MODERATION_TERMINATION_WINDOW_DAYS`, `MODERATION_TERMINATION_REQUIRES_HUMAN`) in the same parse-or-default shape as the three that exist, and `standing_of` per data-model.md § 5
- [ ] T059 [US5] Consult `standing_of` from `publishing::require_attestation` in `src/server/src/publishing.rs` so all four paths are covered by one call (FR-018, FR-021)
- [ ] T060 [US5] Write an `account_notices` row when a strike is recorded, in `src/server/src/graphql/mutations_moderation.rs` — what it was, that it counts, how many remain, when it ages out (FR-028)
- [ ] T061 [US5] Add `myStanding` and `accountStanding` to `src/server/src/graphql/queries/standing.rs` per `contracts/standing-and-termination.md`, register them, and add an SDL guard test
- [ ] T062 [US5] Create `apps/web/src/pages/user/StandingPage.tsx` — strikes, what each was, when each ages out, and the notices (FR-029)
- [ ] T063 [US5] Add `apps/web/e2e/account-standing.spec.ts` covering US5's three acceptance scenarios (quickstart Scenario E)

**Checkpoint**: "they open themselves up to lose the ability to share" is a behaviour

---

## Phase 8: User Story 7 — three strikes, the window, and both remedies (Priority: P2)

**Goal**: the third strike disables the account, opens a thirty-day window in
which download and appeal are both available, and ends in a real deletion or a
restoration — never in a deletion caused by the instance being slow.

**Independent Test**: drive an account to three strikes and confirm it was warned
twice, is disabled on the third, can download and appeal, and is deleted at day
thirty only if neither remedy was used and the operator has asked for that.

### Tests for User Story 7

- [ ] T064 [P] [US7] Add tests in `src/server/src/moderation/standing.rs`: an open appeal blocks execution past the due date (FR-034); a rejected appeal resumes from the date already passed, buying no second window; an upheld appeal closes the termination and removes the strike (FR-033)
- [ ] T065 [P] [US7] Add a test asserting downloading writes nothing to the termination and appealing writes nothing to the download — both orders (FR-032)
- [ ] T066 [P] [US7] Add a test asserting opening a termination against the **only** administrator does not disable them (FR-039)
- [ ] T067 [P] [US7] Add a test in `src/server/src/graphql/helpers.rs` asserting exactly the five allowlisted surfaces accept a disabled account and a sixth does not — add one and it must fail
- [ ] T068 [P] [US7] Add a test in `src/server/src/users/mod.rs` asserting deletion leaves a world that has live members other than the account (FR-038)

### Implementation for User Story 7

- [ ] T069 [US7] Create the migration `src/server/migrations/2026-09-07-133000-0000_account_terminations/up.sql` and `down.sql` per data-model.md § 5, including the partial unique index on `account_id WHERE closed_at IS NULL`; run `make schema` and extend the models
- [ ] T070 [US7] Implement `open_termination`, `close_termination` and `run_due_standing_work` in `src/server/src/moderation/standing.rs`, snapshotting `requires_human` and `deletion_due_at` at open so a later settings change does not move a running window
- [ ] T071 [US7] Add `spawn_standing_task` to `src/server/src/moderation/standing.rs` and call it from `src/app/src/main.rs` beside the four tasks already there, at a 300s tick — the fifth instance of the shape `src/server/src/lore_sync/schedule.rs:108` documents as house style
- [ ] T072 [US7] Also call `run_due_standing_work` from the admin moderation query and from the sign-in of an account with an open termination, so the sweep is a backstop and not the mechanism (and so a test need not wait five minutes)
- [ ] T073 [US7] Add `standing` to `AuthenticatedUser` in `src/server/src/auth_middleware.rs` — resolve the session as now, do **not** 401, since a blanket refusal there destroys the export and with it the remedy
- [ ] T074 [US7] Add `authenticated_user_even_if_disabled` to `src/server/src/graphql/helpers.rs` and make `authenticated_user` refuse a disabled account; opt in exactly the five surfaces in `contracts/standing-and-termination.md`
- [ ] T075 [US7] Implement `fileAppeal` and `resolveAppeal` in `src/server/src/graphql/mutations_standing.rs` per the contract, writing `appeal_resolved` notices
- [ ] T076 [US7] Fill in `export_user_data_payload` in `src/server/src/users/mod.rs` — actors, items, abilities, lore and collections. The placeholders have been empty since May 2026 and `apps/web/e2e/user-data-export.spec.ts` pins that as a known gap citing this spec. A download missing a person's characters is not the remedy FR-032 promises
- [ ] T077 [US7] Rewrite that pinning test in `apps/web/e2e/user-data-export.spec.ts` to assert the content rather than the emptiness, and amend `docs/adrs/` ADR-011 to say the placeholders are filled — the test's own comment asks whoever fills them in to do exactly this
- [ ] T078 [US7] Make `delete_user_data_owned` in `src/server/src/users/mod.rs` skip a world with live members other than the account, close its public paths, and list it for an administrator (FR-038)
- [ ] T079 [US7] Extend `apps/web/src/pages/user/StandingPage.tsx` with the window, the download and the appeal — the one page a disabled account can reach
- [ ] T080 [US7] Turn `repeatInfringerFlags` into terminations an admin can act on, in `apps/web/src/pages/admin/ModerationReviewPage.tsx` — today it renders a list of ids nothing acts on
- [ ] T081 [US7] Extend `apps/web/e2e/account-standing.spec.ts` with US7's seven acceptance scenarios (quickstart Scenario G), including the appeal-unresolved-at-day-thirty case and the strike-ages-out-while-disabled case

**Checkpoint**: the consequence the whole position rests on exists, and is not cruel by accident

---

## Phase 9: User Story 8 — running it makes it yours (Priority: P2)

**Goal**: the person who becomes an operator is told what they are taking on, at
the moment they take it on, and it is recorded.

**Independent Test**: bring up a fresh instance, walk setup, and confirm the
statement is shown, must be acknowledged, and is retrievable afterwards.

### Tests for User Story 8

- [ ] T082 [P] [US8] Add a test in `src/server/src/auth/admin_setup.rs` asserting `admin_setup_basic` cannot complete without an operator acknowledgement (FR-041)
- [ ] T083 [P] [US8] Add a test in `src/server/src/publishing.rs` asserting an instance with no notice contact refuses every publishing path **and** that a world, a scene and a roll still work (FR-053)

### Implementation for User Story 8

- [ ] T084 [US8] Require `operatorAcknowledgement: AttestationInput!` on `admin_setup_basic` in `src/server/src/auth/admin_setup.rs`, recording it through `attestation.rs` with `purpose = 'operator'`
- [ ] T085 [US8] Surface a changed operator statement to an administrator for re-acknowledgement rather than applying it silently, in `src/server/src/auth/admin_setup.rs` (FR-044)
- [ ] T086 [P] [US8] Render the operator statement inside the running instance at `apps/web/src/pages/legal/` so it is readable without a repository (FR-045)
- [ ] T087 [P] [US8] Add the acknowledgement step to the setup flow in `apps/web/src/pages/` — whichever component drives `adminSetupBasic`
- [ ] T088 [US8] Make the legal pages and the DMCA page name the instance's operator as the responsible party and give a way to reach them, and say plainly that the project cannot act on content in an instance it does not run (FR-046, FR-048)
- [ ] T089 [US8] Set a notice contact in `src/server/seeds/` so the existing e2e suite can still publish (closing T005)
- [ ] T090 [US8] Add `apps/web/e2e/operator-acknowledgement.spec.ts` covering quickstart Scenario H against a fresh instance

**Checkpoint**: the one link in the chain the project cannot enforce is at least stated to the person it applies to

---

## Phase 10: Polish & Cross-Cutting Concerns

- [ ] T091 [P] Document the environment values this feature adds — the four ladder settings and `THUNDERFORGE_NOTICE_CONTACT` — beside the three moderation values, which appear nowhere outside `specs/015-dmca-notice-takedown/tasks.md` today
- [ ] T092 [P] Record in `specs/038-scene-audio-and-tab-share/spec.md`'s planning notes that audio inherits the publishing gate by construction via the SDL guard test, so FR-026d needs no separate decision
- [ ] T093 [P] Record the scene gap: `collections::moderation_entity_type`'s `"scene" => None` means a scene in a shared collection is never withheld and records no adoption. Add it to `specs/015-dmca-notice-takedown/` as an open item — it is not this feature's to close
- [ ] T094 Make five guards fail on purpose per quickstart.md § "Making the guards fail on purpose" and record in the commit that each was seen to bite
- [ ] T095 Run quickstart Scenarios A–H by hand against `make dev` and note anything the suite does not catch
- [ ] T096 Run `cargo test --workspace -j 4`, `make lint` (lint-host + lint-wasm + file length) and `pnpm --filter @thunderforge/web test`. The wasm half matters for a negative reason: this feature must not have touched the engine
- [ ] T097 Run the full suite via `node scripts/e2e-parallel.mjs --shards=2` and record the figures in the commit body
- [ ] T098 Run `pnpm verify` and fix what it reports **in the code this feature added** — keep it to that; wide lint passes get their own commit

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies. **T004 blocks Phase 6 absolutely** — the constitution's guardrail requires the determination on record *before* implementation begins, not alongside it. T001–T003 may be written while Phase 3 proceeds but must land in the same change set (Principle IV)
- **Prose (Phase 2)**: blocks Phase 3, because the version identity is the hash of the final words. It does not block Phase 1
- **Foundational (Phase 3)**: blocks every story. T011→T012→T013→T014 are ordered; T017 is independent of all of them; T015 and T016 need T013
- **US1+US3 (Phase 4)**: needs Phase 3. This alone closes all three gaps the spec was written about
- **US2+US4 (Phase 5)**: needs Phase 4 to have produced an attestation worth reading
- **US6 (Phase 6)**: needs T004 and Phase 3. Independent of Phases 4, 5, 7 and 9 — it shares only `mutations_*_shares.rs` with Phase 4
- **US5 (Phase 7)**: needs Phase 3's `publishing.rs`. Independent of US6
- **US7 (Phase 8)**: needs US5's `standing.rs` and `strike_count`
- **US8 (Phase 9)**: needs Phase 3's `legal/mod.rs` and `attestation.rs` only. Can run beside any other phase
- **Polish (Phase 10)**: after the stories being shipped

### Within Each User Story

- Migration, then `make schema`, then models, then the module that uses them
- Server tests accompany the surface they test, in the module, as this codebase does
- Server surface before the web client that calls it
- E2E last within a story, because it needs both halves

### Parallel Opportunities

- T001, T002, T003 — three ADRs, three files (T004 is a gate and is not one of them)
- T009 and T010 — different files, both after T006–T008
- T023, T024, T025 — three share modules, one shape
- T028, T029, T030, T031 — three detail pages and one API module
- T040–T043, T055–T056, T064–T068, T082–T083 — test tasks within a story
- Phase 9 (US8) shares no file with Phases 6, 7 or 8 and can be handed to whoever is free

---

## Parallel Example: Setup

```bash
Task: "Write ADR-093 in docs/adrs/20260907-080-the_sharing_attestation_record.md"
Task: "Write ADR-095 in docs/adrs/20260907-082-account_standing_and_the_termination_window.md"
Task: "Write ADR-096 in docs/adrs/20260907-083-the_operator_acknowledgement.md"
```

## Parallel Example: the four publishing paths

```bash
# After T022 has established the shape on collections:
Task: "Gate create_actor_share_link_impl in src/server/src/graphql/mutations_actor_shares.rs"
Task: "Gate create_item_share_link_impl in src/server/src/graphql/mutations_item_shares.rs"
Task: "Gate create_ability_share_link_impl in src/server/src/graphql/mutations_ability_shares.rs"
```

---

## Implementation Strategy

### MVP (US1 + US3 + US2)

Phases 1 (T001, T005), 2, 3, 4 and 5. That is **all three gaps the spec was
written about**, closed: the terms appear on all four paths, the server is the
party that requires them, and what happens is recorded as evidence.

1. Phase 1 — ADR-093, and confirm the seed's notice contact
2. Phase 2 — the prose, with a reviewer. T007 can wait if US6 is deferred; T006
   and T008 cannot
3. Phase 3 — archive, record, gate
4. Phase 4 — four paths, four dialogs, one gate
5. Phase 5 — the record is readable and survives a revision
6. **Stop and validate**: quickstart Scenarios A–D by hand, plus the suite
7. Demonstrable: share an actor, watch the agreement appear where nothing ever
   appeared, then show an administrator who agreed and to exactly what words

**Why US2 is inside the MVP rather than after it.** The checklist calls FR-006
"the requirement the position rests on", and it is right: asking without
recording produces the friction of a legal control and none of its value. A
release that shows four dialogs and keeps nothing is worse than today, because
it looks finished.

**Why US4 comes with it.** It is one join. Doing it later means writing the
attestation read path twice, and the second version is the one where somebody
returns the current constant because it was easier.

### Incremental delivery after MVP

1. **US5** (Phase 7) — the ladder. Small, because the counting exists; it is the
   half of the owner's position that says a consequence follows
2. **US7** (Phase 8) — the window. The largest phase here and the one with the
   most net-new behaviour: no account disablement or deletion of this kind
   exists anywhere in the codebase today. **Sequence it after spec 040's mail if
   at all possible** — until mail exists, "the person is told" means "told the
   next time they open the product", and a person in a thirty-day window is
   exactly the person who has stopped opening it
3. **US6** (Phase 6) — the takedown's reach, if and only if T004 is accepted.
   Independent of everything except Phase 3, so it can be handed to whoever is
   free the moment the determination lands
4. **US8** (Phase 9) — the operator acknowledgement. Independent, cheap, and the
   thing that becomes urgent the day somebody else deploys an instance

---

## Notes

- [P] tasks touch different files and have no incomplete dependency
- Every story is independently testable; the checkpoints say what "done" looks
  like without the later phases
- Verify per target, per Principle V: native `cargo` for the server, wasm for
  the engine (to prove it was not disturbed), `tsc`/`vitest` for the web, e2e
  for the story
- Commit per task or coherent group; the ADRs land with the code, not after
- **T006, T007 and T008 are reviewer tasks.** Nothing in this repository is
  legal advice, `legal/README.md` says the prose needs review before launch, and
  this feature makes three changes to it that a lawyer should see
