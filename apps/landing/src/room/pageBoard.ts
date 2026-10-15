import type { Place } from "./pageRoom.ts";

// Spec 098 FR-008: the dungeon on a wide page's table.
//
// The room lays its board on the page's grid and says where (`publishBoard`);
// the page's table draws the map there and walks its hero in by the top.
// When the hero stands on the room's first square, the table seats the room
// (`seat`): from then the room's commands go to the page's table, on its
// walker, until the table stands it up again. Page pixels throughout, y down.

type Pt = { x: number; y: number };

export interface Board {
  /** The map's image, and its top-left and size on the page. */
  src: string;
  x: number;
  y: number;
  w: number;
  h: number;
  /** The room's units to a square: the page's cell is one square. */
  grid: number;
  /** The room's first square's centre, and the square above it, outside. */
  start: Pt;
  entry: Pt;
  /** The room's lowest point, its controls under the board. */
  bottom: number;
  /** Whether the visitor has played: the hero is off its first square. */
  played: boolean;
}

const BOARD = "thunderforge:room-board";
const SEAT = "thunderforge:room-seat";

let board: Board | null = null;
let seated: Place | null = null;
/** Where the room left the hero when it was stood up, in page pixels. */
let left: Pt | null = null;

export function publishBoard(next: Board | null) {
  board = next;
  window.dispatchEvent(new Event(BOARD));
}

export const currentBoard = () => board;

export function watchBoard(fn: (b: Board | null) => void): () => void {
  const tell = () => fn(board);
  window.addEventListener(BOARD, tell);
  return () => window.removeEventListener(BOARD, tell);
}

/** Seats the room at `place`, or stands it up (`null`); the room hears it at once. */
export function seat(place: Place | null) {
  if (place === seated || (!place && !seated)) return;
  left = null;
  seated = place;
  window.dispatchEvent(new Event(SEAT));
}

export const seatedAt = () => seated;

export function watchSeat(fn: (place: Place | null) => void): () => void {
  const tell = () => fn(seated);
  window.addEventListener(SEAT, tell);
  tell();
  return () => window.removeEventListener(SEAT, tell);
}

/** The room, standing up, says where its hero was. */
export function leaveAt(at: Pt) {
  left = at;
}

export const leftAt = () => left;
