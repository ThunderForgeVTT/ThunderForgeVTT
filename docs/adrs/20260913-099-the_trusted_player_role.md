# ADR-099: The Trusted Player Role

**Date:** 2026-09-13
**Status:** **ACCEPTED** by the accountable owner, 2026-09-13, the day it was proposed.
**Participants:** ThunderForgeVTT Team
**Related:** spec 048 (decision 4, FR-030a … FR-033c), spec 050 (decision 8, FR-010 … FR-020a), `crates/thunderforge-authz/src/role.rs`, ADR-097

---

## Problem Statement

A world has three roles today, and they are ordered in `thunderforge-authz`:

| Role | Stored as | Documented as |
|---|---|---|
| Owner | `Owner` | holds the world |
| Game Master | `GM` | runs the world; full authority over content |
| Player | `Player` | plays |

Two new pieces of work need someone between Player and Game Master.

- **A world's book material** — which books from the owner's shelf a table has
  switched on, and what the world has changed about them (spec 050). The first
  implementation limited this to the world's owner alone.
- **Adopting what players bring** (spec 048). A player's character sheet is
  staged in a compendium named for them, and someone has to decide what the
  table will play with.

At most tables the person helping with that is a trusted friend, not a second
Game Master. Making them a Game Master gives away far more than the job needs:
scenes, walls, members, rollback, everything a world's content authority
reaches. Leaving them a Player means the Game Master does all of it alone.

## Decision

**A fourth role, Trusted Player, ranked between Player and Game Master.**

```text
Owner         > holds the world
Game Master   > runs the world
Trusted Player> co-DM for content: book material, and adopting what players bring   ← new
Player        > plays
```

A Trusted Player **may**:

- switch books on and off in a world's book list, always from the **world
  owner's** shelf (spec 050 FR-010a), never their own;
- browse the entries of the books a world has switched on (spec 049 FR-042);
- change what the world inherited from those books — its deltas (FR-020a);
- see what each player has staged, and adopt it piece by piece or all at once
  (spec 048 FR-032 … FR-033a).

A Trusted Player **may not** do what makes a Game Master a Game Master. That
includes everything the Game Master role already gates today (scenes, walls,
lighting, membership and the rest), plus rolling a character back (spec 048
FR-044b) and importing onto somebody else's actor (FR-041).

**Rank is still the model.** Anything gated `>= GameMaster` keeps excluding a
Trusted Player without being touched. The new rules are gated
`>= TrustedPlayer`, through a named predicate beside `runs_the_world` —
something like `manages_content` — rather than a comparison written out at
each call site. The role module exists precisely because comparisons written
by hand drifted.

## Consequences

### What implementation must get right

Adding a variant between two existing ones is safe for `>=` rank comparisons,
and quietly wrong anywhere the code tests for **equality with Player**, or
treats "not a Game Master" as "is a Player". Four places were found, and each
has to be decided rather than assumed:

1. **The database refuses the new value today.**
   `world_members.role` carries `CHECK (role IN ('Owner', 'GM', 'Player'))`
   (migration `2026-05-06-120100-0008`). A migration has to widen it, with a
   `down.sql` that refuses to drop the value while any member holds it.
2. **There are two role enums, not one.** `thunderforge_core::models::invites::WorldMemberRole`
   sits beside `thunderforge_authz::Role`, and the invite path and adapters
   convert between them. Both gain the variant, and the round-trip tests that
   already guard `Role::from_stored` must cover it.
3. **Exhaustive matches** in `graphql/mutations_reconcile.rs` treat `Player` as
   "may do nothing to what they do not own". A Trusted Player must fall on the
   Player side of those rules except where book material or adoption says
   otherwise. The compiler forces each match to be revisited; the danger is
   the `_ =>` arm nobody wrote yet.
4. **The web app compares role strings** in `worldMembersCollection.ts`,
   `useWorldRole.ts`, `useWorldMembers.ts`, `types/world.ts` and the players
   page. It needs one role type, not more string literals.

### What a Trusted Player is shown

Everywhere else a Trusted Player sees exactly what a Player sees. Being trusted
with content is not a licence to see hidden tokens, fog, or anything the server
withholds from players (spec 045 FR-033, spec 048 FR-037). Any exception
belongs in a spec, not in a component.

### Granting it

Only the Owner or a Game Master may make someone a Trusted Player, and only the
Owner or a Game Master may take it away. It is granted per world. Being trusted
at one table carries nothing to another.

### Found in passing

`role.rs`'s doc comments are shifted by one variant. `Player` carries the
Owner's description ("Holds the world…"), and `Owner` carries a one-line stub.
This is corrected in the same change, since that change touches every variant
anyway.

## Found in implementation (commit `16e3a1f`)

The four hazards above were not the whole list. Building it found more, and
one of them was a hole that predates this ADR.

- **A Game Master could make anybody an Owner.** `update_member_role` checked
  whether the caller could manage the target's *current* role, and never
  compared the *new* role with the caller's own rank. A Game Master could
  therefore promote any Player (including an account of their own) to Owner,
  and that new Owner could then demote the real one: a world seized by
  somebody trusted only to run it. Closed by `can_assign`: a caller may
  assign a role at or below their own rank and never above it. Tested
  (`a_game_master_cannot_make_anybody_an_owner`).
- **The reconcile hazard was not where this ADR said.** Its `Role` is a
  separate two-value enum in `thunderforge_cache_core`, so the compiler never
  flags it. The real danger was a string match whose `_ => Player` arm gave a
  Trusted Player the right answer only by luck. It now uses the ranking.
- **A silent demotion in `adapters.rs`.** Anything unrecognised defaulted to
  Player, so a Trusted Player written back through it would have been quietly
  demoted. Fixed, with round-trip tests.
- **The web app disagreed with the server.** `worldMembersCollection`'s
  `canManageRole` let a Game Master manage other Game Masters, which the server
  refused. The authoring-tool grants card and the claim gate compared role
  strings directly. All of them now follow the ranking.
- **Switching a book on means seeing the owner's shelf.** Not stated above. The
  list of books offered to a non-owner now carries each book's name and size,
  not its file hash.
- **Browsing a book's entries was first left Game Master only**, following spec
  049 FR-042, because this ADR didn't say otherwise. The owner settled it the
  same day: **a Trusted Player browses a world's books like a Game Master.**
  FR-020a lets them change what a world inherited, and nobody can change
  entries they cannot read. FR-042 is amended to match.

## Alternatives Considered

**Rename today's Game Master to Trusted Player**, and call the Owner the Game
Master. Rejected by the owner. It keeps three roles by renaming stored values,
code, UI and specs everywhere, and it changes what "Game Master" means in a
product whose specs use the term precisely.

**A per-member "trusted" flag on a Player** rather than a role. Rejected by the
owner. It turns trust into a capability that sits outside the ranking the role
module was built to make the single source of truth, which is the drift that
module exists to end.

**Widen Game Master instead**, letting any Game Master manage book material.
Correct as far as it goes, and part of this decision. But on its own it leaves
the friend who helps with content with a choice between too much power and
none.
