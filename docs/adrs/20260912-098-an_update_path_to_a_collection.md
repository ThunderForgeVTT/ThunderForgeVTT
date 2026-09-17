# ADR-098: An Update Path to a Collection

**Date:** 2026-09-12
**Status:** **ACCEPTED, with conditions**, by the accountable owner, 2026-09-13. A versioned collection with no path to adopted copies is **not** a centralized public repository. Sync-back ships only after spec 015 T042 closes, and only while the three conditions below hold.
**Participants:** ThunderForgeVTT Team
**Amends:** ADR-069 (a link-shared collection is not a centralized public repository)
**Related:** spec 050 (FR-100 … FR-105, decision 5), spec 026, spec 015 (notice and takedown), ADR-079, ADR-097, the constitution's DMCA / Content Moderation Guardrail
**Blocks:** spec 050 sync-back (spec 049 tasks.md phase 15)

---

## Problem Statement

Spec 050 lets a Game Master **sync a change back** to a collection on their own
shelf: an improvement made in one world reaches the library, and the worlds
they start afterwards get it. It is what stops the delta model becoming a trap
where work done in a world is stuck in that world forever.

ADR-069 determined that a link-shared collection is **not** a "centralized
public repository" under spec 015's policy, with one accepted risk. Its own
"what this determination does not cover" section says:

> **Versioned collections or any update path to already-copied content.** Spec
> 026 places these out of scope; an update path is a genuinely new distribution
> model and **re-opens this determination.**

Spec 050 FR-104 gives a collection versions, and FR-100 gives it an update
path from a world. On the plain wording above, **this is the case ADR-069 says
it does not cover.**

The constitution's DMCA guardrail therefore applies, and requires two things
before implementation begins:

1. the notice-and-takedown programme is operational;
2. an explicit, on-record determination of whether the proposed feature
   constitutes a centralized public repository — and if so, redesign or an
   explicit acceptance of the risk by an accountable owner.

## What is *not* in question

Everything else in specs 049 and 050 is clear of this. Uploaded content is
structurally incapable of leaving the account that imported it (ADR-097), and
049 FR-054a makes it unable to enter the shared-collection path ADR-069
reasoned about at all. The reader, the review, the send, the shelf, the book
list and the deltas engage none of it.

This ADR is about **one phase**, and it is deliberately the last one, so that a
refusal or a delay costs that feature and nothing before it.

## The question, stated so it is not answered by accident

ADR-069's limit names two things with an "or" between them. They are not the
same, and sync-back may trigger only one.

**Limb 1 — "an update path to already-copied content."** Does a sync-back reach
copies other people have already adopted?

Spec 026's model says an adopted copy is **independent**: `collections/copy.rs`
deliberately keeps no link from a copy back to its source, and ADR-069 reasoned
partly from the absence of exactly that record. (ADR-079 later added a link for
takedown fan-out, in a table no user can read and which is used by takedown and
by nothing else.)

If a copy stays independent — if syncing back updates the owner's collection
and reaches nobody else's copy — then this limb is **not** triggered, and no
new distribution model exists. Somebody who adopted a collection has what they
adopted; the owner improving their own shelf does not push anything at them.

**Limb 2 — "versioned collections."** This one *is* triggered, plainly. FR-104
creates a new version of the base and keeps the previous one recoverable.

So the question for the owner is narrower than "is this a repository":

> Does a collection that has **versions on the owner's own shelf**, but **no
> path to anybody else's copy**, change the distribution model ADR-069
> assessed — or is it the same single-copy distribution with better bookkeeping
> behind it?

A reasonable reading is the latter. That reading is not the implementer's to
assert, which is why this document stops here.

## What this ADR is asking for

**Before spec 050's sync-back phase begins:**

1. **Re-confirm condition (a).** The notice-and-takedown programme was
   confirmed operational for ADR-069 and extended by ADR-079. This needs
   confirming as *still* true, not re-establishing.
2. **Make the determination above**, in writing, in this document, and sign it.
3. **If the answer is that it does change the model**, the phase is redesigned
   or the risk is explicitly accepted on the record — the guardrail's own
   words.

If any of ADR-069's standing conditions have since stopped holding, that ADR
says plainly that it "needs revisiting rather than the requirement being
relaxed". The same applies here.

## Determination (owner, 2026-09-13)

**Not a centralized public repository.** Accepted, on the following
reasoning, and on three conditions.

### Why

1. **A share link already serves live content.** Verified in the code:
   `shared_collection_impl` (`graphql/mutations_collection_shares.rs`) loads a
   shared collection's members at read time, on every request, and resolves
   each through moderation. A link has never served a snapshot, and a
   collection's owner can already change what it shows by editing it. ADR-069
   assessed and accepted exactly that model. Sync-back is one more route to
   editing a thing that is already live, not a new way to distribute it.
