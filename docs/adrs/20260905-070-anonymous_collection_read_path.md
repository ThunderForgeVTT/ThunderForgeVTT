# A Collection's Preview Is Read Without an Account

- **Date**: 2026-09-05
- **Status**: Accepted
- **Spec**: `specs/026-content-collections/` (FR-009a, FR-009b, FR-009c, FR-009d, FR-009e)
- **Follows**: ADR-049 (share-link repository determination), ADR-069 (collection share determination)
- **Governs**: Constitution Principle III — ownership and authorization at the data boundary

## The decision

`sharedCollection(shareCode)` resolves **without a session**. Anyone holding a
collection's link may see what is in it while signed out. Copying still requires
an account and authority in a destination world.

## Why this needs an ADR rather than a requirement alone

Principle IV requires an ADR when a change moves an established ownership or
access boundary, and this moves one. It is not a new feature behaving in a new
way; it is a departure from how three shipped features behave.

## What shipped before this, precisely

Spec 025's three share queries are **authenticated**:

```rust
// mutations_ability_shares.rs — and the same in the item and actor modules
async fn shared_ability(&self, ctx: &Context<'_>, share_code: String) -> ... {
    let state = app_state(ctx)?;
    // Authenticated, but deliberately no membership check.
    let _ = authenticated_user(ctx)?;
```

The comment is exact about what it waives: **membership**, not authentication. A
share link today reaches any signed-in user; it does not reach the public.

**Spec 026's clarification got this wrong, and the record says so.** FR-009a was
chosen on 2026-09-04 on the stated grounds that anonymous viewing "matches
spec 025's existing share behaviour". It does not. The error was found during
Phase 0 research by reading the resolvers, and the spec was corrected in place —
FR-009a now records the correction — rather than the premise being quietly
dropped.

## Why the decision stands anyway

The false premise was a supporting argument, not the substance. Re-examined on
its own:

- **The feature's purpose requires it.** A collection is shared with a friend
  running their own game. Requiring an account to *look* puts a signup wall in
  front of content its owner deliberately published, and the most common
  recipient — someone deciding whether this platform is worth joining — is
  exactly who it turns away.
- **A session is not what protects the content.** What protects it is that the
  code is ~80 bits of v4-derived entropy with no timestamp (FR-008), and that
  the owner can revoke it at any time (FR-010). An attacker who cannot guess a
  code is not stopped harder by also needing a free account, and one who
  *can* guess a code is not stopped by one either.
- **Authorization is not relaxed — only authentication.** The anonymous path
  returns exactly one collection, the one whose code was presented, whose owner
  explicitly shared it. It reveals nothing about the source world (FR-009,
  FR-009d), and it writes nothing.

## What this costs, and what pays for it

**Cost 1 — the guessing surface is now open to everyone.** Answered by FR-009c:
a rate limiter of this feature's own, keyed on the caller, applied before the
lookup. The existing `rate_limit_auth_requests` cannot serve — it keys on the
request path and returns early outside `/authentication/`, and every GraphQL
operation in this product arrives at one path. The new limiter deliberately does
**not** honour `THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT`: the e2e harness sets that
on every run, and a limiter switched off during the test written to prove it is
a limiter nobody tests.

**Cost 2 — existence becomes probeable.** Answered by FR-009d: an unknown code,
a revoked share and a deleted collection return the same sentence. Distinguishing
them is a probe, so they are not distinguished.

**Cost 3 — inconsistency.** The product will have anonymous collection shares
and authenticated singleton shares at the same time. FR-009e leaves that
standing on purpose: the argument above applies to the three shipped shares
equally, but relaxing authentication on live share paths is a security change to
features spec 026 does not otherwise touch, and it should be decided rather than
inherited as a side effect. Recorded as a follow-up, not as an oversight.

## Consequences

- `sharedCollection` must not call `authenticated_user(ctx)?`. A future reader
  "restoring consistency" with the other three share queries would be reverting
  a decision, not fixing an omission — this ADR is why.
- `copySharedCollectionToWorld` **must** authenticate and check destination
  authority (FR-009b, FR-016). Viewing and copying diverge at exactly that call.
- ADR-069's determination depends on this being bounded: reachable-by-code is
  not findable-by-anyone. Making collections findable would invalidate that
  determination, not merely widen this one.
