/**
 * The core (FR-015 to FR-020): what a session may send, and when. It knows
 * no OTLP and no DOM. The wire format is the `TelemetrySink` port's business,
 * and redaction is the `Redactor` port's.
 */

import {
  ERROR_EVENTS,
  SAMPLED_EVENTS,
  filterAttributes,
  filterSpanAttributes,
  type AttrValue,
  type Attrs,
  type EventName,
  type FunnelStep,
  type ServiceName,
  type SpanName,
} from "./allowList.ts";
import type { TelemetryConfig } from "./config.ts";
import {
  errorAttributes,
  foldKey,
  withPersonalDetailsRemoved,
  type ErrorSource,
  type Redactor,
} from "./errors.ts";
import { errorsOnly, type Privacy } from "./privacy.ts";
import { BoundedQueue } from "./queue.ts";
import type { Batch, LogRecord } from "./records.ts";
import {
  loadSession,
  randomHex,
  saveSession,
  takePendingSteps,
  type SessionStorage,
} from "./session.ts";

export const MAX_EVENTS = 2000;
export const MAX_ERRORS = 50;

export interface TelemetrySink {
  /** Resolves to the number of records it dropped for size, if any. */
  send(batch: Batch, opts: { keepalive: boolean }): Promise<void | number>;
}

export interface SpanRef {
  traceId: string;
  spanId: string;
}

export interface Telemetry {
  event(name: EventName, attrs?: Attrs): void;
  error(source: ErrorSource, error: unknown): void;
  span(
    name: SpanName,
    start: number,
    end: number,
    attrs?: Attrs,
    parent?: SpanRef,
  ): SpanRef;
  funnel(step: FunnelStep, attrs?: Attrs): void;
  traceparent(): string | null;
  flush(keepalive: boolean): void;
}

const NO_SPAN: SpanRef = { traceId: "0".repeat(32), spanId: "0".repeat(16) };

export const noopTelemetry: Telemetry = {
  event() {},
  error() {},
  span: () => NO_SPAN,
  funnel() {},
  traceparent: () => null,
  flush() {},
};

export interface CreateTelemetryOptions {
  service: ServiceName;
  version: string;
  config: TelemetryConfig;
  sink: TelemetrySink;
  redact: Redactor;
  storage: SessionStorage;
  now: () => number;
  random: () => number;
  privacy: Privacy;
  /** Device facts for the resource (`ua.ts`). */
  resource?: Record<string, AttrValue>;
  /** `deployment.environment` on the anonymous tier (FR-019a). */
  anonymousEnvironment?: "production" | "self-hosted";
}

/** The resource of FR-019 and FR-019a. */
export function resourceFor(
  o: CreateTelemetryOptions,
): Record<string, AttrValue> {
  const tier = o.config.tier === "operator" ? "operator" : "anonymous";
  const out: Record<string, AttrValue> = {
    ...(o.resource ?? {}),
    "service.name": o.service,
    "service.version": o.version,
    "thunderforge.tier": tier,
  };
  const env =
    tier === "anonymous"
      ? (o.anonymousEnvironment ?? "self-hosted")
      : o.config.environment;
  if (env) out["deployment.environment"] = env;
  if (o.config.instanceId)
    out["thunderforge.instance.id"] = o.config.instanceId;
  return out;
}

interface Fold {
  record: LogRecord;
  count: number;
  sentCount: number;
}

