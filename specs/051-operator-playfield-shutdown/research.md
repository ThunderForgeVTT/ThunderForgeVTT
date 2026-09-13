# Research: Pausing a World's Play

**Spec**: [spec.md](spec.md) · **Plan**: [plan.md](plan.md) · **Date**: 2026-09-13

Each section is a decision the plan rests on. The facts behind them were read
out of the code on 2026-09-13; file references are to that tree.

## What exists today, in one paragraph

Operators are `users.is_admin` (one concept, 2FA mandatory, guarded in GraphQL by
`helpers::admin_user` and on REST by `require_admin_user`). There is **no
operator task queue** (spec 042 is a draft) and **no general audit table**; each
domain keeps its own append-only record, and `content_moderation_actions`
deliberately has no foreign key so it outlives what it describes. Live play
travels on the `/api/ws` GraphQL socket as per-subscription streams
(`worldEventsCreated`, `playField`, `peerSignals`, `playersOnline`), each
authorised once at open; `session_lifetime::until_session_ends` is the only
thing that ends a stream mid-flight, by polling Postgres every five seconds. The
world has **no status column** and **no single world-access choke point**:
`is_dm_of_world` (~60 call sites), `is_dm_of_scene` (~50),
`require_world_member` (~31) and `actor_in_world` share the job, and site admins
short-circuit all of them except `require_world_member`. The takedown filing
(`submit_takedown_notice_impl`) already resolves every target to its world
(`resolve_entity_owner`) and has a non-fatal post-work hook for lore.

---

## R1. How a live table is reached within five seconds

**Decision**: Two signals, one fast and one enforcing.

1. **Fast, for honest clients**: pausing records a world event,
   `EVENT_CODE_WORLD_PLAY_PAUSED` (28), carrying only the time. The event
   listener's 100 ms poll delivers it to every open `worldEventsCreated`
   stream, and the client leaves the playfield on receipt.
2. **Enforcing, for every client**: `until_session_ends` is generalised into
   `until_stream_must_end(state, session_id, world_id, inner)`, whose five-second
   tick asks one query: *is this session still live, and is this world not
   paused?* When the world is paused the stream yields one final error carrying
   the extension code `WORLD_PLAY_PAUSED`, then completes. Every world-scoped
   subscription is wrapped in it, including `peerSignals`, which today is not.

