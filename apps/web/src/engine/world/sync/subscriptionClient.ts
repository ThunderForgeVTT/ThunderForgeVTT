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

/**
 * Spec 005 (data-model.md "LiveSyncState", T014-T016): the one `graphql-ws`
 * WebSocket connection this module maintains is shared by every
 * `subscribeToWorldEvents` caller, so its connection state is tracked here
 * as a single module-level value rather than per-subscription — there is
 * only ever one underlying socket to be `live` or `reconnecting`.
 *
 * `connecting`: the very first handshake, before this tab has ever seen a
 * successful connection.
 * `live`: connected, events flowing normally.
 * `reconnecting`: a previously-live connection dropped and `graphql-ws`'s
 * own retry loop (configured below with `retryAttempts: Infinity`) is
 * backing off before its next attempt — this persists indefinitely; there
 * is no dead-end/terminal state requiring manual action (FR-009a).
 */
export type LiveSyncState =
  | { status: "connecting" }
  | { status: "live" }
  | { status: "reconnecting"; attempt: number };

type LiveSyncStateListener = (state: LiveSyncState) => void;

let client: Client | null = null;
let liveSyncState: LiveSyncState = { status: "connecting" };
let hasConnectedOnce = false;
const liveSyncStateListeners = new Set<LiveSyncStateListener>();

function setLiveSyncState(next: LiveSyncState): void {
  liveSyncState = next;
  for (const listener of liveSyncStateListeners) {
    listener(next);
  }
}

/** Current connection state, for a caller that just wants a snapshot
 * (e.g. to initialize `useState`) without waiting for the next change. */
export function getLiveSyncState(): LiveSyncState {
  return liveSyncState;
}

/** Subscribes to every `LiveSyncState` transition for this tab's one
 * shared connection. Returns an unsubscribe function. Does not call
 * `listener` immediately with the current state — callers that need the
 * current value up front should also call `getLiveSyncState()`. */
export function subscribeToLiveSyncState(listener: LiveSyncStateListener): () => void {
  liveSyncStateListeners.add(listener);
  return () => {
    liveSyncStateListeners.delete(listener);
  };
}

// research.md §5: standard exponential backoff (1s, 2s, 4s, 8s...) capped
// at 30s, per the clarified "retry indefinitely" answer (FR-009a) —
// `retryAttempts: Infinity` below is what makes the retry loop itself
// never give up; this only controls the *delay* between attempts.
const RECONNECT_BASE_DELAY_MS = 1000;
const RECONNECT_MAX_DELAY_MS = 30000;

function getClient(): Client {
  if (!client) {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    client = createClient({
      url: `${protocol}//${window.location.host}/api/ws`,
      retryAttempts: Infinity,
      retryWait: async (retries) => {
        // Only a previously-*live* connection dropping counts as
        // "reconnecting" (data-model.md) — a still-connecting first
        // handshake retrying stays in `connecting` rather than jumping to
        // `reconnecting` with a confusing attempt count.
        if (hasConnectedOnce) {
          setLiveSyncState({ status: "reconnecting", attempt: retries + 1 });
        }
        const delayMs = Math.min(
          RECONNECT_BASE_DELAY_MS * 2 ** retries,
          RECONNECT_MAX_DELAY_MS,
        );
        await new Promise((resolve) => setTimeout(resolve, delayMs));
      },
      on: {
        connected: () => {
          hasConnectedOnce = true;
          setLiveSyncState({ status: "live" });
        },
        closed: () => {
          if (!hasConnectedOnce) {
            setLiveSyncState({ status: "connecting" });
          }
          // A previously-live drop is reflected by `retryWait` above
          // (fired before the next attempt); nothing to set here for that
          // case beyond what `retryWait` already will.
        },
      },
    });
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
