import { describe, expect, it } from "vitest";

import { FALLBACK_REVEAL_MS, revealDelayMs } from "../revealDelay";

const timings = {
  tumbleMs: 300,
  stepMs: 500,
  holdMs: 2500,
  fadeMs: 400,
  reducedMs: 150,
};

describe("revealDelayMs (spec 083 FR-017)", () => {
  it("waits for the engine's tumble", () => {
    expect(
      revealDelayMs({ engineReady: true, timings, reducedMotion: false }),
    ).toBe(300);
  });

  it("waits only for the fade-in under reduced motion", () => {
    expect(
      revealDelayMs({ engineReady: true, timings, reducedMotion: true }),
    ).toBe(150);
  });

  it("falls back to 1200 ms when the engine module has not loaded", () => {
    expect(
      revealDelayMs({ engineReady: true, timings: null, reducedMotion: false }),
    ).toBe(FALLBACK_REVEAL_MS);
    expect(FALLBACK_REVEAL_MS).toBe(1200);
  });

  it("does not wait on a board with no engine", () => {
    expect(
      revealDelayMs({ engineReady: false, timings, reducedMotion: false }),
    ).toBe(0);
  });
});