2. **The limb that carries the concern is not engaged.** ADR-069's limit names
   "versioned collections **or** any update path to already-copied content".
   The substance is the second: pushing changes to people who took copies.
   Adopted copies are independent (`collections/copy.rs`), and sync-back never
   reaches them. Only the "versioned" wording is literally met.
3. **Versions are the owner's history, not distribution.** Nobody but the owner
   can read an earlier version.
4. **Versions make it safer, not riskier.** A bad sync can be undone, and with
   T042 closed a takedown reaches every member type a collection can hold.

Also considered: S3 object versioning as a "native" mechanism. Rejected,
because collection content lives in Postgres rows and the object store holds
only files, and because a versioned bucket keeps every noncurrent version after
a delete. That would break spec 050's promise that nothing is retained on
deletion, and it would let a takedown leave earlier versions retrievable, the
problem ADR-079 had to fix for copies.

### Conditions — breaking any one of them re-opens this determination

1. **An earlier version is readable by the owner alone.** No share link may
   pin a version, and nothing may expose history to a link viewer or an
   adopter.
2. **Sync-back never reaches an adopted copy.** Copies stay independent.
3. **A takedown withholds content from every version**, not only the current
   one, resolved through `moderation::effective_status` like everything else.

These are not decoration. As ADR-069 says of its own conditions: if any fails
to ship, this ADR needs revisiting rather than the requirement being relaxed.

A determination is not legal advice, and this one has not been reviewed by a
lawyer. It is a product and policy decision by the project's owner, as ADR-069
was.

## Condition (a), answered by the owner (2026-09-13)

Re-confirmed rather than re-established: spec 015 is 41 of 42 tasks done and its
end-to-end tests (`dmca-takedown.spec.ts`, `dmca-counter-notice.spec.ts`) are
in the suite.

The one open task matters here. **Spec 015 T042**: a scene is not a moderated
entity type, so a takedown cannot reach a scene inside a collection, and an
adopted scene records no adoption for a takedown to follow. The owner chose to
**close T042 before sync-back ships** rather than ship with the gap recorded or
keep scenes out of syncable collections. Sync-back therefore waits on both this
determination and T042.

## How the conditions hold as built (2026-09-16)

Sync-back shipped in spec 049 phase 15 **for collections on the owner's shelf**
(`compendiums` rows of origin `Authored`). Versions are rows of
`shelf_collection_versions`; every write, removal, sync and restore keeps the
version it replaces, and a restore is a new version, never a rewind.

1. **Condition 1 holds by construction.** A shelf collection has no share link
   and cannot be adopted: those belong to spec 026's shared collections, a
   different table. `versions::history`, `versions::read_at` and
   `versions::restore` refuse everyone but the account owner, and no GraphQL
   field exposes a version to anyone else.
2. **Condition 2 holds by construction**, for the same reason: there is no
   adopted copy of a shelf collection for a sync to reach. A sync writes the
   owner's own rows and the syncing world's deltas, nothing else.
3. **Condition 3 holds vacuously.** No notice can name a shelf entry — the
   current version included — because shelf entries are not a moderated
   entity type. There is nothing for a takedown to withhold from an earlier
   version that it could withhold from the current one.

**Where condition 3 gets re-tested:** the day shelf entries become moderatable.
To make that re-test real rather than remembered, every read of an earlier
version goes through one function, `compendium::versions::read_at`, and
`versions_tests.rs::an_earlier_version_is_read_in_one_function` fails if a
second read of the stored entries appears, through Diesel or SQL.

**The caveat, stated plainly:** `read_at` covers *earlier* versions only. The
version in force is also read by `compendium::collections::download` and by
`library::book_list::entries_served_by` (what worlds are served). A future
moderation check must go into those two as well as `read_at`, or the current
version would be withheld less than its history.

Imported books have no versions and no sync-back at any volume of change
(spec 050 FR-101): the server refuses with the reason, and a database trigger
refuses a version row for any compendium that is not `Authored`, however it is
written.

## Consequences if accepted

- Spec 050's sync-back is unblocked, and only for authored **collections**.
  There is no route from a world to an imported compendium's base at any volume
  of change, and that asymmetry is enforced by ADR-097's invariant rather than
  by remembering to check.
- Versions accumulate on the owner's shelf. Storage cost is bounded by their
  own editing, not by anyone else's adoption.
- ADR-069's limit is amended to record which of its two limbs was engaged and
  what was decided about it, so the next person to reach this line reads a
  decision rather than an open question.

## Consequences if refused or deferred

- Phases 1 to 14 of the arc ship unchanged. What is lost is pushing a world's
  improvement back to the shelf — a real feature, and one nothing else depends
  on.
- A Game Master's work stays in the world where they did it. Spec 050 decision
  5's note about the delta model becoming a trap stands as a known cost.
