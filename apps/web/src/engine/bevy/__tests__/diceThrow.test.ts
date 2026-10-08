import { afterEach, describe, expect, it, vi } from "vitest";

import type { WorldRollRecord } from "@/types/roll";

import {
  buildDiceThrow,
  engineDiceTimings,
  setDiceTimingsSource,
  watchReducedMotion,
} from "../diceThrow";

const roll: WorldRollRecord = {
  __typename: "WorldRoll",
  id: "roll-1",
  rollerId: "user-1",
  rollerName: "Ayla",
  label: "Stealth",
  formula: "1d20 + MODIFIER",
  bindings: [{ placeholder: "MODIFIER", value: 3 }],
  resolution: {
    formula: "1d20 + MODIFIER",
    dice: [
      {
        sidesKind: "NUMERIC",
        numericSides: 20,
        rolls: [4, 13],
        steps: ["REROLL"],
        kept: true,
        finalValue: 13,
      },
    ],
    resultKind: "TOTAL",
    resultValue: 16,
  },
  visibility: "EVERYONE",
  createdAt: "2026-10-07T12:00:00Z",
  revealedAt: null,
  revealedByName: null,
  facets: [],
};

describe("buildDiceThrow (contracts/engine-dice.md)", () => {
  it("keeps every field the engine reads, in WorldRoll's names", () => {
    expect(buildDiceThrow(roll)).toEqual({
      id: "roll-1",
      rollerName: "Ayla",
      label: "Stealth",
      formula: "1d20 + MODIFIER",
      bindings: [{ placeholder: "MODIFIER", value: 3 }],
      resultKind: "TOTAL",
      resultValue: 16,
      dice: [
        {
          sidesKind: "NUMERIC",
          numericSides: 20,
          rolls: [4, 13],
          steps: ["REROLL"],
          kept: true,
          finalValue: 13,
        },
      ],
    });
  });

  it("defaults steps and bindings to [] for a roll stored before spec 083", () => {
    const old = {
      ...roll,
      bindings: undefined,
      resolution: {
        ...roll.resolution,
        dice: [{ ...roll.resolution.dice[0], steps: undefined }],
      },
    } as unknown as WorldRollRecord;
    const payload = buildDiceThrow(old);
    expect(payload.bindings).toEqual([]);
    expect(payload.dice[0].steps).toEqual([]);
  });
});

describe("engineDiceTimings", () => {
  afterEach(() => setDiceTimingsSource(null));

  it("is null when no engine module is loaded", () => {
    expect(engineDiceTimings()).toBeNull();
  });

  it("reads the engine's export", () => {
    setDiceTimingsSource(
      () =>
        '{"tumbleMs":1200,"stepMs":500,"holdMs":2500,"fadeMs":400,"reducedMs":150}',
    );
    expect(engineDiceTimings()?.tumbleMs).toBe(1200);
    expect(engineDiceTimings()?.reducedMs).toBe(150);
  });
});

describe("watchReducedMotion (research R11)", () => {
  afterEach(() => vi.unstubAllGlobals());

  function stubMatchMedia(matches: boolean) {
    const listeners = new Set<(event: MediaQueryListEvent) => void>();
    const query = {
      matches,
      addEventListener: (_: string, fn: (e: MediaQueryListEvent) => void) =>
        listeners.add(fn),
      removeEventListener: (_: string, fn: (e: MediaQueryListEvent) => void) =>
        listeners.delete(fn),
    };
    vi.stubGlobal(
      "matchMedia",
      vi.fn(() => query),
    );
    return (next: boolean) =>
      listeners.forEach((fn) => fn({ matches: next } as MediaQueryListEvent));
  }

  it("sends the initial value, sends on change, and stops when disposed", () => {
    const change = stubMatchMedia(true);
    const send = vi.fn();
    const stop = watchReducedMotion(send);
    expect(send).toHaveBeenCalledExactlyOnceWith(true);
    change(false);
    expect(send).toHaveBeenLastCalledWith(false);
    stop();
    change(true);
    expect(send).toHaveBeenCalledTimes(2);
  });
});
