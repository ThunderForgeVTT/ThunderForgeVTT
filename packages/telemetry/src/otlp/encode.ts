/**
 * OTLP/JSON (R10): the logs and traces request bodies, split so that no body
 * passes 60 KB, which keeps every `keepalive` post under the browser's 64 KB.
 */

import type { AttrValue } from "../allowList.ts";
import type {
  Batch,
  LogRecord,
  SpanRecord,
  TelemetryRecord,
} from "../records.ts";

export const MAX_BODY_BYTES = 60_000;
export const SCOPE = { name: "@thunderforge/telemetry" };

type AnyValue =
  | { stringValue: string }
  | { intValue: string }
  | { doubleValue: number }
  | { boolValue: boolean };

export interface KeyValue {
  key: string;
  value: AnyValue;
}

export function anyValue(v: AttrValue): AnyValue {
  if (typeof v === "boolean") return { boolValue: v };
  if (typeof v === "number") {
    return Number.isInteger(v) ? { intValue: String(v) } : { doubleValue: v };
  }
  return { stringValue: v };
}

export function keyValues(attrs: Record<string, AttrValue>): KeyValue[] {
  return Object.entries(attrs).map(([key, v]) => ({ key, value: anyValue(v) }));
}

const nanos = (ms: number) => `${Math.round(ms)}000000`;

function logRecord(r: LogRecord) {
  return {
    timeUnixNano: nanos(r.time),
    observedTimeUnixNano: nanos(r.time),
    severityNumber: r.severity === "error" ? 17 : 9,
    severityText: r.severity === "error" ? "ERROR" : "INFO",
    body: { stringValue: r.body ?? r.name },
    attributes: keyValues(r.attrs),
  };
}

function span(r: SpanRecord) {
  const out: Record<string, unknown> = {
    traceId: r.traceId,
    spanId: r.spanId,
    name: r.name,
    kind: 1,
    startTimeUnixNano: nanos(r.start),
    endTimeUnixNano: nanos(r.end),
    attributes: keyValues(r.attrs),
  };
  if (r.parentSpanId) out.parentSpanId = r.parentSpanId;
  return out;
}

export function logsBody(
  resource: Record<string, AttrValue>,
  records: LogRecord[],
) {
  return {
    resourceLogs: [
      {
        resource: { attributes: keyValues(resource) },
        scopeLogs: [{ scope: SCOPE, logRecords: records.map(logRecord) }],
      },
    ],
  };
}

export function tracesBody(
  resource: Record<string, AttrValue>,
  records: SpanRecord[],
) {
  return {
    resourceSpans: [
      {
        resource: { attributes: keyValues(resource) },
        scopeSpans: [{ scope: SCOPE, spans: records.map(span) }],
      },
    ],
  };
}

export interface EncodedPost {
  path: "/v1/logs" | "/v1/traces";
  body: string;
}

const bytes = (s: string) => new TextEncoder().encode(s).length;

function split<T extends TelemetryRecord>(
  records: T[],
  encode: (rs: T[]) => string,
  max: number,
): { bodies: string[]; oversized: number } {
  const bodies: string[] = [];
  let oversized = 0;
  let group: T[] = [];
  const envelope = bytes(encode([]));
  let size = envelope;
  for (const r of records) {
    // The record's share: its body less the empty envelope, plus a comma.
    const own = bytes(encode([r])) - envelope + 1;
    if (envelope + own > max) {
      oversized += 1;
      continue;
    }
    if (group.length > 0 && size + own > max) {
      bodies.push(encode(group));
      group = [];
      size = envelope;
    }
    group.push(r);
    size += own;
  }
  if (group.length > 0) bodies.push(encode(group));
  return { bodies, oversized };
}

/** The posts a batch becomes, and how many records were too big to send. */
export function encodeBatch(
  batch: Batch,
  max = MAX_BODY_BYTES,
): { posts: EncodedPost[]; oversized: number } {
  const logs = batch.records.filter((r): r is LogRecord => r.kind === "log");
  const spans = batch.records.filter((r): r is SpanRecord => r.kind === "span");
  const l = split(
    logs,
    (rs) => JSON.stringify(logsBody(batch.resource, rs)),
    max,
  );
  const s = split(
    spans,
    (rs) => JSON.stringify(tracesBody(batch.resource, rs)),
    max,
  );
  return {
    posts: [
      ...l.bodies.map((body) => ({ path: "/v1/logs" as const, body })),
      ...s.bodies.map((body) => ({ path: "/v1/traces" as const, body })),
    ],
    oversized: l.oversized + s.oversized,
  };
}
