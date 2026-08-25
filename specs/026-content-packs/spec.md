# Feature Specification: Content Packs

**Feature Branch**: `026-content-packs`

**Created**: 2026-08-25

**Status**: Stub — not specified, not scheduled

**Input**: Direction recorded during spec 025's DMCA determination
(`docs/adrs/20260825-049-share_link_dmca_repository_determination.md`): "for now
content is shared in singletons or in packs (packs are a new concept we haven't
designed for) — the idea is that users can correlate items, actors, scenes, the
works into a pack and share the pack with versioning, but that's later work."

---

## ⚠️ This is a stub

Nothing here is specified. It exists so the concept is captured with its
constraints attached, rather than being reinvented later without them. It is
**not** ready for `/speckit-plan`; run `/speckit-specify` properly when the work
is actually scheduled.

## The idea in one paragraph

Today a share link points at exactly one artifact — one ability, one item, one
actor. A **content pack** would let a user correlate many artifacts (items,
actors, scenes, abilities, lore entries) into a single named, **versioned**
bundle and share that bundle as a unit. A recipient could copy the whole pack
into a world of their own in one action, rather than copying a dozen artifacts
one link at a time.

## Why it is worth capturing now

Two reasons, both about not losing context:

1. **It is the natural next shape of sharing**, and spec 025 built the
   single-artifact version of exactly this machinery (share code, read-only
   preview, deep copy into a destination world, revocation). A pack is that
   mechanism with a different unit, not a new subsystem.
2. **It inherits a governance position that took real work to establish.**
   Writing it down here means the next person does not have to rediscover it.

## Inherited constraints (non-negotiable, from ADR-049)

Any pack design MUST satisfy all of these. They are not defaults to be
revisited — they are the conditions the platform's DMCA determination rests on.

- **Non-shared by default.** A pack leaves its world only by an explicit user
  action.
- **Non-discoverable by default.** Reachable only by possessing its link.
- **No enumeration.** No query lists packs — by world, by user, or globally.
- **Unguessable codes**, v4-derived (never v7 — a v7 UUID front-loads a
  timestamp, which both narrows the search space and leaks creation time).
- **Owner-controlled and revocable**, with a distinct "no longer available"
  state.
- **Takedown-effective.** A moderated artifact inside a pack must not be
  reachable through the pack. This is the one that needs genuine design thought
  rather than copying: a pack is a *collection*, so "one member is disabled"
  needs a defined behaviour — omit that member, block the whole pack, or
  something else.
- **Copy is a one-time deep copy** producing independent records with no
  referential link back to the source.

### And one that does not carry over

**ADR-049 does NOT automatically cover packs.** Bundling changes the unit of
distribution, which is material enough to require its own FR-012 review under
spec 015 before packs ship. Do not treat spec 025's determination as
pre-approval.

## Content ownership (from ADR-049)

The world owner owns what they author; the platform hosts it and reserves the
right to forward a DMCA notice to the world owner responsible. If you create it,
you own it — if you copied it from a source you do not hold rights to and shared
it on, that is a problem that can be forwarded to you.

Packs sharpen this, because a pack makes it *easy* to bundle and redistribute a
lot of material at once. Whatever the eventual design, the moment of pack
creation or sharing is the natural place to restate that responsibility.

## Open questions (not answered here)

- **Versioning semantics.** What does a new pack version mean for someone who
  already copied v1? Nothing (copies are independent, matching today's
  single-artifact behaviour), or is there an update path? The former is
  consistent with ADR-049's one-time-deep-copy invariant; the latter is a
  genuinely new distribution model and would need its own review.
- **Partial copy.** Can a recipient take some members of a pack, or is it
  all-or-nothing?
- **Cross-type references.** An actor in a pack may know abilities and carry
  items. Does the pack pull those in automatically, and what happens when a
  referenced artifact is not included?
- **Moderation of a collection.** See the takedown-effective constraint above.
- **Scenes specifically.** Scenes carry background image assets in object
  storage, unlike every artifact type shared today. Copying a scene means
  copying or re-referencing binary assets, which the current share machinery
  does not do at all.

## Explicitly NOT this feature

A **public registry / browsable pack marketplace**. ADR-049 records that as a
future consideration only, gated on substantial demonstrated demand and a fresh
FR-012 review. Packs as specified here are shared by link, exactly like
singletons. Do not let "packs" become a registry by increment.

## Prior art in this repo

- `specs/025-world-abilities-compendium/contracts/ability-share.md` — the
  single-artifact share contract this would generalize.
- `src/server/src/graphql/mutations_item_shares.rs` — the shipped
  implementation, including `generate_share_code`, the `CopyError` orphan-rule
  workaround, and the transactional deep-copy path.
- `docs/adrs/20260825-049-share_link_dmca_repository_determination.md` — the
  governing determination.
