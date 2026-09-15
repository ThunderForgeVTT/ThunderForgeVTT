import type { Page } from "@playwright/test";
import { camera, canvasBox } from "./lightingProbe";

/**
 * Pointing at a board point, wherever the camera is.
 *
 * World y points up and screen y down; the camera's centre is the canvas's
 * centre, and its scale is world units per CSS pixel. Asked of the engine
 * each time, because a scene load or a follow can move the camera between two
 * steps of a test.
 */
export async function boardToScreen(
  page: Page,
  point: { x: number; y: number },
): Promise<{ x: number; y: number }> {
  const box = await canvasBox(page);
  const cam = (await camera(page)) ?? { x: 0, y: 0, scale: 1 };
  const scale = cam.scale > 0 ? cam.scale : 1;
  return {
    x: box.x + box.width / 2 + (point.x - cam.x) / scale,
    y: box.y + box.height / 2 - (point.y - cam.y) / scale,
  };
}

/** A left click on a board point. */
export async function clickBoard(
  page: Page,
  point: { x: number; y: number },
): Promise<void> {
  const at = await boardToScreen(page, point);
  await page.mouse.move(at.x, at.y);
  await page.mouse.down();
  // Held across a frame: a press and release inside one Bevy frame is a
  // click the engine never sees.
  await page.waitForTimeout(120);
  await page.mouse.up();
}

/**
 * A right-click on a board point.
 *
 * The engine reports a right-click on release, and only if the pointer did
 * not travel (a right-drag pans), so the press is held still across a frame.
 */
export async function rightClickBoard(
  page: Page,
  point: { x: number; y: number },
): Promise<void> {
  const at = await boardToScreen(page, point);
  await page.mouse.move(at.x, at.y);
  await page.mouse.down({ button: "right" });
  await page.waitForTimeout(120);
  await page.mouse.up({ button: "right" });
}
