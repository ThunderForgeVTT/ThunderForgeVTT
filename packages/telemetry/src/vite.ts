/**
 * Dev and preview only (R14): serve `telemetry.json` and the demo's
 * `connect-src`, as the server and the landing's nginx do. Off unless
 * `THUNDERFORGE_PREVIEW_TELEMETRY` holds a config.
 */

import type { Plugin } from "vite";
import { endpointOrigin, parseConfig, type TelemetryConfig } from "./config.ts";

export const OFF_JSON = '{"enabled":false}';

export function connectSrcFor(cfg: TelemetryConfig | null): string {
  const origin = cfg?.enabled ? endpointOrigin(cfg.endpoint) : "";
  return origin
    ? `connect-src 'self' data: blob: ${origin}`
    : "connect-src 'self' data: blob:";
}

/** The config the env var holds, or null for off. */
export function previewConfig(raw: string | undefined): TelemetryConfig | null {
  if (!raw) return null;
  try {
    return parseConfig(JSON.parse(raw));
  } catch {
    return null;
  }
}

export function servedJson(cfg: TelemetryConfig | null): string {
  return cfg ? JSON.stringify(cfg) : OFF_JSON;
}

export function servedTelemetry(opts: {
  paths: string[];
  env?: Record<string, string | undefined>;
}): Plugin {
  const env = opts.env ?? process.env;
  const cfg = previewConfig(env.THUNDERFORGE_PREVIEW_TELEMETRY);
  const body = servedJson(cfg);
  const csp = connectSrcFor(cfg);
  const paths = new Set(opts.paths);
  const middleware = (
    req: { url?: string },
    res: {
      setHeader(k: string, v: string): void;
      statusCode: number;
      end(b: string): void;
    },
    next: () => void,
  ) => {
    const path = (req.url ?? "").split("?")[0];
    if (!paths.has(path)) return next();
    res.statusCode = 200;
    res.setHeader("Content-Type", "application/json");
    res.setHeader("Cache-Control", "no-store");
    res.end(body);
  };
  return {
    name: "thunderforge-served-telemetry",
    config() {
      const headers = { "Content-Security-Policy": csp };
      return { server: { headers }, preview: { headers } };
    },
    configureServer(server) {
      server.middlewares.use(middleware);
    },
    configurePreviewServer(server) {
      server.middlewares.use(middleware);
    },
  };
}
