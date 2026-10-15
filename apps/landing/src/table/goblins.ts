import type { Pt } from "./journey.ts";
import type { Route } from "./route.ts";

// Spec 098 US2: the goblins below the tavern fall as the hero passes them,
// and get up again when the visitor scrolls back above them. "Passes" is
// by the route: a goblin is down once the hero is further along it than the
// point where the route first comes level with the goblin.

/** How far along `r` the hero has passed a foe standing at `at`. */
export function passedAt(r: Route, at: Pt): number {
  const { pts, cum } = r;
  for (let i = 1; i < pts.length; i++) {
    const a = pts[i - 1]!;
    const b = pts[i]!;
    if (b.y < at.y || a.y >= at.y) continue;
    return cum[i - 1]! + ((at.y - a.y) / (b.y - a.y)) * (cum[i]! - cum[i - 1]!);
  }
  return pts.length > 0 && pts[0]!.y >= at.y ? 0 : Infinity;
}

/** Which of `foes` are down with the hero `s` along `r`. */
export function downAt(r: Route, foes: Array<{ id: string; at: Pt }>, s: number): Set<string> {
  return new Set(foes.filter((f) => s >= passedAt(r, f.at)).map((f) => f.id));
}

