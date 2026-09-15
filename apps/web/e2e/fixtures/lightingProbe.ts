import { expect, type Page } from "@playwright/test";

/**
 * Reading the lit board back: what the engine's darkness layer drew, and what
 * the engine hid. Shared by `scene-lighting.spec.ts`, where it began, and the
 * spec 045 lighting specs (`carried-light.spec.ts`, `darkvision-range.spec.ts`),
 * which ask the same questions of a game system's lights and sight.
 */

export async function shadowQuads(page: Page): Promise<number> {
  return page.evaluate(async () => {
    const mod = (await import(
      /* @vite-ignore */ "/src/engine/bevy/stats.ts"
    )) as typeof import("../../src/engine/bevy/stats");
    const stats = await mod.readEngineStats();
    return stats?.shadowQuads ?? 0;
  });
}

export async function wallCount(page: Page): Promise<number> {
  return page.evaluate(() => window.__worldProbe?.state()?.counts.walls ?? 0);
}

export type Counts = {
  tokens: number;
  walls: number;
  lights: number;
  shapes: number;
};

export async function storeCounts(page: Page): Promise<Counts | null> {
  return page.evaluate(
    () => (window.__worldProbe?.state()?.counts ?? null) as Counts | null,
  );
}

export type Camera = { x: number; y: number; scale: number };

/** A light a token carries, where the engine is lighting it from (T065). */
export type CarriedLight = {
  tokenId: string;
  x: number;
  y: number;
  bright: number;
  dim: number;
};

export type EngineProbe = {
  camera?: () => Camera | null;
  hiddenTokens?: () => string[];
  dimTokens?: () => string[];
  carriedLights?: () => CarriedLight[];
  tokenFootprints?: () => { tokenId: string }[];
  placedLights?: () => PlacedLight[];
};

/** A light a Game Master placed, with the reaches this engine lights by. */
export type PlacedLight = { lightId: string; bright: number; dim: number };

/** The placed light `lightId` on this canvas, or `null`. */
export async function placedLightOf(
  page: Page,
  lightId: string,
): Promise<PlacedLight | null> {
  return page.evaluate(
    (id) =>
      (window as unknown as { __engineProbe?: EngineProbe }).__engineProbe
        ?.placedLights?.()
        .find((light) => light.lightId === id) ?? null,
    lightId,
  );
}

/**
 * Every token this engine draws, by id — asked of the engine, which is the
 * only place a token with no store row (the sandbox's demo tokens, once
 * spawned in every session) would show up.
 */
export async function engineTokenIds(page: Page): Promise<string[]> {
  return page.evaluate(() =>
    (
      (
        window as unknown as { __engineProbe?: EngineProbe }
      ).__engineProbe?.tokenFootprints?.() ?? []
    )
      .map((token) => token.tokenId)
      .sort(),
  );
}

export async function camera(page: Page): Promise<Camera | null> {
  return page.evaluate(
    () =>
      (
        window as unknown as { __engineProbe?: EngineProbe }
      ).__engineProbe?.camera?.() ?? null,
  );
}

export async function hiddenTokens(page: Page): Promise<string[]> {
  return page.evaluate(
    () =>
      (
        window as unknown as { __engineProbe?: EngineProbe }
      ).__engineProbe?.hiddenTokens?.() ?? [],
  );
}

/** The tokens this canvas draws dimly — shown, but only as a suggestion. */
export async function dimTokens(page: Page): Promise<string[]> {
  return page.evaluate(
    () =>
      (
        window as unknown as { __engineProbe?: EngineProbe }
      ).__engineProbe?.dimTokens?.() ?? [],
  );
}

/** The light `tokenId` carries on this canvas, or `null` if it carries none. */
export async function carriedLightOf(
  page: Page,
  tokenId: string,
): Promise<CarriedLight | null> {
  return page.evaluate(
    (id) =>
      (window as unknown as { __engineProbe?: EngineProbe }).__engineProbe
        ?.carriedLights?.()
        .find((light) => light.tokenId === id) ?? null,
    tokenId,
  );
}

export async function canvasBox(page: Page) {
  const canvas = page.locator("canvas");
  await canvas.scrollIntoViewIfNeeded();
  const box = await canvas.boundingBox();
  if (!box) {
    throw new Error("Bevy canvas element not found");
  }
  return box;
}

