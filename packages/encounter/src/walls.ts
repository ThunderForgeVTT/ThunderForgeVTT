/**
 * Spec 091: the wall rule, ported from canvas-core.
 *
 * The engine and the server judge a move with
 * `thunderforge_canvas_core::wall::movement_blocked_by`. The page plays the
 * server here, so it judges with the same rule, line for line.
 *
 * The Rust works in f32. Centred half-square coordinates on a 4095-wide map
 * give cross products past 2^23, where f32 rounds and f64 does not. So every
 * arithmetic step goes through `Math.fround`. One f32 operation done in f64
 * and then rounded gives the f32 answer, because f64 holds more than twice
 * f32's bits.
 */

const f = Math.fround;

export interface Point {
  x: number;
  y: number;
}

/** `wall.rs` `DoorState`, as the engine's `upsert_wall` spells it. */
export type DoorState = "none" | "open" | "closed";

/** A wall, in the shape of the engine's `upsert_wall` payload (`payloads.rs` `WorldWallPayload`). */
export interface Wall {
  id: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  blocksVision: boolean;
  blocksMovement: boolean;
  doorState: DoorState;
}

const start = (wall: Wall): Point => ({ x: f(wall.x1), y: f(wall.y1) });
const end = (wall: Wall): Point => ({ x: f(wall.x2), y: f(wall.y2) });
const f32 = (point: Point): Point => ({ x: f(point.x), y: f(point.y) });

/**
 * Whether the wall stops a token right now. An open door never does.
 * Ports `Wall::currently_blocks_movement` (`wall.rs`).
 */
export function currentlyBlocksMovement(wall: Wall): boolean {
  if (wall.doorState === "open") return false;
  return wall.blocksMovement;
}

/** `segments_intersect`'s `cross`, one f32 step at a time. */
function cross(o: Point, a: Point, b: Point): number {
  return f(f(f(a.x - o.x) * f(b.y - o.y)) - f(f(a.y - o.y) * f(b.x - o.x)));
}

/** `segments_intersect`'s `on_segment`: q inside the box of p and r. */
function onSegment(p: Point, q: Point, r: Point): boolean {
  return (
    q.x <= Math.max(p.x, r.x) &&
    q.x >= Math.min(p.x, r.x) &&
    q.y <= Math.max(p.y, r.y) &&
    q.y >= Math.min(p.y, r.y)
  );
}

/**
 * Whether segment p1-p2 meets segment p3-p4. Touching counts.
 * Ports `segments_intersect` (`wall.rs`).
 */
export function segmentsIntersect(
  a1: Point,
  a2: Point,
  b1: Point,
  b2: Point,
): boolean {
  const [p1, p2, p3, p4] = [f32(a1), f32(a2), f32(b1), f32(b2)];
  const d1 = cross(p3, p4, p1);
  const d2 = cross(p3, p4, p2);
  const d3 = cross(p1, p2, p3);
  const d4 = cross(p1, p2, p4);

  if (
    ((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) &&
    ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0))
  ) {
    return true;
  }
  if (d1 === 0 && onSegment(p3, p1, p4)) return true;
  if (d2 === 0 && onSegment(p3, p2, p4)) return true;
  if (d3 === 0 && onSegment(p1, p3, p2)) return true;
  if (d4 === 0 && onSegment(p1, p4, p2)) return true;
  return false;
}

/**
 * The first wall in the way of a move, or null. A move to the same point is
 * never blocked, even standing on a wall.
 * Ports `movement_blocked_by` (`wall.rs`).
 */
export function movementBlockedBy(
  from: Point,
  to: Point,
  walls: readonly Wall[],
): Wall | null {
  const [a, b] = [f32(from), f32(to)];
  if (a.x === b.x && a.y === b.y) return null;
  for (const wall of walls) {
    if (
      currentlyBlocksMovement(wall) &&
      segmentsIntersect(a, b, start(wall), end(wall))
    ) {
      return wall;
    }
  }
  return null;
}

/**
 * The first wall in the way anywhere along a route, judged leg by leg. Fewer
 * than two points is no route, and is not blocked.
 * Ports `path_blocked_by` (`wall.rs`).
 */
export function pathBlockedBy(
  path: readonly Point[],
  walls: readonly Wall[],
): Wall | null {
  for (let i = 1; i < path.length; i += 1) {
    const blocked = movementBlockedBy(path[i - 1]!, path[i]!, walls);
    if (blocked) return blocked;
  }
  return null;
}

/** f32::EPSILON. */
const EPSILON = 2 ** -23;

const length = (x: number, y: number): number =>
  f(Math.sqrt(f(f(x * x) + f(y * y))));

/**
 * How far a point is from the segment a-b.
 * Ports `distance_to_segment` (`door_icon.rs`).
 */
export function distanceToSegment(point: Point, a: Point, b: Point): number {
  const [pt, s, e] = [f32(point), f32(a), f32(b)];
  const ab = { x: f(e.x - s.x), y: f(e.y - s.y) };
  const lengthSquared = f(f(ab.x * ab.x) + f(ab.y * ab.y));
  if (lengthSquared <= EPSILON) return length(f(pt.x - s.x), f(pt.y - s.y));
  const ap = { x: f(pt.x - s.x), y: f(pt.y - s.y) };
  const dot = f(f(ap.x * ab.x) + f(ap.y * ab.y));
  const t = Math.min(1, Math.max(0, f(dot / lengthSquared)));
  const near = { x: f(s.x + f(ab.x * t)), y: f(s.y + f(ab.y * t)) };
  return length(f(pt.x - near.x), f(pt.y - near.y));
}

/** A token near a door: its centre and the side of its footprint, in world units. */
export interface NearbyToken {
  center: Point;
  side: number;
}

/**
 * Whether a token can reach a door: the gap between its footprint and the
 * door is at most one square. The same reach that shows the door's icon.
 * Ports `icon_shown`'s token test (`door_icon.rs`).
 */
export function doorInReach(
  wall: Wall,
  token: NearbyToken,
  square: number,
): boolean {
  const distance = distanceToSegment(token.center, start(wall), end(wall));
  return f(distance - f(f(token.side) / 2)) <= f(square);
}
