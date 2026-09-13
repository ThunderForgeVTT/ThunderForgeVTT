# ADR-098: An Update Path to a Collection

**Date:** 2026-09-12
**Status:** **PROPOSED — determination NOT yet made.** This document sets out the question and the evidence; the determination itself is the accountable owner's to make and sign. See "What this ADR is asking for".
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
