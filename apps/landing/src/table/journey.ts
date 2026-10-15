// Prototype: the whole page on the engine's table.
//
// The page's words stay in the DOM. Behind them one engine canvas fills the
// viewport, and the hero walks a path down the page between the sections'
// boxes as the visitor scrolls. These are the sums, kept pure so they can be
// tested without a browser.

import { eveningFor } from "./daylight.ts";

/** A box on the page, in page pixels (y down from the document's top). */
export interface Box {
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Pt {
  x: number;
  y: number;
}

/** The document's size, in CSS pixels. */
export interface Doc {
  w: number;
  h: number;
  /** The page's y at the world's origin: its middle when first measured at
   * this width. It stays while the page grows or shrinks, so a line of new
   * words moves nothing on the table. */
  mid: number;
}

/**
 * Page pixels to the engine's world: one world unit is one pixel at scale 1,
 * the origin is the page's `mid`, and +y is up.
 */
export const toWorld = (p: Pt, doc: Doc): Pt => ({ x: p.x - doc.w / 2, y: doc.mid - p.y });

/** The engine's world to page pixels. */
export const fromWorld = (p: Pt, doc: Doc): Pt => ({ x: p.x + doc.w / 2, y: doc.mid - p.y });

/** The table's height: the engine centres its mat on the origin, so the mat
 * reaches as far above the origin as below it, and covers the whole page. */
export const tableHeight = (doc: Doc) => 2 * Math.max(doc.mid, doc.h - doc.mid);

/** The hero's path down the page: along one gutter, crossing in the gaps. */
export function pathDown(boxes: Box[], doc: Doc, cell: number): Pt[] {
  if (boxes.length === 0) return [{ x: cell, y: cell }, { x: cell, y: Math.max(cell, doc.h - cell) }];
  const side = (i: number) => {
    const box = boxes[i]!;
    const gutter = Math.max(cell * 0.75, box.x / 2);
    return i % 2 === 0 ? gutter : doc.w - gutter;
  };
  const mid = (i: number) => boxes[i]!.y + boxes[i]!.h / 2;
  const out: Pt[] = [{ x: side(0), y: cell }];
  for (let i = 0; i < boxes.length; i++) {
    out.push({ x: side(i), y: mid(i) });
    if (i + 1 < boxes.length) {
      const bottom = boxes[i]!.y + boxes[i]!.h;
      const top = boxes[i + 1]!.y;
      const gap = (bottom + top) / 2;
      out.push({ x: side(i), y: gap }, { x: side(i + 1), y: gap });
    }
  }
  out.push({ x: side(boxes.length - 1), y: Math.max(cell, doc.h - cell) });
  return out;
}

/** The point `t` (0..1) of the way along `path`, by length. */
export function along(path: Pt[], t: number): Pt {
  if (path.length === 0) return { x: 0, y: 0 };
  if (path.length === 1) return path[0]!;
  const lengths = path.slice(1).map((p, i) => Math.hypot(p.x - path[i]!.x, p.y - path[i]!.y));
  const total = lengths.reduce((a, b) => a + b, 0);
  let left = Math.min(1, Math.max(0, t)) * total;
  for (let i = 0; i < lengths.length; i++) {
    const len = lengths[i]!;
    if (left <= len || i === lengths.length - 1) {
      const k = len === 0 ? 0 : Math.min(1, left / len);
      const a = path[i]!;
      const b = path[i + 1]!;
      return { x: a.x + (b.x - a.x) * k, y: a.y + (b.y - a.y) * k };
    }
    left -= len;
  }
  return path[path.length - 1]!;
}

/** How far down the page the visitor is, 0 at the top and 1 at the bottom. */
export function fraction(scrollY: number, doc: Doc, viewportH: number): number {
  const span = doc.h - viewportH;
  if (span <= 0) return 1;
  return Math.min(1, Math.max(0, scrollY / span));
}

/**
 * How dark it has got by the first screen's end, and by the room. The engine
 * blends its darkness in linear light, so what looks like evening on screen
 * takes far more than half of it.
 */
export const EVENING = 0.95;
export const NIGHT = 0.985;

/**
 * The most the dark may change per pixel scrolled (spec 098 FR-012): 0.07
 * per 100 px. The evening used to fall over one screen's height, so a tall
 * screen still crossed it in a few flicks and a short one in fewer.
 */
export const MAX_DUSK_PER_PX = 0.0007;
/** The evening's fall in page pixels: a smoothstep's steepest slope is 1.5. */
const FALL_PX = (1.5 * EVENING) / MAX_DUSK_PER_PX;

/**
 * The dusk over the page at `scrollY`, 0 (daylight) to `NIGHT`. Day holds
 * until `dayEnds`, the evening falls over the next `FALL_PX` of the page (a
 * screen, if a screen is taller), and the night deepens slowly until
 * `nightAt`, the room, and holds to the page's end.
 */
export function duskAt(scrollY: number, dayEnds: number, nightAt: number, viewportH: number): number {
  if (scrollY <= dayEnds) return 0;
  const falls = Math.max(FALL_PX, viewportH);
  const fall = Math.min(1, (scrollY - dayEnds) / falls);
  const span = Math.max(1, nightAt - dayEnds - falls);
  const deep = Math.min(1, (scrollY - dayEnds - falls) / span);
  return fall < 1 ? EVENING * smooth(fall) : EVENING + (NIGHT - EVENING) * deep;
}

/**
 * Spec 099 US4: the hour at `scrollY`, for the engine's clock. The
 * afternoon passes over the first screen; from `dayEnds` on, the hour is
 * the one whose dark is `duskAt`'s, so the page darkens as 098 measured it.
 */
export function hourAt(scrollY: number, dayEnds: number, nightAt: number, viewportH: number): number {
  if (scrollY <= dayEnds) return 12 + (6 * Math.max(0, scrollY)) / Math.max(1, dayEnds);
  return eveningFor(duskAt(scrollY, dayEnds, nightAt, viewportH));
}

const smooth = (t: number) => t * t * (3 - 2 * t);

/**
 * A pair of torches in the gap above every section after the first, one at
 * each edge of the page: their light falls on the margins and the gaps, not
 * on the words.
 */
export function torchesAlong(boxes: Box[], doc: Doc, cell: number): Pt[] {
  return boxes.slice(1).flatMap((box, i) => {
    const above = boxes[i]!;
    const y = (above.y + above.h + box.y) / 2;
    const edge = Math.max(cell * 0.25, Math.min(box.x, above.x) / 2);
    return [
      { x: edge, y },
      { x: doc.w - edge, y },
    ];
  });
}