**Rationale**: The five-second poll already exists, already works across
processes, and its constant was chosen for exactly this sentence ("I ended that
session" and "that screen stopped updating" are the same event). Folding the
pause into the same query keeps a table of ten at two queries a second rather
than four. The event makes the common case sub-second and costs nothing new.
The final error item, rather than a silent completion, is what lets the client
tell "paused" from "the server went away" (research found `subscribeToWorldEvents`
maps completion to "done" and the page silently freezes).

**Fail direction**: the poll keeps `until_session_ends`'s rule that an unreadable
answer counts as *not ended*. Its argument transfers unchanged: world events come
from the same database, so a client whose world was paused during an outage
receives nothing to be wrong about, and the next tick ends it. Every *write* and
every stream *open* fails closed, because they need the database anyway.

**Alternatives considered**:
- *An in-process broadcast to close sockets.* No per-world socket registry
  exists (`WorldRouter` can only subscribe/publish/reap), and an in-process
  signal misses other processes. Rejected for the reason `session_lifetime.rs`
  already records.
- *Revoking every member's login session.* Signs people out of the whole
  instance for one world; heavier than a pause and it is ADR-077's lever, not
  this one.
- *Closing the socket rather than the stream.* graphql-ws multiplexes the whole
  schema on one socket per tab; closing it would cut the person's other worlds'
  and admin traffic too. Ending the world's streams and refusing their reopening
  is the world-scoped act.

## R2. Where the lock is enforced

**Decision**: A new gate, `play_pause::refuse_if_paused(conn, world_id)`, called
explicitly at every world-scoped entry point that starts or continues play or
writes the world, and a **surface test** that makes the list closed.

- **Refused while paused** (error code `WORLD_PLAY_PAUSED`):
  - opening any world-scoped subscription (`worldEventsCreated`,
    `playersOnline`, `playField`, `peerSignals`);
  - `heartbeat`, `worldEventsSince`, `worldSyncPlan`, `launchScene`;
  - `reconcileQueuedChanges` (per change, see R5);
  - **every root mutation that writes world-scoped data**: tokens, walls,
    lights, interactives (including `activateInteractive`), combat, chat, rolls,
    scenes, compendium, lore, staging, the library's per-world book list, world
    settings, invitations and membership changes;
  - world-scoped REST writes: scene/asset uploads, map import.
- **Still answered while paused** (FR-024 and the spec's *readable, not editable*
  default): the world's name and members, the pause's *that and when*, and
  read-only queries of compendium, lore and staging.
- **The operator's own world**: the gate ignores `is_admin`. Site admins
  short-circuit `actor_in_world` and `is_dm_*`; the gate is called beside those
  checks, not inside them, so an operator in their own paused world is refused
  like everyone else and lifts the pause from the admin portal.

The surface test (`graphql/play_pause_surface_tests.rs`) follows
`admin_surface_tests` and `disabled_surface_tests`: it introspects the schema's
root `Mutation` and `Subscription` fields and requires each to be classified in
one of three tables: *gated*, *not world-scoped*, or *operator*. An unclassified
field fails the build. A second half runs each gated field against a paused
world and asserts the refusal.

**Rationale**: There is no choke point, and making one inside
`require_world_member`/`is_dm_*` would also refuse the reads FR-024 keeps, and
would be bypassed by the admin short-circuit. The project already solves "a
rule that must hold at every one of many resolvers" with a closed-list test that
fails on an unclassified field; this is the third use of that pattern, not a new
invention.

**Alternatives considered**:
- *A Postgres trigger refusing writes to world tables while paused.* The truest
  backstop, and this arc used triggers for the origin rule. Rejected for now:
  many play tables reach their world only through a scene, so a trigger per
  table needs a join per write; it would also refuse the world's deletion
  cascade and system writes; and it cannot gate streams or reads at all, so the
  GraphQL gate is needed regardless. Recorded in the ADR as the next step if the
  surface test ever proves insufficient.
- *An async-graphql extension that inspects arguments named `worldId`.* Magic
  keyed on an argument name; misses mutations that take a `sceneId` or
  `tokenId`. Rejected.

## R3. What "in live play" means for a takedown

**Decision**: A world is *in live play* when any member's heartbeat has reached
the server in the last **45 seconds**. The heartbeat marks this durably, at most
once every 30 seconds per world per process, in `world_live_play(world_id,
last_beat_at)`. A takedown on any content of a world in live play raises a
request; so does a takedown on an adopted copy in another world
(`reach::fan_out_disable`) when *that* world is in live play.

**Rationale**:
- The in-memory presence registry is accurate but per process, and a DMCA
  decision must not depend on which process filed the notice. A request that is
  missed leaves exactly the gap this spec closes; a request raised a minute late
  or unnecessarily costs an operator a glance.
- The heartbeat's own comment argues against a row per beat (a WAL record and a
  dead tuple every five seconds per client). A throttled per-world mark is one
  write every 30 seconds per *world*, not per client, and records a fact another
  process genuinely needs. The heartbeat comment will be amended to say so.
- **World-level, not scene-level.** The world sync plan loads a world's content
  (compendium, lore, visible scenes) into every live client, so "content loaded
  by a live session" is, honestly, "content of a world with a live session".
  Trying to say which scene or entry a particular browser has loaded would claim
  a precision the client cache makes false. This refines the spec's assumption
  rather than contradicting it.
- FR-031's *whether the world is being played when the operator looks* reads the
  same mark at read time.

**Alternatives considered**: `worlds.active_scene_id` (every launched world has
one, so it means nothing); the in-memory presence registry (per process);
`players_online` (written only by the unused `/api/events` socket).

## R4. Requests, their races, and gathering triggers

**Decision**: The first operator request table, shaped generically (FR-037).

- `world_play_pause_requests` has a partial unique index *one pending request per
  world*. Raising a trigger is `INSERT … ON CONFLICT DO NOTHING` then attach the
  trigger to whichever pending request exists (FR-033).
- If the world is already paused, the trigger is attached to the active pause
  instead and no request is created (FR-036).
- Deciding is a single conditional update, `UPDATE … SET state = $decision,
  decided_by, decided_at WHERE id = $1 AND state = 'pending' RETURNING …`. Zero
  rows means somebody else decided first; the loser reads back `decided_by` and
  `decided_at` and is told (FR-035).
- `world_play_pauses` has a partial unique index *one active pause per world*;
  an approval or an immediate pause that loses a race to another pause attaches
  its grounds and triggers to the winner rather than failing.

**Rationale**: Research found the existing decisions (`legal_enquiries`, appeals)
check-then-update without a precondition on the write, so two operators can both
pass. The conditional update is the smallest thing that is actually correct. The
partial unique index is the `account_terminations_one_open` pattern already in
the schema. Keeping `trigger_kind` an open enum (`takedown`, `operator`,
`abuse_report`) lets spec 042's queue or an abuse intake raise requests later
without a migration to the request shape.

