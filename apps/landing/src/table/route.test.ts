import assert from "node:assert/strict";
import { test } from "node:test";
import { buildRoute, length, pointAt, sAt, stepToward, type Planner } from "./route.ts";

// Run with `node --test src/table/route.test.ts` (Node strips the types).

const VH = 900;

test("a route with no planner is the stations joined straight", () => {
  const r = buildRoute([{ at: { x: 0, y: 0 } }, { at: { x: 0, y: 300 } }, { at: { x: 400, y: 600 } }], null);
  assert.equal(length(r), 300 + 500);
  assert.deepEqual(pointAt(r, 150), { x: 0, y: 150 });
  assert.deepEqual(pointAt(r, 550), { x: 200, y: 450 });
  assert.deepEqual(pointAt(r, -5), { x: 0, y: 0 });
  assert.deepEqual(pointAt(r, 1e9), { x: 400, y: 600 });
});

test("a planned leg goes by the planner's waypoints, and keeps the stations as its ends", () => {
  const plan: Planner = (a, b) => [{ x: a.x + 1, y: a.y }, { x: 100, y: 0 }, { x: b.x, y: b.y + 1 }];
  const r = buildRoute([{ at: { x: 0, y: 0 } }, { at: { x: 100, y: 100 } }], plan);
  assert.deepEqual(r.pts, [{ x: 0, y: 0 }, { x: 100, y: 0 }, { x: 100, y: 100 }]);
  assert.equal(r.marks[1]!.s, 200);
});

test("a planner with no way falls back to the straight leg", () => {
  const r = buildRoute([{ at: { x: 0, y: 0 } }, { at: { x: 0, y: 100 } }], () => null);
  assert.equal(r.pts.length, 2);
});

test("the scroll holds the hero at a station, then walks it on", () => {
  const r = buildRoute(
    [{ at: { x: 100, y: 400 }, hold: 200 }, { at: { x: 100, y: 2000 } }, { at: { x: 100, y: 4000 } }],
    null,
  );
  const max = 5000 - VH;
  assert.equal(sAt(r, 0, VH, max), 0);
  assert.equal(sAt(r, 100, VH, max), 0);
  assert.ok(sAt(r, 400, VH, max) > 0);
  // The station is at the screen's middle when the scroll puts it there.
  assert.equal(sAt(r, 2000 - VH / 2, VH, max), r.marks[1]!.s);
});

test("scroll only moves the hero forward, and back the same way", () => {
  const plan: Planner = (a, b) => [a, { x: 900, y: a.y }, { x: 900, y: b.y }, b];
  const r = buildRoute(
    [{ at: { x: 300, y: 400 }, hold: 150 }, { at: { x: 300, y: 1600 }, hold: 100 }, { at: { x: 100, y: 3200 } }],
    plan,
  );
  const max = 4000 - VH;
  let last = -1;
  const seen: number[] = [];
  for (let y = 0; y <= max; y += 25) {
    const s = sAt(r, y, VH, max);
    assert.ok(s >= last - 1e-6, `s fell back at ${y}: ${s} < ${last}`);
    last = s;
    seen.push(s);
  }
  for (let y = max, i = seen.length - 1; y >= 0; y -= 25, i--) assert.equal(sAt(r, y, VH, max), seen[i]);
  assert.equal(last, length(r));
});

test("on a straight fall the hero stays in the middle of the screen", () => {
  const r = buildRoute([{ at: { x: 50, y: 450 }, hold: 100 }, { at: { x: 50, y: 5450 } }], null);
  const max = 6000 - VH;
  for (let y = 0; y <= 5000; y += 100) {
    const at = pointAt(r, sAt(r, y, VH, max)).y - y;
    assert.ok(at > VH * 0.3 && at < VH * 0.7, `at scroll ${y} the hero is at ${at}`);
  }
});

test("the step closes on the goal from either side and never passes it", () => {
  assert.equal(stepToward(0, 0.3), 0.3);
  assert.equal(stepToward(0, 10), 3);
  assert.equal(stepToward(0, 1000), 120);
  assert.equal(stepToward(1000, 0), 880);
  assert.equal(stepToward(9, 10), 10);
});
