import { describe, expect, it, vi } from "vitest";

/**
 * Spec 082 SC-005: a tool a Game Master takes away leaves the player's rail
 * without a reload, because the rail re-asks when the world says so.
 */

const events: unknown[] = [];
vi.mock("@/engine/world/sync", () => ({
  subscribeToWorldEvents: () => ({
    [Symbol.asyncIterator]: () => ({
      next: async () =>
        events.length
          ? { value: events.shift(), done: false }
          : { value: undefined, done: true },
      return: async () => ({ value: undefined, done: true }),
    }),
  }),
}));

const { AUTHORING_TOOLS_CHANGED, watchAuthoringTools } =
  await import("@/api/authoringTools");

describe("watching a world's authoring tools", () => {
  it("calls back on a tools change and on nothing else", async () => {
    events.push(
      { id: 1, event_code: 12 },
      { id: 2, event_code: AUTHORING_TOOLS_CHANGED },
      { id: 3, eventCode: AUTHORING_TOOLS_CHANGED },
    );
    const onChanged = vi.fn();
    const stop = watchAuthoringTools("world-1", onChanged);
    await vi.waitFor(() => expect(onChanged).toHaveBeenCalledTimes(2));
    stop();
  });
});
