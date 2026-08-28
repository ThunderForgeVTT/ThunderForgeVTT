/**
 * The playback facet: what the server has decided, in order, replayable.
 *
 * The live sync path consumes `worldEventsCreated` and applies each event
 * immediately — correct for keeping a board current, and useless for
 * answering "what just happened?". The event is read, acted on, and gone.
 * A session that is adjudicated server-side needs the opposite property:
 * the sequence has to survive being applied, so it can be re-read, shown as
 * a log, or replayed into a fresh client.
 *
 * That is the difference between sync and playback, and it is why this is a
 * separate facet rather than a flag on the existing subscription. Sync
 * answers "is the board current". Playback answers "how did it get this
 * way" — the GM's scrollback, a rejoining player catching up, and a
 * reproducible trace when a ruleset decision is disputed.
 *
 * The server's own bus is already shaped for this: `world_events` rows are
 * ordered, carry an `event_code`, and deliberately carry an id rather than
 * a body (a GM-only chat message must not travel to clients it is hidden
 * from — see `world_events.rs`). So playback is a sequence of *notices*,
 * and a consumer that needs detail refetches it under its own
 * authorisation. Nothing here widens what a client can see.
 *
 * Bounded by design: a session can run for hours, and an unbounded log is
 * a memory leak with a plausible excuse.
 */

import {
  subscribeToWorldEvents,
  type WorldEventLike,
} from "../sync/subscriptionClient";

/** Entries retained. Roughly a long session's worth of notable events. */
export const DEFAULT_HISTORY_LIMIT = 500;

/**
 * One thing the server recorded.
 *
 * `sequence` is assigned by this client on arrival, not read from the
 * server: the subscription payload carries no ordinal, and a monotonic
 * local counter is enough for the two things playback needs it for —
 * resuming from a cursor and rendering in order.
 */
export interface PlaybackEntry {
  sequence: number;
  /** The server's `EVENT_CODE_*` discriminant (`world_events.rs`). */
  eventCode: number;
  /** Wall-clock arrival, for display. Not an ordering key: two events can
   *  share a millisecond, `sequence` cannot. */
  receivedAt: number;
  /** The raw payload, for consumers that know the code. */
  event: WorldEventLike;
}

export type PlaybackListener = (entry: PlaybackEntry) => void;

export interface PlaybackFacet {
  /** Everything retained, oldest first. */
  history(): PlaybackEntry[];
  /** Entries after `cursor`, for a consumer resuming where it left off. */
  since(cursor: number): PlaybackEntry[];
  /** The latest sequence, to be handed back to `since` later. */
  cursor(): number;
  /**
   * Live entries as they arrive. The listener is *not* replayed the
   * backlog: a caller that wants both asks for `history()` first and then
   * subscribes, so it controls the join point rather than this guessing.
   */
  subscribe(listener: PlaybackListener): () => void;
  /**
   * Re-emit retained entries to one listener, oldest first. The replay is
   * synchronous and does not touch live subscribers, so replaying into a
   * newly opened log view cannot disturb anything already running.
   */
  replay(listener: PlaybackListener, from?: number): void;
  /** Stop consuming the server stream. */
  stop(): void;
}

export function createPlaybackFacet(
  worldId: string,
  options: { limit?: number } = {},
): PlaybackFacet {
  const limit = options.limit ?? DEFAULT_HISTORY_LIMIT;
  const entries: PlaybackEntry[] = [];
  const listeners = new Set<PlaybackListener>();
  let sequence = 0;
  let stopped = false;

  void (async () => {
    for await (const event of subscribeToWorldEvents(worldId)) {
      if (stopped) return;

      sequence += 1;
      const entry: PlaybackEntry = {
        sequence,
        // The subscription payload has been seen under both spellings;
        // normalising here keeps every consumer from repeating the guard.
        eventCode: event.eventCode ?? event.event_code ?? -1,
        receivedAt: Date.now(),
        event,
      };

      entries.push(entry);
      if (entries.length > limit) entries.shift();

      // A throwing listener must not stop the stream for the others, or
      // one broken log view takes the whole session's playback with it.
      for (const listener of listeners) {
        try {
          listener(entry);
        } catch {
          /* a listener's failure is its own */
        }
      }
    }
  })();

  return {
    history: () => [...entries],
    since: (cursor) => entries.filter((entry) => entry.sequence > cursor),
    cursor: () => sequence,
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    replay(listener, from = 0) {
      for (const entry of entries) {
        if (entry.sequence > from) listener(entry);
      }
    },
    stop() {
      stopped = true;
      listeners.clear();
    },
  };
}
