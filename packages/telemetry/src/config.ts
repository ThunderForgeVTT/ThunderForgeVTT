/**
 * The served config, read once before `load` (contracts/served-config-and-csp.md).
 *
 * Anything that is not a clear "on" is off: a failed fetch, a body that does
 * not parse, a missing `enabled`, an endpoint that is not an http(s) URL. Off
 * means the chunk is never imported, so a broken config costs nothing.
 */

export type Tier = "anonymous" | "operator";

export interface TelemetryConfig {
  enabled: boolean;
  endpoint?: string;
  sampleRate?: number;
  environment?: string;
  tier?: Tier;
  instanceId?: string;
}

/** The config, or `null` for off. */
export function parseConfig(json: unknown): TelemetryConfig | null {
  if (!json || typeof json !== "object") return null;
  const raw = json as Record<string, unknown>;
  if (raw.enabled !== true) return null;
  if (typeof raw.endpoint !== "string") return null;
  let url: URL;
  try {
    url = new URL(raw.endpoint);
  } catch {
    return null;
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") return null;
  const endpoint = raw.endpoint.trim().replace(/\/+$/, "");
  const rate =
    typeof raw.sampleRate === "number" && Number.isFinite(raw.sampleRate)
      ? Math.min(1, Math.max(0, raw.sampleRate))
      : 1;
  const config: TelemetryConfig = {
    enabled: true,
    endpoint,
    sampleRate: rate,
    tier: raw.tier === "operator" ? "operator" : "anonymous",
  };
  if (typeof raw.environment === "string" && raw.environment.length > 0) {
    config.environment = raw.environment.slice(0, 64);
  }
  if (typeof raw.instanceId === "string" && raw.instanceId.length > 0) {
    config.instanceId = raw.instanceId.slice(0, 64);
  }
  return config;
}

/** Fetches and parses the config. Any failure is off. */
export async function readConfig(
  url: string,
  fetchImpl: typeof fetch,
): Promise<TelemetryConfig | null> {
  try {
    const res = await fetchImpl(url, {
      credentials: "omit",
      cache: "no-store",
    });
    if (res.status !== 200) return null;
    return parseConfig(await res.json());
  } catch {
    return null;
  }
}

/** The endpoint's origin, for `connect-src`. Empty when it has none. */
export function endpointOrigin(endpoint: string | undefined): string {
  if (!endpoint) return "";
  try {
    return new URL(endpoint).origin;
  } catch {
    return "";
  }
}
