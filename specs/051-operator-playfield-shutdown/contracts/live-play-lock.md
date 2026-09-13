# Contract: The live-play lock

**Spec**: FR-020 to FR-024 · **Research**: R1, R2, R5, R6, R7

## The gate

```rust
// src/server/src/play_pause/gate.rs
pub fn refuse_if_paused(conn: &mut PgConnection, world_id: Uuid) -> Result<(), PlayPaused>;
pub fn refuse_scene_if_paused(conn: &mut PgConnection, scene_id: Uuid) -> Result<(), PlayPaused>; // resolves the scene's world
impl From<PlayPaused> for async_graphql::Error // code WORLD_PLAY_PAUSED
```

- Reads only the partial unique index on `world_play_pauses`. Fails **closed**:
  an unreadable answer refuses.
- **Blind to `is_admin`.** It is called next to the existing role checks, never
  inside `actor_in_world` or `is_dm_*`, so the site-admin short-circuit does not
  skip it.
- Called **before** any write, inside the same connection the resolver uses.

## The closed list

`src/server/src/graphql/play_pause_surface_tests.rs` has three tables, and every
root `Mutation` and `Subscription` field of the schema must appear in exactly one:

| Table | Meaning | Examples |
|---|---|---|
| `GATED` | world-scoped; calls the gate | `heartbeat`, `launchScene`, `moveOwnToken`, `activateInteractive`, `reconcileQueuedChanges`, `worldEventsCreated`, `peerSignals`, `playField` |
| `NOT_WORLD_SCOPED` | touches no single world | `login`, `updateProfile`, `submitTakedownNotice`, library shelf fields |
| `OPERATOR` | admin surface; must not be gated | `pauseWorldPlay`, `liftWorldPlayPause`, `decidePlayPauseRequest` |

Plus world-scoped **queries that start play**, listed explicitly in `GATED`:
`worldSyncPlan`, `worldEventsSince`. Read-only world queries are not gated
(FR-024, and the spec's *readable, not editable* default).

Half one fails on any unclassified field. Half two pauses a seeded world and calls
each `GATED` field as its Game Master and as a site admin who is a member, and
asserts `WORLD_PLAY_PAUSED` for every one.

World-scoped REST writes (scene and asset uploads, map import) call the gate in
their handlers and answer `423 Locked` with `{"code":"WORLD_PLAY_PAUSED"}`. A
REST test covers each.

`/api/events/{world_id}` is removed (research R7).

## Streams

```rust
// src/server/src/graphql/session_lifetime.rs
pub fn until_stream_must_end<S, T>(state: AppState, session_id: Uuid, world_id: Uuid, inner: S)
    -> impl Stream<Item = async_graphql::Result<T>>
```

- Opening: the subscription resolver calls the gate first; a paused world never
  opens a stream.
- Every tick (`LIVENESS_POLL`, 5 s): one query for *session live AND world not
  paused*.
  - Session gone → stream completes, as today.
  - World paused → the stream yields **one** `Err` with code `WORLD_PLAY_PAUSED`,
    then completes.
  - Unreadable → carries on (see research R1 for why this one direction fails
    open).
- Wraps: `worldEventsCreated`, `playersOnline`, `playField`, `peerSignals`.
  `until_session_ends` remains for streams that are not world-scoped.

## Offline reconciliation

`reconcileQueuedChanges(worldId, changes)` on a paused world returns a report in
which every change is `rejected` with reason `PlayPaused`. It does not throw
(throwing leaves changes queued and retried forever). Nothing is applied.

## Client signals

Any of these sends the page to `/world/:id/paused`:

| Signal | Source |
|---|---|
| world event code 28 | `worldEventsCreated`, within ~100 ms |
| a stream error with `WORLD_PLAY_PAUSED` | any wrapped subscription, within 5 s |
| `heartbeat` refused with `WORLD_PLAY_PAUSED` | `heartbeat.ts`; must **not** count as a network failure or switch to offline queueing |
| any mutation or query refused with `WORLD_PLAY_PAUSED` | `graphqlClient.ts`, handled once centrally |
| reconcile report containing `PlayPaused` | `offlineQueue.ts`; the rejected changes are reverted and counted |

On arrival at the notice the page closes its peer connections and its
`graphql-ws` world subscriptions.

## The notice

`/world/:id/paused`, a page outside the playfield.

- **Says**: play in *World name* has been paused by an operator of this
  instance, since *time*. What you can do: go to your worlds; this page will
  notice when play resumes. If offline changes were dropped: "*N* changes you
  made while offline weren't kept."
- **Never says**: why, who, a trigger, the word "takedown", "report",
  "violation" or anything that reads as blame (FR-011).
- **Polls** `worldPlayState` every 30 s and on window focus; when `paused` is
  false it offers *Return to the world*.
- **Accessibility**: WCAG 2.2 AA; the heading receives focus on arrival and the
  status is in a polite live region; text holds at 200% zoom and meets contrast
  at the room-screen size (SC-008). The e2e suite has no automated
  accessibility audit today; this feature adds `@axe-core/playwright` as a dev
  dependency of `apps/web` and runs it on the notice and the operator page.
