/**
 * Rooms for the tests. Not exported from the package.
 *
 * - `smallRoom`: a room five squares wide with a hall to the north, on a
 *   grid of 100. Its north door is `door-hall`, its west door `door-west`.
 * - `provingGround`: the landing's real map, with the facts the plan
 *   proposes (T031 confirms them on the engine).
 */

import { readFileSync } from "node:fs";
import { readMap, type MapFile, type MapFileWall } from "./map.ts";
import type { RoomScript } from "./script.ts";

const wall = (
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  door = false,
): MapFileWall => ({
  x1: x1 * 100,
  y1: y1 * 100,
  x2: x2 * 100,
  y2: y2 * 100,
  blocksMovement: true,
  blocksVision: true,
  doorState: door ? "closed" : "none",
  perimeter: false,
});

const SMALL: MapFile = {
  name: "small",
  width: 1000,
  height: 1000,
  gridSize: 100,
  walls: [
    // The room: x -2.5 to 2.5, y -2 to 2.
    wall(-2.5, -2, -0.5, -2),
    wall(-0.5, -2, 0.5, -2, true),
    wall(0.5, -2, 2.5, -2),
    wall(-2.5, 2, 2.5, 2),
    wall(2.5, -2, 2.5, 2),
    wall(-2.5, -2, -2.5, 0),
    wall(-2.5, 0, -2.5, 1, true),
    wall(-2.5, 1, -2.5, 2),
    // The hall: y -6 to -2.
    wall(-2.5, -6, -2.5, -2),
    wall(2.5, -6, 2.5, -2),
    wall(-2.5, -6, 2.5, -6),
  ],
  lights: [
    {
      x: 0,
      y: -400,
      radius: 500,
      brightRadius: 250,
      color: "ffeccd8b",
      intensity: 1,
      castsShadows: true,
    },
  ],
};

const shared = {
  north: -1 as const,
  goalDoor: "door-hall",
  roll: { formula: "1d20+2", dc: 12 },
};

export function smallRoom(): RoomScript {
  return {
    ...shared,
    map: readMap(SMALL),
    start: { x: 0, y: 1.5 },
    doorNames: {
      "door-hall": "the door to the hall",
      "door-west": "the west door",
    },
    areas: [
      {
        phrase: "a dark room",
        bounds: { x: -2.5, y: -2, width: 5, height: 4 },
      },
      { phrase: "the hall", bounds: { x: -2.5, y: -6, width: 5, height: 4 } },
    ],
    chest: {
      tokenId: "chest",
      square: { x: 1, y: -4.5 },
      interactiveId: "chest-region",
      region: { x: -0.5, y: -6, width: 3, height: 3 },
      art: "/room/chest.webp",
      openedArt: "/room/chest-open.webp",
    },
    scene: {
      background: "/maps/small.webp",
      torch: { bright: 2, dim: 4 },
      camera: { x: -2.5, y: -6, width: 5, height: 8 },
    },
  };
}

const DEMO = new URL(
  "../../../apps/landing/public/maps/demo.json",
  import.meta.url,
);

export function provingGround(): RoomScript {
  const file = JSON.parse(readFileSync(DEMO, "utf8")) as MapFile;
  return {
    ...shared,
    // On the engine ArrowUp is +y, and the hall is drawn below the room
    // (spec 091 T031, e2e/room-script.spec.ts): the hall is south.
    north: 1,
    map: readMap(file),
    start: { x: -3, y: 1.5 },
    doorNames: {
      "door-hall": "the door to the hall",
      "door-west": "the door to the west room",
    },
    areas: [
      {
        phrase: "a dark room",
        bounds: { x: -9.5, y: -2, width: 14, height: 12 },
      },
      {
        phrase: "the hall",
        bounds: { x: -17.5, y: -10, width: 35, height: 8 },
      },
      {
        phrase: "the west room",
        bounds: { x: -17.5, y: -2, width: 8, height: 12 },
      },
    ],
    chest: {
      tokenId: "chest",
      square: { x: -1, y: -3.5 },
      interactiveId: "chest-region",
      region: { x: -2.5, y: -5, width: 3, height: 3 },
      art: "/room/chest.webp",
      openedArt: "/room/chest-open.webp",
    },
    scene: {
      background: "/maps/demo.webp",
      torch: { bright: 4, dim: 8 },
      camera: { x: -9.5, y: -7, width: 14, height: 14 },
    },
  };
}
