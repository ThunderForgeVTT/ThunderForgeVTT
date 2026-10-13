import assert from "node:assert/strict";
import { test } from "node:test";
import {
  distanceToSegment,
  doorInReach,
  movementBlockedBy,
  pathBlockedBy,
  segmentsIntersect,
  type Point,
  type Wall,
} from "./walls.ts";

// Ported from crates/thunderforge-canvas-core/src/wall_tests.rs and
// wall_room_and_clear_tests.rs. The Rust names are kept in the test names.

const p = (x: number, y: number): Point => ({ x, y });

/** wall_tests.rs `barrier`: a wall across the board at y = 0 that stops a token. */
function barrier(id: string, over: Partial<Wall> = {}): Wall {
  return {
    id,
    x1: -50,
    y1: 0,
    x2: 50,
    y2: 0,
    blocksVision: true,
    blocksMovement: true,
    doorState: "none",
    ...over,
  };
}

test("segments_intersect_crossing", () => {
  assert.equal(segmentsIntersect(p(0, 0), p(10, 10), p(0, 10), p(10, 0)), true);
});

test("segments_intersect_parallel_non_touching", () => {
  assert.equal(segmentsIntersect(p(0, 0), p(10, 0), p(0, 5), p(10, 5)), false);
});

test("a_move_across_a_blocking_wall_is_stopped_by_it, and named", () => {
  const blocked = movementBlockedBy(p(0, -10), p(0, 10), [barrier("w-1")]);
  assert.equal(blocked?.id, "w-1");
});

test("a_move_alongside_a_wall_is_not_stopped", () => {
  assert.equal(
    movementBlockedBy(p(-20, 10), p(20, 10), [barrier("w-1")]),
    null,
  );
});

test("an open door never blocks (a_move_through_an_open_door_is_not_stopped)", () => {
  const door = barrier("door-1", { doorState: "open" });
  assert.equal(movementBlockedBy(p(0, -10), p(0, 10), [door]), null);
});

test("a closed door blocks like any wall (a_move_through_a_closed_door_is_stopped_like_any_wall)", () => {
  const door = barrier("door-1", { doorState: "closed" });
  assert.equal(movementBlockedBy(p(0, -10), p(0, 10), [door])?.id, "door-1");
});

test("a_wall_that_does_not_block_movement_does_not_stop_one", () => {
  const window = barrier("window-1", {
    blocksMovement: false,
    blocksVision: true,
  });
  assert.equal(movementBlockedBy(p(0, -10), p(0, 10), [window]), null);
});

test("touching counts: slipping_through_the_point_where_two_walls_meet_is_stopped", () => {
  const north = barrier("w-north", { x1: 0, y1: 0, x2: 0, y2: 50 });
  const east = barrier("w-east", { x1: 0, y1: 0, x2: 50, y2: 0 });
  assert.notEqual(
    movementBlockedBy(p(-10, -10), p(10, 10), [north, east]),
    null,
  );
});

test("touching counts: a move that ends exactly on a wall is stopped", () => {
  assert.equal(
    movementBlockedBy(p(0, -10), p(0, 0), [barrier("w-1")])?.id,
    "w-1",
  );
});

test("touching counts: a move that ends on a wall's end is stopped", () => {
  assert.equal(
    movementBlockedBy(p(60, -10), p(50, 0), [barrier("w-1")])?.id,
    "w-1",
  );
});

test("a_move_that_goes_nowhere_is_never_stopped, even standing on a wall", () => {
  const standingOnIt = p(0, 0);
  assert.equal(
    movementBlockedBy(standingOnIt, standingOnIt, [barrier("w-1")]),
    null,
  );
  // And leaving the wall is judged from there, not refused for standing on it.
  assert.equal(
    movementBlockedBy(p(0, 0), p(0, 0), [
      barrier("w-1", { doorState: "closed" }),
    ]),
    null,
  );
});

test("a_route_is_judged_leg_by_leg_not_end_to_end", () => {
  const walls = [barrier("w-1")];
  const thereAndBack = [p(-10, -10), p(-10, 10), p(10, -10)];
  assert.equal(pathBlockedBy(thereAndBack, walls)?.id, "w-1");
  assert.equal(
    movementBlockedBy(thereAndBack[0]!, thereAndBack[2]!, walls),
    null,
    "the endpoints alone read as a legal move, which is why the path is judged",
  );
});