/**
 * Zooms out with the mouse wheel until the camera's scale reaches `minScale`,
 * then waits for the camera to stop gliding.
 *
 * At the default 1:1 camera a point outside two 500-unit light pools is off
 * a 720-pixel-tall canvas, or under the Play chrome at its edges. The wheel
 * is the real control, and the probe says where the camera ended up, so
 * nothing below assumes a zoom level.
 */
export async function zoomOutTo(page: Page, minScale: number): Promise<Camera> {
  const box = await canvasBox(page);
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  for (let notch = 0; notch < 30; notch += 1) {
    const cam = await camera(page);
    if (cam && cam.scale >= minScale) break;
    await page.mouse.wheel(0, 120);
    await page.waitForTimeout(150);
  }
  let previous: Camera | null = null;
  await expect
    .poll(
      async () => {
        const cam = await camera(page);
        const settled =
          cam !== null &&
          previous !== null &&
          cam.x === previous.x &&
          cam.y === previous.y &&
          cam.scale === previous.scale;
        previous = cam;
        return settled ? cam.scale : 0;
      },
      {
        timeout: 15_000,
        intervals: [250],
        message: "the camera zooms out with the wheel, and settles",
      },
    )
    .toBeGreaterThanOrEqual(minScale);
  return previous!;
}

/**
 * The mean luminance (Rec. 709, 0..255) of a small square of screen pixels
 * around each world point.
 *
 * Read from a Playwright screenshot, not from the canvas: wgpu does not ask
 * for `preserveDrawingBuffer`, so reading the WebGL canvas back returns
 * transparent black on a timing whim (see `fixtures/canvasPixels.ts`). The
 * PNG is decoded by the page's own decoder, as that fixture does, so there
 * is no decoding dependency. A patch rather than a pixel, so one
 * anti-aliased grid line or wall edge cannot decide the result.
 */
export async function luminanceAt(
  page: Page,
  points: { x: number; y: number }[],
): Promise<number[]> {
  const box = await canvasBox(page);
  const viewport = page.viewportSize()!;
  const clip = {
    x: Math.max(box.x, 0),
    y: Math.max(box.y, 0),
    width: Math.min(box.x + box.width, viewport.width) - Math.max(box.x, 0),
    height: Math.min(box.y + box.height, viewport.height) - Math.max(box.y, 0),
  };
  const cam = (await camera(page))!;

  // Only the middle of the canvas is the map: the tool rail, the dock and the
  // dice roller are painted over its edges. A sample point outside this
  // inset would be measuring the chrome, so it fails here rather than there.
  const inset = 0.2;
  const screen = points.map(({ x, y }) => {
    const sx = box.x + box.width / 2 + (x - cam.x) / cam.scale;
    const sy = box.y + box.height / 2 - (y - cam.y) / cam.scale;
    expect(
      sx >= clip.x + clip.width * inset &&
        sx <= clip.x + clip.width * (1 - inset) &&
        sy >= clip.y + clip.height * inset &&
        sy <= clip.y + clip.height * (1 - inset),
      `world (${x}, ${y}) lands at screen (${sx.toFixed(0)}, ${sy.toFixed(0)}), clear of the Play chrome`,
    ).toBe(true);
    return { x: sx - clip.x, y: sy - clip.y };
  });

  const png = await page.screenshot({ clip });
  return page.evaluate(
    async ({ base64, screen, clipWidth }) => {
      const response = await fetch(`data:image/png;base64,${base64}`);
      const bitmap = await createImageBitmap(await response.blob());
      const surface = new OffscreenCanvas(bitmap.width, bitmap.height);
      const context = surface.getContext("2d")!;
      context.drawImage(bitmap, 0, 0);
      // A screenshot is in device pixels; the probe maps to CSS pixels.
      const ratio = bitmap.width / clipWidth;
      const half = 3;
      return screen.map(({ x, y }) => {
        const cx = Math.round(x * ratio);
        const cy = Math.round(y * ratio);
        const { data } = context.getImageData(
          cx - half,
          cy - half,
          half * 2 + 1,
          half * 2 + 1,
        );
        let sum = 0;
        for (let i = 0; i < data.length; i += 4) {
          sum += 0.2126 * data[i] + 0.7152 * data[i + 1] + 0.0722 * data[i + 2];
        }
        return sum / (data.length / 4);
      });
    },
    { base64: png.toString("base64"), screen, clipWidth: clip.width },
  );
}
