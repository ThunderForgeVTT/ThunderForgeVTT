# ADR-097: Origin as a Non-Editable Invariant

**Date:** 2026-09-12
**Status:** **PROPOSED** — requires acceptance by the accountable owner, not the implementer. See "Who signs this" below.
**Participants:** ThunderForgeVTT Team
**Related:** spec 049 (FR-050 … FR-057, decision 4), spec 050 (FR-050 … FR-054), spec 026 (collections), spec 016 (pack legal blocks), ADR-069, ADR-079
**Blocks:** spec 049 phase 8

---

## Problem Statement

A Game Master reads a commercial rulebook into their library. The owner's rule
for what happens next has been settled since spec 047:

> "In real life I can show my book to the whole table, but I cannot have my
> book and lend it out."

The table may read it over their shoulder. The book does not travel.

Enforcing that needs an answer to one question — **may this piece of content
leave?** — and the first answer we reached was *licence*: determine whether a
work is commercial, and restrict the commercial ones.

That answer does not survive contact.

- It needs a **judgement**, made by the person with the least incentive to
  answer strictly.
- Where it is automated, it rests on PDF metadata, which is frequently absent
  and frequently wrong.
- It can be got wrong in the direction that costs the most: one permissive
  misclassification distributes somebody's commercial book.

## Decision

**Sharing is decided by where content came from, not by what licence it
carries.** There are exactly two origins, and the line between them is
mechanical rather than a matter of opinion:

| Origin | What it is | May it leave? |
|---|---|---|
| **Authored** | Made in ThunderForge through the authoring tools | Yes — it is what a collection is made of |
| **Uploaded** | Read out of a document somebody supplied | **Never** |

Content shipped in a **system pack** is a third thing and is untouched by this:
the platform distributes it under the pack's own `legal` block (spec 016), and
nobody uploaded it.

### Three properties that make it enforceable

**1. Recorded automatically, never declared.** The system already knows with
certainty which of its two paths a piece of content arrived by. Nobody is asked
and nobody can answer wrongly.

**2. Not editable by any role.** There is no path — not for a Game Master, not
for an instance administrator — that turns uploaded content into authored
content. A value that can be flipped is a value that will be flipped, and this
is the one field every sharing rule is enforced against.

**3. An invariant, not a check.** A collection cannot contain uploaded content
**by construction**, enforced where content is written rather than at each
route that might lead outward. Share, publish, export and collection-adoption
are not *the* list of routes; they are today's list. Enumerating them is a game
lost the first time somebody adds a route without reading spec 049.

### The subtlety that is easy to implement wrongly

Origin is per **entry**, not per compendium. When a world changes something it
inherited (spec 050's delta model), the three forms that change can take do not
share an origin:

- **changed** or **hidden** an uploaded entry → still uploaded, still cannot be
  shared. A mutation has no meaning apart from the thing it mutates.
- **added** a world-only entry beside it → authored, and shareable. Somebody's
  own homebrew does not become unshareable by sitting in a world that also has
  books switched on.

## Who signs this

**The accountable owner, not the implementer.** The precedent is explicit:
ADR-079 was accepted by the owner "because it is about liability, so it was
theirs to sign and not the implementer's", and ADR-069 before it. This decision
is the same kind — it is the rule the platform's copyright position rests on.

Spec 049's phase 8 does not begin until this is ACCEPTED.

## Alternatives Considered

**A Game Master declares whether a book is commercial, defaulting to
commercial.** Honest, and the person holding the book does know. Rejected: it
depends on them answering strictly, and it puts a legal outcome behind a
checkbox that costs them something to tick.

**Derive it from the system pack's `legal` block.** Automatic, and already
exists for SRD content. Rejected: it says nothing at all about a third-party
book read into a 5e world, which is most of them.

**Detect it from the document** — publisher metadata, a copyright page, an
OGL/CC notice. Rejected as a sole source: detection that is wrong in the
permissive direction is exactly the failure that matters, and PDF metadata is
routinely missing or inherited from a template.

**Any of the above as a suggestion the Game Master confirms.** Better than each
alone, and still a judgement with a wrong answer available. The origin rule
needs no judgement from anyone, which is why it wins.

## Consequences

- **A known cost, recorded rather than discovered**: somebody who uploads a
  document they wrote themselves, or an openly licensed one, cannot share what
  comes out of it. Two routes remain and the refusal must name them — author it
  through the authoring tools, or propose it as a system pack. That is a real
  loss, and smaller than the cost of any rule requiring somebody to correctly
  classify a PDF.
- Every refusal states the origin as the reason, so a person is never left
  guessing why something is greyed out.
- It **narrows** what ADR-069's determination has to carry rather than widening
  it: uploaded content becomes structurally incapable of entering the
  shared-collection path that determination reasoned about.
- Spec 049's Q1 — "how is commercial determined?" — is withdrawn rather than
  answered. It has no answer because it has no question.
