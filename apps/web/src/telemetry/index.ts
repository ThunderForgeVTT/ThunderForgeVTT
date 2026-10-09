/**
 * The web app's telemetry chunk (spec 086 FR-017). `main.tsx` imports it only
 * after `/telemetry.json` said yes and the page has loaded, so a session with
 * telemetry off never downloads it.
 */

import {
  createTelemetry,
  deviceAttributes,
  privacyOf,
  sessionStore,
  type Telemetry,
  type TelemetryConfig,
} from "@thunderforge/telemetry";
import { pageView, startCollectors } from "@thunderforge/telemetry/browser";
import { otlpHttpSink } from "@thunderforge/telemetry/otlp";
import { redact } from "../services/feedbackRedaction";
import { routeTemplate, watchRoutes } from "./routes";

interface NavigatorExtras {
  userAgentData?: {
    brands?: { brand: string; version: string }[];
    platform?: string;
    mobile?: boolean;
  };
  deviceMemory?: number;
}

function device() {
  const nav = navigator as Navigator & NavigatorExtras;
  return deviceAttributes({
    userAgent: nav.userAgent,
    brands: nav.userAgentData?.brands,
    platform: nav.userAgentData?.platform,
    mobile: nav.userAgentData?.mobile,
    width: window.innerWidth,
    deviceMemory: nav.deviceMemory,
    hardwareConcurrency: nav.hardwareConcurrency,
  });
}

/** What `bootTelemetry` calls once the chunk has arrived. */
export function start(config: TelemetryConfig): Telemetry {
  const fetchImpl = globalThis.fetch.bind(globalThis);
  const t = createTelemetry({
    service: "thunderforge-web",
    version: import.meta.env.VITE_APP_VERSION ?? "dev",
    config,
    sink: otlpHttpSink(config.endpoint ?? "", fetchImpl),
    // The feedback report's rules (spec 037), so a bug report and an error
    // record never disagree about what is personal.
    redact: (text) => redact(text).text,
    storage: sessionStore(),
    now: () => Date.now(),
    random: Math.random,
    privacy: privacyOf(navigator as Navigator & Record<string, unknown>),
    resource: device(),
  });
  startCollectors(t, {
    routeTemplate: () => routeTemplate(location.pathname),
  });
  watchRoutes((route) => pageView(t, route));
  return t;
}
