import { getWasmModule } from "./wasmModule";

/**
 * Where a board point is on the page, in client pixels — for anchoring a menu
 * opened from the keyboard, where there is no pointer to open it at.
 *
 * One projection for one gesture, from the camera the engine already
 * publishes (`camera_state`), not a per-frame stream. `null` before the
 * engine or its canvas exists.
 */
export async function boardPointToClient(
  x: number,
  y: number,
): Promise<{ x: number; y: number } | null> {
  const module = await getWasmModule();
  const cameraState = (module as { camera_state?: () => string }).camera_state;
  const canvas = document.querySelector<HTMLCanvasElement>("canvas");
  if (!cameraState || !canvas) return null;
  const camera = JSON.parse(cameraState()) as {
    x: number;
    y: number;
    scale: number;
  };
  const box = canvas.getBoundingClientRect();
  const scale = camera.scale > 0 ? camera.scale : 1;
  return {
    x: box.left + box.width / 2 + (x - camera.x) / scale,
    y: box.top + box.height / 2 - (y - camera.y) / scale,
  };
}

/** The board point at the middle of the canvas: where the camera looks. */
export async function boardCentre(): Promise<{ x: number; y: number } | null> {
  const module = await getWasmModule();
  const cameraState = (module as { camera_state?: () => string }).camera_state;
  if (!cameraState) return null;
  const camera = JSON.parse(cameraState()) as { x: number; y: number };
  return { x: camera.x, y: camera.y };
}

/**
 * Where the engine's camera is, in world units, or `null` if it cannot say —
 * for turning a click on the page into a point on the map
 * (`./screenToWorld`). Read-only.
 */
export async function getCameraState(): Promise<{
  x: number;
  y: number;
  scale: number;
} | null> {
  try {
    const module = await getWasmModule();
    const read = (module as { camera_state?: () => string }).camera_state;
    if (!read) return null;
    return JSON.parse(read()) as { x: number; y: number; scale: number };
  } catch {
    return null;
  }
}
