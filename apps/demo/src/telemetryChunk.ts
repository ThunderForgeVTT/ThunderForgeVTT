/**
 * Spec 086 US1: the demo's telemetry chunk, imported by `telemetry.ts` only
 * once the served config said yes and the page has loaded.
 */
import {
  createTelemetry,
  privacyOf,
  sessionStore,
  type Telemetry,
  type TelemetryConfig,
} from "@thunderforge/telemetry";
import { pageView, startCollectors } from "@thunderforge/telemetry/browser";
import { otlpHttpSink } from "@thunderforge/telemetry/otlp";
import { getEngineState, onGridSnapChanged } from "@/engine/bevy";
import { watchEngineLoad } from "@/engine/bevy/loadTelemetry";
import { redact } from "@/services/feedbackRedaction";
import { device } from "@/telemetry";
import { routeTemplate, watchRoutes } from "@/telemetry/routes";
import { allowTelemetryTo } from "./guard/telemetryPass";
import { demoState } from "./backend/state";

const BASE = import.meta.env.BASE_URL.replace(/\/$/, "");

/** The route template under the demo's base, as the web app names it. */
function demoRoute(): string {
  const path = location.pathname.startsWith(BASE)
    ? location.pathname.slice(BASE.length)
    : location.pathname;
  return routeTemplate(path || "/");
}

/**
 * `landing` when the visitor came from the landing page on this origin,
 * `direct` otherwise. Only the answer is sent, never the referrer.
 */
export function entryOf(
  referrer: string,
  origin: string,
): "landing" | "direct" {
  try {
    const from = new URL(referrer);
    return from.origin === origin && from.pathname === "/"
      ? "landing"
      : "direct";
  } catch {
    return "direct";
  }
}

function activeSceneHasMap(): boolean {
  const state = demoState();
  const scene = state.scenes.find(
    (s) => s.sceneId === state.world.activeSceneId,
  );
  return Boolean(
    scene?.backgroundUrl ||
    scene?.backgroundAssetId ||
    scene?.backgroundImagePath,
  );
}

/**
 * `map_loaded`: the engine's first frame with the scene's map. The engine
 * reports the snapping switch on its first frame (spec 077), so that report
 * is the frame; an engine that was already running when the chunk arrived
 * has drawn one.
 */
function watchMapLoaded(t: Telemetry): void {
  const reached = () => {
    if (activeSceneHasMap()) t.funnel("map_loaded");
  };
  if (getEngineState().started) reached();
  onGridSnapChanged(reached);
}

/** What `bootTelemetry` calls once the chunk has arrived. */
export function start(config: TelemetryConfig): Telemetry {
  // FR-021: the guard opens the one way out before anything is sent.
  allowTelemetryTo(config);
  const t = createTelemetry({
    service: "thunderforge-demo",
    version: import.meta.env.VITE_APP_VERSION ?? "dev",
    config,
    // Through the guard, which passes only this endpoint's logs and traces.
    sink: otlpHttpSink(config.endpoint ?? "", (input, init) =>
      window.fetch(input, init),
    ),
    redact: (text) => redact(text).text,
    storage: sessionStore(),
    now: () => Date.now(),
    random: Math.random,
    privacy: privacyOf(navigator as Navigator & Record<string, unknown>),
    resource: device(),
    // The landing's nginx says `production`; an instance serving its own
    // demo says what it was given, which is `self-hosted` by default.
    anonymousEnvironment:
      config.environment === "production" ? "production" : "self-hosted",
  });
  startCollectors(t, { routeTemplate: demoRoute });
  watchRoutes(() => pageView(t, demoRoute()));
  t.funnel("demo_opened", {
    entry: entryOf(document.referrer, location.origin),
  });
  watchMapLoaded(t);
  watchEngineLoad(t);
  return t;
}
