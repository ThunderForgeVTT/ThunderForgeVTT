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
  /**
   * The server's `world_events.id` — monotonic, and the whole basis of the
   * reconnect catch-up below. Optional because a replayed event constructed
   * from the catch-up query and a live one both flow through here, and
   * because older callers construct these in tests.
   */
  id?: number;
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
 * `disconnected`: reconnection has been failing long enough, or the browser
 * says there is no network at all, that this is worth telling the user about
 * and worth queueing their edits for (spec 028 US7, T073).
 *
 * # Why `disconnected` is separate from `reconnecting`
 *
 * They are the same underlying situation and a different thing to say about
 * it. A dropped frame reconnects in about a second, and announcing "you are
 * offline" for it would train people to ignore the notice — so
 * `reconnecting` stays quiet and optimistic. What US7 needs is the moment
 * that stops being a blip, because that is when edits start going to the
 * outbox instead of the wire and the user has to know their work is being
 * held rather than sent.
 *
 * Still not terminal, and still requires no manual action: the retry loop
 * runs unchanged underneath, and a successful connection returns to `live`
 * from here exactly as it does from `reconnecting`.
 */
export type LiveSyncState =
  | { status: "connecting" }
  | { status: "live" }
  | { status: "reconnecting"; attempt: number }
  | { status: "disconnected"; attempt: number; browserOffline: boolean };

type LiveSyncStateListener = (state: LiveSyncState) => void;

/**
 * The highest `world_events.id` this tab has processed, per world.
 *
 * Module-level rather than per-subscription on purpose. A fully-mounted world
 * page holds four to six independent subscriptions on this one socket (scene
 * launch, canvas content, chat, combat, genie, playback), and every one of
 * them sees every event for its world. A cursor per subscription would mean
 * six clients each asking the server for the same backlog and six copies of
 * every replayed event — and since the handlers are refetch-on-nudge, that is
 * a refetch storm at the exact moment a connection has just recovered.
 *
 * One cursor per world, advanced by whichever subscription sees an event
 * first, means one catch-up request and one replay.
 */
const lastSeenEventId = new Map<string, number>();

/**
 * Consumers to hand replayed events to, per world.
 *
 * A replayed event has to reach the same code that would have handled it live
 * — the wall/token/shape/light appliers, the chat and combat panels — and
 * those read from the async iterables `subscribeToWorldEvents` hands out. So
 * the catch-up pushes into exactly those queues rather than inventing a
 * second delivery path that could drift from the first.
 */
const worldConsumers = new Map<string, Set<(event: WorldEventLike) => void>>();

/**
 * Note an event as processed, whether it arrived live or by replay.
 *
 * Monotonic by construction: the server assigns ids in ascending order and a
 * replay can only contain ids we have not seen, but taking the max means a
 * late duplicate can never drag the cursor backwards and cause the same
 * backlog to be requested twice.
 */
function noteSeen(worldId: string, event: WorldEventLike): void {
  if (typeof event.id !== "number") return;
  const current = lastSeenEventId.get(worldId) ?? 0;
  if (event.id > current) lastSeenEventId.set(worldId, event.id);
}

/** What this tab believes it has processed for a world. */
export function lastSeenEventIdFor(worldId: string): number {
  return lastSeenEventId.get(worldId) ?? 0;
}

/**
 * Failed attempts before a drop is called a disconnection.
 *
 * The backoff is 1s, 2s, 4s..., so three attempts is roughly seven seconds of
 * a connection that will not come back. Short enough that someone who has
 * genuinely lost their network is told before they have made much work that
 * needs queueing; long enough that a server restart or a laptop waking up
 * resolves itself without ever having claimed the user was offline.
 */
const DISCONNECTED_AFTER_ATTEMPTS = 3;

/**
 * Decide what to report, given what the socket and the browser each say.
 *
 * Pure, and separated from the client wiring, because this is the part with a
 * judgement in it — everything around it is `graphql-ws` callbacks that
 * cannot be exercised without a socket. Kept total: every combination of
 * inputs names a state, so there is no path that leaves the UI showing a
 * stale one.
 */
