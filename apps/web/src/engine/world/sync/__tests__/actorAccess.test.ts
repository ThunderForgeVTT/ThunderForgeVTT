import { describe, expect, it, vi } from "vitest";

import {
  ACTOR_ACCESS_CHANGED_EVENT_CODE,
  applyActorAccessWorldEvent,
  startActorAccessEventSync,
} from "../actorAccess";

describe("actor access world events", () => {
  it("names the actor whose access changed", () => {
    const onActorAccessChanged = vi.fn();
    applyActorAccessWorldEvent(
      { onActorAccessChanged },
      {
        eventCode: ACTOR_ACCESS_CHANGED_EVENT_CODE,
        tokenEvent: { action: "changed", actorId: "actor-1" },
      },
    );
    expect(onActorAccessChanged).toHaveBeenCalledWith("actor-1");
  });

  it("accepts the snake_case shape the server actually sends", () => {
    const onActorAccessChanged = vi.fn();
    applyActorAccessWorldEvent(
      { onActorAccessChanged },
      {
        event_code: ACTOR_ACCESS_CHANGED_EVENT_CODE,
        token_event: { action: "changed", actorId: "actor-1" },
      },
    );
    expect(onActorAccessChanged).toHaveBeenCalledWith("actor-1");
  });

  it("drops an event that names no actor", () => {
    const onActorAccessChanged = vi.fn();
    applyActorAccessWorldEvent(
      { onActorAccessChanged },
      { eventCode: ACTOR_ACCESS_CHANGED_EVENT_CODE, tokenEvent: {} },
    );
    expect(onActorAccessChanged).not.toHaveBeenCalled();
  });

  it("ignores every other event on the world channel", () => {
    const onActorAccessChanged = vi.fn();
    for (const eventCode of [10, 14, 17, 23, 30]) {
      applyActorAccessWorldEvent(
        { onActorAccessChanged },
        { eventCode, tokenEvent: { actorId: "actor-1" } },
      );
    }
    expect(onActorAccessChanged).not.toHaveBeenCalled();
  });

  it("returns the iterator on cleanup rather than waiting for an event", async () => {
    const ret = vi.fn().mockResolvedValue({ done: true, value: undefined });
    const subscription = {
      [Symbol.asyncIterator]: () => ({
        next: () => new Promise<never>(() => {}),
        return: ret,
      }),
    } as unknown as AsyncIterable<{ eventCode?: number }>;

    const stop = startActorAccessEventSync({}, subscription);
    stop();
    expect(ret).toHaveBeenCalled();
  });
});
