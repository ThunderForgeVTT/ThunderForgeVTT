# Player Tokens Are Re-Created on Arrival, Not Carried

- **Date**: 2026-09-01
- **Status**: Accepted
- **Spec**: `specs/031-playability/` (US4, FR-019)

## Context

A Game Master moving the party to a new scene should not have to place every
player's token by hand while the table waits. Spec 031 US4 asks for the
previous scene's content to clear, the new scene's to load, and the party's
characters to come along.

The obstacle is a decision already taken. **ADR-040 unified the token backing
store onto the scene-scoped `tokens` table**: a token belongs to exactly one
scene. "Bring the party" therefore cannot mean "leave them where they are" —
there is no such place. Something has to happen, and the choice of what changes
an ownership boundary, which is why this is an ADR rather than a task note.

Two candidates were carried through planning:

- **A — re-create on arrival.** Create tokens in the destination scene for the
  party's characters, preserving art, ownership and size. Identity is not
  preserved.
- **B — party membership.** Introduce a token that follows the party, resolved
  per scene. Identity is preserved, at the cost of a second way tokens come
  into existence.

## Decision

**A. Tokens are re-created in the destination scene.**

A token remains what ADR-040 made it: a scene-scoped thing. Moving the party
creates new tokens in the new scene from the party's characters and leaves the
old scene's tokens where they were, untouched.

Three rules follow, and they are the whole contract:

1. **The character is the identity that survives, not the token.** Anything
   that needs to follow a person across scenes — ownership, art, size, the
   claim binding — hangs off the actor, which is already world-scoped.
2. **A character that already has a token in the destination gains no second
   one.** The operation is idempotent per character.
3. **Position is not carried.** The party arrives somewhere the GM chooses or
   at a default entry point; pretending to preserve coordinates across two
   different maps would be a lie dressed as a feature.

## Rationale (Y-Statement)

In the context of moving a party between scenes, facing the fact that tokens
are scene-scoped by ADR-040, we decided **to re-create tokens in the
destination scene** and neglected **introducing a token that follows the
party**, to achieve **the feature with no schema change and no second way for
tokens to come into existence**, accepting **that a token's identity does not
survive the move, so nothing may hold a token id across a scene change**.

## Consequences

**No migration, no new table, no new lifecycle.** The operation is expressible
with the tokens table exactly as ADR-040 left it. That is the main reason to
prefer it: option B would have added a second origin story for tokens, and the
engine has been bitten before by entities that can appear through more than one
path.

**Token ids are not stable across a scene change.** This is the real cost and
it is stated plainly. Any future feature that wants to track a token *through*
a session — a persistent status, an animation, a per-token note — must key off
the actor, not the token. If something one day genuinely needs token identity
to survive, that is the moment to revisit this and consider B.

**The old scene keeps its tokens.** Returning the party to a scene they have
been in before will re-create tokens there too, subject to rule 2, so a room
does not fill up with duplicates on the second visit.

**"Bring the party" is a choice, not a default.** The GM may change scenes
without it, which is the ordinary case for a scene that is not where the party
is going.

## Alternatives Considered

- **B — a token that follows the party.** Rejected for now, not on principle.
  It reads better conceptually and preserves identity, but it adds a second
  mechanism by which tokens exist and touches the boundary ADR-040 deliberately
  simplified. The cost is real and the benefit is currently hypothetical: no
  feature today needs a token id to outlive a scene.
- **Move the token rows to the new scene.** Rejected: it destroys the previous
  scene's arrangement, so a GM who moves the party and comes back finds the
  room they had laid out is gone.
- **Copy every token, not just player characters.** Rejected: it makes the
  option useless for its stated purpose. Non-party tokens belong to the scene
  they were placed in.

## Related Decisions

- **ADR-040** — unified the token backing store onto the scene-scoped table;
  this decision lives inside that constraint.
- **ADR-046** — server-authoritative active scene; the scene change itself is
  broadcast, and the re-creation happens server-side with it.
- **ADR-033** — token data model and ownership; ownership is preserved across
  the re-creation because it derives from the actor.
