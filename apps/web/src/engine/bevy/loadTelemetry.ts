/**
 * The engine's load, as telemetry (spec 086 FR-018, US7).
 *
 * Subscribed from the telemetry chunk to the loader's signals, so the engine
 * never calls a telemetry API. A board that cannot load is reported by its
 * closed reason, never `webgl2Unavailable`'s words, which are for the visitor.
 *
 * A load that starts the engine becomes one `engine.load` span with three
 * children: `download` (first request to last byte), `compile` (last byte to
 * the module being ready; streaming compilation overlaps the download, so
 * this is what is left of it) and `start` (to the frame after the engine's
 * first). The times are the signals' own stamps, so a chunk that arrives
 * late still reports the load as it happened.
 */
import type { Telemetry } from "@thunderforge/telemetry";
import { onEngineLoad, type EngineLoadSubscribe } from "./loadSignals";

export function watchEngineLoad(
  t: Telemetry,
  subscribe: EngineLoadSubscribe = onEngineLoad,
): () => void {
  let downloading: number | null = null;
  let downloaded: { at: number; bytes: number } | null = null;
  let starting: number | null = null;
  let sent = false;

  return subscribe((signal) => {
    switch (signal.stage) {
      case "unavailable":
        t.event("engine.load_failed", {
          reason: signal.reason,
          stage: "probe",
        });
        return;
      case "downloading":
        // A retry starts the clock again.
        downloading = signal.at;
        downloaded = null;
        starting = null;
        return;
      case "downloaded":
        downloaded = { at: signal.at, bytes: signal.bytes };
        return;
      case "starting":
        starting = signal.at;
        return;
      case "started": {
        if (sent || downloading === null || starting === null) return;
        sent = true;
        const root = t.begin(
          "engine.load",
          downloading,
          downloaded ? { bytes: downloaded.bytes } : undefined,
        );
        if (downloaded) {
          t.span(
            "download",
            downloading,
            downloaded.at,
            { bytes: downloaded.bytes },
            root.ref,
          );
        }
        t.span(
          "compile",
          downloaded?.at ?? downloading,
          starting,
          undefined,
          root.ref,
        );
        t.span("start", starting, signal.at, undefined, root.ref);
        root.end(signal.at);
        return;
      }
    }
  });
}
