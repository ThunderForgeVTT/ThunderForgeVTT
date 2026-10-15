// The first screen's mat, as `Hero.tsx` lays it out, for the page's table.
//
// The hero section keeps its own rules (the drag, the snap, the walls that
// refuse a move) and its buttons, so it works with no engine at all. While
// the page holds the table, `PageTable` draws what this says on the engine
// instead: real walls, a real door, real tokens. Page pixels throughout.

export interface MatSeg {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

export interface MatPiece {
  id: string;
  name: string;
  /** The token's SVG, as a data URL. */
  src: string;
  /** The top-left corner, and the side, in page pixels. */
  x: number;
  y: number;
  side: number;
  foe: boolean;
}

export interface Mat {
  walls: MatSeg[];
  door: MatSeg;
  doorOpen: boolean;
  pieces: MatPiece[];
}

const CHANGED = "thunderforge:hero-mat";
let mat: Mat | null = null;

export function publishMat(next: Mat) {
  mat = next;
  window.dispatchEvent(new Event(CHANGED));
}

export const currentMat = () => mat;

export function watchMat(fn: (mat: Mat) => void): () => void {
  const tell = () => mat && fn(mat);
  window.addEventListener(CHANGED, tell);
  tell();
  return () => window.removeEventListener(CHANGED, tell);
}
