/**
 * Spec 086 FR-021: the one way out of the sealed page.
 *
 * A `POST` to the configured telemetry origin's `/v1/logs` or `/v1/traces`
 * goes to the real `fetch`, and only once the served config has said
 * telemetry is on and named that origin. Everything else that leaves the
 * page's own origin is still refused, so on a static host that sends no
 * `connect-src` header this is what limits posts.
 */
import type { TelemetryConfig } from "@thunderforge/telemetry";

const SIGNALS = ["/v1/logs", "/v1/traces"];

let allowed: { origin: string; paths: ReadonlySet<string> } | null = null;

/** Open the way to `config`'s endpoint, or close it with `null` or off. */
export function allowTelemetryTo(config: TelemetryConfig | null): void {
  allowed = null;
  if (!config?.enabled || !config.endpoint) return;
  let endpoint: URL;
  try {
    endpoint = new URL(config.endpoint);
  } catch {
    return;
  }
  if (endpoint.protocol !== "https:" && endpoint.protocol !== "http:") return;
  const prefix = endpoint.pathname.replace(/\/+$/, "");
  allowed = {
    origin: endpoint.origin,
    paths: new Set(SIGNALS.map((signal) => `${prefix}${signal}`)),
  };
}

/** Whether this request is a telemetry post the guard lets through. */
export function isTelemetryPost(method: string, url: URL): boolean {
  return (
    allowed !== null &&
    method === "POST" &&
    url.origin === allowed.origin &&
    url.search === "" &&
    allowed.paths.has(url.pathname)
  );
}