export function connectivityFor(input: {
  hasConnectedOnce: boolean;
  attempt: number;
  browserOffline: boolean;
}): LiveSyncState {
  // `navigator.onLine === false` is one of the few things a browser reports
  // that is worth believing immediately: there is no interface up, so no
  // amount of backoff will help and pretending to reconnect is a fiction.
  // The converse is *not* true — `onLine` true means "an interface exists",
  // not "the server is reachable" — which is why the attempt count still
  // decides everything else.
  if (input.browserOffline) {
    return {
      status: "disconnected",
      attempt: input.attempt,
      browserOffline: true,
    };
  }
  if (!input.hasConnectedOnce) {
    return { status: "connecting" };
  }
  if (input.attempt >= DISCONNECTED_AFTER_ATTEMPTS) {
    return {
      status: "disconnected",
      attempt: input.attempt,
      browserOffline: false,
    };
  }
  return { status: "reconnecting", attempt: input.attempt };
}

/** Whether edits should be queued rather than sent (US7). */
export function isDisconnected(state: LiveSyncState = getLiveSyncState()): boolean {
  return state.status === "disconnected";
}

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

/** How many consecutive retries the socket is currently into. */
let lastAttempt = 0;

/**
 * What the browser says about having a network at all.
 *
 * Guarded because `navigator.onLine` is absent in some contexts, and an
 * absent answer must read as "online" — assuming offline would queue edits
 * that could have been sent.
 */
function browserIsOffline(): boolean {
  return typeof navigator !== "undefined" && navigator.onLine === false;
}

/**
 * React to the browser's own network events.
 *
 * Registered once, at module scope, because the socket's backoff can be
 * mid-30-second-wait when a laptop's wifi drops — without this the UI would
 * keep claiming "reconnecting" for half a minute after the machine knows
 * perfectly well that there is no network.
 */
if (typeof window !== "undefined") {
  window.addEventListener("offline", () => {
    if (hasConnectedOnce) {
      setLiveSyncState(
        connectivityFor({
          hasConnectedOnce,
          attempt: lastAttempt,
          browserOffline: true,
        }),
      );
    }
  });
  window.addEventListener("online", () => {
    // Not `live`: an interface coming back says nothing about the server
    // being reachable. Report the retry that is about to happen and let the
    // socket's own `connected` callback promote it.
    if (hasConnectedOnce && liveSyncState.status === "disconnected") {
      setLiveSyncState({ status: "reconnecting", attempt: lastAttempt });
    }
  });
}

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
        lastAttempt = retries + 1;
        if (hasConnectedOnce) {
          setLiveSyncState(
            connectivityFor({
              hasConnectedOnce,
              attempt: lastAttempt,
              browserOffline: browserIsOffline(),
            }),
          );
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
          lastAttempt = 0;
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
      id
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

  /**
   * Hand one event to this consumer.
   *
   * Shared by the live path and the catch-up replay so a replayed event is
   * indistinguishable from a live one to everything downstream — which is the
   * property that lets the existing per-code handlers stay untouched.
   *
   * Ids that are not newer than what this consumer has already taken are
   * dropped. That is what makes replay safe to overlap with live delivery: an
   * event can legitimately arrive twice — once in the catch-up batch and once
   * on the wire, if it landed while the query was in flight — and applying it
   * twice would mean a duplicate refetch.
   */
  let consumerHighWater = 0;
  const deliver = (event: WorldEventLike) => {
    if (typeof event.id === "number") {
      if (event.id <= consumerHighWater) return;
      consumerHighWater = event.id;
    }
    noteSeen(worldId, event);
    queue.push(event);
    pending?.resolve(false);
    pending = null;
  };

  // Registered so the reconnect catch-up can reach this consumer.
  const consumers = worldConsumers.get(worldId) ?? new Set();
  consumers.add(deliver);
  worldConsumers.set(worldId, consumers);

  const dispose = getClient().subscribe<{ worldEventsCreated: WorldEventLike }>(
    { query: WORLD_EVENTS_SUBSCRIPTION, variables: { worldId } },
    {
      next: (result) => {
        if (result.data?.worldEventsCreated) {
          deliver(result.data.worldEventsCreated);
          return;
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
          // Deregister before disposing, or the catch-up would keep pushing
          // into a queue nobody is reading — a slow leak per unmounted panel.
          worldConsumers.get(worldId)?.delete(deliver);
          if (worldConsumers.get(worldId)?.size === 0) {
            worldConsumers.delete(worldId);
          }
          dispose();
          return { value: undefined, done: true };
        },
      };
    },
  };
}

