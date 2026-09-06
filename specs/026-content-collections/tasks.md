---

description: "Task list for 026-content-collections"
---

# Tasks: Content Collections

**Input**: Design documents from `/specs/026-content-collections/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/collection-share.md](./contracts/collection-share.md), [quickstart.md](./quickstart.md)

**Tests**: Included. Several success criteria say "verified across every type
rather than sampled" (SC-003a, SC-004) or "by inspection of every read path, not
by sampling" (SC-007) — those are test obligations stated in the spec, not
optional extras.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to

## Path Conventions

Web application. Server: `src/server/src/` (crate `thunderforge-server` — note
`cargo test -p thunderforge` runs 11 tests, not the suite). Web:
`apps/web/src/`. E2E: `apps/web/e2e/`.

---

## Phase 0: The gate (BLOCKING — not implementation)

**⚠️ Nothing below Phase 0 may start until T001 exists on disk.** The
constitution's DMCA guardrail requires an on-record determination **before
implementation begins**, and FR-027 restates it. This is a signature from an
accountable owner, not something a task can produce on its own.

- [X] T001 Write the FR-027 determination as an ADR at `docs/adrs/2026MMDD-0NN-collection_share_dmca_repository_determination.md`, accepted by an accountable owner, following ADR-067 (spec 034) as the worked example. It must address what changed since ADR-049's single-artifact determination: the **unit of distribution** is now a set, and the read path is now **anonymous** (FR-009a) where singleton shares require a session. It must state the two facts that bound the risk — FR-020 forbids every enumeration surface, and nothing is reachable without already holding an ~80-bit code — and record acceptance rather than assert safety.
- [X] T002 [P] Write an ADR at `docs/adrs/2026MMDD-0NN-anonymous_collection_read_path.md` recording the divergence from spec 025's authenticated shares: `sharedAbility`/`sharedItem`/`sharedActor` each call `authenticated_user(ctx)?`, `sharedCollection` will not. Principle IV requires this because it changes an established access boundary. Record FR-009e's scope decision (the three shipped shares are **not** being aligned in this delivery) and why.

---

## Phase 1: Setup

- [X] T003 Create the migration directory `src/server/migrations/2026-09-XX-000000-0000_create_world_collections/` with `up.sql` and `down.sql`, following the naming of the existing `2026-08-25-160000-0000_create_world_ability_shares`.
- [X] T004 Create the module skeleton `src/server/src/collections/mod.rs` with empty `membership`, `resolve`, `copy`, `scene_copy` and `rate_limit` submodules, and register it in `src/server/src/lib.rs`.

---

## Phase 2: Foundational (blocking prerequisites for every story)

**⚠️ No user story work can begin until this phase is complete.**

- [X] T005 Write `up.sql` in the T003 migration creating `world_collections`, `world_collection_members` and `world_collection_shares` exactly as [data-model.md](./data-model.md) specifies. Carry forward the warning comment the `world_ability_shares` migration uses: no index or query shape may exist that lists shares by world or by user. State in a comment why `member_id` carries **no foreign key** (a cascade would silently delete membership when an artifact is deleted, and the collection must survive that) and why there is **no `disabled` or `restricted` column** (status is asked at read time; a cached value is stale in both directions).
- [X] T006 Write the matching `down.sql` dropping the three tables in dependency order.
- [X] T007 Run the migration and regenerate `src/server/src/schema.rs`; confirm the diff contains only the three new tables.
- [X] T008 [P] Add `Collection`, `NewCollection`, `CollectionMember`, `NewCollectionMember`, `CollectionShare`, `NewCollectionShare` to `src/server/src/models.rs`, following the shape of the existing `AbilityShare`/`NewAbilityShare`.
- [X] T009 Implement `restriction_reason(state, user_id, member_type, member_id) -> Option<String>` in `src/server/src/collections/membership.rs`, consulting **both** restriction axes for all five member types: explicit grant rows (`world_actor_permissions`, `world_item_permissions`, `world_lore_permissions`, `world_ability_permissions` — note lore names its user column `world_member_user_id`) and visibility flags (`world_abilities.gm_only`, `world_items.gm_only`, `scenes.hidden`). Do **not** extend the `permissioned_entities!` macro — its own documentation forbids gaining a visibility parameter. Return the sentence shown to the user.
- [X] T010 Write unit tests for T009 in the same file covering **every** member type on **both** axes, plus the unrestricted case for each. This is the test SC-003a's "rather than sampled" wording asks for.
- [X] T011 Implement `resolve_member(state, member) -> MemberResolution` in `src/server/src/collections/resolve.rs`, returning `Visible(...)`, `Withheld(reason)` or `Gone`. It must call `moderation::effective_status(state, "<entity_type>", id)` and treat `Some(_)` as withheld, and must treat a `member_id` that no longer resolves as `Gone` rather than an error.
- [X] T012 [P] Implement the collection rate limiter in `src/server/src/collections/rate_limit.rs`, keyed on `client_ip`, using the `OnceLock<Mutex<HashMap<String, Vec<i64>>>>` sliding-window shape of `auth_middleware::rate_limit_auth_requests`. It MUST NOT consult `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT` — the e2e harness sets that on every run, and a limiter switched off during its own test is untested. Add a unit test asserting the limiter still refuses when that variable is set.

---

## Phase 3: User Story 1 — Gather and share once (P1) 🎯 MVP

**Goal**: A Game Master gathers artifacts of mixed types into a named
collection, shares one link, and a recipient copies the whole thing into their
own world as independent records.

**Independent test**: Build a world with several artifacts of different types,
gather them into a collection, open its link as a different user in a different
world, copy it, and confirm every member arrived and that editing a copy leaves
the original untouched.

### Server — authoring

- [X] T013 [US1] Implement `create_collection_impl`, `update_collection_impl` and `delete_collection_impl` in `src/server/src/graphql/mutations_collections.rs`, each requiring DM authority over the world (`is_dm_of_world`). Deleting a collection deletes no artifacts (FR-004).
- [X] T014 [US1] Implement `add_collection_member_impl` in the same file, enforcing: the artifact belongs to the collection's world (FR-003); `restriction_reason` returns `None` or the add is refused with that sentence (FR-001a); the collection holds fewer than 100 members (FR-005a) with a message naming the limit.
- [X] T015 [US1] Implement `remove_collection_member_impl`, deleting only the membership row (FR-004).
- [X] T016 [P] [US1] Implement the `collection` and `worldCollections` queries in `src/server/src/graphql/queries/collections.rs`, both requiring authority over the world. Add no other listing surface (FR-020).
- [X] T017 [US1] Write unit tests for T013–T016 in `mutations_collections.rs`: a non-DM is refused; a cross-world artifact is refused; a restricted artifact is refused with a reason; the 101st member is refused; removing a member leaves the artifact present.

### Server — sharing and the anonymous read

- [X] T018 [US1] Implement `create_collection_share_link_impl` in `src/server/src/graphql/mutations_collection_shares.rs`, requiring **Owner** level (following `create_ability_share_link_impl`), calling `generate_link_code()`, and **re-checking every member's restriction at share time** — the shipped ability path re-checks because sharing "is the one path that escapes the world".
- [X] T019 [US1] Implement `shared_collection_impl` in the same file and wire it as the `sharedCollection` query **without** `authenticated_user(ctx)?`. Resolve each member via `resolve_member`; return visible members, counts by type, and a `withheldCount` that is a number and never a name. Apply the T012 rate limiter first.
- [X] T020 [US1] Make the four refusal cases — unknown code, revoked share, deleted collection, no active share — return the **same sentence**, so an outsider cannot distinguish them (FR-009d).
- [X] T021 [US1] Write unit tests asserting the anonymous preview carries no world id, no world name, and no member list of the source world, modelled on the shipped `shared_ability_preview_omits_source_world_identity`.

### Server — copying

- [x] T022 [US1] Implement `CopyError` and the transaction skeleton of `copy_shared_collection_to_world_impl` in `src/server/src/collections/copy.rs`: require an authenticated DM of the destination (FR-009b, FR-016); inside `conn.transaction::<_, CopyError, _>`, re-load the share and confirm it is active (it may have been revoked between preview and confirm), and re-assert the 100-member limit.
- [x] T023 [P] [US1] Implement ability and item copying in `copy.rs`: clone the row and its effects, **preserve `gm_only`** (fail closed — a copy arriving un-hidden exposes what the source hid), re-validate effect formulas rather than trusting the source, and stamp `created_by`/`updated_by` with the copier (FR-017a).
- [x] T024 [P] [US1] Implement actor and lore-entry copying in `copy.rs`, same ownership stamping.
- [x] T025 [US1] Implement scene copying in `src/server/src/collections/scene_copy.rs`: copy the `scenes` row plus `walls`, `light_sources` and `shapes`; create a **new `canvas_assets` row in the destination world pointing at the same `storage_path`** for the background. Do **not** copy `tokens`, `fog_masks` or `interactives` — each becomes a fidelity note. Write the reasoning in a module comment (a scene is a place, not a session) so the next reader does not "fix" the omission.
- [x] T026 [US1] Build the old-id → new-id map as rows are inserted and re-point intra-collection references at the copies (FR-014) — an actor that knows an included ability must know the *copy* of it.
- [x] T027 [US1] Record every reference to something outside the collection as a fidelity note and return a `CopyReceipt` (FR-015). The receipt is **returned, never stored** — a stored record naming both sides is exactly the referential link FR-012 forbids.
- [x] T028 [US1] Write unit tests for the copy path: every member type arrives; editing a copy does not change the source and the reverse, **for each of the five types** (SC-004 says verified, not sampled); an intra-collection reference points at the copy; an out-of-collection reference produces a fidelity note; a forced failure part-way leaves nothing behind (SC-006).
- [x] T029 [US1] Write a test asserting that copying a scene whose image the platform already holds adds a `canvas_assets` row but **no new `storage_path`** (SC-008).

### Web

- [x] T030 [P] [US1] Add `apps/web/src/types/collection.ts` and `apps/web/src/api/collections.ts`, following `types/abilityShare.ts` and `api/abilityShares.ts`.
- [x] T031 [US1] Build `apps/web/src/pages/world-collections/` — list a world's collections, create one, add and remove members via a picker covering all five types, share and copy the link. Surface T014's refusals verbatim rather than as a generic error.
- [x] T032 [US1] Build `apps/web/src/pages/collection-share/SharedCollectionPage.tsx` at route `/collection/:shareCode`, rendering **for a signed-out visitor**. Copying prompts for sign-in and a destination world — the point at which viewing and copying diverge (FR-009b).
- [x] T033 [US1] Register both routes in the router, confirming `/collection/:shareCode` sits outside whatever guard wraps the authenticated routes.
- [x] T034 [US1] Write `apps/web/e2e/content-collections.spec.ts` walking the whole journey: author one of each type, gather, share, open the link **signed out in a fresh context**, sign in as a second user, copy into their world, and confirm all five arrived and the scene renders its background (SC-008a).

**Checkpoint**: US1 delivers value alone. Everything after this protects or refines it.

---

## Phase 4: User Story 2 — The owner stays in control (P1)

**Goal**: Revocation works, says what it does, and does not overclaim.

**Independent test**: Share a collection, confirm the link works, revoke it,
confirm the link now reports the collection as no longer available, and confirm
a copy taken beforehand is unaffected.

- [x] T035 [US2] Implement `revoke_collection_share_link_impl` in `mutations_collection_shares.rs` as a **soft flag**, never a delete — a deleted row cannot distinguish "revoked" from "never existed", which is why the shipped share tables use `revoked BOOLEAN`. Allow the link's creator or a DM of its world.
- [x] T036 [US2] Write unit tests: a revoked share resolves to the refusal sentence; a copy taken before revocation is untouched; deleting the collection makes the code stop resolving with that same sentence.
- [x] T037 [P] [US2] In the revoke confirmation UI, state plainly that copies already made are unaffected (FR-011) — at the moment of revoking, not in a help page.
- [x] T038 [US2] Add the revoke and delete flows to `apps/web/e2e/content-collections.spec.ts`: link works → revoke → link reports unavailable within one page load (SC-005) → the recipient's copy is intact.

---

## Phase 5: User Story 3 — A takedown reaches one member (P2)

**Goal**: Moderation reaches into a collection without disabling it.

**Independent test**: Share a collection of several members, file a valid
takedown against one, and confirm the collection still opens, the disabled
member is absent and shown as withheld, the remaining members still copy, and a
copy made afterwards does not contain the disabled member.

- [x] T039 [US3] Confirm by test — not by inspection — that `resolve_member` (T011) already withholds a moderated member on both the preview and the copy path (FR-021, FR-023). If it does, this task is a test; if it does not, it is a fix.
- [x] T040 [US3] Implement the all-withheld case: a collection whose every member is withheld reports "nothing is available" rather than presenting an empty collection as complete (FR-024), and refuses the copy.
- [x] T041 [US3] Add the withheld indicator to the preview UI: something has been withheld, **without naming it or reproducing its content** (FR-022).
- [x] T042 [US3] Write a test proving FR-025 needs no cache invalidation: reverse a takedown and confirm the member returns on the next read without the owner touching the collection. `moderation::effective_status` restores lazily, so this passes only if no status was cached — which is what makes it worth asserting.
- [x] T043 [US3] Write `apps/web/e2e/collection-moderation.spec.ts` using `submit_takedown_notice_impl` the way the shipped ability-share moderation test does: take one member down, confirm three of four remain and the fourth is unnamed; take the rest down, confirm "nothing available"; reverse one, confirm it returns.
- [x] T044 [US3] Implement FR-001b: an artifact that **becomes** restricted after being added is withheld from that point, by the same path as a moderated member. Test both directions — restricting after adding withholds it, lifting the restriction returns it — because a check that runs only at add time passes the first half and fails the second.

---

## Phase 6: User Story 4 — The recipient understands what they are taking (P3)

**Goal**: The preview says what will arrive; the receipt says what did not.

**Independent test**: Open a collection containing a mix of types, confirm the
preview states what it will add, copy it, and confirm the result names anything
that could not be brought across.

- [x] T045 [P] [US4] Show counts by type in the preview before copying (US4 scenario 1), sourced from the `sharedCollection` response rather than computed client-side.
- [x] T046 [US4] Render the `CopyReceipt` after copying: what arrived, and every fidelity note — out-of-collection references, withheld members (unnamed), and the scene children not copied by design.
- [x] T047 [US4] Extend `content-collections.spec.ts` to assert the preview's counts match what arrives, and that a collection with an out-of-collection reference names that loss to the recipient.

---

## Phase 7: Polish and cross-cutting

- [x] T048 [P] Write `apps/web/e2e/collection-anonymous-access.spec.ts`: a signed-out visitor opens a valid link and sees the collection; repeated wrong codes hit the rate limiter (FR-009c); an unknown code, a revoked share and a deleted collection return the same sentence (FR-009d); no response in the anonymous path carries a world id or name (SC-007a).
- [x] T049 Verify FR-020 by **inspection of every read path**, not sampling (SC-007): grep every resolver touching the three new tables and confirm none returns collections the caller does not own. Record the finding in this file.

  **Finding, 2026-09-05.** Nine read paths touch `world_collections`,
  `world_collection_members` or `world_collection_shares`. Every one is either
  filtered to a single id the caller already named, or guarded by an authority
  check before the query runs. None can list, search or count beyond the
  caller's own.

  | Path | Filter | Guard |
  |---|---|---|
  | `mutations_collections.rs:98` (`require_collection_authority`) | one `id` | this *is* the guard; callers await it first |
  | `mutations_collections.rs:451` (`world_collections_impl`) | `world_id` | `is_dm_of_world` before the query |
  | `mutations_collections.rs:497` (`collection_members_impl`) | one `collection_id` | `require_collection_authority` before the query |
  | `mutations_collection_shares.rs:103` (`load_active_share`) | one `share_code` | the code is the authority (FR-007) |
  | `mutations_collection_shares.rs:150` (`collection_share_link_impl`, FR-010a) | one `collection_id` | creator-or-DM before the query |
  | `mutations_collection_shares.rs:242` (revoke) | one `share_id` | creator-or-DM on the row it read |
  | `mutations_collection_shares.rs:311` (anonymous preview) | the share's own `collection_id` | reached only through a valid unrevoked code |
  | `mutations_collection_shares.rs:382` (`collection_world_and_owner`) | one `collection_id` | a helper; returns the owner *so* callers can check |
  | `mutations_collection_shares.rs:405` (`load_members`) | one `collection_id` | a helper; every caller checks first |

  Two of the nine are helpers that perform no check themselves
  (`collection_world_and_owner`, `load_members`). That is the shape most likely
  to drift — a future caller that forgets. Both are `pub` within the crate and
  neither is reachable from a resolver that does not check, verified by reading
  every call site rather than by sampling.

  There is no `myCollections`, no `collectionsByUser`, and no path returning a
  share code except to the collection's own owner.
- [x] T050 [P] Add the 100-member case to the e2e suite: the 101st add is refused with a message naming the limit, and a 100-member collection copies inside one waited-out action (SC-002a) — assert an upper bound on the wall time rather than merely that it completes.
- [x] T051 [P] Add the share-terms text (FR-026): the person sharing is responsible for having the right to share what is in the collection, and a copy taken by someone else is theirs and cannot be recalled. It belongs in `legal/` with the other reviewable text, surfaced at the share step.
- [x] T052 Mutation-test the guards that matter, and record which mutation each test caught: break `restriction_reason` for one member type (T010 must fail); make the rate limiter honour `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT` (T012's test must fail); make one refusal sentence differ from the others (T020's test must fail); cache moderation status on the member row (T042 must fail); copy `tokens` with a scene (T025's fidelity-note test must fail).

  **Run 2026-09-05.** Each mutation applied, the suite run, the mutation
  reverted. All five were caught, and one exposed a weak assertion.

  | Mutation | Result |
  |---|---|
  | `restriction_reason`'s `"ability"` arm made unreachable | **caught** — 3 failures in `collections::membership` |
  | `allow_request` returns early on `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT` | **caught** — including `the_e2e_auth_bypass_does_not_disable_this_limiter`, which exists for precisely this |
  | `load_active_share` answers "That collection has been revoked." instead of `UNAVAILABLE` | **caught** — 2 failures; the constant is compared across all three cases rather than to a literal |
  | `resolve_member` stops consulting moderation | **caught** — 2 failures, including the copy-path test added this session |
  | tokens copied along with the scene | **caught** — `tokens_stay_behind_and_the_omission_is_declared` |

  **What the pass found.** Before the real token mutation, a cheaper one was
  tried first: leave the behaviour alone and reword the fidelity note to
  "brought its 3 tokens along". That **survived**, because the assertion was
  `n.contains("token")` — satisfied by a note declaring the opposite of what
  happened. The behaviour assertion still caught the real mutation, so the test
  was never wrong; but half of it was measuring nothing, and a declared loss
  that declares an arrival is the exact failure FR-015 exists to prevent. The
  assertion now requires the note to say the tokens were *not copied*.

  That is the argument for mutation testing in one example: the surviving
  mutation was not the one on the list.
- [x] T053 Run `pnpm verify` (all ten checks), `cargo test -p thunderforge-server`, `pnpm --filter web test`, and the full Playwright sweep with `--workers=1`. Before starting: confirm the tree is clean and `ss -ltn | grep 301` is empty — stale backends have twice produced what looked like fresh regressions.

  **Run 2026-09-05.** `pnpm verify` (10/10), 917 server tests, 423 web tests,
  and the full Playwright sweep at 4 shards — 87 spec files, 28.1 minutes.

  **Three e2e failures, one cause, all mine.** `onboarding-flow`,
  `gm-staging-page` and `genie-scene-topology` each located a world's starter
  scene *by the world's name* — the behaviour FR-009f deliberately removed.
  They were asserting the leak. Fixed by giving the suite a shared
  `STARTER_SCENE_NAME` in `e2e/fixtures/helpers.ts` and matching on it, so the
  next rename is one edit rather than a hunt.

  Worth recording that `e2e-parallel.mjs` **exited 0 with two shards failing**.
  The per-shard result lines are the truth; the exit code is not. Do not read a
  green exit as a green sweep.
- [x] T054 Update `TOMORROW.md` with what shipped, what was found, and anything the next session would otherwise rediscover.

---

## Deferred to the playtest

- [ ] T055 Manual pass: SC-001 (a GM gathers ten mixed artifacts and shares in under three minutes without documentation) and SC-009 (a recipient shown the preview can correctly state what it will add). Both are human-judgement criteria that no automated test can stand in for. Add to spec 032's deferred-manual-pass table.

---

## Dependencies

```
T001 (the FR-027 gate) ─── blocks everything below
        │
