/**
 * Where a point on the screen lands on the map — playtest 2026-09-10 P10.
 *
 * The Text tool used to place text at container-relative *screen* pixels and
 * send them as world coordinates. The engine's world is centred on the camera
 * and grows upward, so text landed away from the click and mirrored
 * vertically. This is the conversion the engine itself makes
 * (`viewport_to_world_2d` on an orthographic camera): the canvas centre is the
 * camera's position, one screen pixel is `scale` world units, and screen y
 * grows downward while world y grows upward.
 */

export interface CameraState {
  x: number;
  y: number;
  scale: number;
}

export interface CanvasBox {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** The world point under `client` (viewport pixels, as a `MouseEvent` gives
 * them), for a canvas drawn at `canvas` by a camera at `camera`. */
export function screenToWorld(
  client: { x: number; y: number },
  canvas: CanvasBox,
  camera: CameraState,
): { x: number; y: number } {
  const offsetX = client.x - (canvas.left + canvas.width / 2);
  const offsetY = client.y - (canvas.top + canvas.height / 2);
  return {
    x: camera.x + offsetX * camera.scale,
    y: camera.y - offsetY * camera.scale,
  };
}