/**
 * Ask the server for everything this tab missed while its socket was down,
 * and deliver it as though it had arrived live.
 *
 * # Why this exists
 *
 * Live delivery is at-most-once and always will be: a socket that is down
 * receives nothing, and a subscriber that falls behind the server's per-world
 * channel has messages dropped on the floor. `graphql-ws` reconnects and
 * resumes, and until now that resumption silently assumed nothing had
 * happened in the gap — a token moved, a scene launched or a message posted
 * during a ten-second reconnect was simply never seen by this tab.
 *
 * # Why it is a query rather than part of the subscription
 *
 * `graphql-ws` re-sends the original subscribe payload verbatim on reconnect,
 * so a cursor passed as a subscription variable would be frozen at whatever
 * it was when the page first loaded — replaying from the same stale point
 * forever and never catching up. Asking as a query reads the cursor at the
 * moment of asking.
 *
 * # The contract with the caller
 *
 * Returns `"caught-up"` when the gap was replayed and downstream handlers
 * have been fed, and `"resync-required"` when the gap was larger than the
 * server will replay. In the second case **nothing has been delivered** and
 * the caller must refetch the world; applying a partial backlog would leave
 * the tab silently behind while believing it was current.
 *
 * Never throws. A catch-up that fails leaves this tab exactly where a
 * reconnect used to leave it, which is the behaviour being improved on, not
 * a new failure mode.
 */
export async function catchUpWorldEvents(
  worldId: string,
): Promise<"caught-up" | "resync-required" | "unavailable"> {
  const afterId = lastSeenEventIdFor(worldId);

  let payload: {
    events?: WorldEventLike[];
    truncated?: boolean;
    latestId?: number;
  } | null = null;

  try {
    const { postGraphQL } = await import("@/api/graphqlClient");
    const data = await postGraphQL<{
      worldEventsSince: {
        events: WorldEventLike[];
        truncated: boolean;
        latestId: number;
      };
    }>(
      `query WorldEventsSince($worldId: UUID!, $afterId: Int!) {
         worldEventsSince(worldId: $worldId, afterId: $afterId) {
           events { id eventCode tokenEvent }
           truncated
           latestId
         }
       }`,
      { worldId, afterId },
    );
    payload = data.worldEventsSince;
  } catch {
    // Offline again, refused, or the server is unhappy. The caller's own
    // resync path is the backstop.
    return "unavailable";
  }

  if (!payload) return "unavailable";

  if (payload.truncated) {
    // Too far behind to replay. Move the cursor to where a resync will leave
    // us, so the *next* reconnect measures its gap from the right place
    // rather than from a point we are about to abandon.
    if (typeof payload.latestId === "number") {
      lastSeenEventId.set(worldId, payload.latestId);
    }
    return "resync-required";
  }

  const consumers = worldConsumers.get(worldId);
  for (const event of payload.events ?? []) {
    // Oldest first, and through the same `deliver` the live path uses — so a
    // replayed event is indistinguishable downstream, and each consumer's own
    // id check drops anything it already handled.
    if (consumers) {
      for (const deliver of consumers) deliver(event);
    } else {
      // Nobody is subscribed yet; the cursor still has to advance or the same
      // backlog is requested again on the next reconnect.
      noteSeen(worldId, event);
    }
  }

  return "caught-up";
}
