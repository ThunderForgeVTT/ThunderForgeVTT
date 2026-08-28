/**
 * "Still here" — the session heartbeat (spec 028 US7).
 *
 * # Why this exists rather than reading the socket
 *
 * The obvious source of "am I connected" is the WebSocket, and it is the
 * wrong one. `graphql-ws` is a lazy client: it drops its connection when it
 * has nothing subscribed and opens a new one on demand. A closed socket
 * therefore means "nothing is subscribed right now", not "this client is
 * offline" — and keying offline behaviour on it produces both failure modes,
 * queueing edits during an ordinary idle moment and reporting a healthy
 * connection that does not exist.
 *
 * A heartbeat is unambiguous. It either reached the server or it did not, and
 * it tells the server something worth knowing at the same time: who is still
 * at the table, so a Game Master can be told when somebody's laptop closes.
 *
 * # Why failures are counted rather than acted on singly
 *
 * One failed request is a garbage collection pause, a train tunnel, a server
 * restarting between deploys. Three consecutive failures is a connection that
 * is gone. The cost of the two mistakes is not symmetric: announcing an
 * outage that is not there interrupts play and trains people to ignore the
 * notice, while noticing fifteen seconds late costs nothing anyone can see.
 */

import { postGraphQL } from "@/api/graphqlClient";

/** How often to say "still here". Matches the server's timeout of 3 beats. */
export const HEARTBEAT_INTERVAL_MS = 5_000;

/** Consecutive failures before this client calls itself disconnected. */
export const HEARTBEAT_FAILURES_BEFORE_OFFLINE = 3;

const HEARTBEAT_MUTATION = `
  mutation Heartbeat($worldId: UUID!, $sceneId: UUID) {
    heartbeat(worldId: $worldId, sceneId: $sceneId)
  }
`;

/**
 * Whether this client should consider itself disconnected.
 *
 * Pure and exported so the threshold is one testable statement rather than a
 * comparison buried in a timer callback, which is the sort of thing that
 * cannot be exercised without waiting out real seconds.
 */
export function isOfflineAfter(consecutiveFailures: number): boolean {
  return consecutiveFailures >= HEARTBEAT_FAILURES_BEFORE_OFFLINE;
}

export type HeartbeatListener = (offline: boolean) => void;

let consecutiveFailures = 0;
let timer: ReturnType<typeof setInterval> | null = null;
let offline = false;
const listeners = new Set<HeartbeatListener>();

/**
 * Round trip of the last beat that arrived, in milliseconds.
 *
 * The heartbeat is already a round trip to the server on a fixed interval,
 * so this costs one subtraction and needs no probe of its own — and it
 * measures the path the client's own liveness depends on, rather than some
 * other endpoint that might be healthy while this one is not.
 *
 * `null` means no beat has completed yet, or the last one failed. It is
 * deliberately not "the previous value, still": a latency figure left on
 * screen while nothing is getting through reads as a working connection,
 * which is the one thing it must never say.
 */
let latencyMs: number | null = null;

/** Round trip of the last successful beat, or `null` if there is none. */
export function getHeartbeatLatencyMs(): number | null {
  return latencyMs;
}

function publish(next: boolean): void {
  if (next === offline) return;
  offline = next;
  for (const listener of listeners) listener(next);
}

/** Whether the last few heartbeats failed. */
export function isHeartbeatOffline(): boolean {
  return offline;
}

/** Observe transitions between connected and disconnected. */
export function subscribeToHeartbeat(listener: HeartbeatListener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Send one beat, updating the failure count. Exported for tests. */
export async function beatOnce(worldId: string, sceneId: string | null): Promise<boolean> {
  const sentAt = Date.now();
  try {
    await postGraphQL<{ heartbeat: boolean }>(HEARTBEAT_MUTATION, {
      worldId,
      sceneId,
    });
    latencyMs = Date.now() - sentAt;
    consecutiveFailures = 0;
    publish(false);
    return true;
  } catch {
    // Deliberately not distinguishing a network failure from a rejection. A
    // heartbeat refused because membership was revoked is every bit as much
    // "this client can no longer act on this world" as a dead network, and
    // the client's response — stop sending, start queueing — is the same.
    latencyMs = null;
    consecutiveFailures += 1;
    publish(isOfflineAfter(consecutiveFailures));
    return false;
  }
}

/**
 * Start beating for a world. Returns a stop function.
 *
 * Beats immediately as well as on the interval, so a page that loads while
 * the network is already down finds out in one round trip instead of five
 * seconds later.
 */
export function startHeartbeat(
  worldId: string,
  getSceneId: () => string | null,
): () => void {
  stopHeartbeat();
  consecutiveFailures = 0;
  void beatOnce(worldId, getSceneId());
  timer = setInterval(() => {
    void beatOnce(worldId, getSceneId());
  }, HEARTBEAT_INTERVAL_MS);

  return stopHeartbeat;
}

/** Stop beating, and forget the failure count. */
export function stopHeartbeat(): void {
  if (timer !== null) {
    clearInterval(timer);
    timer = null;
  }
  consecutiveFailures = 0;
  latencyMs = null;
  publish(false);
}

/** Reset module state. Tests only. */
export function resetHeartbeatForTests(): void {
  stopHeartbeat();
  listeners.clear();
  offline = false;
  latencyMs = null;
}
