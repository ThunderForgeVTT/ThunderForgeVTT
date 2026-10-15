import type { Send } from "../engine/host.ts";
import type { Board } from "../room/pageBoard.ts";
import type { Place } from "../room/pageRoom.ts";
import { toWorld, type Doc, type Pt } from "./journey.ts";
import type { Station } from "./route.ts";

// Spec 098 FR-008: the dungeon on a wide page's table, the table's side.
// The room says where its board lies (`pageBoard.ts`); this draws the map
// there, routes the walker in by its top and out again, and says when the
// room sits. Page pixels throughout, y down, as the route.

/** The walk into the dungeon and out again: in by the top, to its first
 * square, where the hero waits `hold` of scroll, and back out the top. */
export function boardStops(b: Board | null, hold: number): Station[][] {
  if (!b) return [];
  return [[{ at: b.entry }, { at: b.start, hold }, { at: b.entry }]];
}

/** A map's place on the page: the dungeon's board, or the camp. */
export type Box = { x: number; y: number; w: number; h: number };

export const inside = (b: Box, p: Pt) => p.x > b.x && p.x < b.x + b.w && p.y > b.y && p.y < b.y + b.h;

/** A map's squares, as the planner names cells: the walker goes round. */
export function blockedCells(b: Box | null, cell: number): Array<{ q: number; r: number }> {
  if (!b) return [];
  const out: Array<{ q: number; r: number }> = [];
  const q0 = Math.round(b.x / cell);
  const r0 = Math.round(b.y / cell);
  for (let q = q0; q < q0 + Math.round(b.w / cell); q++) {
    for (let r = r0; r < r0 + Math.round(b.h / cell); r++) out.push({ q, r: -(r + 1) });
  }
  return out;
}

/** Where the room's world sits on the table. */
export function placeOf(b: Board, doc: Doc, cell: number, walker: string): Place {
  return { ...toWorld({ x: b.x + b.w / 2, y: b.y + b.h / 2 }, doc), k: cell / b.grid, walker };
}

/** The room stands up once its controls, under the board, have gone up
 * off the screen. Not yet played, it lets its hero go as the hero is about
 * to leave the screen, at the top or the bottom. */
export function standUp(b: Board, scroll: number, vh: number, cell: number): boolean {
  if (b.bottom < scroll) return true;
  if (b.played) return b.y > scroll + vh;
  const edge = cell * 1.5;
  return b.start.y < scroll + edge || b.start.y > scroll + vh - edge;
}

/** How long the hero waits on the room's first square, in page scroll: the
 * wait is centred on the square at the screen's middle, so it runs from the
 * square near the screen's bottom to the square near its top. */
export const boardHold = (vh: number, cell: number) => Math.round(Math.max(vh * 0.2, vh - cell * 4));

const EDGES = ["n", "e", "s", "w"].map((side) => `room-edge-${side}`);

/** The board's map on the table, by an id that changes when it moves: the
 * engine places an image once and never moves it. Its edges are walls to
 * sight, not to feet: the room's own walls leave its outline open, and a
 * torch in it lit the page around. */
export class BoardArt {
  private id: string | null = null;
  private send: Send;
  constructor(send: Send) {
    this.send = send;
  }

  draw(b: Board | null, doc: Doc, live: boolean) {
    const id = b ? `room-map-${[b.x, b.y, b.w, b.h, doc.w, doc.mid].map(Math.round).join("-")}` : null;
    if (this.id && this.id !== id) this.clear();
    if (!b || !id || this.id || !live) return;
    const c = toWorld({ x: b.x + b.w / 2, y: b.y + b.h / 2 }, doc);
    this.send({ type: "upsert_canvas_image_asset", assetId: id, path: b.src, x: c.x, y: c.y, width: b.w, height: b.h });
    const [nw, se] = [toWorld({ x: b.x, y: b.y }, doc), toWorld({ x: b.x + b.w, y: b.y + b.h }, doc)];
    const corners = [nw, { x: se.x, y: nw.y }, se, { x: nw.x, y: se.y }];
    EDGES.forEach((wallId, i) => {
      const a = corners[i]!;
      const z = corners[(i + 1) % 4]!;
      this.send({
        type: "upsert_wall",
        wall: { id: wallId, x1: a.x, y1: a.y, x2: z.x, y2: z.y, blocksVision: true, blocksMovement: false, doorState: "none" },
      });
    });
    this.id = id;
  }

  clear() {
    if (!this.id) return;
    this.send({ type: "remove_canvas_image_asset", assetId: this.id });
    for (const wallId of EDGES) this.send({ type: "remove_wall", wallId });
    this.id = null;
  }
}
