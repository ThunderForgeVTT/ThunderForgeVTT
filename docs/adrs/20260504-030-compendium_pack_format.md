# ADR-030: Compendium Pack Format

**Status:** Open — genuinely undecided, unlike its neighbours

**Decision Date:** 2026-05-04 (stub) · **Reviewed:** 2026-09-04

**Related:** `specs/026-content-collections/` (the live home of this idea), ADR-049
(the governing DMCA determination), ADR-026 (superseded — the *other* pack
question, which is closed)

## Why this file still says nothing, and should

Reviewed on 2026-09-04 alongside ADRs 025 and 026, which were empty stubs whose
questions had quietly been answered elsewhere. This one is different: it is
empty because the decision has not been made, not because it was made
somewhere else. That distinction is the reason to write this paragraph rather
than either fill the file in or delete it.

## What the question is

A format for bundling authored *content* — items, actors, scenes, abilities,
lore entries — so it can be distributed as a unit. Not a system pack (a
ruleset) and not an interface pack (a look): see ADR-026 for why those two are
closed and this is not one of them.

## Where the thinking lives

`specs/026-content-collections/spec.md`, which is an honest stub rather than a spec.
It carries what has actually been established:

- **Seven inherited constraints from ADR-049**, non-negotiable, because the
  platform's DMCA determination rests on them: non-shared and non-discoverable
  by default, no enumeration, v4-derived unguessable codes (never v7),
  revocable with a distinct unavailable state, takedown-effective through the
  collection, and copy as a one-time deep copy.
- **ADR-049 does not pre-approve packs.** Bundling changes the unit of
  distribution, which needs its own FR-012 review under spec 015 before
  anything ships.
- **A naming collision** with spec 032's closed definition of "pack", which
  must be resolved before the word reaches a table, a GraphQL type or a URL.
- **A storage constraint discovered in `storage/dedupe.rs`**: copying a scene
  can share a `storage_path` rather than duplicating bytes, but nothing in this
  product deletes stored objects, and a revocable bundle is exactly the feature
  that invites deletion. Reference counting comes first.

## Why it is not being decided now

Because a format is the last thing to decide, not the first. The open questions
in spec 026 — versioning semantics, partial copy, cross-type references, and
what a moderated member does to its collection — all constrain the format, and
answering the format first would be picking a shape and then discovering what
it cannot carry.

**Do not fill this file in without a spec.** An ADR that records a format
nobody has specified is how a stub becomes a constraint by accident.
