/**
 * Where `graphqlClient` asks for a `traceparent` (spec 086 US7).
 *
 * The telemetry chunk sets the tracer once it has loaded; until then, and
 * with telemetry off or the session unsampled, there is none and a request
 * carries no header. This module imports nothing, so the client never
 * pulls telemetry in.
 */

/** One traced request: its header, and the end of its span. */
export interface RequestTrace {
  traceparent: string;
  end(ok: boolean): void;
}

/** A trace for this document, or `null` when it is not traced. */
export type RequestTracer = (query: string) => RequestTrace | null;

let tracer: RequestTracer | null = null;

/** Set the tracer, or clear it with `null`. Returns the previous one. */
export function setRequestTracer(next: RequestTracer | null) {
  const previous = tracer;
  tracer = next;
  return previous;
}

/** A tracer that throws is a request that is not traced. */
export function traceRequest(query: string): RequestTrace | null {
  try {
    return tracer?.(query) ?? null;
  } catch {
    return null;
  }
}
