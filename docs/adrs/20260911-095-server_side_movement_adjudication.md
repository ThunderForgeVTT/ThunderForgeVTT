# ADR-095: Server-Side Movement Adjudication

**Date:** 2026-09-11
**Status:** PROPOSED
**Participants:** ThunderForgeVTT Team
**Related:** spec 045 US2 (FR-011 … FR-020), spec 003 FR-001 (the passability
toggle), spec 004 (which deferred this), constitution principles I and III

---

## Problem Statement

A wall that blocks movement does not block movement. The flag has been stored,
authored and toggled since spec 003, and nothing has ever read it when a token
moves: `moveOwnToken` and `updateToken` take the position as given, a drag
checks nothing, and the route the engine sends with a committed move was
dropped by the web before it left the browser. Spec 004 deferred the question
explicitly and no later spec picked it up.

The playtest of 2026-09-11 found it the way a table would: heroes walked
through the crypt's walls, and a player aiming straight at one arrived on the
far side.

So the question is not whether to enforce it, but **where**. And the answer has
to survive a fact about this product: the client is a program on someone else's
computer. Any check that lives only in the engine is a check a modified build
skips.

---

## Decision

**Both sides judge, from one shared test. The server's answer is the one that
counts.**

- The geometry lives once, in `thunderforge-canvas-core`, as
  `movement_blocked_by` and `path_blocked_by` — beside `is_visible`, using the
  same segment intersection and the same door rule.
- **The engine calls it to stop a move before anything is sent**, so the player
  sees their token stop at the wall, in the frame they pressed the key.
- **The server calls it to judge what arrives**, in `src/server/src/movement/`.
  A player's move that crosses is refused, naming the wall, and the token stays
  where it was.

Neither is redundant. The engine's check is the one that *feels* like a wall;
the server's is the one that *is* one.

### A Game Master is never judged

`updateToken` stays unjudged. The owner's decision 1 (spec 045): a Game Master
moves any token anywhere, and walls rule players' moves only. This is not an
oversight to be tightened later — it is the rule. A Game Master picking a token
up and putting it down through a wall is a normal thing to do at a table, and a
product that refused it would be wrong about who is in charge.

The practical consequence is that the judged path is `moveOwnToken` — the
mutation a player's own move goes through — and that is deliberately the
narrower surface.

### A move is judged along its path, not between its ends

Walking around a wall and teleporting through it end in the same place. The
endpoints alone cannot tell them apart, and the endpoints are all a position
update has ever carried. So `moveOwnToken` gains an optional path, and the
server judges leg by leg.

When no path is given the server judges the straight line from the token's
stored position to the requested one. That is the honest default for a drag,
which *is* a straight line, and it is the safe default for anything else: a
client that wants a longer route accepted has to say what the route was.

Passing exactly through the point where two walls meet counts as crossing
(FR-016). Treating a mathematical joint as a gap would let a token slip
diagonally between two walls that meet, which is the oldest way out of a locked
room there is.

---

## Alternatives Considered

**Server only.** Simplest to reason about and impossible to bypass — but a
player would press a key, see their token move, and see it snap back a round
trip later. That reads as lag, not as a wall. Principle I puts the simulation
in the ECS precisely so that the board behaves like a board.

**Engine only.** Immediate and cheap, and worthless: a modified client walks
through walls, and Principle III does not permit trusting one. It would also
put the rule somewhere the server cannot state it — a world's own history would
contain moves its walls forbade.

**Trust the engine's `pathCells`.** The route the engine already computes,
validated server-side. Kept for the committed-route case, but it cannot be the
whole answer: a drag has no route to send, and a client that is lying about its
position is equally able to lie about how it got there. The path narrows what a
client may claim; it never replaces judging it.

**Two implementations of the geometry.** The engine is Rust compiled to wasm
and the server is Rust; writing the crossing test twice would have been easy
and would have drifted. One crate, one test, one answer — which is also what
keeps a closed window see-through and impassable in both places at once.

---

## Consequences

- `thunderforge-canvas-core` gains `movement_blocked_by`, `path_blocked_by` and
  `WallSet::movement_blocking_walls`, and is now depended on for a rule rather
  than only for drawing.
- `moveOwnToken` can fail for a new reason, and every caller has to handle a
  refusal by putting the token back where the server says it is.
- A scene with many walls judges every player move against them. At the scale
  involved — tens to low-hundreds of walls, a linear scan — this is not worth
  indexing for, and the `WallSet` comment already says so.
- The server needs the scene's walls to judge a move, which it did not before.
- **A refusal must not leak a secret door.** FR-019: a closed secret door stops
  a player, and the feedback must not say it is a door. The refusal names a
  wall, and a secret door is a wall.
