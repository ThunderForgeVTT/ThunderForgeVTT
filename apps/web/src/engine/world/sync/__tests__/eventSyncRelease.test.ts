/**
 * Stopping a scene-content event sync closes its subscription at once.
 *
 * These four used an AbortController checked inside `for await`. A flag in
 * that loop is only read between events, so on a quiet world `stop()` left
 * the loop parked in `next()` and the server-side subscription open until
 * some later event happened to arrive — the bug `startAppearanceEventSync`
 * and `startGenieSessionEventSync` were already fixed for.
 */
import { describe, expect, it, vi } from "vitest";

import { startLightEventSync } from "../lights";
import { startShapeEventSync } from "../shapes";
import type { WorldEventLike } from "../subscriptionClient";
import { startTokenEventSync } from "../tokens";
import { startWallEventSync } from "../walls";
import type { WorldStore } from "../../store";

/** A subscription on a world where nothing ever happens. */
function quietSubscription() {
  const ret = vi.fn().mockResolvedValue({ done: true, value: undefined });
  const next = vi.fn(() => new Promise<never>(() => {}));
  let iterators = 0;
  const subscription = {
    [Symbol.asyncIterator]: () => {
      iterators += 1;
      return { next, return: ret };
    },
  } as unknown as AsyncIterable<WorldEventLike>;
  return { subscription, ret, iterators: () => iterators };
}

const store = {} as WorldStore;

describe.each([
  ["tokens", startTokenEventSync],
  ["walls", startWallEventSync],
  ["shapes", startShapeEventSync],
  ["lights", startLightEventSync],
] as const)("start %s event sync", (_kind, start) => {
  it("returns the iterator on stop without waiting for an event", () => {
    const { subscription, ret } = quietSubscription();
    const stop = start(store, "scene-1", subscription);
    expect(ret).not.toHaveBeenCalled();
    stop();
    expect(ret).toHaveBeenCalledTimes(1);
  });

  it("leaves no subscription open across repeated start and stop", () => {
    const opened: ReturnType<typeof quietSubscription>[] = [];
    for (let i = 0; i < 5; i += 1) {
      const quiet = quietSubscription();
      opened.push(quiet);
      start(store, "scene-1", quiet.subscription)();
    }
    for (const quiet of opened) {
      expect(quiet.iterators()).toBe(1);
      expect(quiet.ret).toHaveBeenCalledTimes(1);
    }
  });
});
