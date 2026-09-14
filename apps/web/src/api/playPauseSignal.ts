/**
 * Spec 051 US1: the one place a page learns that an operator has paused a
 * world's play (contracts/live-play-lock.md, "Client signals").
 *
 * A pause reaches a browser by several roads at once: world event 28 on the
 * event stream, the final `WORLD_PLAY_PAUSED` error a gated subscription
 * yields as the server ends it, a refused heartbeat, and any query or mutation
 * refused mid-flight. A table sitting on the playfield will usually see three
 * or four of them inside a few seconds.
 *
 * Each road reports here, and this module turns the burst into **one** signal
 * per world. What to do about it (tear play down, go to the notice) belongs to
 * whoever owns the page, which is the same split `sessionExpiry` keeps for a
 * 401 and for the same reason: a transport that navigated would be a transport
 * with an opinion about routing.
 *
 * Kept apart from `api/playPause.ts` because `graphqlClient` reports here, and
 * `playPause.ts` imports `graphqlClient`.
 */

/** The code every refusal because a world is paused carries. */
export const WORLD_PLAY_PAUSED = "WORLD_PLAY_PAUSED";

type Listener = (worldId: string) => void;

const listeners = new Set<Listener>();

/**
 * Worlds whose pause has been announced and not yet re-armed.
 *
 * Re-armed by `rearmPlayPaused` when the person leaves the notice, so a pause
 * met again later (a back button into `/play`, a second visit next week)
 * announces itself again rather than being swallowed as a duplicate.
 */
const announced = new Set<string>();

/** Subscribe to pauses. Returns an unsubscribe function. */
export function onPlayPaused(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Report that `worldId`'s play is paused. Repeats are dropped. */
export function reportPlayPaused(worldId: string): void {
  if (!worldId || announced.has(worldId)) return;
  announced.add(worldId);
  // A copy, because the first listener usually navigates, and a navigation can
  // unmount a component whose own listener has not been called yet.
  for (const listener of [...listeners]) {
    try {
      listener(worldId);
    } catch {
      // One bad listener must not stop the others hearing of the pause.
    }
  }
}

/** Let the next pause of `worldId` be announced again. */
export function rearmPlayPaused(worldId: string): void {
  announced.delete(worldId);
}

/**
 * Read a GraphQL `errors` array for a pause, and report it if there is one.
 *
 * Shared by the HTTP transport and every subscription, which all receive the
 * same shape. Returns whether a pause was found, so a subscription can end
 * quietly instead of treating the error as a failure.
 */
export function reportPlayPausedIn(errors: unknown): boolean {
  if (!Array.isArray(errors)) return false;
  let found = false;
  for (const entry of errors) {
    const extensions = (entry as { extensions?: Record<string, unknown> })
      ?.extensions;
    if (extensions?.code !== WORLD_PLAY_PAUSED) continue;
    found = true;
    if (typeof extensions.worldId === "string") {
      reportPlayPaused(extensions.worldId);
    }
  }
  return found;
}

/**
 * Read the world cache's sync summary for a pause, and report it if there is
 * one (spec 051 US2, T039).
 *
 * `worldSyncPlan` is asked by the engine's own fetch in wasm, not through
 * `graphqlClient`, so its refusal never passes the central check above. The
 * engine resolves every failure to a `"degraded"` summary whose `reason` ends
 * in the server's first error as JSON (`server rejected sync: {...}`). The
 * code is read out of that JSON, never matched in the prose around it.
 *
 * `worldId` is the world the sync was asked about, used only when the error
 * does not name one.
 */
export function reportPlayPausedInSyncReason(
  worldId: string,
  reason: unknown,
): boolean {
  if (typeof reason !== "string") return false;
  const start = reason.indexOf("{");
  if (start < 0) return false;
  let error: unknown;
  try {
    error = JSON.parse(reason.slice(start));
  } catch {
    return false;
  }
  const extensions = (error as { extensions?: Record<string, unknown> } | null)
    ?.extensions;
  if (extensions?.code !== WORLD_PLAY_PAUSED) return false;
  reportPlayPaused(
    typeof extensions.worldId === "string" ? extensions.worldId : worldId,
  );
  return true;
}

type NotKeptListener = (worldId: string) => void;

const notKeptListeners = new Set<NotKeptListener>();

/** Offline changes refused because of a pause, not yet shown, per world. */
const notKept = new Map<string, number>();

/**
 * Record that `count` changes this browser queued offline for `worldId` were
 * refused because its play is paused, and discarded (spec 051 US2, T038).
 *
 * Kept here rather than handed over in navigation state because the order is
 * not fixed: a reconnecting browser is usually sent to the notice by its
 * first refused heartbeat, and the reconcile that learns what was dropped
 * answers after that. The notice takes whatever has arrived when it mounts
 * and listens for the rest.
 */
export function reportChangesNotKept(worldId: string, count: number): void {
  if (!worldId || count <= 0) return;
  notKept.set(worldId, (notKept.get(worldId) ?? 0) + count);
  for (const listener of [...notKeptListeners]) {
    try {
      listener(worldId);
    } catch {
      // As above: one listener must not stop the others.
    }
  }
}

/** Take the count recorded for `worldId`, leaving none behind. */
export function takeChangesNotKept(worldId: string): number {
  const count = notKept.get(worldId) ?? 0;
  notKept.delete(worldId);
  return count;
}

/** Hear when a count is recorded. Returns an unsubscribe function. */
export function onChangesNotKept(listener: NotKeptListener): () => void {
  notKeptListeners.add(listener);
  return () => {
    notKeptListeners.delete(listener);
  };
}

/** Tests only: forget every announcement and listener. */
export function resetPlayPausedForTests(): void {
  announced.clear();
  listeners.clear();
  notKept.clear();
  notKeptListeners.clear();
}
