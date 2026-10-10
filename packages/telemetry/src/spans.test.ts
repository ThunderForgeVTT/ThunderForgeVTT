import assert from "node:assert/strict";
import { test } from "node:test";
import { createTelemetry } from "./telemetry.ts";
import { memoryStorage } from "./session.ts";
import type { Batch, SpanRecord } from "./records.ts";

function make(sampleRate: number, random = () => 0.25) {
  const sent: Batch[] = [];
  const t = createTelemetry({
    service: "thunderforge-web",
    version: "1",
    config: { enabled: true, endpoint: "https://x.example", sampleRate },
    sink: { send: async (b) => void sent.push(b) },
    redact: (s) => s,
    storage: memoryStorage(),
    now: () => 1000,
    random,
    privacy: { gpc: false, dnt: false },
  });
  const spans = () =>
    sent
      .flatMap((b) => b.records)
      .filter((r): r is SpanRecord => r.kind === "span");
  return { t, spans };
}

test("an open span's traceparent names the span it becomes", async () => {
  const { t, spans } = make(1);
  const open = t.begin("graphql.request", 10, {
    "graphql.operation.type": "query",
  });
  assert.match(open.traceparent ?? "", /^00-[0-9a-f]{32}-[0-9a-f]{16}-01$/);
  const [, traceId, spanId] = (open.traceparent ?? "").split("-");
  assert.deepEqual(open.ref, { traceId, spanId });
  open.end(25, { "graphql.root_field": "world" });
  t.flush(false);
  await Promise.resolve();
  const [span] = spans();
  assert.equal(span.name, "graphql.request");
  assert.equal(span.traceId, traceId);
  assert.equal(span.spanId, spanId);
  assert.equal(span.start, 10);
  assert.equal(span.end, 25);
  assert.equal(span.attrs["graphql.operation.type"], "query");
  assert.equal(span.attrs["graphql.root_field"], "world");
});

test("a child opened under a parent shares its trace", async () => {
  const { t, spans } = make(1);
  const parent = t.begin("engine.load", 0);
  const child = t.begin("download", 0, undefined, parent.ref);
  child.end(5);
  parent.end(9);
  t.flush(false);
  await Promise.resolve();
  const [c, p] = spans();
  assert.equal(c.traceId, p.traceId);
  assert.equal(c.parentSpanId, p.spanId);
  assert.equal(p.parentSpanId, undefined);
});

test("an unsampled session gives no traceparent and records nothing", async () => {
  const { t, spans } = make(0, () => 0.9);
  const open = t.begin("graphql.request", 0);
  assert.equal(open.traceparent, null);
  open.end(1);
  t.span("page.load", 0, 1);
  t.flush(false);
  await Promise.resolve();
  assert.deepEqual(spans(), []);
});

test("ending a span twice records it once", async () => {
  const { t, spans } = make(1);
  const open = t.begin("graphql.request", 0);
  open.end(1);
  open.end(2);
  t.flush(false);
  await Promise.resolve();
  assert.equal(spans().length, 1);
});
