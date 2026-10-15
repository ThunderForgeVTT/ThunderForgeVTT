import type { Mat } from "./heroMat.ts";
import { toWorld, type Doc, type Pt } from "./journey.ts";
import type { Station } from "./route.ts";

// Spec 098 US6: the camp beside "Pull up a chair", where the walk ends. The
// camp lays its map on the page's grid and says where (`publishCamp`); the
// page's table seats every hero the visitor did not pick on the logs, and
// walks its own hero in by the top to the empty seat. Page pixels, y down.

export interface CampHero {
  id: string;
  name: string;
  /** The token's SVG, as a data URL. */
  src: string;
}

export interface Camp {
  src: string;
  /** The map's top-left and size on the page, and the page's cell. */
  x: number;
  y: number;
  w: number;
  h: number;
  cell: number;
  party: CampHero[];
  /** The log seats, top-left corners, in the order the party takes them. */
  seats: Pt[];
  /** The empty seat the walker parks on, top-left. */
  park: Pt;
  /** The fire's centre. */
  fire: Pt;
}

export const CAMP_FIRE = "camp-fire";
export const CAMP_GLOW = ["camp-glow", "camp-haze"] as const;

const CHANGED = "thunderforge:camp";
let camp: Camp | null = null;

export function publishCamp(next: Camp | null) {
  camp = next;
  window.dispatchEvent(new Event(CHANGED));
}

export const currentCamp = () => camp;

export function watchCamp(fn: (c: Camp | null) => void): () => void {
  const tell = () => fn(camp);
  window.addEventListener(CHANGED, tell);
  return () => window.removeEventListener(CHANGED, tell);
}

/** The camp on the table: everyone but `own` on the logs, in seat order. */
export function campMat(c: Camp, own: string | null): Mat {
  const sitting = c.party.filter((p) => p.id !== own).slice(0, c.seats.length);
  return {
    walls: [],
    door: null,
    doorOpen: false,
    pieces: sitting.map((p, i) => ({ ...p, x: c.seats[i]!.x, y: c.seats[i]!.y, side: c.cell, foe: false })),
    map: { src: c.src, x: c.x, y: c.y, w: c.w, h: c.h },
  };
}

/** The fire, lighting the logs: the camp's only light by night. The page
 * is dark past the map's edge, so no pool reaches beyond it. The engine's
 * light falls off in a straight line and takes the brightest pool, so three
 * pools of falling strength curve the edge, where one ends in a ring. */
export function campFire(c: Camp, doc: Doc) {
  const at = toWorld(c.fire, doc);
  const room = Math.min(c.fire.x - c.x, c.x + c.w - c.fire.x, c.fire.y - c.y, c.y + c.h - c.fire.y);
  const reach = Math.max(c.cell, room);
  const light = (id: string, radius: number, brightRadius: number, intensity: number) => ({
    type: "upsert_light",
    light: {
      id,
      x: at.x,
      y: at.y,
      radius,
      brightRadius,
      color: "#ff9a3d",
      intensity,
      flicker: 0.25,
      castsShadows: false,
    },
  });
  return [
    light(CAMP_FIRE, reach * 0.6, c.cell, 1),
    light(CAMP_GLOW[0], reach * 0.85, 0, 0.75),
    light(CAMP_GLOW[1], reach, 0, 0.45),
  ];
}

/** The walk's end: in by the top, over the park's column, onto the log. */
export function campStops(c: Camp | null): Station[][] {
  if (!c) return [];
  const x = c.park.x + c.cell / 2;
  return [[{ at: { x, y: c.y - c.cell / 2 } }, { at: { x, y: c.park.y + c.cell / 2 } }]];
}

/** The page's own path stops at the camp: nothing below it to walk to. */
export const beforeCamp = (path: Pt[], c: Camp | null) => (c ? path.filter((p) => p.y < c.y - c.cell) : path);
