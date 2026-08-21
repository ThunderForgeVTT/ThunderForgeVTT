# Contract: GraphQL Subscription Transport (client-side addition + one server-side correction)

## Server contract (existing, one correction — not a new endpoint)

**Endpoint**: `GET /api/ws` (existing, unchanged route) — WebSocket upgrade, `ALL_WEBSOCKET_PROTOCOLS`, already authenticated via the same `require_authenticated_user` middleware as `/graphql` (`main.rs:253-258`).

**Subscription field** (existing, unchanged shape, one authorization correction):

```graphql
type Subscription {
  worldEventsCreated(worldId: ID!): WorldEvent!
}

type WorldEvent {
  id: ID!
  worldId: ID!
  eventCode: Int!
  tokenEvent: JSON
  createdBy: ID!
  updatedBy: ID!
  schemaVersion: Int!
}
```

- **Before this feature**: any authenticated user could pass any `worldId` and receive that world's event stream — a latent authorization gap, harmless only because nothing calls this field today (research.md §2).
- **After this feature**: `worldEventsCreated` MUST reject (or simply not stream to) a requester who is not a member of `worldId`, matching the authorization bar already enforced on every query/mutation path in this codebase.
- No new field, no new event code, no change to `WorldEvent`'s shape — only the added authorization check.

## Client contract (new — this feature's actual deliverable)

Not a server API; documents what the client-side transport module must provide to the four existing sync files (`walls.ts`/`lights.ts`/`shapes.ts`/`tokens.ts`), each of which already expects an `AsyncIterable<WorldEventLike>`:

```ts
function openWorldEventSubscription(worldId: string): {
  events: AsyncIterable<WorldEventLike>;
  state: Observable<LiveSyncState>; // see data-model.md
  close: () => void;
};
```

- `events`: feeds directly into `startWallEventSync(events)` / the light/shape/token equivalents — no change to their call signature.
- `state`: drives the UI's "reconnecting" indicator (FR-009) and gates whether a full re-fetch should run on transition to `live` (research.md §4).
- Reconnection, backoff, and the underlying WebSocket protocol handshake are entirely internal to this module — none of the four existing sync files need to know a reconnect happened; they simply keep consuming `events` across reconnects, since a new `events` iterable is only handed out at construction (the transport module internally swaps its underlying connection without changing the iterable's identity from the caller's perspective — implementation detail, not a caller-facing contract change).

## Verification

- A user who is not a member of `worldId` attempting to open `worldEventsCreated(worldId)` MUST NOT receive any events for that world (server-side correction, research.md §2) — verified by a server test analogous to existing scene-owner authorization tests.
- A wall/light/shape change made by one authenticated client MUST be observed by another client subscribed to the same `worldId`, via `events`, within a few seconds (User Story 1).
- Killing and restoring network connectivity MUST transition `state` from `live` → `reconnecting` → `live`, with a full scene re-fetch occurring on the final transition back to `live` (User Story 2).
