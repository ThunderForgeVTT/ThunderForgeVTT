import assert from "node:assert/strict";
import { test } from "node:test";
import { BoundedQueue, QUEUE_CAPACITY } from "./queue.ts";
import {
  MAX_ERRORS,
  MAX_EVENTS,
  createTelemetry,
  type TelemetrySink,
} from "./telemetry.ts";
import { memoryStorage } from "./session.ts";
import type { Batch, LogRecord } from "./records.ts";

const rec = (n: number): LogRecord => ({
  kind: "log",
  name: `e${n}`,
  time: n,
  severity: "info",
  attrs: {},
});

test("the queue holds 200 and drops the oldest", () => {
  const q = new BoundedQueue();
  assert.equal(QUEUE_CAPACITY, 200);
  for (let i = 0; i < 250; i++) q.push(rec(i));
  assert.equal(q.length, 200);
  assert.equal(q.dropped, 50);
  assert.equal(q.drain()[0].name, "e50");
  assert.equal(q.length, 0);
});

function harness(sink?: TelemetrySink) {
  const sent: Batch[] = [];
  const t = createTelemetry({
    service: "thunderforge-web",
    version: "1",
    config: { enabled: true, endpoint: "https://x.example", sampleRate: 1 },
    sink: sink ?? { send: async (b) => void sent.push(b) },
    redact: (s) => s,
    storage: memoryStorage(),
    now: () => 10,
    random: () => 0,
    privacy: { gpc: false, dnt: false },
  });
  const records = () => sent.flatMap((b) => b.records);
  return { t, sent, records };
}

test("at most 2,000 events per session", () => {
  const { t, records } = harness();
  for (let i = 0; i < MAX_EVENTS + 100; i++) {
    t.event("demo.action", { action: "token_moved" });
    if (i % 100 === 0) t.flush(false);
  }
  t.flush(false);
  assert.equal(
    records().filter((r) => r.name === "demo.action").length,
    MAX_EVENTS,
  );
});

test("at most 50 errors, and a repeat folds into error.count", () => {
  const { t, records } = harness();
  for (let i = 0; i < 80; i++) t.error("onerror", new Error(`distinct ${i}`));
  t.flush(false);
  assert.equal(records().filter((r) => r.name === "error").length, MAX_ERRORS);
});

test("a repeated error is folded while queued and re-sent with its count after", () => {
  const { t, records } = harness();
  t.error("onerror", new Error("same"));
  t.error("onerror", new Error("same"));
  t.flush(false);
  let errs = records().filter((r) => r.name === "error");
  assert.equal(errs.length, 1);
  assert.equal(errs[0].attrs["error.count"], 2);
  t.error("onerror", new Error("same"));
  t.flush(false);
  errs = records().filter((r) => r.name === "error");
  assert.equal(errs.length, 2);
  assert.equal(errs[1].attrs["error.count"], 3);
  t.flush(false);
  assert.equal(records().filter((r) => r.name === "error").length, 2);
});

test("a dead endpoint keeps the queue bounded and reports drops", async () => {
  let calls = 0;
  const { t } = harness({
    send: async () => {
      calls += 1;
      throw new Error("refused");
    },
  });
  for (let i = 0; i < 1000; i++)
    t.event("demo.action", { action: "token_moved" });
  t.flush(false);
  await new Promise((r) => setTimeout(r, 0));
  assert.equal(calls, 1);
});

test("drops are reported once in telemetry.internal", () => {
  const { t, records } = harness();
  for (let i = 0; i < 230; i++)
    t.event("demo.action", { action: "token_moved" });
  t.flush(false);
  t.event("page_view", { route: "/" });
  t.flush(false);
  const internal = records().filter((r) => r.name === "telemetry.internal");
  assert.equal(internal.length, 1);
  assert.equal(internal[0].attrs.dropped, 30);
});
