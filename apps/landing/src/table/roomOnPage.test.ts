import assert from "node:assert/strict";
import { test } from "node:test";
import { blockedCells, boardHold, boardStops, standUp } from "./roomOnPage.ts";

// Run with `node --test src/table/roomOnPage.test.ts` (Node strips the types).

const board = {
  src: "/maps/dungeon-sample.webp",
  x: 192,
  y: 3936,
  w: 528,
  h: 384,
  grid: 128,
  start: { x: 552, y: 3960 },
  entry: { x: 552, y: 3912 },
  bottom: 4800,
  played: false,
};
const VH = 900;

test("the hero goes in by the top, waits on the first square, and comes out by the top", () => {
  const [visit] = boardStops(board, 180);
  assert.deepEqual(
    visit!.map((s) => [s.at.y, s.hold ?? 0]),
    [
      [3912, 0],
      [3960, 180],
      [3912, 0],
    ],
  );
});

test("the board's squares are the planner's cells, so the walk goes round it", () => {
  const cells = blockedCells(board, 48);
  assert.equal(cells.length, 11 * 8);
  assert.deepEqual(cells[0], { q: 4, r: -83 });
});

test("played, the room sits while its controls are on screen, and stands when they go up", () => {
  const played = { ...board, played: true };
  // Scrolled to the controls: the board's top is off the screen, but the room is not.
  assert.equal(standUp(played, 4142, VH, 48), false);
  assert.equal(standUp(played, 4801, VH, 48), true);
  // Back up the page, it holds until its board is off the bottom.
  assert.equal(standUp(played, 3960 - VH + 50, VH, 48), false);
  assert.equal(standUp(played, 3936 - VH - 1, VH, 48), true);
});

test("unplayed, the room lets its hero go as the hero nears either edge", () => {
  assert.equal(standUp(board, 3960 - VH / 2, VH, 48), false);
  assert.equal(standUp(board, 3960 - 100, VH, 48), false);
  assert.equal(standUp(board, 3960 - 50, VH, 48), true);
  assert.equal(standUp(board, 3960 - VH + 100, VH, 48), false);
  assert.equal(standUp(board, 3960 - VH + 50, VH, 48), true);
});

test("the hero waits on the first square from near the screen's bottom to near its top", () => {
  assert.equal(boardHold(900, 48), 708);
  assert.equal(boardHold(200, 48), 40);
});
