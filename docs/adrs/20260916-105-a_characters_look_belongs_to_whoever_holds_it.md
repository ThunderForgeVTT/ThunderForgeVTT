# ADR-105: A Character's Look Belongs to Whoever Holds It

**Date:** 2026-09-16
**Status:** **ACCEPTED** 2026-09-21, once spec 044 Phase 5 proved it (tasks T071, T078 and T079: the server rules for every clause of B6, the player e2e, and the proof run). Proposed 2026-09-16 with spec 044 phase (c); the grant and its two withdrawals are the owner's (spec 044 clarifications).
**Participants:** ThunderForgeVTT Team
**Related:** spec 044 (FR-030, FR-030a, FR-030b, FR-030c, FR-032, FR-033, FR-034, SC-010; contracts §5 B6), ADR-050 (permission declaration), spec 017 (actor claiming)

---

## Problem Statement

ADR-050's ladder — Viewer < Editor < Owner — answers one question about an
actor: may this person change it. A Game Master resolves to Owner on every
actor in their world. A player who has claimed a character resolves to
Viewer on it, because a claim says who *plays* a character, not who may
rewrite it.

Spec 044 gives players a hero builder, and the owner's direction is that the
player who plays a character should be able to give it a face. On the ladder
as it stands they cannot: `uploadActorImage` and `removeActorImage` required
Editor.

## Decision

1. **A live claim confers one right the ladder does not: changing that
   actor's portrait and token.** Implemented once, in
   `src/server/src/auth/actor_imagery.rs`, and called by exactly the two
   imagery mutations. Editor or above may, always; otherwise the caller may
   when they hold the claim, the world allows it, and the character is not
   locked.

2. **The right is imagery-only.** It is not an Editor grant row and it does
   not raise the claim holder's level; every other actor mutation still asks
   the ladder and still refuses them.

3. **The Game Master withdraws it two ways, both immediate:** a world setting
   (`worlds.allow_player_actor_art`, on by default) and a per-character lock
   (`world_actors.art_locked`, off by default). Neither touches the Game
   Master's own authority. Each refusal has its own message and a stable
   `reason` extension (`NOT_HOLDER`, `PLAYER_ART_OFF`, `ART_LOCKED`); a lock
   is reported before the world setting when both apply.

4. **The client is told the same answer the server enforces**, as
   `GraphQLWorldActor.myMayChangeImagery`, computed by the same function.
   A player reaches only an actor's view page (a Viewer is redirected away
   from edit), so the imagery panel is mounted there, gated on that field.

## Rationale

- **Why a right beside the ladder.** A claim is already the relation that
  means "this is my character"; it is created, released and raced over in
  one place (spec 017, spec 031 FR-034). Deriving the look right from it
  means releasing a character withdraws the right with no second record to
  keep in step.
- **Why not an Editor grant.** Editor carries the label, the sheet, the
  abilities and the inventory. FR-030 gives the picture and nothing else. A
  narrower rung on the ladder would change the meaning of every existing
  `myPermissionLevel` check for one feature.
- **Why a setting and a lock rather than an approval queue (FR-030c).** A
  queue makes every change wait on the Game Master and needs a pending state,
  a notification and a review screen, for a change that is cheap to undo: the
  Game Master can replace any look at any time. The two switches cover the
  cases the owner named — a table that wants the Game Master to own every
  image, and one player whose choices do not fit — without making the common
  case slower.

## Consequences

- Two columns and two Game-Master-only mutations
  (`updateWorldAllowPlayerActorArt`, `setActorArtLocked`), both refused while
  the world is paused.
- A collection copy does not carry `art_locked`; the lock is a decision about
  one table, and a copied character starts unlocked.
- Tests: `actor_imagery_tests.rs` checks each clause against both the gate and
  a real mutation; `e2e/hero-builder-player.spec.ts` tries every refusal
  through the page and by calling the mutations directly.

## Alternatives Considered

- **Grant Editor on claim.** Rejected: hands over the sheet (Decision 2).
- **A fourth ladder level ("Look").** Rejected: every permission check in the
  app would have to learn it, for two mutations.
- **Approval queue.** Rejected (FR-030c), see Rationale.
- **Client-side hiding only.** Rejected by Constitution Principle III: the
  server refuses regardless of what is offered.
