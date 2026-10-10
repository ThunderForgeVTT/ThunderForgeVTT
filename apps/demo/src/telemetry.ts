/**
 * Spec 086 US1: the demo's telemetry, the part loaded with the page.
 *
 * It asks `/demo/telemetry.json` first, with the guard's static fetch, and
 * imports `telemetryChunk.ts` only when that said yes and the page has
 * loaded. Off, or any failure, imports nothing. The calls below go through
 * the package's facade, which drops them until the chunk has arrived, apart
 * from a funnel step, which is kept for it (contracts/browser-events.md).
 */
import { bootTelemetry, telemetry } from "@thunderforge/telemetry";
import { staticFetch } from "./guard/install";
import { countAction } from "./backend/telemetryTap";

export function bootDemoTelemetry(): void {
  void bootTelemetry({
    configUrl: `${import.meta.env.BASE_URL}telemetry.json`,
    fetchImpl: staticFetch,
    load: () => import("./telemetryChunk").then((m) => m.start),
  });
}

/** The view switcher, just before it reloads the page. */
export function viewSwitched(): void {
  countAction("view_switched");
  telemetry.flush(true);
}

/** **Start over**, just before it reloads the page. */
export function startedOver(): void {
  countAction("start_over");
  telemetry.flush(true);
}

/** **Run your own**, just before the landing opens. */
export function runYourOwnClicked(): void {
  const attrs = { cta: "run_your_own", placement: "demo_notice" };
  telemetry.event("cta_clicked", attrs);
  telemetry.funnel("cta_clicked", { cta: "run_your_own" });
  telemetry.flush(true);
}
