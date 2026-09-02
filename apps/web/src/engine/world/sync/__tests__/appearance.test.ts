import { describe, expect, it, vi } from "vitest";

import {
  WORLD_APPEARANCE_CHANGED_EVENT_CODE,
  applyAppearanceWorldEvent,
  startAppearanceEventSync,
} from "../appearance";

describe("appearance world events", () => {
  it("passes the new pack id through", () => {
    const onAppearanceChanged = vi.fn();
    applyAppearanceWorldEvent(
      { onAppearanceChanged },
      {
        eventCode: WORLD_APPEARANCE_CHANGED_EVENT_CODE,
        tokenEvent: { action: "changed", interfacePackId: "forged-steel" },
      },
    );
    expect(onAppearanceChanged).toHaveBeenCalledWith("forged-steel");
  });

  it("treats a cleared binding as null rather than as no event", () => {
    const onAppearanceChanged = vi.fn();
    applyAppearanceWorldEvent(
      { onAppearanceChanged },
      {
        eventCode: WORLD_APPEARANCE_CHANGED_EVENT_CODE,
        tokenEvent: { action: "changed", interfacePackId: null },
      },
    );
    expect(onAppearanceChanged).toHaveBeenCalledWith(null);
  });

  it("accepts the snake_case shape the server actually sends", () => {
    const onAppearanceChanged = vi.fn();
    applyAppearanceWorldEvent(
      { onAppearanceChanged },
      {
        event_code: WORLD_APPEARANCE_CHANGED_EVENT_CODE,
        token_event: { interfacePackId: "forge" },
      },
    );
    expect(onAppearanceChanged).toHaveBeenCalledWith("forge");
  });

  it("ignores every other event on the world channel", () => {
    const onAppearanceChanged = vi.fn();
    for (const eventCode of [10, 14, 17, 18, 22]) {
      applyAppearanceWorldEvent({ onAppearanceChanged }, { eventCode });
    }
    expect(onAppearanceChanged).not.toHaveBeenCalled();
  });

  /**
   * The cleanup path this codebase has already been bitten by: on a quiet
   * world a `for await` loop hangs forever and never sees a cancellation
   * flag, so the iterator has to be returned explicitly.
   */
  it("returns the iterator on cleanup rather than waiting for an event", async () => {
    const ret = vi.fn().mockResolvedValue({ done: true, value: undefined });
    const subscription = {
      [Symbol.asyncIterator]: () => ({
        next: () => new Promise<never>(() => {}),
        return: ret,
      }),
    } as unknown as AsyncIterable<{ eventCode?: number }>;

    const stop = startAppearanceEventSync({}, subscription);
    stop();
    expect(ret).toHaveBeenCalled();
  });
});
