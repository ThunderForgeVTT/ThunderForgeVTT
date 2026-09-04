# Feature Specification: Content Collections

**Feature Branch**: `026-content-collections`

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

## The name, and why it is not "pack"

**Resolved 2026-09-04.** This document said "content collection" until spec 032 gave
the word a closed, two-member definition, enforced by a directory:
`packs/systems/README.md` states that "a pack is a system pack or an interface
pack, never both (FR-002), and the directory it lives in is what decides." A
system pack is a ruleset; an interface pack is a look. Both are authored in
this repository, reviewed here, and — per ADR-029 — compiled into the product.

What this feature describes is none of that. It is authored by a *user*, inside
a *world*, at *runtime*, and shared by link to strangers. It lives in the
database, not in `packs/`. Every property that makes a system pack safe to
execute is a property this does not have. Keeping the word would have made
FR-002 false as written and made the author-facing pack READMEs — which promise
you need not read the source — lie about what "pack" means.

So: **collection**. `content_collections` as a table, `/collection/<code>` as a
URL, "share this collection" in the interface.

**"Bundle" was considered and rejected for a specific reason worth recording,**
so nobody proposes it again. `bundle` appears nowhere in this codebase as a
domain noun — only as **"bundled"**, which carries ADR-029's load-bearing
distinction between code compiled into the product (and therefore trusted to
run) and code that is not. A "bundle" that is emphatically not bundled would be
the most confusing name available. "Compendium" was likewise unavailable: it is
already the world's browsable catalogue at `/world/:id/compendium`.

The rest of this document is unchanged in substance. Only the noun moved.

## The idea in one paragraph

Today a share link points at exactly one artifact — one ability, one item, one
actor. A **content collection** would let a user correlate many artifacts (items,
actors, scenes, abilities, lore entries) into a single named, **versioned**
bundle and share that collection as a unit. A recipient could copy the whole collection
into a world of their own in one action, rather than copying a dozen artifacts
one link at a time.

## Why it is worth capturing now

Two reasons, both about not losing context:

1. **It is the natural next shape of sharing**, and spec 025 built the
   single-artifact version of exactly this machinery (share code, read-only
   preview, deep copy into a destination world, revocation). A collection is that
   mechanism with a different unit, not a new subsystem.
2. **It inherits a governance position that took real work to establish.**
   Writing it down here means the next person does not have to rediscover it.

## Inherited constraints (non-negotiable, from ADR-049)

Any collection design MUST satisfy all of these. They are not defaults to be
revisited — they are the conditions the platform's DMCA determination rests on.

- **Non-shared by default.** A collection leaves its world only by an explicit user
  action.
- **Non-discoverable by default.** Reachable only by possessing its link.
- **No enumeration.** No query lists collections — by world, by user, or globally.
- **Unguessable codes**, v4-derived (never v7 — a v7 UUID front-loads a
  timestamp, which both narrows the search space and leaks creation time).
- **Owner-controlled and revocable**, with a distinct "no longer available"
  state.
- **Takedown-effective.** A moderated artifact inside a collection must not be
  reachable through the collection. This is the one that needs genuine design thought
  rather than copying: a collection is exactly that — a *set of members* — so
  "one member is disabled"
  needs a defined behaviour — omit that member, block the whole collection, or
  something else.
- **Copy is a one-time deep copy** producing independent records with no
  referential link back to the source.

### And one that does not carry over

**ADR-049 does NOT automatically cover collections.** Bundling changes the unit of
distribution, which is material enough to require its own FR-012 review under
spec 015 before collections ship. Do not treat spec 025's determination as
pre-approval.

## Content ownership (from ADR-049)

The world owner owns what they author; the platform hosts it and reserves the
right to forward a DMCA notice to the world owner responsible. If you create it,
you own it — if you copied it from a source you do not hold rights to and shared
it on, that is a problem that can be forwarded to you.

Collections sharpen this, because a collection makes it *easy* to bundle and redistribute a
lot of material at once. Whatever the eventual design, the moment of collection
creation or sharing is the natural place to restate that responsibility.

## Open questions (not answered here)

