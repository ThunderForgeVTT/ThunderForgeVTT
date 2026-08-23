---

description: "Task list for feature implementation"
---

# Tasks: DMCA Notice-and-Takedown Process

**Input**: Design documents from `/specs/015-dmca-notice-takedown/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/graphql-moderation.md, quickstart.md (all present)

**Tests**: Not explicitly requested as a separate TDD phase, but this feature is legal-compliance-critical (a missed enforcement point is a real liability, not just a bug) — every mutation/query task below includes its own inline `#[tokio::test]` coverage, following the existing `_impl`-function convention (`graphql/queries/actor.rs`), rather than a separate contract-test phase.

**Organization**: Tasks are grouped by user story (spec.md priorities) to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1–US4)
- Every task includes an exact file path

## Path Conventions

Existing two-project split: `src/server/` (Rust/Axum/Diesel/async-graphql backend), `apps/web/` (React/TypeScript frontend). No `src/engine` changes (this feature never touches canvas/simulation state, per plan.md's Constitution Check).

**Auto-restoration is computed lazily, not via a background job**: per data-model.md's `ModerationVisibility`/state-transition model, "restore when `restorationDueAt` passes with no further claimant action" is evaluated at read time (whenever a case's status is checked) rather than requiring new cron/scheduler infrastructure the codebase doesn't have — the first read after the due date materializes a `content_restored` event for a clean audit trail, matching plan.md's "no new runtime dependency" constraint.

---

## Phase 1: Setup

**Not applicable as a separate phase.** This feature's only "setup" is the migration and the moderation module scaffold, both genuinely blocking — folded into Phase 2 Foundational below.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The moderation table, its Diesel model/GraphQL types, and the shared "effective status" evaluator every content read path and every user story depends on.

**⚠️ CRITICAL**: No user story's acceptance scenarios can be verified until this phase is complete — the enforcement contract (contracts/graphql-moderation.md) requires this to exist before any content resolver can be touched.

