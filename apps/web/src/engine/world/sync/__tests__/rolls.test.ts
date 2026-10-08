import { describe, expect, it, vi } from "vitest";

import type { MaskedRollRecord, WorldRollRecord } from "@/types/roll";

import {
  REPLAY_WINDOW_MS,
  ROLL_MADE_EVENT_CODE,
  ROLL_REVEALED_EVENT_CODE,
  shouldAnimate,
  startRollSync,
} from "../rolls";
import type { WorldEventLike } from "../subscriptionClient";

const whole: WorldRollRecord = {
  __typename: "WorldRoll",
  id: "roll-1",
  rollerId: "user-1",
  rollerName: "Ana",
  label: "Stealth",
  formula: "1d20",
  bindings: [],
  resolution: {
    formula: "1d20",
    dice: [
      {
        sidesKind: "NUMERIC",
        numericSides: 20,
        rolls: [14],
        steps: [],
        kept: true,
        finalValue: 14,
      },
    ],
    resultKind: "TOTAL",
    resultValue: 14,
  },
  visibility: "EVERYONE",
  createdAt: "2026-10-07T12:00:00Z",
  revealedAt: null,
  revealedByName: null,
  facets: [],
};

const masked: MaskedRollRecord = {
  __typename: "MaskedRoll",
  id: "roll-1",
  rollerName: "Ana",
  createdAt: "2026-10-07T12:00:00Z",
  visibility: "GM_EYES",
};

describe("shouldAnimate (research R3)", () => {
  const live = { replayed: false, receivedAt: 1000, answeredAt: 1200 };

  it("animates a whole roll that arrived live and in time", () => {
    expect(shouldAnimate({ entry: whole, ...live })).toBe(true);
  });

  it("does not animate a masked roll or one the viewer may not see", () => {
    expect(shouldAnimate({ entry: masked, ...live })).toBe(false);
    expect(shouldAnimate({ entry: null, ...live })).toBe(false);
  });

  it("does not animate a roll caught up after a reconnect", () => {
    expect(shouldAnimate({ entry: whole, ...live, replayed: true })).toBe(
      false,
    );
  });

  it("does not animate a roll answered after the window", () => {
    expect(
      shouldAnimate({
        entry: whole,
        replayed: false,
        receivedAt: 1000,
        answeredAt: 1000 + REPLAY_WINDOW_MS,
      }),
    ).toBe(false);
  });
});

/** An event stream fed by hand. */
function feed() {
  const queue: WorldEventLike[] = [];
  let wake: (() => void) | null = null;
  const events: AsyncIterable<WorldEventLike> = {
    [Symbol.asyncIterator]: () => ({
      async next() {
        while (queue.length === 0) {
          await new Promise<void>((resolve) => (wake = resolve));
        }
        return { value: queue.shift()!, done: false };
      },
      async return() {
        return { value: undefined, done: true };
      },
    }),
  };
  return {
    events,
    push(event: WorldEventLike) {
      queue.push(event);
      wake?.();
    },
  };
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("startRollSync", () => {
  it("animates a live roll, lists it, and ignores every other event", async () => {
    const { events, push } = feed();
    const animate = vi.fn();
    const onRoll = vi.fn();
    const fetchRoll = vi.fn().mockResolvedValue(whole);
    const stop = startRollSync({
      worldId: "w",
      events,
      animate,
      onRoll,
      fetchRoll,
    });

    push({ id: 1, eventCode: 17, tokenEvent: { messageId: "m" } });
    push({
      id: 2,
      eventCode: ROLL_MADE_EVENT_CODE,
      tokenEvent: { rollId: "roll-1", visibility: "everyone" },
    });
    await settle();
    await settle();

    expect(fetchRoll).toHaveBeenCalledExactlyOnceWith("w", "roll-1");
    expect(animate).toHaveBeenCalledExactlyOnceWith(whole);
    expect(onRoll).toHaveBeenCalledExactlyOnceWith(whole);
    stop();
  });

  it("lists a masked roll without animating it", async () => {
    const { events, push } = feed();
    const animate = vi.fn();
    const onRoll = vi.fn();
    const stop = startRollSync({
      worldId: "w",
      events,
      animate,
      onRoll,
      fetchRoll: vi.fn().mockResolvedValue(masked),
    });
    push({
      id: 3,
      eventCode: ROLL_MADE_EVENT_CODE,
      tokenEvent: { rollId: "roll-1", visibility: "gm_eyes" },
    });
    await settle();
    await settle();
    expect(animate).not.toHaveBeenCalled();
    expect(onRoll).toHaveBeenCalledExactlyOnceWith(masked);
    stop();
  });

  it("lists a replayed roll without animating it", async () => {
    const { events, push } = feed();
    const animate = vi.fn();
    const onRoll = vi.fn();
    const stop = startRollSync({
      worldId: "w",
      events,
      animate,
      onRoll,
      fetchRoll: vi.fn().mockResolvedValue(whole),
    });
    push({
      id: 4,
      eventCode: ROLL_MADE_EVENT_CODE,
      tokenEvent: { rollId: "roll-1", visibility: "everyone" },
      replayed: true,
    });
    await settle();
    await settle();
    expect(animate).not.toHaveBeenCalled();
    expect(onRoll).toHaveBeenCalledOnce();
    stop();
  });

  it("animates a reveal once, as the whole roll", async () => {
    const { events, push } = feed();
    const animate = vi.fn();
    const revealed = { ...whole, revealedAt: "2026-10-07T12:01:00Z" };
    const stop = startRollSync({
      worldId: "w",
      events,
      animate,
      fetchRoll: vi.fn().mockResolvedValue(revealed),
    });
    push({
      id: 5,
      eventCode: ROLL_REVEALED_EVENT_CODE,
      tokenEvent: { rollId: "roll-1", visibility: "gm_only" },
    });
    await settle();
    await settle();
    expect(animate).toHaveBeenCalledExactlyOnceWith(revealed);
    stop();
  });

  it("does nothing for a roll the viewer may not see", async () => {
    const { events, push } = feed();
    const animate = vi.fn();
    const onRoll = vi.fn();
    const stop = startRollSync({
      worldId: "w",
      events,
      animate,
      onRoll,
      fetchRoll: vi.fn().mockResolvedValue(null),
    });
    push({
      id: 6,
      eventCode: ROLL_MADE_EVENT_CODE,
      tokenEvent: { rollId: "roll-1", visibility: "gm_only" },
    });
    await settle();
    await settle();
    expect(animate).not.toHaveBeenCalled();
    expect(onRoll).not.toHaveBeenCalled();
    stop();
  });
});