- **Versioning semantics.** What does a new collection version mean for someone who
  already copied v1? Nothing (copies are independent, matching today's
  single-artifact behaviour), or is there an update path? The former is
  consistent with ADR-049's one-time-deep-copy invariant; the latter is a
  genuinely new distribution model and would need its own review.
- **Partial copy.** Can a recipient take some members of a collection, or is it
  all-or-nothing?
- **Cross-type references.** An actor in a collection may know abilities and carry
  items. Does the collection pull those in automatically, and what happens when a
  referenced artifact is not included?
- **Moderation of a collection.** See the takedown-effective constraint above.
- **Scenes specifically.** Scenes carry background image assets in object
  storage, unlike every artifact type shared today. This was written as the
  most frightening open question here, and it is now the least — because
  `src/server/src/storage/dedupe.rs` (2026-09-03) already solved the hard half
  for a different reason.

  Dedupe stores one copy of any given image however many rows refer to it, and
  the lookup is deliberately **instance-wide**: measured against the dev
  database, 3,815 of 4,387 canvas assets shared bytes with a row in a
  *different world*. The safety argument it makes is exactly the one a
  cross-world copy needs — each asset keeps its own row, with its own
  `asset_id`, `world_id`, `scene_id` and owner, and `canvas_assets_serve`
  authorises against the row it looked up before reading the path that row
  names. Two worlds pointing at one object are still two independent
  permission checks.

  So copying a scene into a recipient's world means **writing a new asset row
  that names the same `storage_path`** — not copying bytes, and not
  re-referencing the source's row. That satisfies ADR-049's one-time-deep-copy
  invariant (the records are independent) without the storage cost the
  invariant appears to imply.

  **The constraint that comes with it**, and it is a hard one: `dedupe.rs`
  states that nothing in this product deletes stored objects, and that this is
  what makes a shared path safe — a reference cannot dangle when references are
  never dropped. A revocable, versioned collection is precisely the feature that
  invites someone to add deletion. **Object deletion must become
  reference-counted before this ships anything that deletes**, asked inside the
  same transaction that removes the row. Otherwise revoking a collection silently
  blanks a scene background in a world nobody touched, and the failure surfaces
  far from its cause.

## Explicitly NOT this feature

A **public registry / browsable collection marketplace**. ADR-049 records that as a
future consideration only, gated on substantial demonstrated demand and a fresh
FR-012 review. Collections as specified here are shared by link, exactly like
singletons. Do not let "collections" become a registry by increment.

## Prior art in this repo

- `specs/025-world-abilities-compendium/contracts/ability-share.md` — the
  single-artifact share contract this would generalize.
- `src/server/src/graphql/mutations_item_shares.rs` — the shipped
  implementation, including `generate_share_code`, the `CopyError` orphan-rule
  workaround, and the transactional deep-copy path.
- `docs/adrs/20260825-049-share_link_dmca_repository_determination.md` — the
  governing determination.
- `src/server/src/storage/dedupe.rs` — instance-wide image dedupe, and the
  written-down reason a shared `storage_path` is safe only while nothing
  deletes objects. Read its header before designing scene copying or collection
  revocation.
- `specs/032-collection-architecture/` and `collections/systems/README.md` — what "collection"
  means in this repository now, and why this document's use of the word is a
  collision rather than an extension. ADRs 059 through 066 are that
  architecture's record.

## What has changed since this stub was written

Kept as a list, because a stub's whole job is to still be true when someone
returns to it.

- **2026-09-02 to 09-04, spec 032 and ADRs 059–066.** "Collection" acquired a closed
  definition (see the top of this document), collections gained a server crate
  (ADR-063), the right to own their own tables, and a declared web surface
  (ADR-066). None of that machinery is reusable here — a content collection is
  user-authored runtime data, not compiled code — but the *name* is now
  contested, and the collection READMEs are the author-facing contract that has to
  stay honest.
- **2026-09-03, `storage/dedupe.rs`.** Shrinks the scene-asset question and
  adds the reference-counting constraint above.

Everything else in this document still holds: ADR-049's constraints are
unchanged, the single-artifact share machinery is still the thing this
generalizes, and this is still a stub.
