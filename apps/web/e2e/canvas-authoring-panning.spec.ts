import { test, expect } from "./fixtures/test";
import {
  uniqueSuffix,
  registerAndCreateWorld,
  createScene,
  waitForEngineReady,
} from "./fixtures/canvasAuthoring";

test.describe("Dragging the map (playtest 2026-09-10 P3)", () => {
  test("a middle-drag and a right-drag both pan; a still right-click does not", async ({
    page,
  }) => {
    test.setTimeout(180_000);

    // The arrow keys stopped panning on 2026-08-26 ("panning is the mouse's
    // job") and the mouse was never given the job; the camera's own tests
    // called `pan()` directly, so they passed with no way to pan at all. This
    // drives a real pointer and asks the engine where the camera went.
    await registerAndCreateWorld(page, `E2E Pan ${uniqueSuffix()}`);
    await createScene(page, "Pan Scene");
    await waitForEngineReady(page);

    const camera = async () =>
      page.evaluate(
        () =>
          (
            window as unknown as {
              __engineProbe?: {
                camera: () => { x: number; y: number; scale: number } | null;
              };
            }
          ).__engineProbe?.camera() ?? null,
      );

    const start = await camera();
    expect(
      start,
      "the engine probe should be available in a dev build",
    ).not.toBeNull();

    const box = await page.locator("canvas").first().boundingBox();
    expect(box).not.toBeNull();
    const cx = box!.x + box!.width / 2;
    const cy = box!.y + box!.height / 2;

    const drag = async (button: "middle" | "right", dx: number, dy: number) => {
      await page.mouse.move(cx, cy);
      await page.mouse.down({ button });
      await page.mouse.move(cx + dx, cy + dy, { steps: 12 });
      await page.mouse.up({ button });
    };

    // Grab-the-map: dragging right moves the camera left, by the drag times
    // the zoom (one screen pixel is `scale` world units).
    await drag("middle", 200, 0);
    await expect
      .poll(async () => (await camera())!.x, { timeout: 10_000 })
      .toBeLessThan(start!.x - 150 * start!.scale);

    // Dragging down moves the camera up, world y growing upward.
    const afterMiddle = (await camera())!;
    await drag("right", 0, 150);
    await expect
      .poll(async () => (await camera())!.y, { timeout: 10_000 })
      .toBeGreaterThan(afterMiddle.y + 100 * afterMiddle.scale);

    // A right-click that does not travel is a click, not a pan.
    const afterRight = (await camera())!;
    await page.mouse.click(cx, cy, { button: "right" });
    await page.waitForTimeout(500);
    const afterClick = (await camera())!;
    expect(
      Math.abs(afterClick.x - afterRight.x) +
        Math.abs(afterClick.y - afterRight.y),
      "a still right-click must not move the camera",
    ).toBeLessThan(1);
  });
});
