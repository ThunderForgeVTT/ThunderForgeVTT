import { readMap, type MapFile, type Point, type RoomScript } from "@thunderforge/encounter";

// Spec 091 T031: the room's fixed facts, on the Proving Ground (`demo.json`,
// MBRound18, CC BY-SA 4.0). Squares unless a name says otherwise; the map is
// centred on the origin, so x edges fall on half squares and y edges on
// whole ones. Confirmed on the engine by `e2e/room-script.spec.ts`:
// - north is +y: ArrowUp raises y, and the engine draws +y up, so the hall
//   (y -10 to -2) is below the room and south of the hero;
// - the hall point is unseen from the start, and seen once the door opens;
// - the chest is drawn dimly from the hall: the lamp's dim ring, not its
//   bright one;
// - `set_carried_light` takes world units.

export const MAP = "/maps/demo.json";
export const BACKGROUND = "/maps/demo.webp";

/** Two squares into the hall past the door's middle, lit by the lamp. */
export const HALL_POINT: Point = { x: -3, y: -4 };

export function roomScript(file: MapFile): RoomScript {
  return {
    map: readMap(file),
    north: 1,
    goalDoor: "door-hall",
    // The door's column, three and a half squares north of its wall.
    start: { x: -3, y: 1.5 },
    doorNames: {
      "door-hall": "the door to the hall",
      "door-west": "the door to the west room",
    },
    // First match wins, so the middle room comes first.
    areas: [
      { phrase: "a dark room", bounds: { x: -9.5, y: -2, width: 14, height: 12 } },
      { phrase: "the hall", bounds: { x: -17.5, y: -10, width: 35, height: 8 } },
      { phrase: "the west room", bounds: { x: -17.5, y: -2, width: 8, height: 12 } },
    ],
    chest: {
      tokenId: "chest",
      square: { x: -1, y: -3.5 },
      interactiveId: "chest-region",
      region: { x: -2.5, y: -5, width: 3, height: 3 },
      art: "/room/chest.webp",
    },
    roll: { formula: "1d20+2", dc: 12 },
    scene: {
      background: BACKGROUND,
      torch: { bright: 4, dim: 8 },
      camera: { x: -9.5, y: -7, width: 14, height: 14 },
    },
  };
}

let pending: Promise<RoomScript> | null = null;

/** The room's script, its walls read from the map file once per page. */
export function loadRoomScript(): Promise<RoomScript> {
  pending ??= fetch(MAP)
    .then((r) => (r.ok ? (r.json() as Promise<MapFile>) : Promise.reject(new Error(`${MAP}: ${r.status}`))))
    .then(roomScript);
  pending.catch(() => (pending = null));
  return pending;
}
