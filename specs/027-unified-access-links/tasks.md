---

description: "Task list for 027 — Unified World Access Links & Consolidated Permission Resolution"
---

# Tasks: Unified World Access Links & Consolidated Permission Resolution

**Input**: Design documents from `/specs/027-unified-access-links/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/)

**Tests**: Included. The spec makes tests load-bearing rather than optional —
SC-003 is *defined* as the existing suite passing unmodified, and two live
defects (research §5, contracts/permission-resolution.md) need regression tests
that fail on `main` and pass after.

**Organization**: Grouped by user story so each ships independently.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: US1–US5, mapping to spec.md
- Exact file paths included

## Path Conventions

Web application per plan.md: Rust server under `src/server/`, shared models
under `src/core/`, React app under `apps/web/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Governance obligation and the additive schema change everything else builds on

- [X] T001 [P] Write ADR-050 at `docs/adrs/20260826-050-permission_declaration_and_world_access_links.md` covering (a) macro-generated permission resolution from a single declaration and why the polymorphic `world_content_permissions` alternative was rejected on `ON DELETE CASCADE` grounds, (b) `world_invites` becoming a revocable/rotatable access link kept distinct from content share links. Status Accepted, accountable owner recorded. Per plan Constitution Check this is a **gate**, not a follow-up (Principle IV)
- [X] T002 Create migration directory `src/server/migrations/2026-08-26-100000-0000_add_revocation_to_world_invites/` with `up.sql` adding `revoked BOOLEAN NOT NULL DEFAULT FALSE` and `rotated_from UUID NULL REFERENCES world_invites(id) ON DELETE SET NULL` to `world_invites`, per data-model.md §1. The `DEFAULT FALSE` is what makes every pre-existing row read as active (FR-007)
- [X] T003 Write the paired `down.sql` in the same directory dropping both columns, and verify it runs against a table containing rows (quickstart Definition of Done)
- [X] T004 Run `diesel migration run` and let `src/server/src/schema.rs` regenerate; confirm the `world_invites` block gains both columns

**Checkpoint**: Schema is ready and the architectural record exists

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Model, payload, and code-generation plumbing that US1/US3/US4 all need

**⚠️ CRITICAL**: US1, US3, and US4 cannot begin until this phase completes. **US2 does not depend on this phase** — see Dependencies.

