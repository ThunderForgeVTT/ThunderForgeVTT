// Prototype: the whole page on the engine's table.
//
// The page's words stay in the DOM. Behind them one engine canvas fills the
// viewport, and the hero walks a path down the page between the sections'
// boxes as the visitor scrolls. These are the sums, kept pure so they can be
// tested without a browser.

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
}

/**
 * Page pixels to the engine's world: one world unit is one pixel at scale 1,
 * the scene is centred on the origin, and +y is up.
 */
export const toWorld = (p: Pt, doc: Doc): Pt => ({ x: p.x - doc.w / 2, y: doc.h / 2 - p.y });

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
 * The dusk over the page at `scrollY`, 0 (daylight) to `NIGHT`. Day holds
 * until `dayEnds`, the evening falls over the screen after it, and the
 * night deepens slowly until `nightAt`, the room. Dawn breaks over the screen
 * before `dawnAt` and it is day again by it.
 */
export function duskAt(
  scrollY: number,
  dayEnds: number,
  nightAt: number,
  dawnAt: number,
  viewportH: number,
): number {
  if (scrollY <= dayEnds || scrollY >= dawnAt) return 0;
  const fall = Math.min(1, (scrollY - dayEnds) / viewportH);
  const span = Math.max(1, nightAt - dayEnds - viewportH);
  const deep = Math.min(1, (scrollY - dayEnds - viewportH) / span);
  const dark = fall < 1 ? EVENING * smooth(fall) : EVENING + (NIGHT - EVENING) * deep;
  return dark * (1 - smooth(dawning(scrollY, dawnAt, viewportH)));
}

/** How far dawn has broken at `scrollY`, 0 (night) to 1 (day, at `dawnAt`). */
export function dawning(scrollY: number, dawnAt: number, viewportH: number): number {
  return Math.min(1, Math.max(0, 1 - (dawnAt - scrollY) / viewportH));
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
