# ADR-102: A Token Is Its Actor, or a Copy of It

**Date:** 2026-09-14
**Status:** **ACCEPTED** 2026-09-14, once spec 046 Phase 6 proved it (tasks T070: the e2e, the server rules and the playtest).
**Participants:** ThunderForgeVTT Team
**Related:** spec 046 (FR-015, FR-016, FR-017, SC-008, clarification Q4), spec 046 research R4, R5, R7, R17, `specs/046-a-fight-that-resolves/data-model.md`, spec 029 (token status), ADR-056 (imagery hangs on the actor)

---

## Problem Statement

A creature on the board had two records of its hit points that could
disagree. The actor's system data (`world_actor_system_data.resource_data`)
drives the sheet and, since spec 029, the bars. The token also carried
`tokens.health` and `tokens.max_health`, which no screen wrote, the engine
read into an unread `DerivedStats.is_dead`, and the token panel drew as a
second, separate bar.

Neither record could hold a crowd. Every token of a goblin pointed at the
goblin's actor, so five goblins placed from one NPC shared one pool: a hit on
one moved all five bars, and zero marked all five out. The only way to give
each its own hit points was an actor per goblin, which is a sheet, a roster
entry and a row of system data for every body on a battle map. The owner's
case is two hundred goblins that need no actors of their own, beside a named
NPC such as "Boblin the goblin" whose token is the one actor he is.

## Decision

1. **Every token is linked or a copy.** `tokens` gains
   `linked BOOLEAN NOT NULL` and `system_data JSONB NULL`, with
   `CHECK (linked = false OR system_data IS NULL)`.
   - A **linked** token has no data of its own. Its hit points are its
     actor's `resource_data`: damage writes there, bars read from there,
     and the sheet and the token are the same creature.
   - An **unlinked copy** holds `system_data`, a `resource_data`-shaped
     object seeded from its actor at the moment it is placed. Damage writes
     there, bars read from there, and neither the actor nor any other copy
     changes.
2. **Placement decides, on the server.** `createToken` resolves the default
   from the actor: a character is linked; an NPC is a copy unless it is
   marked unique (`world_actors.is_unique`); a token with no actor is a
   marker (unlinked, no data, no bars). A Game Master may choose otherwise
   when placing, and may change a placed token with `setTokenLink`. When the
   caller names no token type, the type is the actor's.
3. **Relinking replaces, it does not merge.** Linking a copy discards its
   `system_data`; unlinking a linked token seeds `system_data` from the actor.
4. **`tokens.health` and `tokens.max_health` are retired.** A migration
   copies any non-null value into an unlinked token's `system_data` through
   the hit-point fields the pack declares, then drops both columns. The
   engine's `health` fields and `DerivedStats.is_dead`, the legacy bar in the
   token panel, and the orphan `token_systems.rs` go with them.
5. **`tokens.actor_id` becomes a foreign key** to `world_actors`,
   `ON DELETE SET NULL`. A copy whose actor is deleted keeps its own data and
   is still a creature; a linked token whose actor is deleted is a marker.
6. **One operation writes hit points** (`combat::hit_points`,
   research R5). For a copy it locks the token row and records event 14
   (token changed); for a linked token it locks the actor's system-data row
   and records event 26 (sheet changed). A copy's combatant is matched by its
   token alone, never by the actor it was copied from.

## Rationale

Two pools that can disagree is the defect FR-015 names, so the fix is one
pool per creature rather than a rule for reconciling two. Copies must not
need actors (FR-017), and copies of one NPC must not share a pool, so the
copy's pool lives with the copy.

Keeping it on the token row keeps a scene of two hundred copies a single-table
read for `tokenStatus` (research R17). A side table of token system data was
considered and rejected: a token has at most one record, so the table would
be a join per read for nothing. Keeping `tokens.health` as the copy's pool was
rejected because a pack's resource is a structure (current, maximum,
temporary), not one integer.

Deciding the default on the server means every client that places a token —
the panel, the actor page, a script — gets the same answer, and a player
character can never arrive as a copy that silently stops following its sheet.

## Consequences

- **A copy stops following its NPC.** Editing the goblin's sheet after
  placement changes future copies, not placed ones. That is what a copy is;
  relinking is the way back, and it takes the NPC's values.
- **Other places tokens are inserted keep today's meaning.** The column
  defaults to `true`, so a token written by a path that does not decide
  (bringing the party, tests, older seeds) reads its actor, as every token did
  before this. `createToken` always decides explicitly.
- **Existing rows are backfilled by what their actor is**, not by
  `token_type`: a token of a player character is linked, a token of an NPC
  becomes a copy seeded from that NPC's current data, and a token with no
  actor is a marker. `token_type` was not trusted because NPC tokens were
  being written as `character` (research R4).
- **Only a resource slot can be copied.** A pack that declares its hit points
  in another slot has no damage operation on copies, and says so.
- **Controllers are unchanged** (research R7): a copy's controllers are its
  `owner_user_id` and Owner-permission holders on its actor.
