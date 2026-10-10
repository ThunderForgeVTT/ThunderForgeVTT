import { describe, expect, it } from "vitest";
import type { Attrs, EventName, Telemetry } from "@thunderforge/telemetry";
import { createLoadSignals } from "../loadSignals";
import { watchEngineLoad } from "../loadTelemetry";

function recorder() {
  const events: { name: EventName; attrs?: Attrs }[] = [];
  const t: Telemetry = {
    event: (name, attrs) => events.push({ name, attrs }),
    error() {},
    span: () => ({ traceId: "0".repeat(32), spanId: "0".repeat(16) }),
    funnel() {},
    traceparent: () => null,
    flush() {},
  };
  return { t, events };
}

describe("engine load signals", () => {
  it("send engine.load_failed with the closed reason, never the words", () => {
    const signals = createLoadSignals();
    const { t, events } = recorder();
    watchEngineLoad(t, signals.subscribe);
    signals.emit({ stage: "unavailable", reason: "no_webgl2" });
    expect(events).toEqual([
      {
        name: "engine.load_failed",
        attrs: { reason: "no_webgl2", stage: "probe" },
      },
    ]);
  });

  it("reach a subscriber that arrives after the failure", () => {
    // The chunk loads after the page; a board that failed at once is still
    // reported once telemetry is up.
    const signals = createLoadSignals();
    signals.emit({ stage: "unavailable", reason: "no_webgl2" });
    const { t, events } = recorder();
    watchEngineLoad(t, signals.subscribe);
    expect(events.map((e) => e.name)).toEqual(["engine.load_failed"]);
  });

  it("send nothing for a load that goes on", () => {
    const signals = createLoadSignals();
    const { t, events } = recorder();
    watchEngineLoad(t, signals.subscribe);
    signals.emit({ stage: "downloading" });
    signals.emit({ stage: "starting" });
    expect(events).toEqual([]);
  });

  it("keep a bounded history", () => {
    const signals = createLoadSignals(3);
    for (let i = 0; i < 10; i++) signals.emit({ stage: "downloading" });
    const seen: unknown[] = [];
    signals.subscribe((s) => seen.push(s));
    expect(seen).toHaveLength(3);
  });

  it("unsubscribe", () => {
    const signals = createLoadSignals();
    const { t, events } = recorder();
    const stop = watchEngineLoad(t, signals.subscribe);
    stop();
    signals.emit({ stage: "unavailable", reason: "no_webgl2" });
    expect(events).toEqual([]);
  });
});