test("a_route_that_walks_around_a_wall_is_allowed", () => {
  const walls = [barrier("w-1", { x1: -10, x2: 10 })];
  const around = [p(0, -10), p(20, -10), p(20, 10), p(0, 10)];
  assert.equal(pathBlockedBy(around, walls), null);
  assert.notEqual(movementBlockedBy(around[0]!, around[3]!, walls), null);
});

test("a_path_with_nothing_to_judge_is_not_blocked", () => {
  const walls = [barrier("w-1")];
  assert.equal(pathBlockedBy([], walls), null);
  assert.equal(pathBlockedBy([p(0, -10)], walls), null);
});

test("the first blocking wall in the list is the one named", () => {
  const walls = [barrier("w-1"), barrier("w-2")];
  assert.equal(movementBlockedBy(p(0, -10), p(0, 10), walls)?.id, "w-1");
});

// wall_room_and_clear_tests.rs `room_walls_built_from_the_segments_enclose_their_interior`,
// asked of movement: a room's four walls stop a step out on every side.
test("room walls enclose their interior", () => {
  const corners = [p(-50, -50), p(50, -50), p(50, 50), p(-50, 50)];
  const walls = corners.map((a, i) => {
    const b = corners[(i + 1) % 4]!;
    return barrier(`r${i}`, { x1: a.x, y1: a.y, x2: b.x, y2: b.y });
  });
  for (const outside of [p(0, 200), p(0, -200), p(200, 0), p(-200, 0)]) {
    assert.notEqual(
      movementBlockedBy(p(0, 0), outside, walls),
      null,
      `leaked toward ${outside.x},${outside.y}`,
    );
  }
});

// FR-012. Half-square coordinates on a 4095-wide map give cross products near
// 2^23, where f32 keeps half units and f64 keeps all of them. The exact cross
// product of this move's end against the wall is -0.25. f32 rounds it to 0,
// and the end then touches the wall; f64 keeps it, and the end misses.
test("f32 and f64 disagree here, and the port gives f32's answer", () => {
  const wall = barrier("long", {
    x1: -2047.5,
    y1: -1170,
    x2: 2047.5,
    y2: 1169.5,
  });
  const from = p(1462, 0);
  const to = p(1462, 835);
  const exact =
    (wall.x2 - wall.x1) * (to.y - wall.y1) -
    (wall.y2 - wall.y1) * (to.x - wall.x1);
  assert.equal(
    exact,
    -0.25,
    "in f64 the end is a quarter unit off the wall's line",
  );
  assert.equal(movementBlockedBy(from, to, [wall])?.id, "long");
  assert.equal(
    segmentsIntersect(from, to, p(wall.x1, wall.y1), p(wall.x2, wall.y2)),
    true,
  );
});

// door_icon_tests.rs `distance_to_a_segment`.
test("distance_to_a_segment", () => {
  const a = p(0, 0);
  const b = p(10, 0);
  assert.equal(distanceToSegment(p(5, 3), a, b), 3);
  assert.equal(distanceToSegment(p(13, 4), a, b), 5);
  assert.equal(distanceToSegment(p(3, 4), a, a), 5);
});

// door_icon.rs `icon_shown`'s token test: the gap between a token's footprint
// and the door is at most one square. A door 200 long on the x axis, as in
// door_icon_tests.rs, on a grid of 100.
const DOOR = barrier("door", { x1: -100, x2: 100, doorState: "closed" });
const SQUARE = 100;

test("doorInReach: a footprint one square from the door is in reach", () => {
  assert.equal(
    doorInReach(DOOR, { center: p(0, 150), side: SQUARE }, SQUARE),
    true,
  );
});

test("doorInReach: a footprint one and a half squares off is not", () => {
  assert.equal(
    doorInReach(DOOR, { center: p(0, 200), side: SQUARE }, SQUARE),
    false,
  );
  assert.equal(
    doorInReach(DOOR, { center: p(0, 251), side: SQUARE }, SQUARE),
    false,
  );
});

test("doorInReach: beyond the door's end it is measured to the end", () => {
  assert.equal(
    doorInReach(DOOR, { center: p(400, 0), side: SQUARE }, SQUARE),
    false,
  );
  assert.equal(
    doorInReach(DOOR, { center: p(250, 0), side: SQUARE }, SQUARE),
    true,
  );
});
