# ADR-110: A Claim Grants Editor, and Takes It Back

**Date:** 2026-10-03
**Status:** **PROPOSED** 2026-10-03 with spec 063. The server rules are implemented and proven by the server's own tests; acceptance waits on the e2e proof of the `actors` slice (spec 063 SC-007).
**Participants:** ThunderForgeVTT Team
**Related:** spec 063 (FR-001 to FR-021), ADR-105 (a character's look belongs to whoever holds it — superseded in part, see below), ADR-050 (permission declaration), spec 017 (actor claiming), spec 062 (where the gap was found), spec 044 (FR-030a, FR-030b, FR-032)

---

## Problem Statement

ADR-050's ladder — Viewer < Editor < Owner — decides whether a person may
change an actor. A claim (spec 017) decides who plays one. Until now the two
did not meet: a player who claimed a character, was bound to one by the Game
Master, or created their own resolved to Viewer on it.

ADR-105 looked at that and kept it. It gave the holder one right outside the
ladder, the character's portrait and token, and rejected granting Editor on
claim.

Play found the cost. A player on Viewer cannot spend their character's
resources, take damage, pick up an item or change a word of the sheet. Every
table that wanted a playable character had to have its Game Master open the
ownership block and grant Editor by hand, to every player, for every
character. Spec 062 recorded it as a gap, and its end-to-end test carried
the hand grant as a workaround.

## Decision

1. **Being bound to a character grants Editor on it.** By all three routes
   that create a claim — the player claims, the Game Master binds, the
   player creates their own — and in the same transaction as the claim, so
   neither commits without the other. Editor, not Owner: enough to play the
   character, short of handing it to somebody else.

2. **The grant is an ordinary row in `world_actor_permissions` that
   remembers where it came from.** A `granted_by_claim` column, false for
   every row a person set. The resolver does not read it; the ladder is
   unchanged and every existing Editor check simply starts saying yes.

3. **A claim sets a floor, not a leash.** A player with no row gets Editor,
   flagged. A row below Editor is raised and flagged. A row already at
   Editor or Owner is left exactly as it was, unflagged: it is the Game
   Master's, and the claim does not take it over.

4. **The release takes back only what the claim gave.** Un-claiming,
   re-binding and clearing a binding delete the row while it is still
   flagged, and nothing else. A hand grant of any level survives.

5. **A hand edit takes the decision over.** Setting a level in the
   ownership block clears the flag in the same statement, whatever the level
   and whichever direction. What the Game Master set is then what stands
   after the release. Removing the row during a live claim removes the
   access and the claim does not put it back.

6. **The two art switches still bind the holder.** This is the part of
   ADR-105 that is kept. `actor_imagery.rs` now asks how the caller came by
   Editor: granted by hand, or Owner, passes unconditionally as before;
   granted by the claim goes on to the same three checks a holder faced when
   a claim granted nothing — holds the claim, the world allows it, the
   character is not locked.

7. **Every change of access is announced.** World event code 31, carrying
   the actor's id and no user id, recorded after commit by the claim, the
   binding, the release and both hand edits. An open actor page re-reads the
   actor and learns what its own viewer may now do.

### What this supersedes

ADR-105, Decision 2 — that the holder's right is imagery-only and is not an
Editor grant — and its rejected alternative "Grant Editor on claim". The
rest of ADR-105 stands: the look belongs to whoever holds the character, and
the world setting and the per-character lock withdraw it.

## Rationale

- **The objection ADR-105 raised was to an unbounded grant, and this one is
  bounded.** Editor that arrives with the claim and leaves with it is the
  claim's own right written in the ladder's terms. What ADR-105 feared — a
  player keeping write access to a character they no longer play — is what
  Decision 4 prevents.
- **Provenance on the row, rather than a rule in the resolver.** The
  alternative is to have the resolver consult `world_actor_claims`. That
  keeps no state to get out of step, but it makes Decision 5 inexpressible:
  a Game Master could not lower a holder to Viewer, because the claim would
  keep answering Editor. The flag lets one row say both what the level is
  and whose decision it was.
- **One statement per transition.** The grant is a single upsert whose
  `DO UPDATE` is filtered to rows below Editor; the hand edit clears the
  flag in the upsert that sets the level. There is no read-then-write for
  two requests to interleave in.
- **Ids only in the event.** The event reaches every member of the world
  and the ownership block is the Game Master's to read. Each client asks
  the server for its own answer.

## Consequences

- A Game Master no longer grants Editor by hand for a table to be playable.
  Spec 062's workaround is removed and its record of the gap points here.
- Existing claims are backfilled by the migration with the same floor rule,
  so a world in play gains the access without anybody re-claiming.
- The ownership block now shows a row the Game Master did not create. It
  shows it as Editor like any other; the interface does not yet say which
  rows a claim made.
- `actor_imagery.rs` reads one more row than it did. The ladder is still
  asked first, and Owner returns before the row is read.
- Removing a member already purged every grant they held (spec 027) and
  cascades their claims, so nothing new was needed there; a test now holds
  the pair together.
- A player promoted to Game Master while bound keeps the flagged row. It is
  inert — they resolve to Owner by role — and is removed by the release
  like any other.

## Alternatives Considered

- **Resolve Editor from the claim at read time, storing nothing.** Rejected
  for the reason under Rationale: it cannot represent a Game Master lowering
  a holder, and it would make the ownership block disagree with what the
  resolver answers.
- **A separate table of claim-derived grants.** Rejected: two tables to
  union on every permission check and two places for one (actor, user) pair
  to hold different levels, for the sake of a boolean.
- **Grant Owner on claim.** Rejected: Owner may share and re-grant. Playing
  a character is not authority over who else may.
- **Leave hand-granted rows flagged when the level is unchanged.** Rejected:
  a Game Master who re-sets Editor on a holder has said the access is theirs
  to give, and the release should not undo it.
- **Drop the art switches for holders now that they are Editors.** Rejected:
  the switches exist for exactly these people, and no hand-granted Editor
  was ever bound by them.
