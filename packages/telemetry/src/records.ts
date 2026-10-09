/** What the core queues, before any wire format. */

import type { AttrValue } from "./allowList.ts";

export interface LogRecord {
  kind: "log";
  name: string;
  /** Epoch milliseconds. */
  time: number;
  severity: "info" | "error";
  body?: string;
  attrs: Record<string, AttrValue>;
}

export interface SpanRecord {
  kind: "span";
  name: string;
  traceId: string;
  spanId: string;
  parentSpanId?: string;
  /** Epoch milliseconds. */
  start: number;
  end: number;
  attrs: Record<string, AttrValue>;
}

export type TelemetryRecord = LogRecord | SpanRecord;

export interface Batch {
  resource: Record<string, AttrValue>;
  records: TelemetryRecord[];
}
