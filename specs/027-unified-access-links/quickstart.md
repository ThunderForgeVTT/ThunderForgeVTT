# Quickstart: Unified World Access Links & Consolidated Permission Resolution

Runnable validation scenarios proving the feature works end to end. Each maps
to a user story and its success criteria. Details live in
[data-model.md](./data-model.md) and [contracts/](./contracts/) — this is a
run/verify guide, not an implementation guide.

## Prerequisites

```bash
# From repo root. Containers must be up.
docker compose up -d postgres rustfs

# Migrations — includes this feature's additive world_invites change
diesel migration run

# DATABASE_URL comes from the repo-root .env and is NOT auto-loaded by
# cargo test. Without it every DB-backed test fails with "DATABASE_URL must
# be set" — an environment error, not a code failure.
set -a && source .env && set +a
```

## Running the checks

```bash
# Server — primary coverage for both parts
cargo test -p thunderforge invite
cargo test -p thunderforge permission
cargo test -p thunderforge              # full suite; must stay green

# Frontend
cd apps/web
npx tsc --noEmit --ignoreDeprecations 6.0   # baseline has pre-existing unrelated errors
npx vitest run
npx vite build

# e2e — no Bevy canvas surface in this feature, so it escapes the documented
# "headless Chromium can't render the canvas" limitation.
npx playwright test e2e/access-links.spec.ts --workers=1
```

> **After any schema change, rebuild the dev backend before running e2e.** A
> stale binary predating the migration produces failures that look like code
> bugs and are not. This has cost real debugging time on this project before.

Dev stack for manual verification:

```bash
pnpm dev     # or: cargo run -p thunderforge  +  pnpm -F @thunderforge/web dev
```

---

## Scenario 1 — Kill a leaked link (US1, SC-001)

1. As a GM, open a world's campaign settings and generate an invite link.
2. From a second account, join with it. **Verify** it works — this is the
   control for step 4.
3. Copy the code, then hit **refresh** on that link in the panel.
4. From a third account, attempt to join with the **old** code → refused, on
   the very first attempt, with no grace window (SC-001).
5. Join with the **new** code → succeeds.
6. **Verify** the account from step 2 is still a member and unaffected
   (FR-005) — rotation governs future joins, never retroactive removal.
7. **Verify the inheritance** (FR-014): the replacement shows the same use cap
   and expiry as the original, with the count back at 0.
8. Rotate a link that is already **expired** → a usable link is issued
   (US1 scenario 4).
9. Attempt to rotate an already-**revoked** link → refused.
10. As a non-DM member, attempt rotate and revoke → both refused (FR-008).

## Scenario 2 — Removal actually removes (US2, SC-008)

**The leak check.** This fails on `main` today, at step 4.

1. As GM, add a member and grant them **Editor** on one actor, one item, one
   lore entry, and one **ability**.
2. Remove them from the world.
3. Inspect the four grant tables directly — not the UI. **Verify** zero rows
   remain for that user in that world.
4. ⚠️ **This is the bug**: before the fix, the ability grant survives.
5. Re-invite them and have them rejoin. **Verify** they hold no elevated
   rights on any of the four, and see exactly what a new member sees.
6. **Verify isolation**: a grant that same user holds in a *different* world is
   untouched (US2 scenario 3).
7. Remove a member holding **zero** grants → succeeds quietly, no error.

## Scenario 3 — Link state is legible (US3, SC-004)

1. Create four links and drive each into a distinct state: active, expired
   (set a past expiry), exhausted (consume the cap), revoked.
2. **Verify** the panel reports each distinctly, with the reason — not a bare
   "3/10 uses" string, and never a revoked link rendered as if it works.
3. **Verify** remaining uses and expiry are shown where set.
4. Let a link expire while the panel is open, then refetch → now reports
   expired.
5. **Verify** revoked links remain listed (a GM should see what they retired).

## Scenario 4 — Unusable links fail identically (US4, SC-005)

1. Attempt to join with codes that are **unknown**, **expired**, **exhausted**,
   and **revoked**.
2. **Verify all four responses are indistinguishable** — same message, same
   error extensions, same shape. A dead code must reveal nothing about whether
   it was ever real.
3. As an existing member, open a **valid** link for that same world → the
   distinct already-a-member message.
4. **Verify no use was consumed** by step 3 (data-model §4) — a repeat click
   must not burn the cap.
5. **The concurrency check** (FR-012): drive two simultaneous joins at a link
   with **one** use remaining → exactly one succeeds; the other gets the
   uniform failure. Assert the final `used_count` is exactly the cap, never
   over.
   - ⚠️ This fails on `main` today: the current read-validate-write sequence
     loses updates and admits both (research §5).

## Scenario 5 — Codes and back-compat (SC-006, SC-007)

1. Generate a new link → **verify the code is 20 characters** (FR-006).
2. **Verify it is not time-derived**: generate many in rapid succession and
   confirm no ordering pattern and no collisions. Deriving from a v7 UUID
   caused a real unique-index collision here before (spec 005); this guards
   the regression.
3. **The migration check** (SC-006): with an 8-character code created *before*
   the migration, join with it → **succeeds**. Existing links are grandfathered,
   never force-rotated.
4. **Verify** the pre-migration row reads as `ACTIVE`, not revoked — the
   `DEFAULT FALSE` is what makes this true.

## Scenario 6 — Consolidation changes nothing (US5, SC-003)

The unusual one: this scenario passes by **absence** of change.

