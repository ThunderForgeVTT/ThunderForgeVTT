# Contract: Server mutations

The authorization boundary for this feature. Every operation below is enforced
server-side; clients may apply optimistically but the server decides
(Constitution III). Shapes are described by intent and outcome, not by schema
syntax — the generated schema is the implementation of this contract.

---

## Pick up a placed item

**Intent**: a player takes an item from the map into their inventory.

**Requires**: the caller may act for the receiving character, and the item is
still on the scene.

**Effects, all-or-nothing**: the scene token is removed, and one inventory
entry is created for the receiving character.

| Outcome | Response | Client behaviour |
|---|---|---|
| Accepted | the resulting inventory entry | keep the optimistic removal |
| Already taken | a distinct "no longer there" refusal | restore the token; tell the player it is gone |
| Not permitted | refusal | restore the token |

**Concurrency (FR-016)**: two callers picking up the same item must produce
exactly one inventory entry. Resolved at the database boundary, the same way
spec 017 resolves two players claiming one character — not by client ordering.

**Must not**: leave a token removed with no inventory entry, or create an entry
while the token remains.

---

## Place a token from an actor

**Intent**: a Game Master (or a permitted player) puts an actor's token on the
current scene at a chosen position.

**Requires**: the caller may create a token for that actor in that scene — the
existing token ownership rules, unchanged.

**Effects**: one token is created, snapped per the scene's grid rules.

**Must not**: create a token for a cancelled placement, or accept a position
the grid rules would not permit for a drag (FR-006).

---

## Set a player's character binding

**Intent**: a Game Master binds a player to a character from the players
section.

**Requires**: caller is GM/Owner of the world.

**Consistency (FR-034)**: this is a **third writer** to the same relation the
actor page and the player's own claim already mutate. All three must agree on
who wins; concurrent binding must not leave a character claimed twice or a
player bound to two characters.

---

## Upload actor imagery

**Intent**: store a portrait or token image for an actor.

**Requires**: caller may edit the actor.

**Effects**: the uploaded bytes are converted and stored, and recorded against
the actor under the given role, replacing any existing image for that role.

**Reuses**: the existing image conversion and object-storage path — the same one
lore images use, including its size limit, permission checks and rejection of
oversized uploads.

---

## Record an item price

**Intent**: a Game Master notes a price, or a suggested price, for an item.

**Requires**: caller may edit the item.

**Effects**: one price per item, replacing any previous value.

**Explicitly not**: a transaction, a currency system, or an input to any
system's economy (research R5).

---

## Organise lore

**Intent**: move a lore entry within the tree, and add or remove tags.

**Requires**: caller may edit the entry.

**Must reject**: a move that would create a cycle. Deleting a parent must not
orphan children.

---

## Preload a scene

**Intent**: prepare a scene on the caller's own client without changing the
table.

**Effects**: **none that any other client can observe** (FR-020, SC-004). This
is deliberately not a mutation of the world's active scene — Launch remains the
only operation that changes it (research R1).

---

## Bring the party across a scene change

**Intent**: when changing scenes, carry the selected player characters.

**Blocked on** the ADR in Constitution Check IV.1 — the operation's shape
depends on whether tokens are re-created or follow the party.

**Invariant either way**: a character that already has a token in the
destination scene does not gain a second one.
