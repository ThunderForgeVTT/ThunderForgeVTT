/**
 * Spec 088 (FR-004): the world's link list follows the world's events, so a
 * join or a revoke shows on every GM's list without a reload.
 *
 * The events carry no link code (FR-001: every member reads the stream), so
 * this only says "ask again"; the list itself comes from `worldInvites`,
 * which the server gives only to those who run the world.
 */
import type { WorldEventLike } from "./subscriptionClient";

/** A link was made or changed, a revoke included (server code 2). */
export const WORLD_LINK_CHANGED_EVENT_CODE = 2;
/** Someone joined the world, which is a link's use (server code 3). */
export const MEMBER_JOINED_EVENT_CODE = 3;

export function isWorldLinkEvent(event: WorldEventLike): boolean {
  const code = event.eventCode ?? event.event_code;
  return (
    code === WORLD_LINK_CHANGED_EVENT_CODE || code === MEMBER_JOINED_EVENT_CODE
  );
}

export interface WorldLinkSyncOptions {
  events: AsyncIterable<WorldEventLike>;
  onChange: () => void;
}

/** Calls `onChange` for each link event until the returned stop is called. */
export function startWorldLinkSync({
  events,
  onChange,
}: WorldLinkSyncOptions): () => void {
  const iterator = events[Symbol.asyncIterator]();
  let cancelled = false;

  void (async () => {
    try {
      while (!cancelled) {
        const { value: event, done } = await iterator.next();
        if (done || cancelled || !event) break;
        if (isWorldLinkEvent(event)) onChange();
      }
    } catch (error) {
      if (!cancelled) console.error("World link sync stopped", error);
    }
  })();

  return () => {
    cancelled = true;
    void iterator.return?.();
  };
}
