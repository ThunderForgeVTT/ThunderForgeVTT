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

/** Tests only: forget every announcement and listener. */
export function resetPlayPausedForTests(): void {
  announced.clear();
  listeners.clear();
}
