import assert from "node:assert/strict";
import { test } from "node:test";
import { downAt, passedAt } from "./goblins.ts";
import { buildRoute } from "./route.ts";

const r = buildRoute([{ at: { x: 100, y: 0 } }, { at: { x: 100, y: 1000 } }], null);
const foes = [
  { id: "a", at: { x: 60, y: 300 } },
  { id: "b", at: { x: 140, y: 500 } },
  { id: "c", at: { x: 60, y: 700 } },
];

test("a foe falls when the hero comes level with it", () => {
  assert.equal(passedAt(r, foes[0]!.at), 300);
  assert.deepEqual([...downAt(r, foes, 0)], []);
  assert.deepEqual([...downAt(r, foes, 299)], []);
  assert.deepEqual([...downAt(r, foes, 300)], ["a"]);
  assert.deepEqual([...downAt(r, foes, 600)], ["a", "b"]);
  assert.deepEqual([...downAt(r, foes, 1000)], ["a", "b", "c"]);
});

test("back up the route, they get up again", () => {
  assert.deepEqual([...downAt(r, foes, 450)], ["a"]);
  assert.deepEqual([...downAt(r, foes, 10)], []);
});

test("a foe the route never reaches never falls", () => {
  assert.equal(passedAt(r, { x: 0, y: 5000 }), Infinity);
});
