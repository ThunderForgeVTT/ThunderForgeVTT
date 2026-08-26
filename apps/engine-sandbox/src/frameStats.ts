/**
 * Frame-timing measurement.
 *
 * Reports the 95th percentile alongside the mean, because for interactive
 * rendering the mean is close to useless: a scene that holds 60fps but drops a
 * 200ms frame whenever a light moves *feels* broken while averaging fine. The
 * tail is the user experience.
 *
 * Timing comes from `requestAnimationFrame` deltas — the interval the browser
 * actually presented at, which is what a person perceives. It is not the same
 * as the engine's internal frame time: a browser throttles rAF in a background
 * tab and caps it at the display refresh, so these numbers are a ceiling on
 * smoothness, not a measure of GPU headroom.
 */

export interface FrameStats {
  frames: number;
  /** Seconds the sample covered. */
  duration: number;
  fps: number;
  meanMs: number;
  /** 95th-percentile frame time — the one that matters for feel. */
  p95Ms: number;
  worstMs: number;
  /** Frames that took longer than 1/30s, i.e. visible hitches. */
  hitches: number;
}

const HITCH_THRESHOLD_MS = 1000 / 30;

export class FrameSampler {
  private deltas: number[] = [];
  private last = 0;
  private running = false;
  private handle = 0;

  start(): void {
    this.deltas = [];
    this.last = performance.now();
    this.running = true;

    const tick = () => {
      if (!this.running) return;
      const now = performance.now();
      this.deltas.push(now - this.last);
      this.last = now;
      this.handle = requestAnimationFrame(tick);
    };
    this.handle = requestAnimationFrame(tick);
  }

  stop(): FrameStats {
    this.running = false;
    cancelAnimationFrame(this.handle);

    // The first delta spans whatever happened before sampling began — often a
    // scenario's whole setup — so it is dropped rather than counted as a
    // rendering cost.
    const deltas = this.deltas.slice(1);
    if (deltas.length === 0) {
      return { frames: 0, duration: 0, fps: 0, meanMs: 0, p95Ms: 0, worstMs: 0, hitches: 0 };
    }

    const total = deltas.reduce((sum, d) => sum + d, 0);
    const sorted = [...deltas].sort((a, b) => a - b);
    const p95Index = Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95));

    return {
      frames: deltas.length,
      duration: total / 1000,
      fps: deltas.length / (total / 1000),
      meanMs: total / deltas.length,
      p95Ms: sorted[p95Index],
      worstMs: sorted[sorted.length - 1],
      hitches: deltas.filter((d) => d > HITCH_THRESHOLD_MS).length,
    };
  }
}
