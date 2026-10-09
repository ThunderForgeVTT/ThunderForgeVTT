/**
 * The DOM collectors (FR-016, FR-020). Loaded inside an app's lazy telemetry
 * chunk, never eagerly, so a session with telemetry off never pays for them.
 */

import { onCLS, onFCP, onINP, onLCP, onTTFB, type Metric } from "web-vitals";
import type { Telemetry } from "./telemetry.ts";

export const FLUSH_INTERVAL_MS = 5000;
export const MAX_LONG_TASKS = 100;

export interface CollectorOptions {
  routeTemplate: () => string;
  /** Extra attributes for the first page view (the landing's UTM tags). */
  firstPageView?: Record<string, string | undefined>;
}

function navEntry(): PerformanceNavigationTiming | undefined {
  try {
    return performance.getEntriesByType("navigation")[0] as
      | PerformanceNavigationTiming
      | undefined;
  } catch {
    return undefined;
  }
}

export function referrerOrigin(): string | undefined {
  try {
    if (!document.referrer) return undefined;
    const o = new URL(document.referrer).origin;
    return o === "null" ? undefined : o;
  } catch {
    return undefined;
  }
}

/** A page view, by route template. Apps call this on each route change. */
export function pageView(
  t: Telemetry,
  route: string,
  extra?: Record<string, string | undefined>,
): void {
  t.event("page_view", { route, ...extra });
}

const ms = (n: number) => Math.max(0, Math.round(n));

function navTiming(t: Telemetry, route: string): void {
  const n = navEntry();
  if (!n) return;
  t.event("nav_timing", {
    "dns.ms": ms(n.domainLookupEnd - n.domainLookupStart),
    "connect.ms": ms(n.connectEnd - n.connectStart),
    "ttfb.ms": ms(n.responseStart - n.requestStart),
    "dom.ms": ms(n.domContentLoadedEventEnd - n.startTime),
    "load.ms": ms(n.loadEventEnd - n.startTime),
    "transfer.bytes": ms(n.transferSize),
  });
  const origin = performance.timeOrigin;
  t.span(
    "page.load",
    origin + n.startTime,
    origin + (n.loadEventEnd || n.duration),
    {
      route,
      "nav.type": n.type,
    },
  );
}

function afterLoad(fn: () => void): void {
  if (document.readyState === "complete") {
    setTimeout(fn, 0);
  } else {
    addEventListener("load", () => setTimeout(fn, 0), { once: true });
  }
}

export function startCollectors(t: Telemetry, opts: CollectorOptions): void {
  const sampled = t.traceparent() !== null;
  const route = opts.routeTemplate();

  addEventListener("error", (e: ErrorEvent) => {
    t.error("onerror", e.error ?? e.message);
  });
  addEventListener("unhandledrejection", (e: PromiseRejectionEvent) => {
    t.error("unhandledrejection", e.reason);
  });

  pageView(t, route, {
    "referrer.origin": referrerOrigin(),
    "nav.type": navEntry()?.type,
    ...opts.firstPageView,
  });

  if (sampled) {
    afterLoad(() => navTiming(t, route));
    const report = (m: Metric) =>
      t.event("web_vital", {
        metric: m.name,
        value: m.value,
        rating: m.rating,
      });
    onLCP(report);
    onINP(report);
    onCLS(report);
    onFCP(report);
    onTTFB(report);
    try {
      let seen = 0;
      new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          if (entry.duration <= 50 || seen >= MAX_LONG_TASKS) continue;
          seen += 1;
          t.event("long_task", { "duration.ms": ms(entry.duration) });
        }
      }).observe({ type: "longtask", buffered: true });
    } catch {
      // No long-task API in this browser.
    }
  }

  setInterval(() => t.flush(false), FLUSH_INTERVAL_MS);
  addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") t.flush(true);
  });
  addEventListener("pagehide", () => t.flush(true));
}
