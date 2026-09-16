# ADR-100: An Operator Can Pause a World's Play

**Date:** 2026-09-13
**Status:** **ACCEPTED** by the accountable owner, 2026-09-13, the day it was proposed.
**Participants:** ThunderForgeVTT Team
**Related:** spec 051 (decisions 1–3, FR-001 … FR-064), spec 015 T042, spec 042, ADR-077, ADR-078, ADR-099, `src/server/src/graphql/session_lifetime.rs`

---

## Problem Statement

Every authority inside a world comes from a world role (ADR-099): Owner, Game
Master, Trusted Player, Player. Site admins are not a role in that ladder; the
membership helpers treat an admin *as* an Owner (`actor_in_world` returns
`Actor::site_admin()`, and `is_dm_of_world`/`is_dm_of_scene` short-circuit on
`is_admin`).

Nothing can stop a world's live play from the server. Spec 015 T042 found that a
takedown on a scene being played is only "gone on your next page load", and the
same gap leaves an operator unable to stop abuse at a table or a compromised
account running a session.

Spec 051 asks for an operator to pause a world's play, and for no role inside the
world to be able to undo it.

## Decision

1. **A pause is a site-level lock on one world's live play**, held in
   `world_play_pauses`, and only operators (`is_admin`, 2FA mandatory) can
   create or lift one. No world role, the Owner included, can lift it.
2. **The lock is enforced by a gate that is blind to `is_admin`.**
   `play_pause::refuse_if_paused` is called beside the existing role checks at
   every world-scoped write and every entry point that starts or continues play,
   never inside `actor_in_world` or `is_dm_*`. An operator in their own paused
   world is refused like anyone else. This deliberately narrows the admin-as-Owner
   short-circuit for this one purpose.
3. **The list of gated entry points is closed by a test.**
   `play_pause_surface_tests` requires every root `Mutation` and `Subscription`
   field to be classified as gated, not world-scoped, or operator, and calls
   every gated field against a paused world. The pattern is the one
   `admin_surface_tests` and `disabled_surface_tests` already use.
4. **Live streams are ended by the existing five-second database poll**, extended
   from *session still live* to *session still live and world not paused*, which
   yields a final `WORLD_PLAY_PAUSED` error before completing. A world event makes
   the common case sub-second. Nothing depends on an in-process signal.
5. **A takedown proposes; it does not pause.** A takedown on content of a world
   in live play raises a request that an operator approves or declines. "In live
   play" is a durable, throttled per-world heartbeat mark, so the decision does
   not depend on which process filed the notice.
6. **A pause and a takedown are independent.** Lifting restores play, not
   content. Restoring content does not lift a pause. `moderation` never calls
   into `play_pause` on restoration.
7. **The record outlives the world and the operator's account.** No foreign keys
   to `worlds` or `users`, with name snapshots, like `content_moderation_actions`.
   Members read *that and when* only; grounds, triggers and declined requests are
   operator-only by construction of the queries.

## Consequences

- **Honest limits.** The server cannot erase content a browser already holds,
  including the offline world cache, nor cut a WebRTC peer channel already open
  between two modified browsers. The pause stops everything the server sends or
  accepts; honest clients also close their peer connections.
- **A new habit for resolvers.** Every new world-scoped root field must be
  classified, and if it writes or plays, must call the gate. The surface test
  makes forgetting a build failure, not a silent hole.
- **Offline changes queued against a paused world are discarded**, with the count
  shown, rather than held until a lift.
- **The unused `/api/events/{world_id}` socket is removed.** It checks login but
  not membership, and nothing connects to it.
- **The heartbeat writes a row again**, at most every 30 s per world per process,
  reversing part of its "in memory, not in a row" rationale for a fact another
  process needs.

## Alternatives Considered

- **A Postgres trigger refusing writes to world tables while paused.** The
  strongest backstop. Rejected for now: many play tables reach their world only
  through a scene (a join per write), it would block the world's own deletion
  cascade, and it cannot end streams, so the GraphQL gate is needed regardless.
  **This is the next step if the surface test is ever found to have missed a
  path.**
- **Putting the check inside `require_world_member`/`is_dm_*`.** One edit
  instead of many, but it would also refuse the reads spec 051 keeps (FR-024), and
  the admin short-circuit would skip it.
- **Revoking every member's login.** Signs people out of the whole instance for
  one world, and conflates a world lever with ADR-077's account lever.
- **An in-process broadcast closing sockets.** No per-world socket registry
  exists, and it would miss other processes.
- **Automatic pause on takedown.** Rejected by spec 051 decision 2: a pause with
  no decision is what the owner ruled out.

## Found in implementation

Recorded 2026-09-16, after User Stories 1 to 5 shipped.

**What the surface test caught.**

- **`renameWorld` answered anyone.** Any signed-in caller who named a world's
  id got the world back; the update matched nothing but the reply did not say
  so. Found when a site admin who is a Player member "got through" the gate.
  It now refuses a non-owner (commit `0f9e6f0`).
- **A pack's own fields were invisible to it.** The server's closed-list test
  reads the server's schema, and a system pack's root fields are merged only
  in the app crate, so Genie's thirteen session mutations still moved a
  paused world's state. Packs now declare a `PackSurface` through `inventory`,
  and a second test in `src/app` reads the merged schema by introspection
  and calls every gated pack field on a paused world (commit `4c568ba`).
  `packs/systems/README.md` makes the classification part of the pack
  contract.
- **Two world-token routes let any account write into any world.**
  `createWorldToken` and `upsertWorldToken` checked sign-in and, once gated,
  the pause, but never membership; the gate was the first check they had.
  Nothing called them and ADR-040 had retired their table, so they were
  removed rather than fixed (commit `e9d97c2`).

**Gated fields that surprised.**

- `worldActorSystemDataUpdated` was a stub stream with no membership check at
  all. It now opens only for a member of an unpaused world.
- `reconcileQueuedChanges` cannot simply refuse: a refusal reads to the
  client as a lost connection and is retried. It answers every queued change
  `PLAY_PAUSED` and applies nothing, and the notice says how many were not
  kept.
- `worldSyncPlan` is fetched by the engine rather than the GraphQL client, so
  its refusal arrived only as a degraded sync summary. That turned out to be
  the fastest path off the playfield: a page leaves 46–49 ms after a pause,
  before event 28 reaches it. The five-second liveness tick is proven
  separately, against a client with no logic (`play-pause-stream-poll.spec.ts`).
- Map import is the only world-scoped REST write. Image uploads are GraphQL
  mutations and are gated with the rest.
- A few writes are left open on purpose: `deleteWorld`, `removeCompendium`
  (the book belongs to the importer's shelf, not the world) and
  `deactivateLoreSync` (operators only).

**Whether the trigger backstop is still unneeded.** Yes, with one known gap.
Every path the surface tests missed was a *classification* gap: a field that
was never listed, not a listed field that let a write through. The closed
lists catch the first kind at build time, and the pack surface closes the one
place the lists could not see. A Postgres trigger would have caught none of
the three findings above, since they were about who may call, not whether
the world was paused.

The gap is timing. A mutation that checks the gate and then writes in
separate statements can be beaten by a pause landing between the two;
`create_light_source` is the recorded example. The owner decided on
2026-09-15 to fix this in one pass that wraps gate and write in a transaction
for every mutation with that shape, rather than one mutation at a time. The
cost is a single stray write in a window of milliseconds, not a game played
on after a takedown. A trigger would also close that window, which is why it
stays the named next step if that pass is never made.