**Raised where**: after `submit_takedown_notice_impl`'s blocking work returns,
beside the lore hook, and non-fatal in the same way: the takedown has already
taken effect and must not be undone because a request could not be written. A
failure is logged at `error` with the moderation action id so it can be raised by
hand. The takedown's own inserts are not in a transaction today; this plan does
not change that.

## R5. Offline changes queued against a paused world

**Decision**: `reconcileQueuedChanges` rejects every change in the batch with a
new reason, `PlayPaused`, added to both `thunderforge-cache-core::RejectionReason`
and the server enum. The client **discards** the rejected changes (the existing
`revert` path), and the notice tells the person how many of their offline changes
were not kept.

**Rationale**:
- Throwing an error would leave the changes queued and retried on every
  reconnect, forever (research: `offlineQueue.ts:311-320`).
- Holding them until the pause lifts would apply, days later, edits made against
  a world an operator stopped, which is precisely the path a compromised account
  would use to carry on. FR-023 says *not applied*, and discarding is the only
  reading that stays true after a lift.
- A distinct reason, not `PermissionDenied`, is what lets the notice say what
  happened instead of "you no longer have permission", which would be both wrong
  and alarming.

**Heartbeat**: today three failed beats switch the client to offline queueing, so
a refused heartbeat would silently turn a paused world into offline play. The
heartbeat's refusal carries `WORLD_PLAY_PAUSED`, and `heartbeat.ts` treats that
code as *paused*, never as a network failure.

## R6. What the table sees, and how it learns of a lift

**Decision**: A route outside the playfield, `/world/:id/paused`, reached from
any of the three signals (the world event, a stream's final error, a refused
heartbeat or reconcile). It shows the notice (FR-010 to FR-013), closes the
page's peer connections, and asks `worldPlayState(worldId)` again every 30
seconds and on focus, returning the person to the world when the pause is
lifted. The world page and the world list show the same *that and when* to every
member (SC-004).

**Rationale**: Both existing "the server ended this" patterns (a 401, a disabled
account) navigate away; staying consistent with them keeps the playfield's own
teardown simple. No stream is open while paused, so the lift cannot be pushed;
a 30-second read of a tiny query is the honest substitute.

**Peer channels**: `peerSignals` is gated and ended like every stream, and honest
clients close their WebRTC peer connections on the notice. A peer channel already
open *between two modified browsers* carries bytes the server never sees and
cannot cut. This is the same boundary as content already downloaded, and is
stated as such rather than promised away.

## R7. The unused `/api/events/{world_id}` socket

**Decision**: Remove the route and `network/ws.rs`'s handler in this feature.

**Rationale**: It is mounted (`main.rs:648-668`), checks login but **not world
membership**, and nothing in `apps/` or `packages/` connects to it. It is a live
path a pause would otherwise have to gate, into a world the caller may not even
belong to. Removing dead, under-authorised surface is smaller and safer than
gating it. The `players_online` table and `session::connect_player` go with it
only if nothing else reads them; the tasks verify before deleting.

## R8. The record, and who can see which part

**Decision**:
- `world_play_pauses`, `world_play_pause_requests` and `world_play_pause_triggers`
  have **no foreign key to `worlds` or to `users`**. They store `world_id` with a
  `world_name` snapshot, and actor ids with a name snapshot, so the record
  outlives both the world (FR-053) and the operator's account. This follows
  `content_moderation_actions`, not the `SET NULL` audit tables, because a record
  of who paused a table must not lose its *who*.
- Grounds, triggers and declined requests are readable only through
  `admin_user`-guarded fields. The member-facing `worldPlayState` reads only
  `paused_at` and `lifted_at` from `world_play_pauses`. It cannot leak a declined
  request or a reason, because it never selects from the tables that hold them.
- Provenance (Principle III): `created_by`/`updated_by` on all three tables, with
  the semantic columns (`paused_by`, `lifted_by`, `decided_by`) beside them.

## R9. An ADR

**Decision**: ADR-100, *An operator can pause a world's play*, drafted as
**Proposed** with this plan.

**Rationale**: Constitution Principle IV requires an ADR for a change to an
ownership boundary. Until now every authority inside a world came from a world
role, and site admins acted *as* an Owner. This gives operators a power no world
role can override or undo, and deliberately makes the gate blind to `is_admin`.
It also records R2's rejected trigger backstop and R3's durable live-play mark.