- [ ] T001 Create Diesel migration `create_content_moderation_actions` (`id`, `case_id`, `action_type` varchar, `entity_type` varchar, `entity_id`, `world_id`, `account_id` nullable, `claimant_name`, `claimant_contact`, `copyrighted_work_description`, `infringing_material_location`, `good_faith_statement` bool, `accuracy_statement` bool, `signature`, `validity_result` varchar nullable, `missing_elements` text[] nullable, `counter_notice_id` uuid nullable, `restoration_due_at` timestamptz nullable, `created_at`, `created_by` nullable; indexes on `case_id`, `(entity_type, entity_id)`, and `account_id` for repeat-infringer lookback queries; deliberately NO foreign keys with `ON DELETE CASCADE` to `worlds`/`users`/content tables per FR-013/data-model.md's "must outlive its subject") in `src/server/migrations/<ts>_create_content_moderation_actions/{up,down}.sql` (data-model.md)
- [ ] T002 Run `diesel migration run` against the local dev DB and regenerate `src/server/src/schema.rs` with the new table (depends on T001)
- [ ] T003 [P] Add `ContentModerationAction` Diesel `Queryable`/`Insertable` struct to `src/server/src/models.rs`, matching the existing struct-per-table convention (depends on T002)
- [ ] T004 Create `src/server/src/moderation/mod.rs`: `effective_status(conn, entity_type, entity_id) -> Option<ModerationActionType>` — loads the latest `content_moderation_actions` row for `case_id`s matching `(entity_type, entity_id)`, applies the lazy-restoration rule (a `counter_notice_forwarded` row whose `restoration_due_at` has passed with no newer event for that `case_id` is treated as `content_restored`, and that transition is written back as a real `content_restored` row before returning — "first read after due date materializes the event," per this file's own header note above); returns `None` when no case exists or the latest event is `content_restored`/`notice_rejected_incomplete` (visible) (depends on T003)
- [ ] T005 [P] Create `src/server/src/moderation/validation.rs`: `validate_takedown_notice(input) -> Result<(), Vec<MissingElement>>` and `validate_counter_notice(input) -> Result<(), Vec<MissingElement>>` per data-model.md's validation rules (FR-003/FR-006) — pure functions, no DB access, easy to unit test in isolation (depends on T003)
- [ ] T006 Add `MODERATION_COUNTER_NOTICE_WAITING_PERIOD_DAYS` and `MODERATION_REPEAT_INFRINGER_LOOKBACK_DAYS`/`MODERATION_REPEAT_INFRINGER_THRESHOLD` as config values (env-var-backed, with the 10-14 business day default noted in research.md R3) in `src/server/src/config/mod.rs` (or wherever existing config values of this shape live), NOT hardcoded literals in `moderation/mod.rs` (depends on T004)
- [ ] T007 [P] Add `ModerationEntityType`, `ModerationActionType`, `GraphQLModerationAction`, `GraphQLModerationCase` GraphQL types to `src/server/src/graphql/types.rs`, matching the existing enum/SimpleObject conventions (contracts/graphql-moderation.md) (depends on T003)
- [ ] T008 [P] Add `SubmitTakedownNoticeInput`, `SubmitCounterNoticeInput` GraphQL input types to `src/server/src/graphql/input_types.rs` (contracts/graphql-moderation.md) (depends on T003)
- [ ] T009 Register the `moderation` module in `src/server/src/main.rs` (or wherever top-level modules are declared) (depends on T004, T005)
- [ ] T010 [P] Add `ModerationCaseRecord`, `ModerationActionRecord` TypeScript types to `apps/web/src/types/moderation.ts` (contracts/graphql-moderation.md)
- [ ] T011 [P] Create `apps/web/src/api/moderation.ts` with `fetch`-based GraphQL call stubs mirroring `apps/web/src/api/items.ts`'s `postGraphQL`/CSRF pattern (depends on T010)

**Checkpoint**: The moderation table, effective-status evaluator, and GraphQL scaffolding exist — user story implementation can begin.

---

## Phase 3: User Story 1 - A rights holder submits a takedown notice and the flagged content is disabled (Priority: P1) 🎯 MVP

**Goal**: Anyone can submit a takedown notice through a published channel; a validly-formed notice disables exactly the identified entry (actor, item, or lore entry) for every reader including the owner, while leaving the rest of that world's content untouched.

**Independent Test**: Submit a notice via `submitTakedownNotice` for a test world's actor; confirm the actor disappears from `worldActors` and returns a moderation placeholder from `actor(id)`, while every other actor/item/lore entry in the world stays fully visible (quickstart.md Scenario 1).

### Implementation for User Story 1

- [ ] T012 [US1] Implement `submitTakedownNotice` mutation in `src/server/src/graphql/mutations_moderation.rs`: public (no auth required per contracts/graphql-moderation.md), runs `validate_takedown_notice`, inserts either a `notice_received`+`content_disabled` pair (valid) or a `notice_rejected_incomplete` row (invalid) with a fresh `case_id`, resolves `entity_id`'s owning `world_id`/`account_id` by querying the matching content table for `world_id`/`created_by` at write time (denormalized per data-model.md) (depends on T004, T005, T007, T008)
- [ ] T013 [US1] Implement `moderationCase(caseId)` and `moderationStatus(entityType, entityId)` queries in `src/server/src/graphql/queries/moderation.rs`, the latter delegating directly to `moderation::effective_status` (depends on T004, T007, T012)
- [ ] T014 [US1] Wire `mutations_moderation`/`queries::moderation` into the GraphQL schema root in `src/server/src/graphql.rs` (`pub mod`/`pub use` + `QueryRoot`/`MutationRoot` field additions, mirroring the existing pattern for every other feature module) (depends on T012, T013)
- [ ] T015 [US1] Enforce the visibility check in `world_actors`/`search_actors` (`src/server/src/graphql/queries/actor.rs`): list resolvers exclude any actor whose `moderation::effective_status` is `content_disabled`/`content_remains_disabled`; the single-entity `actor(id)` resolver (wherever it lives — check `queries/actor.rs` and `graphql.rs` for the current single-actor query) returns a moderation-placeholder response instead of real field values for every caller including the owner (contracts/graphql-moderation.md's enforcement contract) (depends on T004, T014)
- [ ] T016 [US1] [P] Same enforcement for `world_items`/`item` in `src/server/src/graphql/queries/item.rs` (depends on T004, T014)
- [ ] T017 [US1] [P] Same enforcement for `world_lore_entries`/`lore_entry` in `src/server/src/graphql/queries/lore.rs` (depends on T004, T014)
- [ ] T018 [US1] Verify (add a resolver test if not already covered by T015-T017's own tests) that a disabled entity does not leak through any OTHER existing read path that surfaces actor/item/lore data — e.g. the Compendium catalog queries, lore in-text link resolution/"linked from" lists (spec 012), and item share previews (spec 013) — since the enforcement contract requires this to hold everywhere, not just the primary detail/list queries (depends on T015, T016, T017)
- [ ] T019 [US1] Create `apps/web/src/pages/legal/DmcaCompliancePage.tsx`: publishes the designated DMCA agent's name/title, mailing address, and electronic contact (FR-001), plain-language distinction between open-licensed system-pack content and user-entered content (FR-014), and embeds the takedown intake form (T020) — reachable at `/legal/dmca` without authentication (FR-001, SC-004)
- [ ] T020 [US1] [P] Create `apps/web/src/components/legal/TakedownNoticeForm.tsx`: collects all `SubmitTakedownNoticeInput` fields, calls `submitTakedownNotice`, and renders either a confirmation (case logged, content disabled) or the specific `missingElements` list on rejection (FR-002, FR-003) (depends on T011)
- [ ] T021 [US1] [P] Create `apps/web/src/components/world/ModeratedContentBanner.tsx`: renders in place of an actor/item/lore entry's detail view when its GraphQL response indicates `moderated: true`, explaining the entry was disabled in response to a takedown notice and (if the caller is the owner) linking to the counter-notice flow (FR-005) (depends on T010)
- [ ] T022 [US1] Wire `ModeratedContentBanner` into `ActorDetailPage.tsx`, `ItemDetailPage.tsx`, and `LoreEntryDetailPage.tsx` (all under `apps/web/src/pages/world/`) as the fallback render path when the fetched entity is a moderation placeholder (depends on T021)
- [ ] T023 [US1] Add `/legal/dmca` route (public, no auth guard) to `apps/web/src/routes/AppRoutes.tsx` and a lazy-loader entry in `apps/web/src/routes/pageLoaders.ts`, plus a link to it from the site's existing footer/legal-links surface (depends on T019)

**Checkpoint**: User Story 1 is fully functional and independently testable — a notice disables exactly one entry, everywhere it could otherwise be read, with a public intake channel.

---

## Phase 4: User Story 2 - A GM whose content was taken down submits a counter-notice (Priority: P2)

**Goal**: The owning GM of disabled content can submit a counter-notice; absent further claimant action within the configured waiting period, the content is automatically restored the next time its status is checked.

**Independent Test**: From a disabled entry (User Story 1's result), submit `submitCounterNotice` as the owner; confirm status becomes `COUNTER_NOTICE_FORWARDED` with `restorationDueAt` set; simulate the due date passing and confirm the next status check restores the entry (quickstart.md Scenario 3), and confirm staff can block that restoration first (quickstart.md Scenario 4).

### Implementation for User Story 2

- [ ] T024 [US2] Implement `submitCounterNotice` mutation in `src/server/src/graphql/mutations_moderation.rs`: requires the caller to be the owning account for the case's `world_id` (reuse whatever existing world-ownership check already gates GM-only actions, e.g. `is_dm_of_world`), runs `validate_counter_notice`, inserts `counter_notice_received` then `counter_notice_forwarded` (setting `restoration_due_at = now() + configured waiting period` from T006) (depends on T005, T006, T012)
- [ ] T025 [US2] Implement `resolveModerationCase(caseId, resolution)` mutation in `src/server/src/graphql/mutations_moderation.rs`: compliance-staff-only (reuse the existing `admin_user`/is_admin gate), inserts a `content_remains_disabled` (or other explicit resolution) row, which the T004 lazy-evaluator will find as the latest event and so never auto-restore past it (depends on T024)
- [ ] T026 [US2] Confirm (via test) that `moderation::effective_status`'s lazy-restoration path (T004) correctly materializes `content_restored` once `restoration_due_at` has passed with no `resolveModerationCase` call in between, and correctly does NOT restore when a `content_remains_disabled` row exists — add these as `#[tokio::test]` cases in `src/server/src/moderation/mod.rs`'s own test module if not already covered (depends on T024, T025)
- [ ] T027 [US2] Wire `submitCounterNotice`/`resolveModerationCase` into the GraphQL schema root alongside T014's wiring (depends on T024, T025)
- [ ] T028 [US2] [P] Create `apps/web/src/components/legal/CounterNoticeForm.tsx`: collects all `SubmitCounterNoticeInput` fields, calls `submitCounterNotice`, shown from `ModeratedContentBanner` (T021) only when the caller owns the content (depends on T011, T021)
- [ ] T029 [US2] Extend `ModeratedContentBanner.tsx` to show the restoration timeline once a counter-notice has been forwarded (`restorationDueAt`) (depends on T028)

**Checkpoint**: User Stories 1 and 2 both work independently — the takedown process is balanced (notice → disable → counter-notice → restore-or-stay-disabled), matching 17 U.S.C. § 512(g).

---

## Phase 5: User Story 3 - The platform maintains a repeat-infringer policy (Priority: P2)

**Goal**: Compliance staff can see an account's full takedown history and identify accounts crossing the published repeat-infringer threshold.

**Independent Test**: Create three separate valid, non-restored takedown cases against the same account within the lookback window; confirm `repeatInfringerFlags` includes that account (quickstart.md Scenario 5).

### Implementation for User Story 3

- [ ] T030 [US3] Implement `moderationHistoryForAccount(accountId)` query in `src/server/src/graphql/queries/moderation.rs`: compliance-staff-only, groups `content_moderation_actions` rows by `case_id` into `GraphQLModerationCase`s ordered chronologically (depends on T013)
- [ ] T031 [US3] [P] Implement `repeatInfringerFlags` query in `src/server/src/graphql/queries/moderation.rs`: compliance-staff-only, counts distinct `case_id`s per `account_id` whose latest event is `content_disabled`/`content_remains_disabled` within the T006 lookback window, returns accounts at or above the T006 threshold (depends on T006, T013)
- [ ] T032 [US3] Wire both queries into the GraphQL schema root (depends on T030, T031)
- [ ] T033 [US3] Create `apps/web/src/pages/admin/ModerationReviewPage.tsx`: internal compliance-staff surface listing flagged (repeat-infringer) accounts and, per account, its full case history via `moderationHistoryForAccount` (depends on T011, T030, T031)
- [ ] T034 [US3] Add an admin-only route for `ModerationReviewPage` (mirroring the existing `apps/web/src/pages/admin/*` route-guard convention) to `apps/web/src/routes/AppRoutes.tsx`/`pageLoaders.ts` (depends on T033)

**Checkpoint**: User Stories 1-3 all work independently — compliance staff have a working repeat-infringer view backed by real case history.

---

## Phase 6: User Story 4 - Guardrails are enforced before any public compendium-sharing feature ships (Priority: P1)

**Goal**: A documented, discoverable checkpoint exists requiring User Stories 1-3 to be operational (and a "centralized public repository" determination to be made) before any feature proposal that exposes compendium content beyond its owning world can proceed.

**Independent Test**: Confirm the ADR and a launch-review checklist entry exist and are linked from wherever this project's feature/design-review process lives, explicitly gating any future cross-world content-visibility feature (quickstart.md Scenario 6 — a documentation/process check, not a runtime test).

### Implementation for User Story 4

- [ ] T035 [US4] Author `docs/adrs/<next-number>-content-moderation-and-dmca-safe-harbor.md` per research.md R4: documents the polymorphic `content_moderation_actions` table decision (R1), the resolver-boundary enforcement decision (R2), and explicitly states the FR-011/FR-012 guardrail (no public compendium-sharing feature ships without this program operational and a "centralized public repository" determination on record) (depends on T001-T031 existing, so the ADR can accurately describe what was actually built)
- [ ] T036 [US4] Add a "DMCA / Content Moderation Guardrail" checklist item to wherever this project's feature-proposal/launch-review process is documented (e.g. `.specify/memory/constitution.md`'s Development Workflow section, or a dedicated `docs/` review-process doc if one exists) — referencing the T035 ADR and spec 015 by name, and explicitly requiring: (a) User Stories 1-3 operational, (b) an explicit "is this a centralized public repository" determination on record — before implementation of any feature exposing one world's compendium content to another world or the public begins (FR-012)

**Checkpoint**: All four user stories are complete — the platform has an operational, documented DMCA program and a preventative guardrail against the highest-liability feature category.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Verification and final wiring that spans multiple user stories.

- [ ] T037 [P] Run `cargo check` and `cargo test` in `src/server` to confirm all new resolvers and inline `#[tokio::test]` coverage pass (constitution Principle V)
- [ ] T038 [P] Run `cargo clippy --all-targets` on the touched native crates (thunderforge, thunderforge_core) and fix any new warnings, keeping the workspace at 0 (per the recent clippy pass)
- [ ] T039 [P] Run `pnpm --filter @thunderforge/web build` and `pnpm --filter @thunderforge/web lint` (scoped check — this repo's frontend lint has pre-existing unrelated baseline problems; confirm you haven't added new ones, not that the whole project is clean)
- [ ] T040 Execute every scenario in `specs/015-dmca-notice-takedown/quickstart.md` against a running local dev stack, including the public-facing `/legal/dmca` reachability check (FR-001/SC-004)
- [ ] T041 [P] Confirm `./scripts/check-file-length.sh` shows no new failures introduced by this feature's files

---

## Dependencies & Execution Order

### Phase Dependencies

- **Foundational (Phase 2)**: BLOCKS all user stories — the moderation table and effective-status evaluator are load-bearing for every story.
- **User Stories (Phase 3-6)**: All depend on Foundational.
  - US1 has no dependency on any other story — it's the MVP.
  - US2 depends on US1's `submitTakedownNotice`/case-creation existing (there must be a disabled case to counter-notice against).
  - US3 depends on US1's case history existing to have anything to evaluate for repeat-infringer status; independent of US2.
  - US4 depends on US1-US3 actually being built (the ADR and guardrail describe what exists) but is otherwise a documentation-only story with no code dependency.
- **Polish (Phase 7)**: Depends on all desired user stories being complete.

### Parallel Opportunities

- T003, and after it lands, T005/T007/T008 (Phase 2) can run in parallel.
- T010/T011 (Phase 2, frontend) can run in parallel with the backend foundational tasks.
- T016, T017 (Phase 3) can run in parallel once T014 lands (different files from T015).
- T020, T021 (Phase 3) can run in parallel once their dependencies land (different files).
- T031 (Phase 5) can run in parallel with T030 once T013 lands.
- T037-T039, T041 (Phase 7) can run in parallel.

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 2: Foundational
2. Complete Phase 3: User Story 1 — a public takedown channel that disables exactly one entry, everywhere it's readable
3. **STOP and VALIDATE**: run quickstart.md's Scenarios 1-2 independently
4. This alone gets the platform to a defensible baseline — a rights holder has somewhere to send a notice and it actually works

### Incremental Delivery

1. Foundational → foundation ready
2. US1 (notice → disable) → validate → this is the actual safe-harbor-critical MVP
3. US2 (counter-notice → restore) → validate — required by law to keep the process balanced, not optional polish
4. US3 (repeat-infringer tracking) → validate
5. US4 (guardrail documentation) → validate — cheap, and blocks the platform's highest-liability future feature category
6. Polish

### Suggested Task Ordering for a Single Implementer

Sequential by phase (T001→T041) is safe and matches dependency order above; within Phase 2 and within each story phase, [P]-marked tasks may be reordered or batched freely.