1. Run the full server suite before and after the consolidation.
   **Verify no existing test's expected outcome was edited** to accommodate it.
   An edit there means behaviour moved — investigate rather than accept.
2. **Verify parity across types**: identical conditions on an actor, item, lore
   entry, and ability resolve to identical rights (US5 scenario 1).
3. **Verify the DM shortcut**: a DM with zero explicit grants anywhere holds
   Owner on all four, and it cannot be removed (US5 scenario 2).
4. **Verify the separate axis** (FR-019, US5 scenario 3): a member with
   **Editor** on a **GM-only** ability still cannot see it. Rights and
   visibility are evaluated independently.
5. **Verify preserved edge behaviours** — each is currently relied upon:
   - an unparseable `level` string falls back to Viewer, not an error
   - `is_admin` short-circuits to Owner
   - multiple simultaneous Owners are all accepted
   - a missing content row errors; a missing grant row does not
6. **The structural check** (SC-002): confirm the cleanup test derives its
   type list from the declaration rather than hardcoding four. A test that
   restates the list cannot catch the omission it exists to prevent.
7. Confirm `lore_permissions.rs`'s `pub use is_dm_of_world` shim is gone and
   the function resolves from `auth/world_membership.rs`.

---

## Definition of done

Status as of 2026-08-26 — verified where marked, with gaps named rather than
quietly ticked.

- [x] `cargo test -p thunderforge` green — **361 passed, 0 failed** (332 before
      this feature; +29 new, zero regressions). `thunderforge_core` 14/14.
- [x] The two live defects have regression tests that failed before the fix:
      the ability-cleanup gap returned `(0, 0, 0, 1)`, and the concurrent-join
      race admitted more than one member against one remaining use.
- [x] **SC-003 holds.** Exactly one pre-existing assertion changed across the
      whole branch — `invite_code.len()` from 8 to 20, the deliberate FR-006
      change, documented in place. The two relocated `is_dm_of_world` tests
      show no diff at all; git matched them as unchanged.
- [x] **SC-002 is structural.** `purge_covers_every_declared_entity_type`
      asserts against `DECLARED_ENTITIES`, derived from the declaration, so a
      fifth content type declared without cleanup fails there.
- [x] Frontend unit tests green — 48/48, including the link-state matrix and
      the fail-closed case for an unrecognised state.
- [x] `tsc` and `vite build` clean. tsc reports 6 errors, all pre-existing and
      in unrelated files (down from 8 — this feature removed two by deleting a
      dead `@apollo/client` module).
- [x] Zero compiler warnings from new code; the one remaining warning
      (`queries/ability.rs:290`) predates this feature.
- [x] Migration has paired `up.sql`/`down.sql`, and `down.sql` was verified by
      running it and back up against **58 real rows**, including a revoked row
      and a self-referential lineage row. All survived.
- [x] SC-006 verified: 8-character codes created before the migration still
      join, and read as `ACTIVE`.
- [x] SC-007 verified: new codes are 20 characters, with a rapid-succession
      test asserting no ordering pattern and no collisions.
- [x] ADR-050 written and Accepted, covering the macro-over-polymorphic
      decision and the access-link model.
- [x] No GM-facing copy describes the use cap as a security control; a unit
      test asserts it.

### e2e: passes, with a pre-existing flake

`e2e/access-links.spec.ts` (3 tests) passes cleanly on roughly two runs in
three. The failures are always the same stall — `/register`'s lazy chunk never
resolving under the Vite dev server — and are **not caused by this feature**:
`e2e/actor-share.spec.ts`, untouched here, fails identically on the same
locator at the same rate when run alongside. This is the same class of
dev-server flake spec 025 recorded for the compendium route, and it deserves
its own fix rather than being papered over here.

The per-file timeout is raised to 120s because these tests genuinely register
two or three accounts each; hitting the 30s default produced
"locator.fill: Test ended" failures that read like product bugs.

### Found while implementing, fixed here

Three leaks that the spec did not anticipate, each now covered by a test:

- **`worldByInviteCode` disclosed revoked links.** It resolves a code without
  joining and checked expiry and exhaustion but never revocation, so a revoked
  code still returned the world's name and description.
- **`alreadyMember` errored on unknown codes.** The join page requests it
  alongside `worldByInviteCode` in one operation, so an unknown code produced a
  GraphQL error while a revoked one did not — the two rendered differently,
  defeating the uniform-failure design. Caught only because the e2e compares
  the two rendered pages rather than checking each in isolation.
- **A test helper still generated codes from a v7 UUID** — the exact collision
  spec 005 fixed in production code, still live in test code.

### Known, not fixed

- **The `/register` lazy-chunk flake** described above. Pre-existing,
  reproduced on an untouched spec, out of scope here.
- **`useWorldInvites` has no live push transport.** A rotation performed in
  another session will not appear until refetch or remount. Documented in the
  hook; this feature refetches explicitly after mutating rather than pretending
  a subscription exists.
- **The use cap is not enforcement.** Rotation resets the count, so a DM can
  rotate indefinitely. Accepted (a DM can already create unlimited links) and
  recorded in ADR-050 and spec Edge Cases.
- **`max_uses = 0` (unlimited) stays unreachable via the API.** The model and
  the SQL predicate both honour it; no creation path exposes it. Removing the
  branch is a behaviour change outside this spec.
- **The unparseable-`level` fallback is unreachable through the database** —
  every grant table declares `CHECK (level IN ('Viewer','Editor','Owner'))`.
  Asserted on the parsing function instead, and kept as defence in depth.
