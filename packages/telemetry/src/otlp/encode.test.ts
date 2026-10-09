import assert from "node:assert/strict";
import { test } from "node:test";
import { encodeBatch } from "./encode.ts";
import { resourceFor } from "../telemetry.ts";
import type { Batch, LogRecord } from "../records.ts";

const base = {
  service: "thunderforge-demo" as const,
  version: "1.2.3",
  sink: { send: async () => {} },
  redact: (s: string) => s,
  storage: { getItem: () => null, setItem: () => {} },
  now: () => 0,
  random: () => 0,
  privacy: { gpc: false, dnt: false },
};

test("a log record's OTLP/JSON shape", () => {
  const batch: Batch = {
    resource: { "service.name": "thunderforge-demo" },
    records: [
      {
        kind: "log",
        name: "page_view",
        time: 1700000000123,
        severity: "info",
        attrs: { route: "/", "t.ms": 12, value: 1.5, "device.mobile": false },
      },
      {
        kind: "log",
        name: "error",
        time: 1,
        severity: "error",
        body: "boom",
        attrs: {},
      },
    ],
  };
  const { posts } = encodeBatch(batch);
  assert.equal(posts.length, 1);
  assert.equal(posts[0].path, "/v1/logs");
  const body = JSON.parse(posts[0].body);
  const rl = body.resourceLogs[0];
  assert.deepEqual(rl.resource.attributes, [
    { key: "service.name", value: { stringValue: "thunderforge-demo" } },
  ]);
  const [a, b] = rl.scopeLogs[0].logRecords;
  assert.equal(a.timeUnixNano, "1700000000123000000");
  assert.equal(a.severityNumber, 9);
  assert.deepEqual(a.body, { stringValue: "page_view" });
  assert.deepEqual(a.attributes, [
    { key: "route", value: { stringValue: "/" } },
    { key: "t.ms", value: { intValue: "12" } },
    { key: "value", value: { doubleValue: 1.5 } },
    { key: "device.mobile", value: { boolValue: false } },
  ]);
  assert.equal(b.severityNumber, 17);
  assert.deepEqual(b.body, { stringValue: "boom" });
});

test("a span's OTLP/JSON shape", () => {
  const { posts } = encodeBatch({
    resource: {},
    records: [
      {
        kind: "span",
        name: "download",
        traceId: "a".repeat(32),
        spanId: "b".repeat(16),
        parentSpanId: "c".repeat(16),
        start: 1000,
        end: 2000,
        attrs: { bytes: 10 },
      },
    ],
  });
  assert.equal(posts[0].path, "/v1/traces");
  const s = JSON.parse(posts[0].body).resourceSpans[0].scopeSpans[0].spans[0];
  assert.equal(s.traceId, "a".repeat(32));
  assert.equal(s.parentSpanId, "c".repeat(16));
  assert.equal(s.startTimeUnixNano, "1000000000");
  assert.equal(s.endTimeUnixNano, "2000000000");
});

test("the resource per tier (FR-019, FR-019a)", () => {
  const anon = resourceFor({
    ...base,
    config: {
      enabled: true,
      endpoint: "https://t",
      tier: "anonymous",
      environment: "staging",
      instanceId: "id-1",
    },
    anonymousEnvironment: "production",
    resource: { "browser.family": "firefox" },
  });
  assert.deepEqual(anon, {
    "browser.family": "firefox",
    "service.name": "thunderforge-demo",
    "service.version": "1.2.3",
    "thunderforge.tier": "anonymous",
    "deployment.environment": "production",
    "thunderforge.instance.id": "id-1",
  });
  const selfHosted = resourceFor({
    ...base,
    config: { enabled: true, endpoint: "https://t", environment: "staging" },
  });
  assert.equal(selfHosted["deployment.environment"], "self-hosted");
  assert.equal(selfHosted["thunderforge.instance.id"], undefined);
  const op = resourceFor({
    ...base,
    config: {
      enabled: true,
      endpoint: "https://t",
      tier: "operator",
      environment: "staging",
    },
  });
  assert.equal(op["deployment.environment"], "staging");
  assert.equal(op["thunderforge.tier"], "operator");
});

test("a batch over 60 KB is split, and a single huge record is dropped", () => {
  const records: LogRecord[] = Array.from({ length: 40 }, (_, i) => ({
    kind: "log",
    name: "error",
    time: i,
    severity: "error",
    attrs: { "error.stack": "s".repeat(4000) },
  }));
  records.push({
    kind: "log",
    name: "error",
    time: 99,
    severity: "error",
    attrs: { "error.stack": "s".repeat(70_000) },
  });
  const { posts, oversized } = encodeBatch({
    resource: { "service.name": "x" },
    records,
  });
  assert.equal(oversized, 1);
  assert.ok(posts.length >= 3);
  let total = 0;
  for (const p of posts) {
    assert.ok(new TextEncoder().encode(p.body).length <= 60_000);
    total += JSON.parse(p.body).resourceLogs[0].scopeLogs[0].logRecords.length;
  }
  assert.equal(total, 40);
});
