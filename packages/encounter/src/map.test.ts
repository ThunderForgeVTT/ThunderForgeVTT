import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { readMap, toSquareCentre, toSquares, toWorld, type MapFile } from "./map.ts";

// The landing's own copy of the Proving Ground map, read and never written.
const DEMO = new URL(
  "../../../apps/landing/public/maps/demo.json",
  import.meta.url,
);
const file = JSON.parse(readFileSync(DEMO, "utf8")) as MapFile;
const map = readMap(file);

test("the map keeps its size and grid", () => {
  assert.equal(map.name, "demo");
  assert.equal(map.width, 4095);
  assert.equal(map.height, 2340);
  assert.equal(map.gridSize, 117);
});

test("33 walls, each with a stable id", () => {
  assert.equal(map.walls.length, 33);
  const ids = map.walls.map((wall) => wall.id);
  assert.equal(new Set(ids).size, 33, "ids are unique");
  assert.equal(ids[0], "wall-00");
  assert.equal(ids[30], "wall-30");
  assert.deepEqual(ids.slice(31), ["door-hall", "door-west"]);
  // The same file gives the same ids every time.
  assert.deepEqual(
    readMap(file).walls.map((wall) => wall.id),
    ids,
  );
});

test("two doors, both closed", () => {
  const doors = map.walls.filter((wall) => wall.doorState !== "none");
  assert.deepEqual(
    doors.map((door) => [door.id, door.doorState]),
    [
      ["door-hall", "closed"],
      ["door-west", "closed"],
    ],
  );
});

test("door-hall runs x -3.5 to -2.5 squares on y -2", () => {
  const door = map.walls.find((wall) => wall.id === "door-hall")!;
  const a = toSquares({ x: door.x1, y: door.y1 }, map.gridSize);
  const b = toSquares({ x: door.x2, y: door.y2 }, map.gridSize);
  assert.deepEqual([Math.min(a.x, b.x), Math.max(a.x, b.x)], [-3.5, -2.5]);
  assert.deepEqual([a.y, b.y], [-2, -2]);
});

test("a wall keeps what it blocks", () => {
  const wall = map.walls[0]!;
  assert.equal(wall.blocksMovement, true);
  assert.equal(wall.blocksVision, true);
  assert.equal(
    "perimeter" in wall,
    false,
    "the engine's wall has no perimeter field",
  );
});

test("12 lights, each with a stable id and the file's values", () => {
  assert.equal(map.lights.length, 12);
  assert.equal(map.lights[0]!.id, "light-00");
  assert.equal(map.lights[11]!.id, "light-11");
  const lamp = map.lights[1]!;
  assert.deepEqual(lamp, {
    id: "light-01",
    x: -299.8125,
    y: -712.96875,
    radius: 585,
    brightRadius: 292.5,
    intensity: 1,
    color: "ffeccd8b",
    castsShadows: true,
  });
});

test("squares and world units convert both ways", () => {
  assert.deepEqual(toWorld({ x: -3, y: -2 }, 117), { x: -351, y: -234 });
  assert.deepEqual(toSquares({ x: -409.5, y: 234 }, 117), { x: -3.5, y: 2 });
  const there = { x: -2.5625, y: -6.09375 };
  assert.deepEqual(toSquares(toWorld(there, 117), 117), there);
});

test("a moved token's square is read to the half-square", () => {
  assert.deepEqual(toSquareCentre({ x: -351, y: 58.50004959 }, 117), { x: -3, y: 0.5 });
  assert.deepEqual(toSquareCentre({ x: -350.99998, y: -409.5 }, 117), { x: -3, y: -3.5 });
});
