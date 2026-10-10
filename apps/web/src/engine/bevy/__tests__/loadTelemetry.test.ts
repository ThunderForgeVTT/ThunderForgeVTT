import { describe, expect, it } from "vitest";
import type {
  Attrs,
  EventName,
  SpanRef,
  Telemetry,
} from "@thunderforge/telemetry";
import { createLoadSignals } from "../loadSignals";
import { watchEngineLoad } from "../loadTelemetry";

function recorder() {
  const events: { name: EventName; attrs?: Attrs }[] = [];
  const t: Telemetry = {
    event: (name, attrs) => events.push({ name, attrs }),
    error() {},
    span: () => ({ traceId: "0".repeat(32), spanId: "0".repeat(16) }),
    begin: () => ({
      ref: { traceId: "0".repeat(32), spanId: "0".repeat(16) },
      traceparent: null,
      end() {},
    }),
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

/** A recorder of spans, each with the parent it was opened under. */
function spanRecorder() {
  const spans: {
    name: string;
    start: number;
    end: number;
    attrs?: Attrs;
    id: string;
    parent?: string;
  }[] = [];
  let n = 0;
  const t: Telemetry = {
    ...recorder().t,
    span(name, start, end, attrs, parent) {
      const open = t.begin(name, start, attrs, parent);
      open.end(end);
      return open.ref;
    },
    begin(name, start, attrs, parent?: SpanRef) {
      const ref = { traceId: "t".repeat(32), spanId: String(++n) };
      return {
        ref,
        traceparent: null,
        end: (end, more) =>
          spans.push({
            name,
            start,
            end,
            attrs: { ...attrs, ...more },
            id: ref.spanId,
            parent: parent?.spanId,
          }),
      };
    },
  };
  return { t, spans };
}

/** Signals stamped by a clock the test moves. */
function clocked() {
  let at = 0;
  const signals = createLoadSignals(8, () => at);
  return {
    signals,
    at: (ms: number) => {
      at = ms;
    },
  };
}

describe("the engine.load trace", () => {
  it("is one span with download, compile and start children", () => {
    const { signals, at } = clocked();
    const { t, spans } = spanRecorder();
    watchEngineLoad(t, signals.subscribe);
    at(1000);
    signals.emit({ stage: "downloading" });
    at(3000);
    signals.emit({ stage: "downloaded", bytes: 52_000_000 });
    at(3400);
    signals.emit({ stage: "starting" });
    at(3900);
    signals.emit({ stage: "started" });

    const root = spans.find((s) => s.name === "engine.load");
    expect(root).toMatchObject({ start: 1000, end: 3900, parent: undefined });
    expect(root?.attrs).toEqual({ bytes: 52_000_000 });
    const children = spans
      .filter((s) => s.parent === root?.id)
      .map(({ name, start, end }) => ({ name, start, end }));
    expect(children).toEqual([
      { name: "download", start: 1000, end: 3000 },
      { name: "compile", start: 3000, end: 3400 },
      { name: "start", start: 3400, end: 3900 },
    ]);
  });

  it("is timed from the signals, not from when telemetry arrived", () => {
    // The chunk loads after the page; the stamps were taken at the time.
    const { signals, at } = clocked();
    at(10);
    signals.emit({ stage: "downloading" });
    at(20);
    signals.emit({ stage: "downloaded", bytes: 1 });
    at(30);
    signals.emit({ stage: "starting" });
    at(40);
    signals.emit({ stage: "started" });
    at(99_999);
    const { t, spans } = spanRecorder();
    watchEngineLoad(t, signals.subscribe);
    expect(spans.find((s) => s.name === "engine.load")).toMatchObject({
      start: 10,
      end: 40,
    });
  });

  it("restarts the timing on a retried download", () => {
    const { signals, at } = clocked();
    const { t, spans } = spanRecorder();
    watchEngineLoad(t, signals.subscribe);
    at(0);
    signals.emit({ stage: "downloading" });
    at(500);
    signals.emit({ stage: "downloading" });
    at(600);
    signals.emit({ stage: "downloaded", bytes: 1 });
    at(700);
    signals.emit({ stage: "starting" });
    at(800);
    signals.emit({ stage: "started" });
    expect(spans.find((s) => s.name === "engine.load")?.start).toBe(500);
  });

  it("is sent once, and not for a remount that does not start the engine", () => {
    const { signals, at } = clocked();
    const { t, spans } = spanRecorder();
    watchEngineLoad(t, signals.subscribe);
    for (const stage of ["downloading", "starting", "started"] as const) {
      at(1);
      signals.emit({ stage });
    }
    signals.emit({ stage: "downloading" });
    signals.emit({ stage: "starting" });
    signals.emit({ stage: "started" });
    expect(spans.filter((s) => s.name === "engine.load")).toHaveLength(1);
  });

  it("leaves out the download it could not see", () => {
    // wasm-bindgen fetched the module itself, so no bytes were counted.
    const { signals, at } = clocked();
    const { t, spans } = spanRecorder();
    watchEngineLoad(t, signals.subscribe);
    at(0);
    signals.emit({ stage: "downloading" });
    at(50);
    signals.emit({ stage: "starting" });
    at(60);
    signals.emit({ stage: "started" });
    expect(spans.map((s) => s.name).sort()).toEqual([
      "compile",
      "engine.load",
      "start",
    ]);
    expect(spans.find((s) => s.name === "compile")?.start).toBe(0);
  });
});
