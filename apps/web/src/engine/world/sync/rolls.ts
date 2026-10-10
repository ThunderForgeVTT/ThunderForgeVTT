/**
 * rolls.ts — spec 081: a roll made anywhere at the table reaches every board.
 *
 * The server announces a roll with its id and visibility only (codes 36 and
 * 37, `crates/thunderforge-server/src/world_events.rs`), and each client asks
 * `worldRoll` for it. The answer is per viewer — whole, masked or nothing —
 * so a number never rides the stream to someone it is hidden from.
 *
 * A board animates a roll only when it saw it happen (research R3): the event
 * came live rather than from a reconnect's catch-up, the answer was the whole
 * roll, and it came back within `REPLAY_WINDOW_MS` of the event. The rolling
 * tab is no exception — it animates from its own announcement like every
 * other client, which is what lets a sheet in another tab roll onto this
 * board.
 *
 * Spec 088: a clear (code 39) names a time; every roll made at or before it
 * leaves the feed, and a roll still being fetched when it came is dropped.
 */

import { fetchWorldRoll } from "@/api/roll";
import type { WorldRollEntry, WorldRollRecord } from "@/types/roll";

import type { WorldEventLike } from "./subscriptionClient";

export const ROLL_MADE_EVENT_CODE = 36;
export const ROLL_REVEALED_EVENT_CODE = 37;
/** Spec 088: the GM cleared the feed. Payload `{clearedAt}`. */
export const ROLLS_CLEARED_EVENT_CODE = 39;

/**
 * The dice's settle time (1.2 s) plus the slowest fetch worth animating. A
 * roll answered later than this would land after the table has moved on.
 * Measured on this client's clock alone, so clocks never have to agree.
 */
export const REPLAY_WINDOW_MS = 4000;

/** The roll id a roll event names, or `undefined` for any other event. */
export function rollIdOf(event: WorldEventLike): string | undefined {
  const code = event.event_code ?? event.eventCode;
  if (code !== ROLL_MADE_EVENT_CODE && code !== ROLL_REVEALED_EVENT_CODE) {
    return undefined;
  }
  const payload = (event.token_event ?? event.tokenEvent) as
    | Record<string, unknown>
    | undefined;
  const rollId = payload?.rollId;
  return typeof rollId === "string" ? rollId : undefined;
}

/** The time a clear event names, or `undefined` for any other event. */
export function clearedAtOf(event: WorldEventLike): string | undefined {
  const code = event.event_code ?? event.eventCode;
  if (code !== ROLLS_CLEARED_EVENT_CODE) return undefined;
  const payload = (event.token_event ?? event.tokenEvent) as
    | Record<string, unknown>
    | undefined;
  const clearedAt = payload?.clearedAt;
  return typeof clearedAt === "string" ? clearedAt : undefined;
}

/**
 * A timestamp in microseconds since the epoch. `Date.parse` stops at the
 * millisecond, and the server's times carry six digits, so a roll made a
 * microsecond after a clear would read as made at it. A time with no zone
 * (the demo's, as the server's `NaiveDateTime` prints) is read as UTC.
 */
export function microsOf(at: string): number {
  const fraction = /\.(\d+)/.exec(at)?.[1] ?? "";
  let whole = at.replace(/\.\d+/, "");
  if (!/(Z|[+-]\d{2}:?\d{2})$/.test(whole)) whole += "Z";
  return Date.parse(whole) * 1000 + Number(fraction.padEnd(6, "0").slice(0, 6));
}

/** Whether a roll made at `createdAt` was cleared by a clear at `clearedAt`. */
export function wasCleared(
  createdAt: string,
  clearedAt: string | null,
): boolean {
  return clearedAt !== null && microsOf(createdAt) <= microsOf(clearedAt);
}

/** Whether a board animates this roll (research R3). */
export function shouldAnimate(input: {
  entry: WorldRollEntry | null;
  replayed: boolean;
  receivedAt: number;
  answeredAt: number;
}): boolean {
  return (
    !input.replayed &&
    input.entry?.__typename === "WorldRoll" &&
    input.answeredAt - input.receivedAt < REPLAY_WINDOW_MS
  );
}

export interface RollSyncOptions {
  worldId: string;
  events: AsyncIterable<WorldEventLike>;
  /**
   * Throws the roll on the board. Left out where there is no board. Only
   * ever handed a whole `WorldRoll`, never a masked one (research R2).
   */
  animate?: (roll: WorldRollRecord) => void;
  /** Each roll as this viewer may see it, as it arrives or is revealed. */
  onRoll?: (entry: WorldRollEntry) => void;
  /** Spec 088: the GM cleared every roll made at or before `clearedAt`. */
  onCleared?: (clearedAt: string) => void;
  /** Overridable for tests. */
  fetchRoll?: (
    worldId: string,
    rollId: string,
  ) => Promise<WorldRollEntry | null>;
  now?: () => number;
}

/**
 * Fetch, decide and hand on every roll event in `events`. Returns a cleanup
 * function. Holds the iterator and calls `.return()`, for the reason
 * `startPlayPanelEventSync` gives.
 */
export function startRollSync({
  worldId,
  events,
  animate,
  onRoll,
  onCleared,
  fetchRoll = fetchWorldRoll,
  now = () => performance.now(),
}: RollSyncOptions): () => void {
  const iterator = events[Symbol.asyncIterator]();
  let cancelled = false;
  // The latest clear seen, so a fetch in flight when it came is dropped.
  let clearedAt: string | null = null;

  const handle = async (event: WorldEventLike, rollId: string) => {
    const receivedAt = now();
    let entry: WorldRollEntry | null;
    try {
      entry = await fetchRoll(worldId, rollId);
    } catch (error) {
      console.error("Roll sync: failed to fetch a roll", error);
      return;
    }
    if (cancelled || entry === null) return;
    if (wasCleared(entry.createdAt, clearedAt)) return;
    if (
      animate &&
      entry.__typename === "WorldRoll" &&
      shouldAnimate({
        entry,
        replayed: event.replayed === true,
        receivedAt,
        answeredAt: now(),
      })
    ) {
      animate(entry);
    }
    onRoll?.(entry);
  };

  void (async () => {
    try {
      while (!cancelled) {
        const { value: event, done } = await iterator.next();
        if (done || cancelled || !event) break;
        const cleared = clearedAtOf(event);
        if (cleared) {
          clearedAt = cleared;
          onCleared?.(cleared);
          continue;
        }
        const rollId = rollIdOf(event);
        // Not awaited: one slow fetch must not hold the next roll back
        // past its window.
        if (rollId) void handle(event, rollId);
      }
    } catch (error) {
      console.error("Roll sync error:", error);
    }
  })();

  return () => {
    cancelled = true;
    void iterator.return?.();
  };
}
