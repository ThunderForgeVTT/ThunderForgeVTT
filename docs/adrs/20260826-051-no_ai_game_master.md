# ADR-051: ThunderForgeVTT Will Never Build an AI Game Master

**Date:** 2026-08-26
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team
**Accountable owner:** Michael Bruno, project owner

---

## Problem Statement

`docs/Advanced Virtual Tabletop Specification.md` — the speculative
next-generation architecture doc added in `4fe2785` — lists nine system
layers. One of them is **"Artificial Intelligence (GM Assistant)"**:
on-device LLM inference via ONNX/WebGPU, local RAG indexing over the
campaign's CRDT/SQLite store, "generates prep sheets natively."

That document is a technology survey, not a roadmap, and it does not
distinguish between layers we have deliberately not built *yet* and layers
we have deliberately decided *never* to build. An audit of the spec against
the codebase (2026-08-26) scored the AI layer at "0% — not started," which
is the wrong reading: every other 0% layer is a sequencing question, and
this one is not.

Without a recorded decision, the AI-GM layer reads as unclaimed backlog. It
will be re-proposed — by a contributor, by an issue, by a future planning
pass, or by an AI agent reading the spec doc as a to-do list. Each
re-proposal costs a rediscussion of a question that is already settled, and
one of them will eventually be answered "sure, why not" by someone who
never saw this conversation.

Principle IV requires a real ADR before divergent implementation. This ADR
inverts that obligation: it records a decision *not* to implement, so that
implementing would itself be the divergence.

## Decision

**ThunderForgeVTT will never ship an AI Game Master.**

This is a permanent product-identity decision, not a prioritization. It is
not scheduled, not deferred, and not contingent on model quality, cost,
licensing, or on-device inference becoming practical. Improvements in any
of those do not reopen it.

The boundary is **replacement versus augmentation**, and it is drawn at the
human running the table:

**Never — an AI that *is*, or stands in for, the Game Master:**

- An AI that runs a session, in whole or in part.
- An AI that adjudicates for the table, narrates in a GM's place, or
  arbitrates player action as the authority.
- Any mode that lets a group play a ThunderForge world with no human GM.
- Any framing — UI, docs, marketing, or defaults — that presents an AI as a
  substitute for a person in that seat.

**Permitted — AI *tools* a human Game Master opts into and controls:**

- Integration surfaces a GM may connect at their own discretion — an MCP
  server, an API facet, an export or context endpoint that a GM points
  their own tooling at.
- Prep-time aids the GM invokes, reviews, and edits before anything reaches
  the table.
- Lookup, search, and summarization over content the GM already owns.

In every permitted case the GM remains the author and the authority: the AI
is a tool held by a person, never a participant seated at the table. A
feature that would still function with the GM removed is on the wrong side
of this line.

**This decision does not restrict AI or ML applied to problems that are not
the GM's role** — e.g. audio transcription, accessibility, translation, or
asset processing. Those are evaluated on their own merits.

## Rationale

**Tabletop is a community activity, and that is the product.** People come
to a virtual tabletop to be in a room — synchronously, with other people,
under a human who is improvising for them. The GM is not overhead standing
between players and the game; the GM *is* the thing that makes it a
tabletop rather than software. Automating that seat does not make the
product better at what it is, it converts it into a different product.

**That different product already exists, and is not ours to build.** A
single player versus an AI-run rules engine is a turn-based video game.
That genre is served extraordinarily well by studios who are specifically
good at it. We are not competing with them, we are not better positioned
than them, and attempting it would mean doing badly what they do well while
neglecting what we are actually for. We leave that to the brilliant minds
who build it.

**The augmentation line is where the value actually is.** GMs have real,
unglamorous problems — prep time, lookup, session notes, continuity. Tools
that hand a GM leverage over those, on the GM's own terms, are welcome and
are the correct place for any future AI investment here. Tools that hand
the GM's *job* to a model are not.

**It clarifies the architecture, too.** The spec doc's "Synergy 1" argues
that local-first CRDTs are justified largely because they keep campaign
data on-device for private local inference. With the AI-GM layer struck,
that argument loses most of its force — which independently strengthens the
server-authoritative, permission-enforcing path already established by
ADR-046, ADR-048, and ADR-050. Our architecture and our product identity
point the same direction.

## Consequences

- The "Artificial Intelligence (GM Assistant)" layer of
  `docs/Advanced Virtual Tabletop Specification.md` is **WILL NOT BUILD**.
  Any audit, roadmap, or status pass over that document must mark it as
  such, never as "0%", "not started", or "future work" — those imply a
  pending item and will be read as an invitation.
- Feature requests, issues, and PRs proposing an AI GM are closed by
  reference to this ADR rather than re-litigated. Contributors are owed a
  clear, non-hostile pointer here, not a debate.
- Any future AI-adjacent proposal must state, explicitly, which side of the
  replacement/augmentation line it falls on and why. "It only assists"
  is a claim to be checked against the test above: *would this feature
  still function with the human GM removed?*
- Integration facets (MCP server, API surfaces, context endpoints) remain
  open design space. This ADR permits them; it does not commit to them, and
  each still needs its own decision on scope, auth, and data boundaries —
  Principle III applies unchanged to anything that exposes world data.
- No code changes. Nothing is being removed, because nothing was built.
- Contributors and downstream forks are unaffected in what they may build
  under AGPL-3.0-or-later; this governs what *this* project ships.

## Alternatives Considered

- **Say nothing and leave it as an unstarted spec layer** — rejected as the
  status quo that created this ADR. Silence on a decided question is
  indistinguishable from an open backlog item, and the spec doc will
  outlive everyone's memory of the conversation.
- **"Not now, revisit when local inference is good enough"** — rejected
  because it is not true. The objection is not to the technology's
  maturity; it is to the role. A better model does not make replacing the
  GM more desirable, only more achievable, which is the opposite of a
  reason to revisit.
- **Ban all AI in the product outright** — rejected as broader than the
  actual conviction. It would foreclose GM-controlled tooling, MCP
  integration facets, and unrelated accessibility work (transcription,
  translation) that no one objects to. The line belongs at replacing the
  GM, not at the technology.
- **Ship an "AI GM" as an optional, off-by-default mode** — rejected. An
  optional mode is still a shipped mode: it must be built, documented,
  supported, and shown in the UI, and its existence makes the claim that
  the GM's seat is fillable by software. Defaults do not change what a
  product asserts about itself.

## Related Decisions

- ADR-046 (Server-Authoritative Active Scene), ADR-048 (`graphql-ws`
  Live-Sync Transport), ADR-050 (Permission Declaration & World Access
  Links) — the server-authoritative, human-permissioned spine this
  decision is consistent with.
- Constitution Principle III (Ownership & Authorization at the Data
  Boundary) — governs any future AI integration facet that exposes world
  data.
- Constitution Principle IV (Real ADRs and Specs Before Divergent
  Implementation) — the obligation this ADR discharges in the negative.
