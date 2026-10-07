// Wet-erase marker strokes: a line that overshoots its ends a little and
// bows a little in the middle, the way a hand drags a marker along a ruler
// that is not there. Deterministic per seed, so a redraw is the same drawing.

export function rng(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** One stroke from (x1,y1) to (x2,y2) as an SVG path. */
export function stroke(
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  seed = 1,
  { overshoot = 4, bow = 2.5 } = {},
): string {
  const r = rng(seed);
  const len = Math.hypot(x2 - x1, y2 - y1) || 1;
  const ux = (x2 - x1) / len;
  const uy = (y2 - y1) / len;
  const o1 = overshoot * (0.4 + r());
  const o2 = overshoot * (0.4 + r());
  const ax = x1 - ux * o1;
  const ay = y1 - uy * o1;
  const bx = x2 + ux * o2;
  const by = y2 + uy * o2;
  const b = bow * (r() * 2 - 1);
  const mx = (ax + bx) / 2 - uy * b;
  const my = (ay + by) / 2 + ux * b;
  const f = (n: number) => n.toFixed(1);
  return `M${f(ax)} ${f(ay)} Q${f(mx)} ${f(my)} ${f(bx)} ${f(by)}`;
}

export const INKS = {
  black: "var(--ink-black)",
  blue: "var(--ink-blue)",
  red: "var(--ink-red)",
  green: "var(--ink-green)",
} as const;
export type Ink = keyof typeof INKS;
