/**
 * Spec 086 US1: the landing's telemetry chunk, imported by `telemetry.ts`
 * only once the served config said yes and the page has loaded.
 */
import {
  createTelemetry,
  deviceAttributes,
  privacyOf,
  sessionStore,
  type Telemetry,
  type TelemetryConfig,
} from "@thunderforge/telemetry";
import { startCollectors } from "@thunderforge/telemetry/browser";
import { otlpHttpSink } from "@thunderforge/telemetry/otlp";
// The web app's redaction, one implementation of spec 037's rules (R12).
import { redact } from "@thunderforge/feedback-redaction";

type NavigatorExtras = {
  userAgentData?: { brands?: { brand: string; version: string }[]; platform?: string; mobile?: boolean };
  deviceMemory?: number;
};

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

/** The sections scroll depth reports, in the order the page shows them. */
export const SECTIONS = [
  "hero",
  "dream",
  "dice",
  "map",
  "numbers",
  "self-host",
  "telemetry",
  "stance",
  "stars",
  "support",
  "footer",
] as const;

/**
 * The campaign tags from the address, and nothing else from it. A tag is cut
 * to 64 characters of letters, digits, `.`, `_` and `-`, so a link cannot
 * smuggle anything personal through one.
 */
export function utmOf(search: string): Record<string, string | undefined> {
  const params = new URLSearchParams(search);
  const clean = (key: string) => {
    const value = params.get(key)?.replace(/[^A-Za-z0-9._-]/g, "").slice(0, 64);
    return value || undefined;
  };
  return {
    "utm.source": clean("utm_source"),
    "utm.medium": clean("utm_medium"),
    "utm.campaign": clean("utm_campaign"),
  };
}

/**
 * `scroll_depth`: the deepest `[data-section]` the visitor saw, sent when the
 * page is hidden or left, and again only if they then went deeper. Registered
 * before the collectors, so it is in the batch their flush sends.
 */
function watchScrollDepth(t: Telemetry): void {
  if (typeof IntersectionObserver === "undefined") return;
  let deepest = -1;
  let sent = -1;
  const observer = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      const name = (entry.target as HTMLElement).dataset.section ?? "";
      const at = (SECTIONS as readonly string[]).indexOf(name);
      if (at > deepest) deepest = at;
    }
  });
  document.querySelectorAll("[data-section]").forEach((el) => observer.observe(el));
  const report = () => {
    if (deepest <= sent) return;
    sent = deepest;
    t.event("scroll_depth", { section: SECTIONS[deepest] });
  };
  addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") report();
  });
  addEventListener("pagehide", report);
}

/** What `bootTelemetry` calls once the chunk has arrived. */
export function start(config: TelemetryConfig): Telemetry {
  const t = createTelemetry({
    service: "thunderforge-landing",
    version: import.meta.env.VITE_APP_VERSION ?? "dev",
    config,
    sink: otlpHttpSink(config.endpoint ?? "", (input, init) => fetch(input, init)),
    redact: (text) => redact(text).text,
    storage: sessionStore(),
    now: () => Date.now(),
    random: Math.random,
    privacy: privacyOf(navigator as Navigator & Record<string, unknown>),
    resource: device(),
    anonymousEnvironment: config.environment === "production" ? "production" : "self-hosted",
  });
  // One page, so one route.
  watchScrollDepth(t);
  startCollectors(t, { routeTemplate: () => "/", firstPageView: utmOf(location.search) });
  t.funnel("landing_viewed");
  return t;
}
