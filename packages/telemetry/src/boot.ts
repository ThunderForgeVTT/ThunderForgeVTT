/**
 * What an app imports eagerly (FR-017). It reads the config before `load`,
 * waits for `load`, checks GPC and DNT, and only then imports the app's
 * chunk. Off, or any failure, resolves to `noopTelemetry` and imports nothing.
 */

import type { FunnelStep } from "./allowList.ts";
import { readConfig, type TelemetryConfig } from "./config.ts";
import {
  addPendingStep,
  memoryStorage,
  type SessionStorage,
} from "./session.ts";
import { noopTelemetry, type Telemetry } from "./telemetry.ts";

let current: Telemetry = noopTelemetry;

/**
 * The app's telemetry: `noopTelemetry` until the chunk arrives, and the real
 * one after. A funnel step reached before then is kept and sent on arrival.
 */
export const telemetry: Telemetry = {
  event: (name, attrs) => current.event(name, attrs),
  error: (source, error) => current.error(source, error),
  span: (name, start, end, attrs, parent) =>
    current.span(name, start, end, attrs, parent),
  funnel: (step, attrs) => {
    if (current === noopTelemetry)
      addPendingStep(sessionStore(), step as FunnelStep);
    else current.funnel(step, attrs);
  },
  traceparent: () => current.traceparent(),
  flush: (keepalive) => current.flush(keepalive),
};

export function sessionStore(): SessionStorage {
  try {
    const s = globalThis.sessionStorage;
    if (s) return s;
  } catch {
    // Storage blocked.
  }
  return fallback;
}
const fallback = memoryStorage();

function loaded(): Promise<void> {
  if (typeof document === "undefined" || document.readyState === "complete") {
    return Promise.resolve();
  }
  return new Promise((resolve) =>
    addEventListener("load", () => resolve(), { once: true }),
  );
}

export async function bootTelemetry(opts: {
  configUrl: string;
  fetchImpl?: typeof fetch;
  /** Told the served config, or `null` for off, before anything loads. The
   *  demo's notice says where counts go from it. */
  onConfig?: (config: TelemetryConfig | null) => void;
  load: () => Promise<(cfg: TelemetryConfig) => Telemetry>;
}): Promise<Telemetry> {
  let config: TelemetryConfig | null = null;
  try {
    const fetchImpl = opts.fetchImpl ?? globalThis.fetch.bind(globalThis);
    config = await readConfig(opts.configUrl, fetchImpl);
  } catch {
    config = null;
  }
  try {
    opts.onConfig?.(config);
  } catch {
    // A listener's failure is not telemetry's.
  }
  try {
    if (!config) return noopTelemetry;
    await loaded();
    const start = await opts.load();
    current = start(config);
    return current;
  } catch {
    return noopTelemetry;
  }
}
