# ADR-048: `graphql-ws` as the Client-Side Live-Sync Transport (Recorded Post-Hoc)

**Date:** 2026-08-25
**Status:** ACCEPTED
**Participants:** ThunderForgeVTT Team

---

## Problem Statement

Spec 005 (`specs/005-live-canvas-sync/`) built and shipped `apps/web`'s
first persistent, reconnecting, non-`fetch` client network channel — a
GraphQL-over-WebSocket subscription client consuming
`worldEventsCreated(worldId)` — plus a required, blocking authorization
fix on that subscription. Spec 005's own plan.md called this out
explicitly as "architecturally new... an ADR should record the library
choice, the cookie-based auth reuse, and the full-refetch-on-reconnect
resync strategy," and its Constitution Check gate result was recorded as
**"PASS, conditional on... the Principle IV ADR being authored alongside
the transport implementation."**

That condition was never fulfilled — the feature shipped (confirmed
directly against the code: `apps/web/src/engine/world/sync/subscriptionClient.ts`,
`graphql-ws` in `apps/web/package.json`, the membership check in
`SubscriptionRoot::world_events_created`, `WorldPage.tsx`'s wiring) but
this ADR was never written, and spec 005's own `tasks.md` was left with
most of its checkboxes unmarked despite the underlying work being done.
This ADR closes that compliance gap after the fact, and is honest about
what shipped versus what spec 005 also called for but did not ship.

## Decision

**`graphql-ws`, one client-side singleton per browser tab, subscribing to
the existing (already-built, previously-unused) `worldEventsCreated`
GraphQL subscription over the existing `/api/ws` route.**

1. `subscriptionClient.ts` lazily creates one module-level `graphql-ws`
   `Client` per tab (`createClient({ url: ".../api/ws" })`), multiplexing
   any number of subscription operations over one WebSocket connection.
   `subscribeToWorldEvents(worldId)` wraps `Client.subscribe`'s
   callback-based API into a plain `AsyncIterable`, matching the shape
   every `apply*WorldEvent`/`start*EventSync` pair in this directory
   (`walls.ts`, `tokens.ts`, `shapes.ts`, `lights.ts`, `genieSession.ts`)
   was already written to consume — those functions predate this ADR and
   were simply never fed a real event source until this shipped.
2. **Authentication**: no client-side token handling — `/api/ws` is
   already a cookie-authenticated route (the same session cookie every
   other request uses), so the WebSocket upgrade authenticates
   automatically. No new auth mechanism introduced.
3. **Authorization (Principle III, required, not optional)**:
   `SubscriptionRoot::world_events_created` previously validated only
   that `world_id` was a well-formed UUID — any authenticated user could
   subscribe to *any* world's live events by guessing/enumerating IDs.
   Fixed as a required corequisite of shipping this transport (not
   deferred): the resolver now checks caller membership via the same
   `require_world_member`/`authenticated_user` pattern every other
   world-scoped resolver in this codebase already uses (`graphql.rs`,
   near the `world_events_created` implementation).
4. **Multiple independent consumers**: `subscribeToWorldEvents` returns a
   fresh, independent subscription on every call (the server re-subscribes
   to its broadcast channel per operation) — deliberately, since a plain
   async iterable is single-consumer, and this directory's several
   `start*EventSync` callers each need their own full copy of the event
   stream rather than splitting one shared stream between them.

### What did NOT ship as part of this — RESOLVED 2026-08-25

Spec 005's User Story 2 (P2: "a connected client's live sync survives a
brief disconnect," `tasks.md` T012-T016) called for an explicit
`LiveSyncState` (`connecting`/`live`/`reconnecting`) surfaced to the UI,
and — the more load-bearing half — **a full re-fetch of the current
scene's walls/lights/shapes/tokens on every transition back into `live`**,
specifically to backfill whatever changed while a client was disconnected
(GraphQL subscriptions do not replay missed events; a dropped WebSocket
that later reconnects only resumes *future* events, not what happened
during the outage). This section originally documented that gap as real
and unshipped, discovered by reading the actual code rather than trusting
`tasks.md`.

**As of 2026-08-25, this is closed.** `subscriptionClient.ts` now
registers `graphql-ws`'s `on: { connected, closed }` lifecycle callbacks
and a `retryWait` implementing exponential backoff (1s/2s/4s/8s...
capped at 30s), with `retryAttempts: Infinity` so the retry loop itself
never gives up (spec 005 FR-009a's "no dead-end state"). It exposes
`LiveSyncState`/`getLiveSyncState()`/`subscribeToLiveSyncState()` — same
module, three exports, rather than the single `{ events, state, close }`
object the original contract doc specified (see `tasks.md`'s T005
deviation note), but functionally equivalent. `WorldPage.tsx` consumes
this to (a) render a persistent, non-blocking "reconnecting" indicator
(`live-sync-reconnecting-indicator`) satisfying FR-009, and (b) trigger a
full re-fetch via the existing `loadWallsIntoStore`/`loadLightsIntoStore`/
`loadShapesIntoStore`/`loadTokensIntoStore` functions on every transition
into `live` that follows a genuine prior disconnect (guarded by a
`wasLiveRef`, so the ordinary initial-mount connection doesn't redundantly
re-trigger loaders the existing per-resource effects already run).
Verified live against a real dev stack by `apps/web/e2e/live-sync.spec.ts`
(4/4 consecutive clean runs) — the WebSocket is severed and restored via
Playwright's `routeWebSocket`/`connectToServer` rather than
`context.setOffline(true)`, since a full network cutoff also breaks Vite
dev-server's own dynamic module fetches and crashes the page before the
WebSocket drop is even observed.

## Consequences

- `apps/web` gained its first persistent client network channel beyond
  `fetch` — a category precedent future real-time features (e.g. a future
  presence indicator, per `docs/research/session-hosting-architecture-spike.md`
  §1.4's finding that presence tracking is currently a server-side no-op
  stub) can reuse rather than reinventing.
- The `world_events_created` authorization gap is closed — this
  subscription is safe to be load-bearing traffic now, which it is
  (walls/lights/shapes/tokens across specs 001-004, scene-launch
  broadcast per ADR-046).
- **Resolved 2026-08-25** (see "What did NOT ship" above): reconnect-
  triggered full resync now exists — a client's live view no longer
  silently drifts stale after a connection drop.

## Alternatives Considered

Not re-litigated here — spec 005's own `research.md` (§1-6) already
covers transport library choice (`graphql-ws` vs. alternatives),
authentication strategy, and the reconnect-resync design this ADR
confirms was never actually implemented. This ADR exists to satisfy
Constitution Principle IV's requirement after the fact, not to redo that
analysis.
