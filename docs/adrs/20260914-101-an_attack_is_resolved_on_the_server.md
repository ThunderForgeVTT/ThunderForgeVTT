# ADR-101: An Attack Is Resolved on the Server, and Its Damage Is an Offer

**Date:** 2026-09-14
**Status:** **PROPOSED**
**Participants:** ThunderForgeVTT Team
**Related:** spec 046 (US1, US2, FR-001–FR-009, FR-002a, decisions 1 and 3, clarifications Q1–Q3 and Q5), spec 046 research R1, R7, R8, R15, R16, `specs/046-a-fight-that-resolves/contracts/fight.md`, ADR-044 (the server is the only source of a roll), ADR-095 (server-side movement adjudication), ADR-102 (a token is its actor, or a copy of it)

---

## Problem Statement

An attack was `rollDice` with a formula. The server rolled it and handed the
number back to the person who asked; nothing aimed it at anyone, nothing
compared it with a defence, nothing told any other seat it had happened, and
nothing about it could change a creature's hit points. In the playtest Aria
rolled 22 to hit and Brom's screen showed nothing.

Joining the roll to the board raises three questions of ownership at once:

- **Who adjudicates.** A hit changes somebody else's creature. A client that
  rolled and reported would be trusted with another player's hit points.
- **Who decides the damage lands.** The owner's answer (decision 1) is that
  the person on the receiving end does, not the software and not a world
  setting — with one automatic path, for the Game Master's own NPCs.
- **Who is told what.** Every member of a world receives every world event,
  and the event stream has no per-viewer filter. An attack by a creature a
  player cannot see, or whose name the Game Master hid, must not tell that
  player who made it (FR-002a).

## Decision

1. **The server resolves an attack, in one mutation.** `makeAttack` checks,
   in this order: the world is not paused (C10); the caller controls the
   attacker or runs the world (C2); it is the attacker's turn unless the
   attack is a reaction or the caller runs the world (C1). It then rolls the
   ability's or item's `ATTACK_ROLL` formula with the dice crate, reads the
   target's defence as its pack declares it (`combat.defence`), compares
   (total ≥ defence hits), rolls damage on a hit, and writes one
   `world_attacks` row per attack. The rolls are ordinary roll records
   (ADR-044), linked from the attack. Reach, range and line of sight never
   refuse (C3); they are flags on the record.
2. **A hit's damage is an offer.** It creates a `world_offers` row, pending,
   addressed to the target's token. Whoever controls that token — the
   players `moveOwnToken` already lets move it (research R7) — takes it or
   declines it, once (C6). A Game Master may resolve any pending offer on a
   player's behalf at any time, and the table sees that the Game Master did
   (FR-009). An offer never expires or resolves itself (FR-008). Taking one
   applies the change through the one hit-point operation (ADR-102) in the
   same transaction as the offer's new status.
3. **Auto-apply is the Game Master's, and only for what they run.** A world
   default (off) and a per-encounter override. It applies a hit without an
   offer only when the target has no controller but the Game Masters, the
   attack named a target and hit, and it had line of sight or does not need
   it (research R15). Damage to a creature any player controls is always an
   offer.
4. **An offer is bound to the record it was rolled against.** It stores the
   token's link state when made. Taking an offer whose token was relinked or
   unlinked since is refused with a sentence saying so; declining still works.
   Otherwise a hit rolled against a goblin copy would land on its NPC's sheet.
5. **What a viewer is told is decided when they ask.** Events 29 and 30 carry
   an attack's or offer's id and nothing else. `attack(id)`, `sceneAttacks`
   and `pendingOffers` are built per viewer: for a player, a party (attacker
   or target) is `{tokenId: null, label: "Unknown"}` when its name is hidden
   from players or none of the player's controlled tokens sees it under
   `thunderforge_canvas_core::vision::visibility_of`, against the scene's
   walls, lights, carried lights, ambient light and each eye's declared
   darkvision. A redacted attacker also withholds the ability's name; a
   redacted target withholds its defence. A token the player controls is
   always known to them. Game Masters see everything.
6. **An attack made offline is an intent, resolved at replay.** It is queued
   like a move (`thunderforge_cache_core::queue::ATTACK_INTENT_TYPE`) and
   made by `make_attack` against the state the server then holds; a turn
   refusal is reported as `NOT_YOUR_TURN` and spends nothing.

## Rationale

Constitution Principle III puts authority at the data boundary, and every
piece this needs already lives on the server: the dice crate with an injected
RNG, the hit-point operation, the turn check, and the same visibility function
the engine hides tokens with. One mutation keeps the order of judgements in
one place.

Rejected: a client that rolls and reports (it could claim any total);
overloading `rollDice` with a target (every free roll would pay for combat
checks, and the dice panel is not an attack); a world setting choosing between
"report" and "apply" (decision 1 moved the choice to the target's controller).

Redaction on read rather than in the payload follows the pattern chat and
hidden names already use, and is the only answer while events reach every
member. Redacting at write time was rejected: the same attack must read
differently to different viewers, and differently to one viewer after they
walk round a corner.

## Consequences

- **A stricter rule than spec 045's tokens.** Spec 045 hides an unseen token
  by not drawing it and still sends it. This withholds an unseen attacker's
  identity from the answer. A player reading network traffic can still find a
  hidden token's position in the scene's token list, and cannot learn that it
  attacked or with what. Withholding tokens themselves remains spec 045's
  out-of-scope item.
- **One visibility pass per viewer per scene per read.** Bounded by the
  viewer's controlled tokens and the attacks on the page (research R8).
- **Hit points change only by an operation somebody accepted**, or by the
  Game Master's own hand or auto-apply. Nobody is told by the software that
  they have been hit.
- **Structured attack fields land on abilities and items** (`reach`,
  `range_normal`, `range_long`, `needs_line_of_sight`, `action_cost`,
  `legendary_cost`, `multiattack`). Phase 7 measures reach and sight into the
  flags; Phases 8 and 9 spend the costs.
- **Imported monsters still arrive without structured attacks** (research R2's
  known gap); spec 047 turns creature entries into abilities.