export function createTelemetry(o: CreateTelemetryOptions): Telemetry {
  const limited = errorsOnly(o.privacy);
  const session = loadSession({
    storage: o.storage,
    now: o.now,
    random: o.random,
    sampleRate: o.config.sampleRate ?? 1,
    decideSampling: !limited,
  });
  const sampled = session.sampled && !limited;
  const resource = resourceFor(o);
  const queue = new BoundedQueue();
  const redact = withPersonalDetailsRemoved(o.redact);
  const folds = new Map<string, Fold>();
  let events = 0;
  let errors = 0;
  let internalErrors = 0;
  let droppedReported = 0;
  let droppedBySize = 0;

  const common = (name: string) => ({
    "event.name": name,
    "session.id": session.id,
    "t.ms": Math.max(0, Math.round(o.now() - session.start)),
  });

  const guard = (fn: () => void) => {
    try {
      fn();
    } catch {
      internalErrors += 1;
    }
  };

  const log = (
    name: string,
    attrs: Attrs | undefined,
    severity: LogRecord["severity"],
  ) => {
    const record: LogRecord = {
      kind: "log",
      name,
      time: o.now(),
      severity,
      attrs: { ...filterAttributes(attrs), ...common(name) },
    };
    queue.push(record);
    return record;
  };

  const t: Telemetry = {
    event(name, attrs) {
      guard(() => {
        const isError = ERROR_EVENTS.has(name);
        if (limited && !isError) return;
        if (SAMPLED_EVENTS.has(name) && !sampled) return;
        if (isError) {
          if (errors >= MAX_ERRORS) return;
          errors += 1;
        } else {
          if (events >= MAX_EVENTS) return;
          events += 1;
        }
        log(name, attrs, isError ? "error" : "info");
      });
    },

    error(source, error) {
      guard(() => {
        const attrs = errorAttributes(source, error, redact);
        const key = foldKey(attrs);
        const fold = folds.get(key);
        if (fold) {
          fold.count += 1;
          if (queue.has(fold.record)) {
            fold.record.attrs["error.count"] = fold.count;
            fold.sentCount = fold.count;
          }
          return;
        }
        if (errors >= MAX_ERRORS) return;
        errors += 1;
        const record = log("error", { ...attrs, "error.count": 1 }, "error");
        record.body = String(attrs["error.message"] ?? "");
        folds.set(key, { record, count: 1, sentCount: 1 });
      });
    },

    span(name, start, end, attrs, parent) {
      const ref: SpanRef = {
        traceId: parent?.traceId ?? randomHex(16, o.random),
        spanId: randomHex(8, o.random),
      };
      guard(() => {
        if (!sampled || events >= MAX_EVENTS) return;
        events += 1;
        queue.push({
          kind: "span",
          name,
          traceId: ref.traceId,
          spanId: ref.spanId,
          parentSpanId: parent?.spanId,
          start,
          end: Math.max(start, end),
          attrs: { ...filterSpanAttributes(attrs), "session.id": session.id },
        });
      });
      return ref;
    },

    funnel(step, attrs) {
      guard(() => {
        if (limited || session.steps.includes(step)) return;
        session.steps.push(step);
        saveSession(o.storage, session);
        t.event("funnel", { ...attrs, step });
      });
    },

    traceparent() {
      if (!sampled) return null;
      return `00-${randomHex(16, o.random)}-${randomHex(8, o.random)}-01`;
    },

    flush(keepalive) {
      guard(() => {
        for (const fold of folds.values()) {
          if (fold.count > fold.sentCount && !queue.has(fold.record)) {
            const again: LogRecord = {
              ...fold.record,
              time: o.now(),
              attrs: { ...fold.record.attrs, "error.count": fold.count },
            };
            fold.record = again;
            fold.sentCount = fold.count;
            queue.push(again);
          }
        }
        const records = queue.drain();
        const dropped = queue.dropped + droppedBySize - droppedReported;
        if (!limited && (dropped > 0 || internalErrors > 0)) {
          // Added after the drain, so reporting a drop never causes one.
          const name = "telemetry.internal";
          records.push({
            kind: "log",
            name,
            time: o.now(),
            severity: "info",
            attrs: {
              internal_errors: internalErrors,
              dropped: Math.max(0, dropped),
              ...common(name),
            },
          });
          droppedReported += Math.max(0, dropped);
          internalErrors = 0;
        }
        if (records.length === 0) return;
        o.sink
          .send({ resource, records }, { keepalive })
          .then((n) => {
            if (typeof n === "number" && n > 0) droppedBySize += n;
          })
          .catch(() => {
            // A failed send is dropped (FR-020), never reported as an error.
          });
      });
    },
  };

  for (const step of takePendingSteps(o.storage)) t.funnel(step);
  return t;
}
