/** The OTLP/HTTP adapter for the `TelemetrySink` port (FR-020). */

import type { TelemetrySink } from "../telemetry.ts";
import { encodeBatch } from "./encode.ts";

export { encodeBatch, logsBody, tracesBody, MAX_BODY_BYTES } from "./encode.ts";

/**
 * Posts each batch once. A failed post is dropped, never retried and never
 * reported as an error. `credentials: "omit"`, so no cookie ever leaves.
 */
export function otlpHttpSink(
  endpoint: string,
  fetchImpl: typeof fetch,
): TelemetrySink {
  const base = endpoint.replace(/\/+$/, "");
  return {
    async send(batch, { keepalive }) {
      const { posts, oversized } = encodeBatch(batch);
      await Promise.all(
        posts.map((p) =>
          fetchImpl(`${base}${p.path}`, {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: p.body,
            credentials: "omit",
            keepalive,
          }).catch(() => undefined),
        ),
      );
      return oversized;
    },
  };
}
