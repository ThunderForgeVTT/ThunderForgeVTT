import type { Pt } from "./journey.ts";

// Spec 098: the hero's walk down a wide page, as a route.
//
// The route runs through stations (the tavern seat, its door, the goblins,
// the chosen map, the dungeon, the campfire), and each leg between two is a
// path round the walls on the table: the engine's `plan_path` when it runs,
// a straight line when it does not. Scroll is the clock: `sAt` turns the
// scroll into a distance along the route that keeps the hero near the
// screen's middle, and `pointAt` turns that distance into a place. Scrolling
// up walks the same route back. Page pixels throughout, y down.

export interface Station {
  at: Pt;
  /** How much scroll, in pixels, the hero stands here for. */
  hold?: number;
}

/** A way from `a` to `b` round the walls, or `null` for none known. */
export type Planner = (a: Pt, b: Pt) => Pt[] | null;

export interface Route {
  /** Every waypoint, stations included. */
  pts: Pt[];
  /** The distance along the route at each waypoint. */
  cum: number[];
  /** Each station: its distance along the route, its place and its hold. */
  marks: Array<{ s: number; at: Pt; hold: number }>;
}

/** How much of a leg's clock is its fall down the page, against its length.
 * All length and the hero lags the screen on a leg that crosses the page;
 * all fall and it darts across one that does not go down. */
const FALL = 0.8;

const dist = (a: Pt, b: Pt) => Math.hypot(b.x - a.x, b.y - a.y);

/** The route through `stations`, each leg by `plan` where it knows one. */
export function buildRoute(stations: Station[], plan: Planner | null): Route {
  const pts: Pt[] = [];
  const cum: number[] = [];
  const marks: Route["marks"] = [];
  const add = (p: Pt) => {
    const last = pts[pts.length - 1];
    if (last && dist(last, p) < 0.01) return;
    cum.push(last ? cum[cum.length - 1]! + dist(last, p) : 0);
    pts.push(p);
  };
  stations.forEach((st, i) => {
    if (i > 0) {
      const from = stations[i - 1]!.at;
      const way = plan?.(from, st.at) ?? null;
      // A planned way ends where it was asked to; the ends are the stations'.
      for (const p of way && way.length >= 2 ? way.slice(1, -1) : []) add(p);
    }
    add(st.at);
    marks.push({ s: cum[cum.length - 1]!, at: st.at, hold: st.hold ?? 0 });
  });
  return { pts, cum, marks };
}

export const length = (r: Route) => r.cum[r.cum.length - 1] ?? 0;

/** The place `s` pixels along the route. */
export function pointAt(r: Route, s: number): Pt {
  const { pts, cum } = r;
  if (pts.length === 0) return { x: 0, y: 0 };
  if (s <= 0 || pts.length === 1) return pts[0]!;
  if (s >= length(r)) return pts[pts.length - 1]!;
  let lo = 0;
  let hi = cum.length - 1;
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (cum[mid]! <= s) lo = mid;
    else hi = mid;
  }
  const a = pts[lo]!;
  const b = pts[hi]!;
  const len = cum[hi]! - cum[lo]!;
  const k = len === 0 ? 0 : (s - cum[lo]!) / len;
  return { x: a.x + (b.x - a.x) * k, y: a.y + (b.y - a.y) * k };
}

/**
 * How far a leg has come at `s`, 0 to 1: mostly by how far down it has got,
 * so the hero keeps pace with the screen, partly by length, so a stretch
 * that only crosses the page still takes some scroll.
 */
function progress(r: Route, k: number, s: number): number {
  const a = r.marks[k]!;
  const b = r.marks[k + 1]!;
  const span = b.s - a.s;
  if (span <= 0) return 1;
  const byLength = (s - a.s) / span;
  const fall = b.at.y - a.at.y;
  if (fall <= 1) return byLength;
  // The lowest the leg has reached by `s`: it never climbs back on the clock.
  let low = a.at.y;
  for (let i = 0; i < r.pts.length && r.cum[i]! <= s; i++) if (r.cum[i]! >= a.s) low = Math.max(low, r.pts[i]!.y);
  low = Math.max(low, pointAt(r, s).y);
  const byFall = Math.min(1, Math.max(0, (low - a.at.y) / fall));
  return FALL * byFall + (1 - FALL) * byLength;
}

/** The scroll bands the stations hold over: each centred where the station
 * is at the screen's middle, none before the last. */
export function bands(r: Route, viewportH: number, maxScroll: number): Array<[number, number]> {
  const out: Array<[number, number]> = [];
  let floor = -Infinity;
  for (const m of r.marks) {
    const anchor = Math.min(maxScroll, Math.max(0, m.at.y - viewportH / 2));
    const start = Math.max(floor, anchor - m.hold / 2);
    const end = Math.max(start, anchor + m.hold / 2);
    out.push([start, end]);
    floor = end;
  }
  return out;
}

/** Where along the route the hero belongs at `scroll`; never back as the
 * scroll goes on, and the same place every time the scroll is the same. */
export function sAt(r: Route, scroll: number, viewportH: number, maxScroll: number): number {
  const { marks } = r;
  if (marks.length === 0) return 0;
  const b = bands(r, viewportH, maxScroll);
  if (scroll <= b[0]![1]) return marks[0]!.s;
  for (let k = 0; k + 1 < marks.length; k++) {
    const [, end] = b[k]!;
    const [next, nextEnd] = b[k + 1]!;
    if (scroll > nextEnd) continue;
    if (scroll >= next) return marks[k + 1]!.s;
    const f = (scroll - end) / Math.max(1e-6, next - end);
    // `progress` only grows along a leg: halve the leg until it says `f`.
    let lo = marks[k]!.s;
    let hi = marks[k + 1]!.s;
    for (let i = 0; i < 32 && hi - lo > 0.05; i++) {
      const mid = (lo + hi) / 2;
      if (progress(r, k, mid) < f) lo = mid;
      else hi = mid;
    }
    return (lo + hi) / 2;
  }
  return marks[marks.length - 1]!.s;
}

/** One frame's step from `s` towards `goal`: brisk far off, gentle close in,
 * never past it. */
export function stepToward(s: number, goal: number): number {
  const d = goal - s;
  const a = Math.abs(d);
  if (a < 0.5) return goal;
  return s + Math.sign(d) * Math.min(a, Math.max(3, a * 0.12));
}

/**
 * A wide page's stations so far: the hero's seat (if it has one), the
 * tavern's back door, straight down from it to the gutter path, and the
 * gutter path on down the page, with the `stops` (the picked map) slotted
 * in where they come on the way down.
 */
export function pageStations(
  seat: Pt | null,
  exit: Pt,
  rest: Pt[],
  cell: number,
  hold: number,
  stops: Station[] = [],
): Station[] {
  const below: Station[] = [
    ...rest.filter((p) => p.y > exit.y + cell).map((at) => ({ at })),
    ...stops.filter((st) => st.at.y > exit.y + cell),
  ].sort((a, b) => a.at.y - b.at.y);
  const out: Station[] = [];
  if (seat) out.push({ at: seat, hold });
  out.push({ at: exit, hold: seat ? 0 : hold });
  const first = below[0];
  if (first && Math.abs(first.at.x - exit.x) > 1) out.push({ at: { x: exit.x, y: first.at.y } });
  out.push(...below);
  return out;
}
