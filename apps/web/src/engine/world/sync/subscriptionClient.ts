/**
 * apps/web/src/engine/world/sync/subscriptionClient.ts
 *
 * The frontend half of live cross-client sync. The backend has had a
 * fully working real-time stack this whole time — a Postgres listener
 * broadcasting onto a channel (src/server/src/network/listener.rs), a
 * `worldEventsCreated(worldId)` GraphQL subscription off that channel
 * (src/server/src/graphql.rs's `SubscriptionRoot`), and a mounted axum
 * WebSocket route at `/api/ws` (async-graphql-axum's `GraphQLWebSocket`,
 * `main.rs`) — but nothing in this app ever opened a WebSocket to it.
 * Every `apply*WorldEvent`/`start*EventSync` pair in this directory
 * (walls.ts, tokens.ts, shapes.ts, lights.ts, genieSession.ts) was
 * already written expecting exactly this: an
 * `AsyncIterable<{ event_code, token_event }>`.
 *
 * One `graphql-ws` client per browser tab (module-level singleton — the
 * client multiplexes any number of subscriptions over one WebSocket
 * connection). `subscribeToWorldEvents` can be called more than once for
 * the same `worldId`: each call is an independent GraphQL subscription
 * operation (the server re-subscribes to its broadcast channel fresh per
 * operation and filters by world_id), so every `start*EventSync` caller
 * gets its own full copy of the event stream rather than racing each
 * other over one shared iterable (a plain async iterable is
 * single-consumer — handing the same instance to five independent
 * `for await` loops would split events between them, not broadcast).
 */

import { createClient, type Client } from "graphql-ws";

export interface WorldEventLike {
  event_code?: number;
  eventCode?: number;
  token_event?: unknown;
  tokenEvent?: unknown;
}

let client: Client | null = null;

function getClient(): Client {
  if (!client) {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    client = createClient({ url: `${protocol}//${window.location.host}/api/ws` });
  }
  return client;
}

const WORLD_EVENTS_SUBSCRIPTION = `
  subscription WorldEventsCreated($worldId: String!) {
    worldEventsCreated(worldId: $worldId) {
      eventCode
      tokenEvent
    }
  }
`;

/**
 * Adapts `graphql-ws`'s callback-based `Client.subscribe` into an async
 * iterable — the pattern documented in `graphql-ws`'s own README for
 * consumers that want `for await` rather than callbacks. Returns a fresh,
 * independent subscription each call.
 */
export function subscribeToWorldEvents(worldId: string): AsyncIterable<WorldEventLike> {
  type Pending = { resolve: (done: boolean) => void; reject: (err: unknown) => void };

  const queue: WorldEventLike[] = [];
  let pending: Pending | null = null;
  let error: unknown = null;
  let done = false;

  const dispose = getClient().subscribe<{ worldEventsCreated: WorldEventLike }>(
    { query: WORLD_EVENTS_SUBSCRIPTION, variables: { worldId } },
    {
      next: (result) => {
        if (result.data?.worldEventsCreated) {
          queue.push(result.data.worldEventsCreated);
        }
        pending?.resolve(false);
        pending = null;
      },
      error: (err) => {
        error = err;
        pending?.reject(err);
        pending = null;
      },
      complete: () => {
        done = true;
        pending?.resolve(true);
        pending = null;
      },
    },
  );

  return {
    [Symbol.asyncIterator]() {
      return {
        async next(): Promise<IteratorResult<WorldEventLike>> {
          if (queue.length === 0 && !done && !error) {
            const isDone = await new Promise<boolean>((resolve, reject) => {
              pending = { resolve, reject };
            });
            if (isDone) {
              return { value: undefined, done: true };
            }
          }
          if (error) {
            const err = error;
            error = null;
            throw err;
          }
          if (queue.length > 0) {
            return { value: queue.shift() as WorldEventLike, done: false };
          }
          return { value: undefined, done: true };
        },
        async return(): Promise<IteratorResult<WorldEventLike>> {
          dispose();
          return { value: undefined, done: true };
        },
      };
    },
  };
}
