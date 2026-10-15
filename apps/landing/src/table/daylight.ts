// The engine's daylight curve (spec 099), as the page needs it: to turn the
// dusk it wants at a scroll into the hour that brings it. The curve itself
// is `crates/thunderforge-canvas-core/src/daylight.rs`; this is a copy of
// its six turns and its easing, and `daylight.test.ts` holds them to it.

/** How dark the night is, at midnight. */
export const NIGHT_DARK = 0.98;

/** The darkness at each turn of the day, as `[hour, darkness]`. */
const CURVE: Array<[number, number]> = [
  [0, NIGHT_DARK],
  [5, NIGHT_DARK],
  [7, 0],
  [18, 0],
  [20, 0.6],
  [24, NIGHT_DARK],
];

const ease = (t: number) => 0.6 * t + 0.4 * t * t * (3 - 2 * t);

/** How dark the map is at `hour`, as the engine draws it. */
export function darknessAt(hour: number): number {
  const h = ((hour % 24) + 24) % 24;
  for (let i = 1; i < CURVE.length; i++) {
    const [h0, d0] = CURVE[i - 1]!;
    const [h1, d1] = CURVE[i]!;
    if (h <= h1) return d0 + (d1 - d0) * ease(Math.min(1, Math.max(0, (h - h0) / (h1 - h0))));
  }
  return NIGHT_DARK;
}

/** The evening hour, 18:00 to midnight, that is `darkness` dark. */
export function eveningFor(darkness: number): number {
  let lo = 18;
  let hi = 24;
  if (darkness <= 0) return lo;
  if (darkness >= NIGHT_DARK) return hi;
  for (let i = 0; i < 40; i++) {
    const mid = (lo + hi) / 2;
    if (darknessAt(mid) < darkness) lo = mid;
    else hi = mid;
  }
  return (lo + hi) / 2;
}
