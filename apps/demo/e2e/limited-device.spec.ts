import { expect, test } from "@playwright/test";

/**
 * A device whose GPU stops at a small texture.
 *
 * WebGL2 guarantees a 2048px texture and nothing more; phones, integrated
 * chips and hardened browsers (LibreWolf's minimum-capability mode reports
 * exactly 2048) live there. Found on the dev instance, 2026-10-05: a 2560px
 * window on such a browser panicked the engine in `Surface::configure`, and
 * every one of the demo's maps is wider than 2048 besides.
 *
 * The limit is faked at 2048 — wgpu's own WebGL2 floor, so anything lower
 * gets no device at all — by answering `MAX_TEXTURE_SIZE` differently on the
 * WebGL2 prototype, in a window wider than that. The engine's GL backend reads the limit through the
 * same method, so wgpu believes it too and refuses anything larger: the
 * board's surface and the map's texture both face a real ceiling here, not a
 * number the page merely reports.
 */

const WORLD_ID = "d0000000-0000-4000-0002-000000000001";
const FAKE_MAX_TEXTURE_SIZE = 2048;

test("a device with a small texture limit still draws the board and the map", async ({
  browser,
}) => {
  const context = await browser.newContext({
    viewport: { width: 2560, height: 1440 },
  });
  await context.addInitScript((limit) => {
    const proto = WebGL2RenderingContext.prototype;
    const real = proto.getParameter;
    proto.getParameter = function (this: WebGL2RenderingContext, name: number) {
      if (name === this.MAX_TEXTURE_SIZE) return limit;
      return real.call(this, name);
    };
  }, FAKE_MAX_TEXTURE_SIZE);
  const page = await context.newPage();

  const panics: string[] = [];
  const resampled: string[] = [];
  page.on("console", (message) => {
    const text = message.text();
    if (/panicked|Validation Error|RuntimeError/.test(text)) panics.push(text);
    if (/texture ceiling/.test(text)) resampled.push(text);
  });
  page.on("pageerror", (error) => panics.push(error.message));

  await page.goto(`/demo/world/${WORLD_ID}/play`);
  const canvas = page.locator("canvas");
  await expect(canvas).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("gm-tool-walls")).toBeVisible({
    timeout: 60_000,
  });

  // The surface: the canvas's backing store never exceeds the limit on
  // either side, however wide the window is.
  const measure = () =>
    canvas.evaluate((element) => {
      const c = element as HTMLCanvasElement;
      return { width: c.width, height: c.height };
    });
  // A fresh canvas is 300x150 until the engine's windowing layer sizes it
  // to its container, so wait for a width that could only be the board's.
  await expect
    .poll(async () => (await measure()).width, { timeout: 30_000 })
    .toBeGreaterThan(FAKE_MAX_TEXTURE_SIZE / 2);
  const size = await measure();
  expect(size.width).toBeLessThanOrEqual(FAKE_MAX_TEXTURE_SIZE);
  expect(size.height).toBeLessThanOrEqual(FAKE_MAX_TEXTURE_SIZE);

  // The map: Grassy Path Ambush is 4080x2295 and went through the engine's
  // resample rather than being refused by the GPU.
  await expect
    .poll(() => resampled.length, { timeout: 60_000 })
    .toBeGreaterThan(0);
  expect(resampled[0]).toMatch(/4080x2295 exceeds this device's 2048px/);

  // Give the render world a few frames with the resampled texture before
  // declaring it survived.
  await page.waitForTimeout(1_000);
  expect(panics, "the engine ran without a panic").toEqual([]);

  await context.close();
});
