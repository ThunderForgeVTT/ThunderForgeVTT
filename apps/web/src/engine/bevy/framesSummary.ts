/**
 * The board's frame rate, as one `engine.frames` record (spec 086 FR-018,
 * US7, contracts/browser-events.md).
 *
 * Once the engine has started, `stats.ts`'s mirror is read once a second
 * into a fixed ring of 600 samples. At `pagehide` the ring becomes one
 * record of percentiles and a token-count bucket, with no ids, and is
 * emptied. `engine.frames` is a sampled event, so an unsampled session
 * drops it in the telemetry package.
 */
import type { Attrs, Telemetry } from "@thunderforge/telemetry";
import { onEngineLoad, type EngineLoadSubscribe } from "./loadSignals";
import { readEngineStats, type EngineStats } from "./stats";

export const FRAME_SAMPLES = 600;

export function createFrameRing(size = FRAME_SAMPLES) {
  let items: EngineStats[] = [];
  return {
    push(sample: EngineStats) {
      items.push(sample);
      if (items.length > size) items.shift();
    },
    samples: () => items.slice(),
    clear() {
      items = [];
    },
  };
}

/** The contract's buckets: `0`, `1-10`, `11-50`, `51-200`, `>200`. */
export function tokensBucket(count: number): string {
  if (count <= 0) return "0";
  if (count <= 10) return "1-10";
  if (count <= 50) return "11-50";
  if (count <= 200) return "51-200";
  return ">200";
}

/** Nearest rank: the smallest value with at least `p`% at or below it. */
function percentile(values: number[], p: number): number {
  const sorted = [...values].sort((a, b) => a - b);
  const rank = Math.max(1, Math.ceil((p / 100) * sorted.length));
  return Math.round(sorted[rank - 1] * 10) / 10;
}

export function framesRecord(samples: EngineStats[]): Attrs | null {
  if (samples.length === 0) return null;
  return {
    "fps.p5": percentile(
      samples.map((s) => s.fps),
      5,
    ),
    "fps.p50": percentile(
      samples.map((s) => s.fps),
      50,
    ),
    "frame_ms.p95": percentile(
      samples.map((s) => s.frameTimeMs),
      95,
    ),
    "tokens.bucket": tokensBucket(samples[samples.length - 1].tokens),
  };
}

export interface WatchFramesOptions {
  subscribe?: EngineLoadSubscribe;
  read?: () => Promise<EngineStats | null>;
  /** Run `fn` once a second; returns the stop. */
  every?: (fn: () => void) => () => void;
  /** Run `fn` at `pagehide`; returns the stop. */
  onPageHide?: (fn: () => void) => () => void;
}

const everySecond = (fn: () => void) => {
  const id = setInterval(fn, 1000);
  return () => clearInterval(id);
};

const atPageHide = (fn: () => void) => {
  addEventListener("pagehide", fn);
  return () => removeEventListener("pagehide", fn);
};

/**
 * Sample once the engine has started, and send at `pagehide`. Register
 * before the collectors' own `pagehide` flush, so the record goes with it.
 */
export function watchFrames(
  t: Telemetry,
  o: WatchFramesOptions = {},
): () => void {
  const read = o.read ?? readEngineStats;
  const every = o.every ?? everySecond;
  const ring = createFrameRing();
  let stopTicking: (() => void) | null = null;

  const unsubscribe = (o.subscribe ?? onEngineLoad)((signal) => {
    if (signal.stage !== "started" || stopTicking) return;
    stopTicking = every(() => {
      read()
        .then((sample) => {
          if (sample) ring.push(sample);
        })
        .catch(() => {
          // A readout that fails is a second with no sample.
        });
    });
  });
  const stopHide = (o.onPageHide ?? atPageHide)(() => {
    const record = framesRecord(ring.samples());
    ring.clear();
    if (record) t.event("engine.frames", record);
  });

  return () => {
    unsubscribe();
    stopHide();
    stopTicking?.();
    stopTicking = null;
  };
}
