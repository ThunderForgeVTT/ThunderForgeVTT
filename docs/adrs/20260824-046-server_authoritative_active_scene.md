# ADR-046: Server-Authoritative Active Scene, Broadcast Over the Existing World-Events Transport

**Date:** 2026-08-24
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team

---

## Problem Statement

Spec 022 (Scene Management Overhaul, User Story 1) requires that a GM's
"Launch" action on a scene immediately unload/load that scene for
*every world member currently in an active Play session* — not just the
GM's own browser tab.

Today, "which scene is loaded" is 100% local, per-browser-tab React
state (`WorldPage.tsx`'s `selectedSceneId`), set only by the
`SceneSwitcher`'s local `onValueChange`. There is no server-side concept
of a world's "current" scene at all, and changing scenes in one tab does
nothing for any other connected client (confirmed by research: no
`active_scene_id`-shaped column, no mutation, no `world_events` row tied
to scene selection exists anywhere in the codebase today).

This crosses an ownership boundary the constitution calls out explicitly
(Principle IV): introducing a new piece of server-authoritative state
(which scene is "live" for a world) and a new live-broadcast event type
is architecturally significant enough to require this ADR before
implementation, not a decision to make silently inside a migration.

## Decision

**Make the active scene server-authoritative, and broadcast changes to
it over the already-live `world_events` transport** — not a new
transport, not client-local-only state.

1. Add `worlds.active_scene_id UUID NULL` (migration
   `src/server/migrations/<ts>_add_world_scene_settings/`). `NULL` means
   "nothing launched yet" — Play renders an empty/unloaded canvas rather
   than defaulting to the world's first scene (spec.md Clarifications).
2. Add `launchScene(worldId, sceneId)` — GM/Owner-only (`is_dm_of_world`
   pattern, consistent with every other DM-only mutation in this
   codebase), validates the scene belongs to the world, sets
   `active_scene_id`, and calls the existing `record_world_event()`
   (`src/server/src/world_events.rs`) with a new event code. That
   function already does the real work this feature needs: it inserts an
   audit row and issues `pg_notify('world_events_channel', ...)`.
3. Client-side, `WorldPage.tsx` already keeps an open
   `subscribeToWorldEvents(worldId)` WebSocket/GraphQL-subscription loop
   (feeding the existing wall/token/light/shape live-sync handlers under
   `apps/web/src/engine/world/sync/`). This gets one more handler: on the
   new event, call `setSelectedSceneId(sceneId)` — the exact state
   variable that already drives every scene-content loader effect in
   that file. No new subscription, no new WebSocket connection, no new
   client-side polling.

This was verified to be a real, already-live transport (not aspirational
scaffolding) before this decision was made: `record_world_event` →
`network/listener.rs`'s Postgres-polling `LISTEN` task →
`broadcast::Sender<WorldEvent>` → `SubscriptionRoot::world_events_created`
→ `/api/ws` (`async-graphql-axum::GraphQLWebSocket`) →
`apps/web/src/engine/world/sync/subscriptionClient.ts`'s `graphql-ws`
client. This exact chain already carries live wall/token/light/shape/
genie-session traffic in production Play sessions today.

## Alternatives Considered

- **Poll for the active scene on an interval**: rejected — strictly
  worse latency than the already-available push transport for no
  benefit, and the spec's SC-006 ("every currently-connected member is
  viewing the newly-launched scene within seconds") doesn't require
  accepting polling's inherent lag.
- **A dedicated WebSocket/channel just for scene-launch events**:
  rejected — redundant with the general-purpose `world_events` channel
  already carrying structurally similar broadcast-to-all-world-members
  events; would duplicate the entire NOTIFY→broadcast→subscription→
  WebSocket chain for no functional gain.
- **Keep scene selection purely client-local and have each client poll
  `GET scene` / re-query on an interval to "catch up"**: rejected — this
  is not "live" in any meaningful sense and directly contradicts the
  spec's explicit requirement that a switch "takes effect immediately...
  with no separate rejoin step required."
- **Route scene switching through the Bevy world-store bridge's existing
  sync primitives instead of `world_events`**: considered, but the
  world-store bridge (Constitution Principle I) is scoped to
  *simulation content* (tokens, walls, lights, fog) within an already-
  loaded scene, not to the meta-decision of *which* scene is loaded.
  Reusing `world_events` for a world-level (not scene-content-level)
  broadcast keeps that boundary intact — the engine still owns loading
  the chosen scene's contents once `sceneId` changes; the event system
  only carries the *decision* of which scene that is.

## Consequences

- `worlds.active_scene_id` is the first server-side, cross-client
  "what's currently being played" concept in this codebase. Any future
  feature that cares about "the world's current scene" (spectator views,
  a session-log timeline, etc.) should read this column rather than
  inventing a parallel notion.
- The `world_events` event-code catalog (`world_events.rs`) gains one
  more entry; future additions to that channel should continue to be
  narrow, minimal-payload events (`{ worldId, sceneId }` here), not a
  place to smuggle full entity state.
- The server-side listener's ~100ms Postgres-polling interval
  (`network/listener.rs`, a pre-existing, documented tradeoff — true
  `LISTEN`/`NOTIFY` async streaming was judged too complex in
  `tokio-postgres` at the time it was built) is inherited as-is by this
  feature. It comfortably satisfies SC-006's "within seconds" bar and
  was not revisited as part of this decision.
