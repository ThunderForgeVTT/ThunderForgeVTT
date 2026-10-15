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

export function roomScript(file: MapFile): RoomScript & Required<Pick<RoomScript, "chest">> {
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

/**
 * The tavern the hero passes on the way down (`tavern-sample.json`,
 * MBRound18, CC BY-SA 4.0): 11 by 8 squares on grass, the building 9 by 6
 * in the middle with a door in its north wall and one in its south. The
 * hero comes in from the road above and leaves by the south door, down the
 * page toward the dungeon. The map lists the south door first.
 */
export const TAVERN_MAP = "/maps/tavern-sample.json";
export const TAVERN_BACKGROUND = "/maps/tavern-sample.webp";

/** Inside the tavern's walls. */
const TAVERN_INSIDE = { x: -4.5, y: -3, width: 9, height: 6 };

export function tavernScript(file: MapFile): RoomScript {
  return {
    map: readMap({ ...file, walls: file.walls.filter((w) => !w.perimeter) }, ["door-south", "door-north"]),
    north: 1,
    goalDoor: "door-north",
    // On the road above, two squares west of the door.
    start: { x: -2, y: 3.5 },
    doorNames: {
      "door-north": "the tavern's door",
      "door-south": "the back door",
    },
    areas: [
      { phrase: "the tavern", bounds: TAVERN_INSIDE },
      { phrase: "the yard outside the tavern", bounds: { x: -5.5, y: -4, width: 11, height: 8 } },
    ],
    // Out the back door onto the path below.
    exit: { from: TAVERN_INSIDE, region: { x: -5.5, y: -4, width: 11, height: 1 } },
    roll: { formula: "1d20+2", dc: 12 },
    scene: {
      background: TAVERN_BACKGROUND,
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

/** The page's rooms, in the order the hero walks them. */
export const ROOMS = {
  tavern: { map: TAVERN_MAP, build: tavernScript },
  dungeon: { map: MAP, build: roomScript },
} as const;

export type RoomName = keyof typeof ROOMS;

const pending: Partial<Record<RoomName, Promise<LoadedRoom>>> = {};

/** A room's script, its walls read from the map file once per page. */
export function loadRoomScript(name: RoomName = "dungeon"): Promise<LoadedRoom> {
  const { map, build } = ROOMS[name];
  const p = (pending[name] ??= fetch(map)
    .then((r) => (r.ok ? (r.json() as Promise<MapFile & { imageBytes?: number }>) : Promise.reject(new Error(`${map}: ${r.status}`))))
    .then((file) => ({ script: build(file), imageBytes: file.imageBytes ?? null })));
  p.catch(() => delete pending[name]);
  return p;
}
