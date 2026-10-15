import { readMap, type MapFile, type Point, type RoomScript } from "@thunderforge/encounter";

// Spec 091 T031: the room's fixed facts, on the dungeon the owner drew for
// this page (`dungeon-sample.json`, MBRound18, CC BY-SA 4.0): 11 by 8
// squares, open at the top and the bottom, one door between a dark room and
// the hall below it. Squares unless a name says otherwise; the map is
// centred on the origin, so x edges fall on half squares and y edges on
// whole ones. Confirmed on the engine by `e2e/room-script.spec.ts`:
// - north is +y: ArrowUp raises y, and the engine draws +y up, so the hall
//   (y -4 to -1) is below the room and south of the hero;
// - the hall point is unseen from the start, and seen once the door opens;
// - `set_carried_light` takes world units.

export const MAP = "/maps/dungeon-sample.json";
export const BACKGROUND = "/maps/dungeon-sample.webp";

/** Two squares into the hall below the door's middle. */
export const HALL_POINT: Point = { x: 2, y: -2.5 };

export function roomScript(file: MapFile): RoomScript {
  return {
    // The importer closes the map's open edges; this one is open on purpose,
    // top and bottom, so the hero walks in from above.
    map: readMap({ ...file, walls: file.walls.filter((w) => !w.perimeter) }, ["door-hall"]),
    north: 1,
    goalDoor: "door-hall",
    // The door's column, on the open top row.
    start: { x: 2, y: 3.5 },
    doorNames: {
      "door-hall": "the door to the hall",
    },
    // First match wins, so the passage under the door counts as the hall.
    areas: [
      { phrase: "a dark room", bounds: { x: -5.5, y: 0, width: 11, height: 4 } },
      { phrase: "the hall", bounds: { x: -5.5, y: -4, width: 11, height: 4 } },
    ],
    chest: {
      tokenId: "chest",
      // South of the door, two squares east of the passage.
      square: { x: 4, y: -1.5 },
      interactiveId: "chest-region",
      region: { x: 2.5, y: -3, width: 3, height: 3 },
      art: "/room/chest.webp",
      openedArt: "/room/chest-open.webp",
    },
    roll: { formula: "1d20+2", dc: 12 },
    scene: {
      background: BACKGROUND,
      torch: { bright: 4, dim: 8 },
      camera: { x: -5.5, y: -4, width: 11, height: 8 },
    },
  };
}

export interface LoadedRoom {
  script: RoomScript;
  /** The background's weight, which the room states beside its button. */
  imageBytes: number | null;
}

let pending: Promise<LoadedRoom> | null = null;

/** The room's script, its walls read from the map file once per page. */
export function loadRoomScript(): Promise<LoadedRoom> {
  pending ??= fetch(MAP)
    .then((r) => (r.ok ? (r.json() as Promise<MapFile & { imageBytes?: number }>) : Promise.reject(new Error(`${MAP}: ${r.status}`))))
    .then((file) => ({ script: roomScript(file), imageBytes: file.imageBytes ?? null }));
  pending.catch(() => (pending = null));
  return pending;
}
