// Spec 091 FR-021: the room's keys reach the engine only from the room's own
// controls, never from the window. Bevy's winit listens for keys on the
// canvas itself, which the page keeps out of the tab order, so the room
// hands it a synthetic keydown and keyup pair, as the web app's
// `canvasKeyboard.ts` does.

import type { Direction } from "@thunderforge/encounter";

/** The engine's key for each way. ArrowUp raises y, which is north. */
const KEY: Record<Direction, { key: string; code: string }> = {
  north: { key: "ArrowUp", code: "ArrowUp" },
  south: { key: "ArrowDown", code: "ArrowDown" },
  east: { key: "ArrowRight", code: "ArrowRight" },
  west: { key: "ArrowLeft", code: "ArrowLeft" },
};

/** The way an arrow or WASD key goes, or null for any other key. */
export function directionOf(e: KeyboardEvent | { key: string }): Direction | null {
  switch (e.key) {
    case "ArrowUp":
    case "w":
    case "W":
      return "north";
    case "ArrowDown":
    case "s":
    case "S":
      return "south";
    case "ArrowRight":
    case "d":
    case "D":
      return "east";
    case "ArrowLeft":
    case "a":
    case "A":
      return "west";
    default:
      return null;
  }
}

/** Long enough for the engine to see the key down for a frame. */
const HOLD_MS = 60;

/** One step's key on the canvas: down, then up a frame later. */
export function pressOnCanvas(canvas: HTMLCanvasElement, dir: Direction) {
  const { key, code } = KEY[dir];
  const fire = (type: "keydown" | "keyup") =>
    canvas.dispatchEvent(new KeyboardEvent(type, { key, code, bubbles: false, cancelable: true }));
  fire("keydown");
  setTimeout(() => fire("keyup"), HOLD_MS);
}
