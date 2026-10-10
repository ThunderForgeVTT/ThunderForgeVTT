import { describe, expect, it } from "vitest";
import type { Attrs, EventName, Telemetry } from "@thunderforge/telemetry";
import { noopTelemetry } from "@thunderforge/telemetry";
import {
  createFrameRing,
  framesRecord,
  tokensBucket,
  watchFrames,
} from "../framesSummary";
import { createLoadSignals } from "../loadSignals";
import type { EngineStats } from "../stats";

const stats = (fps: number, frameTimeMs: number, tokens = 3): EngineStats => ({
  fps,
  frameTimeMs,
  sprites: 0,
  tokens,
  tokensCulled: 0,
  lights: 0,
  walls: 0,
  shadowQuads: 0,
});

describe("tokensBucket", () => {
  it("puts a count in the contract's buckets", () => {
    expect([0, 1, 10, 11, 50, 51, 200, 201].map(tokensBucket)).toEqual([
      "0",
      "1-10",
      "1-10",
      "11-50",
      "11-50",
      "51-200",
      "51-200",
      ">200",
    ]);
  });
});

describe("the frame ring", () => {
  it("keeps at most its size, oldest out first", () => {
    const ring = createFrameRing(3);
    for (let i = 1; i <= 5; i++) ring.push(stats(i, 1));
    expect(ring.samples().map((s) => s.fps)).toEqual([3, 4, 5]);
    ring.clear();
    expect(ring.samples()).toEqual([]);
  });
});

describe("framesRecord", () => {
  it("is percentiles and a token bucket, and nothing else", () => {
    const samples = Array.from({ length: 100 }, (_, i) =>
      stats(i + 1, 100 - i, 42),
    );
    expect(framesRecord(samples)).toEqual({
      "fps.p5": 5,
      "fps.p50": 50,
      "frame_ms.p95": 95,
      "tokens.bucket": "11-50",
    });
  });

  it("is nothing without samples", () => {
    expect(framesRecord([])).toBeNull();
  });
});

describe("watchFrames", () => {
  function harness() {
    const events: { name: EventName; attrs?: Attrs }[] = [];
    const t: Telemetry = {
      ...noopTelemetry,
      event: (name, attrs) => events.push({ name, attrs }),
    };
    const signals = createLoadSignals(8, () => 0);
    let tick: (() => void) | null = null;
    let hide: (() => void) | null = null;
    let reading: EngineStats | null = stats(60, 16);
    const stop = watchFrames(t, {
      subscribe: signals.subscribe,
      read: async () => reading,
      every: (fn) => {
        tick = fn;
        return () => {
          tick = null;
        };
      },
      onPageHide: (fn) => {
        hide = fn;
        return () => {
          hide = null;
        };
      },
    });
    return {
      events,
      signals,
      tick: async () => {
        tick?.();
        await Promise.resolve();
        await Promise.resolve();
      },
      hide: () => hide?.(),
      ticking: () => tick !== null,
      set: (s: EngineStats | null) => {
        reading = s;
      },
      stop,
    };
  }

  it("samples nothing until the engine has started", async () => {
    const h = harness();
    expect(h.ticking()).toBe(false);
    h.hide();
    expect(h.events).toEqual([]);
    h.signals.emit({ stage: "started" });
    expect(h.ticking()).toBe(true);
  });

  it("sends one engine.frames record at pagehide, then starts again", async () => {
    const h = harness();
    h.signals.emit({ stage: "started" });
    await h.tick();
    await h.tick();
    h.set(null); // a second with no counters is no sample
    await h.tick();
    h.hide();
    expect(h.events).toEqual([
      {
        name: "engine.frames",
        attrs: {
          "fps.p5": 60,
          "fps.p50": 60,
          "frame_ms.p95": 16,
          "tokens.bucket": "1-10",
        },
      },
    ]);
    h.hide();
    expect(h.events).toHaveLength(1);
  });

  it("stops", () => {
    const h = harness();
    h.signals.emit({ stage: "started" });
    h.stop();
    expect(h.ticking()).toBe(false);
  });
});
