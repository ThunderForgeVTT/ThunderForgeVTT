import { describe, expect, it, vi } from "vitest";

import {
  MEMBER_JOINED_EVENT_CODE,
  WORLD_LINK_CHANGED_EVENT_CODE,
  isWorldLinkEvent,
  startWorldLinkSync,
} from "../worldLinks";
import type { WorldEventLike } from "../subscriptionClient";

/** A stream that hands out `events`, then waits until it is closed. */
function streamOf(events: WorldEventLike[]): {
  stream: AsyncIterable<WorldEventLike>;
  closed: () => boolean;
} {
  let closed = false;
  let index = 0;
  return {
    closed: () => closed,
    stream: {
      [Symbol.asyncIterator]: () => ({
        next: () =>
          index < events.length
            ? Promise.resolve({ value: events[index++], done: false })
            : new Promise<IteratorResult<WorldEventLike>>(() => {}),
        return: () => {
          closed = true;
          return Promise.resolve({ value: undefined, done: true });
        },
      }),
    },
  };
}

describe("spec 088: a world's links follow its event stream", () => {
  it("knows a made, revoked or used link from anything else", () => {
    expect(isWorldLinkEvent({ eventCode: WORLD_LINK_CHANGED_EVENT_CODE })).toBe(
      true,
    );
    expect(isWorldLinkEvent({ event_code: MEMBER_JOINED_EVENT_CODE })).toBe(
      true,
    );
    expect(isWorldLinkEvent({ eventCode: 39 })).toBe(false);
    expect(isWorldLinkEvent({})).toBe(false);
  });

  it("asks for the list again on each link event, and on nothing else", async () => {
    const onChange = vi.fn();
    const { stream, closed } = streamOf([
      { id: 1, eventCode: 2 },
      { id: 2, eventCode: 39 },
      { id: 3, eventCode: 3 },
    ]);
    const stop = startWorldLinkSync({ events: stream, onChange });
    await vi.waitFor(() => expect(onChange).toHaveBeenCalledTimes(2));
    stop();
    expect(closed()).toBe(true);
  });
});
