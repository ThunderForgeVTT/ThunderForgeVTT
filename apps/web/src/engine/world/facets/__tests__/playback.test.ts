import { describe, expect, it, vi } from "vitest";

/**
 * Stopping playback lets go of the server stream at once (spec 051 T023).
 *
 * A `for await` loop only notices a stop between events, so a quiet world, or
 * a paused one whose events have ended, kept its subscription open on the
 * server until the server's own liveness tick ended it.
 */

const dispose = vi.fn();
vi.mock("../../sync/subscriptionClient", () => ({
  subscribeToWorldEvents: () => ({
    [Symbol.asyncIterator]() {
      return {
        // A world with nothing to say: the next event never comes.
        next: () => new Promise(() => undefined),
        return: async () => {
          dispose();
          return { value: undefined, done: true };
        },
      };
    },
  }),
}));

const { createPlaybackFacet } = await import("../playback");

describe("playback", () => {
  it("closes its subscription when stopped, without waiting for an event", () => {
    const playback = createPlaybackFacet("world");
    expect(dispose).not.toHaveBeenCalled();

    playback.stop();
    playback.stop();

    expect(dispose).toHaveBeenCalledTimes(1);
  });
});
