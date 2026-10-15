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
  /** None while the first screen has no marker room (a wide page's tavern). */
  door: MatSeg | null;
  doorOpen: boolean;
  pieces: MatPiece[];
  /** A map under it all (a wide page's tavern), its top-left and size. */
  map?: { src: string; x: number; y: number; w: number; h: number };
  /** Where the walker leaves the mat for the page: the tavern's back door. */
  exit?: { x: number; y: number };
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