Phase 1 (T003–T004)
        │
Phase 2 (T005–T012) ─── blocks all user stories
        │
        ├── Phase 3 / US1 (T013–T034) ─── MVP
        │        │
        │        ├── Phase 4 / US2 (T035–T038)   needs a share to revoke
        │        ├── Phase 5 / US3 (T039–T044)   needs members to moderate
        │        └── Phase 6 / US4 (T045–T047)   needs a preview and a receipt
        │
        └── Phase 7 (T048–T054)
```

**Within Phase 2**: T005 → T006 → T007 → T008; T009/T010 and T012 are
independent of the schema work only after T007 lands.

**Within US1**: T013–T017 (authoring) and T018–T021 (sharing) both depend on
Phase 2 but not on each other. T022 gates T023–T027; T023 and T024 are parallel.
T030 is parallel with all server work. T034 needs everything.

**US2, US3 and US4 are independent of each other** — all three depend only on
US1.

## Parallel opportunities

- **T001 and T002** — two ADRs, different files.
- **T009/T010 and T012** — the restriction check and the rate limiter share nothing.
- **T023 and T024** — ability/item copying and actor/lore copying.
- **T030** — web types and API client, against the contract, before the server exists.
- **T037, T045, T050, T051** — independent UI and test work.

## Implementation strategy

**MVP is Phase 3 (US1) plus Phase 4 (US2)**, and US2 is not deferrable behind
US1 despite the phase order. The spec is explicit: ADR-049's determination rests
on collections being owner-controlled and revocable, so a build that shares
before it can revoke has shipped the half that creates the liability without the
half that answers it. Treat T035–T038 as part of the first shippable increment.

**Then US3**, which cannot be built before there is something to moderate.
**Then US4**, which makes the feature pleasant rather than correct — a
collection that copies correctly but explains itself poorly is a worse product,
not a broken one.
