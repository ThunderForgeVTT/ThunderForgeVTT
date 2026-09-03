# A Counted Acknowledgement Guards a Change That Looks Destructive

- **Date**: 2026-09-03
- **Status**: Accepted
- **Spec**: `specs/033-abilities-vocabulary/` (FR-024 to FR-033, SC-005 to SC-008)
- **Related**: Constitution Principle III (authorization at the data boundary)

## Context

Changing a world's game system is one click today.
`update_world_game_system_impl` (`src/server/src/graphql.rs:2009-2066`) checks
that the caller is a DM and that the id is non-empty, then writes the column.
There is no warning, no count, and no record — it does not even emit a world
event, unlike its sibling `update_world_interface_pack_impl` a few lines above.

A GM who has authored months of content against one ruleset can change it by
selecting from a dropdown.

The operation is **not destructive**. Nothing is deleted, rewritten or
re-tagged; content authored under the previous system stays exactly as
authored and becomes visible again if that system is made active again. But it
*looks* destructive, and its effect — a compendium that suddenly presents
differently — is alarming enough that a GM needs to know what is about to
happen before it does.

That combination is the interesting one. A warning for a genuinely destructive
operation writes itself. A warning for a reversible one has to be severe enough
to be read and honest enough to still be true, and those pull in opposite
directions.

## Decision

**The operation is guarded by an acknowledgement of the counts, enforced
server-side.**

1. **Count first, and count truthfully.** `worldContentInventory` returns
   per-kind counts — actors, abilities and items — with the system each was
   authored under, plus how many abilities will become *unrecognised* under the
   target system. DM-only: the counts describe content a player may not see.
2. **The warning names real numbers**, names the systems by display name, and
   says plainly that affected content becomes **hidden, not destroyed**, and
   that switching back restores it. It must not say "delete", "lose" or
   "destroy", because none of those is what happens.
3. **Two distinct confirmations**, the second naming the target system. The
   existing single confirmation is *not* reused — it exists for spec 016's
   legal notice, and one control meaning both "I read the licence" and "I
   accept this data consequence" weakens both.
4. **The server refuses without the acknowledgement.** The mutation takes a
   digest over the counts; the server recomputes and refuses on mismatch. A
   guard that exists only in the dialog is not a guard.
5. **A world with no content keeps the one-click path**, with no warning and no
   second confirmation.

### Why a digest and not a boolean

`acknowledged: true` satisfies the letter of FR-028 and none of its intent. A
caller can pass it having never seen a count — which is precisely the bypass
the requirement exists to prevent — and it stays `true` if the world's content
changed while the dialog was open.

A digest over the counts means "I acknowledge **these** numbers". A world that
gained an actor between the dialog opening and the GM confirming is
re-confirmed rather than switched behind their back.

**Not a stored token.** That needs a table and an expiry policy for something
open for seconds, and the counts are already the thing being acknowledged.

### Why "empty" excludes scenes

A world with zero actors, zero abilities and zero items switches without
ceremony. Scenes and lore do not count, and this is load-bearing rather than an
oversight: spec 010 guarantees **every world is created with a default scene
already made**. Counting scenes would mean no world is ever empty, the
one-click path would be unreachable, and a GM would meet the red warning on a
world they created a minute earlier.

A warning shown when nothing is at stake is one people learn to click through,
and then it is not protecting anything when something is.

## Consequences

- **The warning's honesty is a maintenance obligation.** FR-026 forbids
  overstating, so if the operation ever *does* become destructive, this wording
  becomes a lie rather than merely stale. The counting query and the dialog are
  one piece of work for that reason.
- **The mutation gains a world event**, closing a small asymmetry: a world's
  palette changing is announced today and its ruleset changing is not.
- **A stale digest is refused**, which means a GM who leaves the dialog open
  while content changes is asked again. That is the correct outcome and it will
  occasionally look like a bug to someone who does not know why.
- **The inventory needs the target system's vocabulary** to count what will
  become unrecognised, which is the one part of this that depends on the
  vocabulary half of spec 033.

## Alternatives considered

- **A boolean acknowledgement.** Rejected above.
- **A server-minted confirmation token, stored with an expiry.** Rejected — a
  table and a cleanup policy for a dialog measured in seconds, when the counts
  are already the thing being acknowledged.
- **Typing the world's name to confirm**, as destructive-action dialogs often
  do. Rejected: it is the convention for *irreversible* operations, and using
  it here would tell a GM this is worse than it is. FR-026's honesty
  requirement cuts both ways.
- **No guard, on the grounds that it is reversible.** Rejected — reversible is
  not the same as harmless, and a GM discovering their compendium looks
  different with no idea why has been failed even if nothing was lost.

## What would change this

- **The operation becoming genuinely destructive**, at which point the wording
  and probably the confirmation shape both change.
- **Content kinds growing.** The inventory counts three kinds because those are
  the three that carry a system tag. A fourth would need adding here, and the
  digest changing with it.
