import assert from "node:assert/strict";
import { test } from "node:test";
import { beforeCamp, campMat, campStops, type Camp } from "./campMat.ts";

// Run with `node --test src/table/campMat.test.ts` (Node strips the types).

const at = (c: number, r: number) => ({ x: 768 + c * 48, y: 6000 + r * 48 });
const camp: Camp = {
  src: "/maps/camp-sample.webp",
  x: 768,
  y: 6000,
  w: 384,
  h: 384,
  cell: 48,
  party: ["sir-pip", "mira-starweave", "fenna-swiftbow", "brother-oak"].map((id) => ({ id, name: id, src: "" })),
  seats: [at(1, 1), at(6, 3), at(6, 5), at(5, 1)],
  park: at(3, 1),
  fire: at(4, 4),
};

test("everyone the visitor did not pick sits on the logs, in seat order", () => {
  const m = campMat(camp, "mira-starweave");
  assert.deepEqual(
    m.pieces.map((p) => [p.id, p.x, p.y]),
    [
      ["sir-pip", 816, 6048],
      ["fenna-swiftbow", 1056, 6144],
      ["brother-oak", 1056, 6240],
    ],
  );
  assert.equal(m.walls.length, 0);
});

test("a hero of the visitor's own leaves the whole party on the logs", () => {
  assert.equal(campMat(camp, null).pieces.length, 4);
});

test("the walk comes in by the top over the empty seat and sits on it", () => {
  assert.deepEqual(campStops(camp)[0]!.map((s) => s.at), [
    { x: 936, y: 5976 },
    { x: 936, y: 6072 },
  ]);
});

test("the page's path ends above the camp", () => {
  const path = [
    { x: 24, y: 5000 },
    { x: 24, y: 5960 },
    { x: 24, y: 7000 },
  ];
  assert.deepEqual(beforeCamp(path, camp), path.slice(0, 1));
  assert.deepEqual(beforeCamp(path, null), path);
});
