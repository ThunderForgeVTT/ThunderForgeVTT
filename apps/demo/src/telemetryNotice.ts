/**
 * Spec 086 FR-024: the demo notice's telemetry line, from the served config.
 * Anonymous counts go to ThunderForge, operator counts to whoever runs this
 * server, and with telemetry off the notice says nothing about it.
 */
import type { TelemetryConfig } from "@thunderforge/telemetry";

export interface TelemetryLine {
  text: string;
  href: string;
}

/** The landing's **What we measure**. */
const WHAT_WE_MEASURE = "/#telemetry";

export function telemetryLine(
  config: TelemetryConfig | null,
): TelemetryLine | null {
  if (!config?.enabled) return null;
  const to =
    config.tier === "operator" ? "this server's operator" : "ThunderForge";
  return {
    text: `Anonymous usage counts go to ${to}; what you type does not.`,
    href: WHAT_WE_MEASURE,
  };
}

/** The line, for `useSyncExternalStore`: `null` until the config is read. */
export function createNoticeStore() {
  let line: TelemetryLine | null = null;
  const listeners = new Set<() => void>();
  return {
    get: () => line,
    set(config: TelemetryConfig | null) {
      line = telemetryLine(config);
      for (const l of listeners) l();
    },
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

export const noticeStore = createNoticeStore();
