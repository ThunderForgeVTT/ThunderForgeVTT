/**
 * Spec 086 US1: the landing's telemetry, the part loaded with the page.
 *
 * It asks `/telemetry.json` first and imports `telemetryChunk.ts` only when
 * that said yes and the page has loaded. Off, or any failure, imports
 * nothing (FR-017).
 *
 * The calls to action report through one delegated `click` listener on
 * `[data-cta]` anchors, so no section knows about telemetry. A landing click
 * is only the `cta_clicked` event: the funnel's `cta_clicked` step is the
 * demo's **Run your own**, the seventh step, and a **Try the demo** click
 * taking it first would end the funnel before the demo began
 * (contracts/browser-events.md).
 */
import { bootTelemetry, telemetry } from "@thunderforge/telemetry";
import { FRAME_FAILED, type FrameFailed } from "./engine/frameEvents.ts";

export function ctaOf(
  target: EventTarget | null,
): { cta: string; placement: string } | null {
  const el =
    target instanceof Element
      ? target.closest<HTMLElement>("a[data-cta]")
      : null;
  const cta = el?.dataset.cta;
  const placement = el?.dataset.placement;
  return cta && placement ? { cta, placement } : null;
}

export function bootLandingTelemetry(): void {
  document.addEventListener(
    "click",
    (e) => {
      const found = ctaOf(e.target);
      if (!found) return;
      telemetry.event("cta_clicked", found);
      // The link is leaving the page.
      telemetry.flush(true);
    },
    { capture: true },
  );
  // Spec 090 FR-018: the engine frame held back or failed. The closed stage
  // and reason only; the frame's words are for the visitor.
  addEventListener(FRAME_FAILED, (e) => {
    const { stage, reason } = (e as CustomEvent<FrameFailed>).detail;
    telemetry.event("engine.load_failed", { stage, reason });
  });
  void bootTelemetry({
    configUrl: "/telemetry.json",
    load: () => import("./telemetryChunk.ts").then((m) => m.start),
  });
}
