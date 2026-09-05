# Quickstart: Content Collections

**Feature**: `026-content-collections` · **Date**: 2026-09-04

How to prove this feature works, end to end. Every scenario below maps to a
success criterion; none of them is satisfied by a unit test alone.

---

## Prerequisites

```bash
# One stack, with the auth rate-limit bypass — reusing a dev stack for e2e
# without it produces 429s that masquerade as product bugs.
THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1 pnpm dev
```

That variable disables the **auth** limiter only. This feature's collection
limiter deliberately does not honour it (contracts, `sharedCollection`), so the
FR-009c scenario below still exercises a live limiter.

Before a run: confirm the tree is clean and `ss -ltn | grep 301` is empty. Stale
backends squatting the shard ports have twice produced what looked like fresh
regressions.

---

## Server checks

```bash
cargo test -p thunderforge-server collection    # this feature's unit tests
cargo test -p thunderforge-server               # the full suite, ~664 before this feature
cargo check                                     # native; the engine crate is wasm-only
```

---

## Scenario 1 — Gather, share, copy (US1, SC-001/002/003/004)

1. Sign in, create a world, author one of each: item, actor, ability, lore
   entry, scene with a background image.
2. Create a collection, add all five.
3. Share it; copy the link.
4. **In a different browser context, signed out**, open the link. Expect the
   five members previewed and **no sign-in wall** (FR-009a). Expect nothing
   naming the source world (FR-009, SC-007a).
5. Sign in as a second user with their own world; copy.
6. **Expect**: all five present; the scene renders its background (SC-008a);
   the actor still knows the copied ability, not the source's (FR-014).
7. Edit a copy; confirm the source is unchanged, and the reverse (SC-004) —
   for **every** type, not a sample.

---

## Scenario 2 — Revoke (US2, SC-005)

1. Share a collection, confirm the link opens, copy it as a recipient.
2. Revoke. Confirm the interface says copies already made are unaffected
   **at the moment of revoking** (FR-011).
3. Reload the link: "no longer available", not an error page (FR-010).
4. Confirm the recipient's copies are intact (SC-005).
5. Delete a different collection outright; confirm its link behaves the same
   way, and is not distinguishable from a code that never existed (FR-009d).

---

## Scenario 3 — Takedown reaches one member (US3, SC-010)

1. Share a collection of four members.
2. File a takedown against one via the moderation path
   (`submit_takedown_notice_impl` — the shipped ability-share test is the
   worked example).
3. **Expect**: the collection still opens; three members shown; "something has
   been withheld" with **no name** (FR-022); a copy made now creates three
   records, not four (FR-021).
4. Take down the remaining three. Expect "nothing is available", not an empty
   collection presented as complete (FR-024).
5. Reverse one takedown. Expect it back **without rebuilding the collection**
   (FR-025) — this should need no new code, because `effective_status` restores
   lazily and no status is cached.

---

## Scenario 4 — Restricted members are refused (SC-003a)

For **each** of actor, item, ability, lore entry (grant rows) and scene
(`hidden`), plus `gm_only` on abilities and items:

1. Restrict the artifact to a subset of the world's members.
2. Attempt to add it to a collection. **Expect a refusal naming the reason**
   (FR-001a).
3. Then the reverse: add an unrestricted artifact, restrict it afterwards,
   reopen the share. **Expect it withheld** (FR-001b) — this is the case that
   fails if the check runs only at add time.

SC-003a says verified across every type rather than sampled. Both halves, all
types.

---

## Scenario 5 — The limit (FR-005a, SC-002a)

1. Add 100 members. Expect the 101st refused, with a message naming the limit.
2. Copy the 100-member collection. Expect it to complete as **one action the
   recipient waits out** — not a background job (SC-002a). Time it.

---

## Scenario 6 — The anonymous read path holds (FR-009c/d, SC-007a)

1. Signed out, request `sharedCollection` with a wrong code repeatedly. Expect
   the limiter to refuse before the guessing is unbounded.
2. Confirm a nonexistent code, a revoked share and a deleted collection all
   return the **same** sentence (FR-009d).
3. Confirm no response anywhere in the anonymous path carries a world id, a
   world name, or a member list of the source world.

---

## Scenario 7 — Storage is not duplicated (SC-008)

```sql
-- Before and after copying a collection containing a scene whose background
-- the platform already stores.
SELECT count(DISTINCT storage_path) FROM canvas_assets;
```

Expect the count **unchanged**. A new asset row is expected; a new
`storage_path` is not.

---

## Full sweep

```bash
pnpm verify                       # all ten checks
pnpm --filter web test            # web unit
pnpm --filter web exec playwright test --workers=1
```

A sweep failure that does not reproduce in isolation is not yet a finding, and
when re-running to check, re-run both sides under the same conditions.
