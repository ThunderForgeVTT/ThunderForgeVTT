/**
 * The engine's load, as telemetry (spec 086 FR-018).
 *
 * Subscribed from the telemetry chunk to the loader's signals, so the engine
 * never calls a telemetry API. A board that cannot load is reported by its
 * closed reason, never `webgl2Unavailable`'s words, which are for the visitor.
 */
import type { Telemetry } from "@thunderforge/telemetry";
import { onEngineLoad, type EngineLoadSubscribe } from "./loadSignals";

export function watchEngineLoad(
  t: Telemetry,
  subscribe: EngineLoadSubscribe = onEngineLoad,
): () => void {
  return subscribe((signal) => {
    if (signal.stage === "unavailable") {
      t.event("engine.load_failed", { reason: signal.reason, stage: "probe" });
    }
  });
}
