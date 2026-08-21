# Phase 1 Data Model: Live Cross-Client Canvas Sync via GraphQL Subscriptions

## No schema changes

This feature introduces **zero new tables, columns, or migrations**. Per research.md §1, `world_events` (`src/server/src/schema.rs`) and its emission via `record_world_event` (`src/server/src/world_events.rs`) already exist and already fire on every wall/light/shape/token mutation. This section documents the existing shapes this feature wires a client to, and the one required server-side authorization correction — not new persisted design.

## Existing entities referenced by this feature

### `world_events` (unchanged schema, existing authorization gap corrected)

| Field | Type | Relevance to this feature |
|---|---|---|
| `id` | i64 | Ordering/dedup reference, not persisted client-side |
| `world_id` | UUID | Subscription scoping key — `worldEventsCreated(worldId)`'s filter |
| `event_code` | i32 | Already-defined codes: 10 (wall), 11 (light), 12 (shape), 13 (map import), 14 (token) — this feature adds no new code |
| `token_event` | nullable JSON | Generic per-event payload (name is historical; carries wall/light/shape/token change payloads alike) |
| `created_by`, `updated_by` | UUID | Unchanged provenance |

**Required correction (research.md §2)**: `SubscriptionRoot::world_events_created` (`src/server/src/graphql.rs:1443-1500`) must add a world-membership check on the requesting user before allowing the subscription to proceed — reusing whatever existing `world_members` lookup already backs authorization elsewhere in this file/`mutations_*.rs`. This is a correction to existing (unused) code, not a new entity or column.

## New (in-memory only) shape: client-side subscription/connection state

**Not persisted.** A client-side state machine, conceptually:

```ts
type LiveSyncState =
  | { status: "connecting" }
  | { status: "live" }
  | { status: "reconnecting"; attempt: number }
```

- `connecting`: initial WebSocket handshake in progress.
- `live`: subscription active, events flowing normally.
- `reconnecting`: connection dropped; automatic retry in progress with exponential backoff (research.md §5); UI shows a persistent indicator per the clarified retry policy. There is no additional terminal/error state that requires manual action — `reconnecting` persists indefinitely until `live` is restored, per FR-009a.

On transition from `reconnecting`/`connecting` → `live`, the client performs a full re-fetch of the current scene (reusing spec 004 research's identified `loadWallsIntoStore`/`loadLightsIntoStore`/`loadShapesIntoStore`/`loadTokensIntoStore` functions in `WorldPage.tsx`), per the reconnect-resync clarification.

## Existing client-side sync functions this feature activates (unchanged logic, per FR-005)

- `apps/web/src/engine/world/sync/walls.ts`'s `startWallEventSync`
- `apps/web/src/engine/world/sync/lights.ts`'s equivalent
- `apps/web/src/engine/world/sync/shapes.ts`'s equivalent
- `apps/web/src/engine/world/sync/tokens.ts`'s equivalent (relevant once spec 004 lands, per User Story 3; usable today against the existing `upsert_token` path in the interim)

Each already consumes a `for await (const event of subscription)` loop over a `AsyncIterable<WorldEventLike>` — this feature's only job regarding these four functions is to supply that `subscription` argument for the first time, from the new client transport, not to modify their internals (FR-005).