- [X] T005 [P] Create `src/server/src/graphql/share_codes.rs` exporting a single `generate_link_code() -> String` producing 20 uppercase hex characters from an independent `Uuid::new_v4()`. Carry forward the comment explaining why the source must never be a v7 UUID (spec 005's real unique-index collision — research §6)
- [X] T006 Repoint `src/server/src/graphql/mutations_ability_shares.rs`, `mutations_item_shares.rs`, and `mutations_actor_shares.rs` to `generate_link_code()`, deleting their three local copies of `generate_share_code()`. Behaviour is identical — same length, same source
- [X] T007 Add `revoked: bool` and `rotated_from: Option<Uuid>` to `WorldInvite` and `NewWorldInvite` in `src/server/src/models.rs`
- [X] T008 Update both `From` impls for `WorldInvite` in `src/server/src/adapters.rs` (lines ~248 and ~265) to carry the two new fields across the Diesel ↔ core boundary
- [X] T009 Add `revoked: bool` to `WorldInvite` in `src/core/src/models/invites.rs` and make `is_valid()` return false when set. Preserve the existing `max_uses == 0` unlimited branch exactly — it is unreachable via the API but must keep working for any row that has one (research §7)
- [X] T010 [P] Add a `WorldAccessLinkState` GraphQL enum (`ACTIVE`/`EXPIRED`/`EXHAUSTED`/`REVOKED`) plus a pure derivation function honouring the precedence order in data-model.md §2, in `src/server/src/graphql/mutations_invites.rs`
- [X] T011 Extend `WorldInvitePayload` in `src/server/src/graphql/mutations_invites.rs` with `state`, `remaining_uses` (null when `max_uses == 0`), and `rotated_from`. Keep the existing `status` string, marked deprecated, for one release per contracts/graphql-access-links.md
- [X] T012 [P] Add unit tests for the state derivation in `src/server/src/graphql/mutations_invites.rs` covering all four states and the revoked-and-expired precedence case

**Checkpoint**: Payload and code generation ready — US1, US3, US4 can begin

---

## Phase 3: User Story 1 — Kill a leaked invite link (Priority: P1) 🎯 MVP

**Goal**: A GM can revoke a link outright, or rotate it so the old code dies immediately and a replacement takes its place.

**Independent Test**: Create a link, join with it to prove it works, rotate it, confirm the old code is refused on its very next use and the new one succeeds — with no part of US2–US5 present.

### Tests for User Story 1 ⚠️

> Write these first; they must fail before implementation.

- [X] T013 [P] [US1] Test that a rotated link's old code fails on its next use, and the new code succeeds, in `src/server/src/graphql/mutations_invites.rs` tests (contract assertion 1, SC-001)
- [X] T014 [P] [US1] Test that rotation inherits `max_uses` and `expires_at` while resetting `used_count` to 0, in `src/server/src/graphql/mutations_invites.rs` tests (assertion 4, FR-014)
- [X] T015 [P] [US1] Test that members who joined via the retired code keep their membership after rotation, in `src/server/src/graphql/mutations_invites.rs` tests (assertion 3, FR-005)
- [X] T016 [P] [US1] Test that rotating an expired or exhausted link yields a usable link, and rotating an already-revoked link is refused, in `src/server/src/graphql/mutations_invites.rs` tests (assertions 5–6)
- [X] T017 [P] [US1] Test that revoke is idempotent and that a non-DM is refused for both revoke and rotate, in `src/server/src/graphql/mutations_invites.rs` tests (assertions 7–8, FR-008)
- [X] T018 [P] [US1] **Regression test that fails on `main`**: two concurrent joins against a link with one use remaining — exactly one succeeds, and final `used_count` never exceeds `max_uses`, in `src/server/src/graphql/mutations_invites.rs` tests (assertion 11, FR-012, research §5)

### Implementation for User Story 1

- [X] T019 [US1] Replace the read-validate-write sequence in `join_world_impl` (`src/server/src/graphql/mutations_invites.rs`, currently ~lines 284–314) with the single conditional `UPDATE … WHERE` from data-model.md §4 that carries the whole validity predicate and returns `id, world_id`. Zero rows means unusable
- [X] T020 [US1] Wrap the use-consumption and the `world_members` insert in one transaction in `join_world_impl` in `src/server/src/graphql/mutations_invites.rs`, so a failed membership insert returns the use
- [X] T021 [US1] Implement `revoke_invite_code_impl(&AppState, user_id, is_admin, invite_id)` in `src/server/src/graphql/mutations_invites.rs` — DM-gated, idempotent, returns the updated payload
- [X] T022 [US1] Implement `rotate_invite_code_impl(&AppState, user_id, is_admin, invite_id)` in `src/server/src/graphql/mutations_invites.rs` as the two-statement transaction in data-model.md §3: revoke the old row guarded on `revoked = FALSE`, insert the replacement with inherited cap/expiry, `used_count = 0`, and `rotated_from` set. Returns the **new** link
- [X] T023 [US1] Switch `generate_invite_code_impl` in `src/server/src/graphql/mutations_invites.rs` to `generate_link_code()`, replacing the 8-character derivation
- [X] T024 [US1] Expose `revokeInviteCode` and `rotateInviteCode` on `InviteMutation` in `src/server/src/graphql/mutations_invites.rs`, following the existing thin-resolver-over-`_impl` convention
- [ ] T025 [P] [US1] Add `revokeInviteCode` and `rotateInviteCode` operations to `apps/web/src/api/world.ts` using the shared `postGraphQL` transport
- [ ] T026 [US1] Add explicit refetch after a successful revoke or rotate in `apps/web/src/hooks/useWorldInvites.ts` — the hook has no live push transport, so nothing else will deliver the change (research §8)
- [ ] T027 [US1] Add refresh and revoke controls to `apps/web/src/components/campaign/CampaignSettingsPanel.tsx`, with a confirm step on revoke since it is irreversible
- [ ] T028 [P] [US1] Create `apps/web/e2e/access-links.spec.ts` covering quickstart Scenario 1: generate → join → rotate → old code refused → new code works

**Checkpoint**: A GM can kill a leaked link. US1 is independently shippable.

---

## Phase 4: User Story 2 — Removing a member actually removes their access (Priority: P1)

**Goal**: Close the live privilege leak where a removed member keeps their ability grants.

**Independent Test**: Grant a member Editor on one actor, item, lore entry, and ability; remove them; confirm zero grants remain across all four. Fails on `main` today.

**No dependency on Phases 2 or 3** — this touches only `remove_member_impl` and can be pulled forward.

### Tests for User Story 2 ⚠️

- [X] T029 [P] [US2] **Regression test that fails on `main`**: a member holding grants on an actor, item, lore entry, and ability has all four removed on world removal, in `src/server/src/graphql/mutations_invites.rs` tests (assertion 9, US2-1)
- [X] T030 [P] [US2] Test that a removed-then-readmitted member holds zero elevated rights on any of the four types, in `src/server/src/graphql/mutations_invites.rs` tests (SC-008, US2-2)
- [X] T031 [P] [US2] Test that removal from World A leaves the same user's World B grants intact, and that removing a member with zero grants succeeds quietly, in `src/server/src/graphql/mutations_invites.rs` tests (assertions 11–12)

### Implementation for User Story 2

- [X] T032 [US2] Add the missing fourth cleanup block to `remove_member_impl` in `src/server/src/graphql/mutations_invites.rs` (after the existing lore block, ~line 576), deleting `world_ability_permissions` rows for the removed user scoped to abilities in that world — mirroring the three existing blocks. **Deliberately hand-written here** so the leak closes independently of the Phase 7 consolidation

**Checkpoint**: The privilege leak is closed. Safe even if US5 slips entirely.

---

## Phase 5: User Story 3 — See and control a link's lifetime (Priority: P2)

**Goal**: The GM panel reports each link's real state and remaining life, so a revoked link never looks live.

**Independent Test**: Drive four links into active/expired/exhausted/revoked and confirm the panel reports each distinctly with its reason.

### Tests for User Story 3 ⚠️

- [ ] T033 [P] [US3] Unit tests for client-side link-state derivation across all four states in `apps/web/src/db/collections/__tests__/worldInvitesCollection.test.ts`
- [ ] T034 [P] [US3] Test that `worldInvites` includes revoked links so a GM can see what they retired, in `src/server/src/graphql/queries/invite.rs` tests (contract: query rules)

### Implementation for User Story 3

- [X] T035 [US3] Map `state`, `remainingUses`, and `rotatedFrom` into the payload returned by `world_invites_impl` in `src/server/src/graphql/queries/invite.rs`, keeping newest-first stable ordering
- [ ] T036 [US3] Extend `WorldInviteDoc` and `computeInviteDerivedData` in `apps/web/src/db/collections/worldInvitesCollection.ts` to carry server-supplied `state` rather than recomputing validity client-side; keep client derivation display-only (data-model.md §2)
- [ ] T037 [US3] Map the new fields through `getWorldInvites` in `apps/web/src/api/world.ts` and the doc construction in `apps/web/src/hooks/useWorldInvites.ts`
- [ ] T038 [US3] Render state badges, remaining uses, and expiry per link in `apps/web/src/components/campaign/CampaignSettingsPanel.tsx`, replacing the bare `status` string. **No copy may describe the use cap as a security control** (spec Edge Cases)
- [ ] T039 [P] [US3] Reflect link state in `apps/web/src/components/world/SessionSetupInviteLink.tsx` so an unusable link is never presented as shareable

**Checkpoint**: Link state is legible everywhere it is shown

---

## Phase 6: User Story 4 — An unusable link fails cleanly (Priority: P2)

**Goal**: Unknown, expired, exhausted, and revoked codes are indistinguishable; a repeat click never burns a use.

**Independent Test**: Join with a code in each failure category plus a never-issued one and confirm all four responses are identical.

### Tests for User Story 4 ⚠️

- [X] T040 [P] [US4] Test that unknown, expired, exhausted, and revoked codes all fail with an identical message and identical error extensions, in `src/server/src/graphql/mutations_invites.rs` tests (assertion 9, SC-005)
- [X] T041 [P] [US4] Test that an existing member opening a valid link gets the distinct already-a-member message and that **no use is consumed**, in `src/server/src/graphql/mutations_invites.rs` tests (assertion 10, US4-2)

### Implementation for User Story 4

- [X] T042 [US4] Move the already-a-member check **before** use consumption in `join_world_impl` (`src/server/src/graphql/mutations_invites.rs`), per the contractual order of operations in contracts/graphql-access-links.md
- [X] T043 [US4] Collapse the distinct failure strings in `join_world_impl` in `src/server/src/graphql/mutations_invites.rs` to the single uniform message `This invite link is no longer available.`, matching the wording `load_active_share` already uses for content shares. Ensure the unknown-code path and the zero-rows-updated path return the identical error value
- [ ] T044 [P] [US4] Surface the uniform message without embellishment in `apps/web/src/pages/world/JoinWorldPage.tsx`, and extend `apps/web/e2e/access-links.spec.ts` with quickstart Scenario 4

**Checkpoint**: Failure reveals nothing; a valid link never loses a use to a double click

---

## Phase 7: User Story 5 — One authorization path for every content type (Priority: P3)

**Goal**: Four near-verbatim permission modules collapse to one declaration, and removal cleanup derives from it so the US2 defect cannot recur.

**Independent Test**: All four types resolve identically under identical conditions, and the cleanup test derives its type list from the declaration rather than restating it.

**Behaviour must not change.** SC-003 is satisfied only if no existing test's expected outcome is edited.

### Tests for User Story 5 ⚠️

- [ ] T045 [P] [US5] Parity tests asserting all four content types resolve identically under identical conditions, in `src/server/src/auth/permissioned_entities.rs` tests (assertions 1–3, US5-1/2)
- [ ] T046 [P] [US5] Tests pinning the preserved edge behaviours before the refactor: unparseable `level` falls back to Viewer, `is_admin` short-circuits to Owner, multiple simultaneous Owners are accepted, a missing content row errors while a missing grant row does not — in `src/server/src/auth/permissioned_entities.rs` tests (assertions 4–7)
- [ ] T047 [P] [US5] Test that a member with **Editor** on a GM-only ability still cannot see it, in `src/server/src/auth/ability_permissions.rs` tests (assertion 8, FR-019, US5-3)
- [ ] T048 [US5] Structural test that the type set walked by `purge_member_grants` is derived from the declaration, not a hardcoded list of four, in `src/server/src/auth/permissioned_entities.rs` tests (assertion 13, SC-002). **Must fail if a declared type is skipped by cleanup** — a test restating the list cannot catch the bug it exists to prevent

### Implementation for User Story 5

- [ ] T049 [US5] Move `is_dm_of_world` from `src/server/src/auth/actor_permissions.rs` into `src/server/src/auth/world_membership.rs`, beside the `require_world_member` it already calls (research §2)
- [ ] T050 [US5] Delete the `pub use crate::auth::actor_permissions::is_dm_of_world;` shim at `src/server/src/auth/lore_permissions.rs:12` rather than repointing it
- [ ] T051 [US5] Update all `is_dm_of_world` import paths across the ~49 call sites — 20 `use` statements plus 7 fully-qualified inline paths in `src/server/src/graphql.rs`. Every miss is a compile error, not a runtime bug
- [ ] T052 [US5] Create `src/server/src/auth/permissioned_entities.rs` with the `permissioned_entities!` macro and the single four-entry declaration from contracts/permission-resolution.md. Generated function **names and signatures must match today's exactly** so no resolver call site changes
- [ ] T053 [US5] Generate `purge_member_grants(conn, world_id, user_id)` over all declared entries in `src/server/src/auth/permissioned_entities.rs`, summing rows removed
- [ ] T054 [US5] Reduce `src/server/src/auth/actor_permissions.rs` to the macro-generated resolution, removing the hand-written `effective_actor_permission`/`require_actor_permission` bodies while keeping the module's doc comment and its existing tests untouched
- [ ] T055 [P] [US5] Same reduction for `src/server/src/auth/item_permissions.rs`
- [ ] T056 [P] [US5] Same reduction for `src/server/src/auth/lore_permissions.rs`, with `world_member_user_id` supplied as the declaration's `user_fk` — absorbed, never migrated (research §3)
- [ ] T057 [US5] Same reduction for `src/server/src/auth/ability_permissions.rs`, **keeping `is_ability_visible_to` hand-written and outside the macro** along with its doc comment. The macro must not gain a visibility parameter "for symmetry" (FR-019)
- [ ] T058 [US5] Replace all four hand-written cleanup blocks in `remove_member_impl` (`src/server/src/graphql/mutations_invites.rs`, ~lines 519–576) — including T032's — with one `purge_member_grants` call
- [ ] T059 [US5] Run the full `cargo test -p thunderforge` suite and **confirm no pre-existing test was modified** (SC-003). An edited expectation means behaviour moved; investigate rather than accept

**Checkpoint**: One declaration governs resolution and cleanup for every content type

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T060 [P] Run every scenario in [quickstart.md](./quickstart.md) and record results, naming any gap rather than ticking it
- [ ] T061 [P] Verify SC-006 with a test in `src/server/src/graphql/mutations_invites.rs`: an 8-character code inserted directly (as a pre-migration row would be) still joins successfully, and its row reads `ACTIVE`
- [ ] T062 [P] Verify SC-007 with a test in `src/server/src/graphql/share_codes.rs`: newly issued codes are 20 characters, and rapid-succession generation produces no ordering pattern and no collisions (the spec 005 regression guard)
- [ ] T063 Update `specs/027-unified-access-links/quickstart.md`'s Definition of Done with verified/not-verified status, naming what was checked by test versus by hand
- [ ] T064 [P] Confirm ADR-050 (T001) still matches what was actually built; amend it if the implementation diverged
- [ ] T065 Run `cargo test -p thunderforge`, `npx tsc --noEmit --ignoreDeprecations 6.0`, `npx vitest run`, and `npx vite build`; rebuild the dev backend before any e2e run, since a stale binary predating the migration produces failures that look like code bugs

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies
- **Phase 2 (Foundational)**: Depends on Phase 1 — blocks US1, US3, US4
- **Phase 3 (US1)**: Depends on Phase 2
- **Phase 4 (US2)**: **Depends on nothing.** Can start immediately, in parallel with Phase 1
- **Phase 5 (US3)**: Depends on Phase 2; T035 reads fields added in T011
- **Phase 6 (US4)**: Depends on Phase 3 — T042/T043 modify the `join_world_impl` that T019/T020 rewrite
- **Phase 7 (US5)**: Depends on Phase 4 only insofar as T058 subsumes T032
- **Phase 8 (Polish)**: Depends on all desired stories

### Pull US2 forward if the privilege leak matters

US2 is a **live security defect** and has no prerequisites — it touches only
`remove_member_impl`. It is listed at Phase 4 to follow spec priority order,
but T029–T032 can be done first, on their own, and shipped before anything
else here. That is the point of fixing it by hand at T032 rather than waiting
for T058 to generate it.

### Within Each User Story

- Tests before implementation, and the two regression tests (T018, T029) must
  be **observed failing on `main`** before their fix lands
- Server before frontend
- `_impl` free functions before the thin GraphQL resolvers over them

### Parallel Opportunities

- T001 runs alongside T002–T004 (documentation vs. schema)
- All of Phase 4 (T029–T032) runs alongside Phases 1–3
- T005 and T010 are independent within Phase 2
- All test tasks marked [P] within a story
- T055 and T056 are independent files; T054 and T057 are not (T057 also edits hand-written code)

---

## Parallel Example: User Story 1

```bash
# All six US1 tests together — same file, so write as one batch then verify all fail:
Task: "Rotated old code fails on next use (T013)"
Task: "Rotation inherits cap/expiry, resets count (T014)"
Task: "Retired-code members keep membership (T015)"
Task: "Expired rotates fine; revoked refuses (T016)"
Task: "Revoke idempotent; non-DM refused (T017)"
Task: "Concurrent last-use admits exactly one (T018)"

# Frontend tasks in parallel once the server side lands:
Task: "Add revoke/rotate to apps/web/src/api/world.ts (T025)"
Task: "Create apps/web/e2e/access-links.spec.ts (T028)"
```

---

## Implementation Strategy

### MVP (User Story 1)

1. Phase 1: Setup — migration + ADR-050
2. Phase 2: Foundational — payload, code generator
3. Phase 3: US1 — revoke, rotate, atomic join
4. **STOP and VALIDATE**: quickstart Scenario 1
5. A GM can now kill a leaked link. Ship it.

### Recommended real ordering

Given US2 is a live privilege leak with zero prerequisites:

1. **Phase 4 (US2) first** — smallest change, closes a security bug, ships alone
2. Phase 1 → Phase 2 → Phase 3 (US1) — the headline capability
3. Phases 5–6 (US3, US4) — make it legible and safe to fail
4. Phase 7 (US5) — the consolidation, last, changing nothing observable
5. Phase 8 — polish

### Incremental Delivery

Each of US1, US2, US3, US4 is independently demoable. US5 is deliberately
*not* demoable — its success is the absence of change (SC-003), so validate it
by the test suite rather than by demonstration.

---

## Notes

- [P] = different files, no dependencies
- Two tasks are regression tests that **must fail on `main`**: T018 (concurrent
  use consumption) and T029 (ability grant cleanup). If either passes before
  the fix, the test is wrong, not the code
- T059 and T048 are the load-bearing checks for the consolidation; neither is
  satisfied by "the suite is green"
- Rebuild the dev backend after the migration before running any e2e
- Commit after each task or logical group
