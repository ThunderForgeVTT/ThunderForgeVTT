// Prototype: the whole page on the engine's table.
//
// The page's words stay in the DOM. Behind them one engine canvas fills the
// viewport; each section's box is a walled block on the table, and the hero
// walks a path down the page between them as the visitor scrolls. These are
// the sums, kept pure so they can be tested without a browser.

import type { Wall } from "@thunderforge/encounter";

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

/** Each box as four walls on the table, so the hero paths around it. */
export function wallsOf(boxes: Box[], doc: Doc): Wall[] {
  const out: Wall[] = [];
  for (const box of boxes) {
    const tl = toWorld({ x: box.x, y: box.y }, doc);
    const br = toWorld({ x: box.x + box.w, y: box.y + box.h }, doc);
    const corners: Pt[] = [tl, { x: br.x, y: tl.y }, br, { x: tl.x, y: br.y }];
    corners.forEach((a, i) => {
      const b = corners[(i + 1) % 4]!;
      out.push({
        id: `page-${box.id}-${i}`,
        x1: a.x,
        y1: a.y,
        x2: b.x,
        y2: b.y,
        blocksVision: false,
        blocksMovement: true,
        doorState: "none",
      });
    });
  }
  return out;
}

/** How far down the page the visitor is, 0 at the top and 1 at the bottom. */
export function fraction(scrollY: number, doc: Doc, viewportH: number): number {
  const span = doc.h - viewportH;
  if (span <= 0) return 1;
  return Math.min(1, Math.max(0, scrollY / span));
}
