# A Shared Ability, Item or Actor Is Read Without an Account

- **Date**: 2026-09-06
- **Status**: Accepted
- **Spec**: `specs/026-content-collections/` (FR-009e, the follow-up it deferred)
- **Follows**: ADR-049 (share-link repository determination), ADR-069 (collection
  share determination), ADR-070 (the anonymous collection read path)
- **Governs**: Constitution Principle III — ownership and authorization at the
  data boundary

## The decision

`sharedAbility(shareCode)`, `sharedItem(shareCode)` and `sharedActor(shareCode)`
resolve **without a session**, on the same terms ADR-070 set for
`sharedCollection`. Anyone holding one of these links may see the artifact while
signed out. Copying still requires an account and authority in a destination
world.

This closes FR-009e. The product stops having two rules for the same act.

## Why this is a separate ADR from 070

ADR-070 made the case and then deliberately declined to apply it here:

> Cost 3 — inconsistency. The product will have anonymous collection shares and
> authenticated singleton shares at the same time. FR-009e leaves that standing
> on purpose: the argument above applies to the three shipped shares equally,
> but relaxing authentication on live share paths is a security change to
> features spec 026 does not otherwise touch, and it should be decided rather
> than inherited as a side effect.

So the argument was never in doubt — ADR-070 says outright that it "applies to
the three shipped shares equally". What was withheld was the decision, because
the change lands on three shipped, live paths rather than on a feature being
built. This ADR is that decision, taken on its own and not as a side effect of
building something else.

It is written separately rather than as an amendment to 070 so that the record
shows two acts: one feature choosing anonymity for itself, and a later,
deliberate choice to move three existing boundaries. Folding the second into the
first would make it look like the side effect ADR-070 refused to let it be.

## What changes, precisely

The three resolvers each carried this shape:

```rust
// Authenticated, but deliberately no membership check.
let _ = authenticated_user(ctx)?;
```

The session requirement goes; the discarded binding goes with it. In its place,
each resolver takes the same rate limiter `sharedCollection` uses, applied
**before** the lookup — for the reason ADR-070 gives, that an unguessable code
is unguessable only while the number of guesses is bounded.

Two consequences of ADR-070 that were written for collections now bind all four
paths, and are implemented here rather than left as prose:

- **One refusal for every failure.** Each module gets a single `UNAVAILABLE`
  constant, replacing repeated string literals. An unknown code, a revoked
  share, a deleted artifact and a moderated artifact must be indistinguishable,
  and four literals that happen to match today are not that guarantee — they are
  four chances to drift apart.
- **`AnonymousCaller` stops being a collections type.** It moves out of
  `mutations_collection_shares` to `graphql::anonymous`, because a newtype that
  four resolvers depend on is not owned by one of them.

## What this does not change

- **Copying still authenticates.** `copySharedAbilityToWorld` and its two
  siblings keep `authenticated_user(ctx)?` and their destination-authority
  checks. Viewing and copying are different acts (FR-009b), and this ADR moves
  only the first.
- **Moderation still blocks.** A moderated artifact refuses through the same
  sentence as a revoked one. A share was never a moderation bypass and is not
  one now.
- **Nothing becomes enumerable.** ADR-049's determination and ADR-069's both
  rest on reachable-by-code not being findable-by-anyone. No query added here
  lists shares by world, by user, or in aggregate. Widening *who* may present a
  code does not widen *what* can be found without one, and that distinction is
  what both determinations actually rest on.

## The second half of FR-009e: an owner can revoke after closing the tab

Spec 026 recorded a second defect in the same follow-up:

> Implementing FR-010 without it produced a revoke that only worked inside the
> page that minted the link: with no read path, closing the tab permanently
> removed the owner's ability to revoke. The three shipped single-artifact
> shares have the same defect today.

Collections answered it with `collectionShareLink(collectionId)`. The three get
the equivalent — `abilityShareLink`, `itemShareLink`, `actorShareLink` — each
authenticated, each scoped to one artifact the caller already has authority
over, each returning the active share or null.

This matters more once viewing is anonymous, not less. A link that reaches the
public and cannot be recalled by its owner is the failure mode ADR-049's
ownership model exists to prevent: the world owner owns what they author, and
ownership without a revoke that survives a closed tab is a claim rather than a
control.

These queries are not the enumeration FR-020 forbids, for the reason
`collection_share_link_impl` already records: they are scoped to a single
artifact the caller can already read, and add no surface from which anything
can be discovered.

## Consequences

- None of the three `shared*` resolvers may call `authenticated_user(ctx)?`.
  As with ADR-070, a future reader "restoring consistency" by adding one back
  would be reverting a decision — but the consistency now runs the other way,
  and all four paths agree.
- Every one of the four anonymous reads must rate-limit before its lookup. A new
  share type added later inherits the requirement, not the exemption.
- The three share-view pages must render for a signed-out visitor. A resolver
  that answers anonymously behind a route that redirects to login is the same
  wall in a different place.
- ADR-070's Cost 3 is discharged. Its text stands as written — it was correct at
  the time — and this ADR is the follow-up it named.
