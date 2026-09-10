# ADR-079: Adoption Provenance and the Reach of a Takedown

**Date:** 2026-09-09
**Status:** **PROPOSED — awaiting the accountable owner's acceptance.**
**Participants:** ThunderForgeVTT Team
**Amends:** ADR-069 (collection share DMCA repository determination)
**Reverses:** spec 026 FR-012 ("no referential link back to the source")
**Related:** spec 039 US6 (FR-022 … FR-023d, SC-006, SC-008), ADR-070, ADR-071

> **This decision is not mine to make.** ADR-069 was accepted by the
> accountable owner with a stated risk accepted on the record; this amends it,
> and it is about liability. Spec 039's task plan makes acceptance a blocking
> gate (T004), and Phase 6 does not exist until it is answered either way.
>
> **If declined**: strike FR-022–FR-023d and SC-006/SC-008 from
> `specs/039-sharing-attestation/spec.md`, delete Phase 6 from its task list,
> mark this ADR REJECTED, and leave `legal/sharing-terms.md`'s "a copy someone
> takes is theirs, and cannot be recalled" exactly as written. Phases 1–5 —
> the attestation itself — are independent of this and do not wait.

---

## Problem Statement

`collections/copy.rs` keeps **no link** from an adopted copy back to what it was
copied from. That was deliberate: spec 026 FR-012 said a copy is independent,
and ADR-069's determination that a collection share is not a "centralized public
repository" reasoned partly from the absence of exactly that record.

The consequence is that a takedown does not reach. A notice arrives about a
shared collection; the share is disabled and the original is disabled; every
copy anybody took keeps working, and the instance has no way to know they exist.
The content is off one URL and still on the instance.

So: **a notice that cannot be honoured is a programme that does not work.** And
the record that would let it be honoured is the record ADR-069 was glad not to
have.

---

## Decision (proposed)

**Keep the link, in a table no user can ever read, used by takedown fan-out and
by nothing else.**

`content_adoptions` records that a copy happened: what was copied, what it
became, who took it, and when. It is written by `copy.rs` in the transaction
that makes the copy.

### The determination, on the properties ADR-069 actually relied on

Not on whether the word "repository" fits — on what made a repository dangerous
in that reasoning:

- **It is not readable by any user, ever.** No query, field, subscription or
  route exposes it. `moderation::reach` reads it and nothing else does. The
  plan's Constraints section states this as a rule for future work, not just a
  description of the first implementation: *no new user-facing query may list
  adoptions, by anybody, for any reason.*
- **It is not enumerable in either direction, even for an administrator.**
  There is no "what came from this" listing and no "what did this world take"
  listing. The only access is a bounded walk from one entity id **that a notice
  has already named** — that is, from content somebody has already accused.
- **It indexes nothing that was not already published.** A copy exists because
  a person holding a link chose to make it. The record says the copy happened;
  it makes no content reachable by anyone who could not already reach it.
- **It exists solely to make a takedown effective**, which is the opposite of
  the exposure ADR-069 weighed. A determination forbidding it would be a
  determination that the safest posture is one in which a notice cannot be
  honoured.

### What a takedown does to a copy

**Disabled, never deleted.** An adopter who took something in good faith has
built on it; deleting their work punishes them for somebody else's infringement.
Disabling stops the instance serving the infringing content while leaving the
adopter's own additions intact and restorable — the same posture the existing
counter-notice path already takes, with its waiting period and lazy
auto-restoration.

Withdrawal reverses the fan-out. If a counter-notice succeeds, the copies come
back with the original.

The walk is bounded by the adoption graph, which is bounded by `MAX_MEMBERS =
100` per copy, so there is no unbounded cascade to reason about.

---

## The cost, stated plainly

**Spec 026 FR-012 said a copy is independent. After this, a copy is independent
*to its adopter* and traceable *to moderation*.** Those are different claims,
and the second one is new.

Three specific consequences the accountable owner is being asked to accept:

1. **The instance holds a record of who took what from whom.** Unreadable by
   users and unenumerable by administrators, but it exists, and a database is a
   thing that can be compelled, stolen, or misconfigured. ADR-069's reasoning
   partly rested on there being nothing to compel.
2. **`legal/sharing-terms.md` becomes untrue** where it says a copy someone
   takes is theirs and cannot be recalled. That sentence has to be reworded
   (T007, a reviewer's decision), and the product must not tell people something
   it no longer does.
3. **An adopter can lose access to work they built on**, through no act of their
   own, because of a notice against somebody else. Mitigated by disabling rather
   than deleting, and by restoration on withdrawal — but it is a real harm to a
   blameless person, and it is the price of a notice that works.

Against those: a notice that actually removes the content it names, which is
what the notice-and-takedown programme is for, and what a safe-harbour posture
assumes is happening.

---

## Alternatives Considered

**Keep no link; a takedown reaches only the original and the share** (today).
Honest about the copies and useless for the purpose. It also makes
`legal/sharing-terms.md` true, which is worth something.

**Keep the link in the copy's own row** rather than a separate table. Rejected:
it puts provenance on user-readable content, where a query would eventually
project it — the enumerability this determination depends on not existing would
be one careless field away.

**Record the link but never act on it**, so it is available if the position
changes. Rejected as the worst of both: all the exposure, none of the benefit,
and a record whose only justification is a use nobody has committed to.

**Delete adopted copies rather than disable them.** Rejected — punishes the
adopter and is irreversible, so a withdrawn notice cannot be undone.

**Reach copies by content hash instead of a provenance record.** Considered
seriously: the asset-dedupe work already hashes content, so identical assets are
findable without a link. It fails on the cases that matter — a copy that was
edited, re-encoded, or partly assembled is no longer the same bytes — and it
would reach *unrelated* content that happens to be identical, in worlds that
never adopted anything. Worse in both directions.

---

## Consequences if accepted

**Good**

- A notice removes the content it names, everywhere the instance controls.
- Withdrawal restores everything, symmetrically.
- The adopter's own work survives a takedown against somebody else's.

**Costs**

- The instance holds provenance it previously refused to hold, and ADR-069's
  reasoning is amended rather than merely extended.
- `legal/sharing-terms.md` needs rewording, by a reviewer, before this ships.
- A blameless adopter can be disrupted by a stranger's infringement.
- A standing constraint on all future work: no user-facing query may list
  adoptions. That is a rule somebody has to keep, and this is where it is
  written down.

## Consequences if declined

- FR-022 through FR-023d and SC-006/SC-008 come out of the spec; Phase 6 does
  not exist.
- `legal/sharing-terms.md` stays exactly as written, and stays true.
- A takedown reaches the original and the share, and nothing else. The instance
  keeps serving copies of content it has been told is infringing, and cannot
  know which they are.
- Phases 1–5 are unaffected: the attestation is recorded and enforced on all
  four publishing paths either way.
