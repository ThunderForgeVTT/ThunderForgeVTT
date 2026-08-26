import { expect, type Locator, type Page } from "@playwright/test";

/**
 * Pixel-level assertions for the Bevy canvas.
 *
 * Why this exists: every other e2e check in this suite verifies state the
 * *server* holds — a wall row was written, a token moved, an import
 * succeeded. None of them look at what the canvas actually draws, and a
 * real regression proved that gap expensive: an imported map's image was
 * fetched and decoded successfully, the `set_scene_background` command
 * reached the engine, every GraphQL assertion passed — and the canvas
 * rendered nothing but its clear colour, because two cameras were fighting
 * over the same render target and each was clearing the other's output.
 * A test that looked at one pixel would have caught it immediately.
 *
 * No PNG-decoding dependency: the screenshot buffer is handed back to the
 * browser, which already has a PNG decoder, and turned into a histogram
 * there via `createImageBitmap` + `OffscreenCanvas`.
 */

/**
 * `ClearColor(Color::srgb(0.133, 0.157, 0.192))` in `src/engine/src/lib.rs`,
 * converted to 8-bit sRGB. This is the "empty canvas" colour — the flat
 * blue-grey you see when the engine is running but drawing nothing.
 */
export const ENGINE_CLEAR_COLOR = { r: 34, g: 40, b: 49 } as const;

export interface CanvasHistogram {
  /** Pixels sampled (the canvas is strided, not read whole). */
  sampled: number;
  /** The single most common colour in the sample. */
  dominant: { r: number; g: number; b: number };
  /** Fraction of sampled pixels sharing the dominant colour, 0..1. */
  dominantFraction: number;
  /** Distinct colours after quantisation — a cheap "how much is going on" signal. */
  distinctColors: number;
}

/** Euclidean-ish closeness in RGB, tolerant of compositing/rounding drift. */
function isNear(
  a: { r: number; g: number; b: number },
  b: { r: number; g: number; b: number },
  tolerance = 6,
): boolean {
  return (
    Math.abs(a.r - b.r) <= tolerance &&
    Math.abs(a.g - b.g) <= tolerance &&
    Math.abs(a.b - b.b) <= tolerance
  );
}

/**
 * Screenshots the middle of the engine canvas and summarises its colours.
 *
 * Two things this deliberately does not do:
 *
 * 1. It does not read the canvas directly. wgpu does not request
 *    `preserveDrawingBuffer`, so `gl.readPixels` / `toDataURL` land after the
 *    compositor has recycled the buffer and come back transparent black no
 *    matter what was drawn — alternating with real content purely on timing.
 *    A Playwright screenshot goes through the compositor and is stable.
 *
 * 2. It does not capture the whole canvas. The canvas is full-viewport and
 *    the Play chrome (tool rail, dock, dice roller) is painted *on top* of
 *    it, so a full-element capture includes that chrome — enough colour to
 *    make a completely blank canvas look like it has content, which defeats
 *    the entire assertion. The clip below is inset well clear of every
 *    docked panel.
 */
export async function canvasHistogram(page: Page): Promise<CanvasHistogram> {
  // The engine's canvas is a direct child of <body> (winit appends it
  // there; see WorldPage.tsx), not inside #game-canvas-container.
  const canvas: Locator = page.locator("canvas").first();
  await expect(canvas).toBeVisible();

  const box = await canvas.boundingBox();
  if (!box) {
    throw new Error("engine canvas has no layout box");
  }

  // Inset by a quarter on each side: the tool rail (3rem) and dock (3rem
  // collapsed, 22rem open) both live at the edges, and this keeps the sample
  // inside the map area even with a section expanded.
  const png = await canvas.screenshot({
    clip: {
      x: box.x + box.width * 0.25,
      y: box.y + box.height * 0.25,
      width: box.width * 0.5,
      height: box.height * 0.5,
    },
  });

  return page.evaluate(async (base64: string) => {
    const response = await fetch(`data:image/png;base64,${base64}`);
    const bitmap = await createImageBitmap(await response.blob());

    const surface = new OffscreenCanvas(bitmap.width, bitmap.height);
    const context = surface.getContext("2d");
    if (!context) {
      throw new Error("OffscreenCanvas 2D context unavailable");
    }
    context.drawImage(bitmap, 0, 0);
    const { data } = context.getImageData(0, 0, bitmap.width, bitmap.height);

    // Stride so a 1600x900 capture costs ~14k samples instead of 1.4M —
    // more than enough resolution to tell "one flat colour" from "a map".
    const stride = 4 * 10;
    // Quantised to 5 bits per channel so JPEG-ish compositing noise and
    // anti-aliasing don't inflate the distinct-colour count.
    const counts = new Map<number, number>();
    let sampled = 0;

    for (let i = 0; i < data.length; i += stride) {
      const key =
        ((data[i] >> 3) << 10) | ((data[i + 1] >> 3) << 5) | (data[i + 2] >> 3);
      counts.set(key, (counts.get(key) ?? 0) + 1);
      sampled += 1;
    }

    let bestKey = 0;
    let bestCount = 0;
    for (const [key, count] of counts) {
      if (count > bestCount) {
        bestCount = count;
        bestKey = key;
      }
    }

    return {
      sampled,
      // Re-expanded to the middle of the quantisation bucket.
      dominant: {
        r: (((bestKey >> 10) & 31) << 3) + 4,
        g: (((bestKey >> 5) & 31) << 3) + 4,
        b: ((bestKey & 31) << 3) + 4,
      },
      dominantFraction: sampled === 0 ? 1 : bestCount / sampled,
      distinctColors: counts.size,
    };
  }, png.toString("base64"));
}

/**
 * Fails when the canvas is (almost) a single flat colour — i.e. the engine
 * is running but drawing nothing.
 *
 * Deliberately not "is it exactly the clear colour": a blank canvas that
 * regressed to black, to white, or to some future clear colour is just as
 * broken, and pinning the assertion to one RGB triple would let those
 * through. The clear colour is still called out by name in the failure
 * message when it matches, because that is the case worth recognising on
 * sight.
 *
 * @param maxDominantFraction Fraction of the canvas one colour may occupy
 *   before it counts as blank. Defaults to 0.98 rather than 1.0 so a map
 *   with large uniform areas (a dark cave, a plain battlemat) still passes
 *   while a genuinely empty canvas does not.
 */
export async function expectCanvasRendersContent(
  page: Page,
  { maxDominantFraction = 0.98 }: { maxDominantFraction?: number } = {},
): Promise<CanvasHistogram> {
  const histogram = await canvasHistogram(page);
  const { dominant, dominantFraction, distinctColors } = histogram;

  const looksLikeClearColor = isNear(dominant, ENGINE_CLEAR_COLOR);
  const percent = (dominantFraction * 100).toFixed(1);
  const rgb = `rgb(${dominant.r}, ${dominant.g}, ${dominant.b})`;

  expect(
    dominantFraction,
    looksLikeClearColor
      ? `Canvas is ${percent}% the engine's clear colour (${rgb}) — the engine is running but rendering nothing. ` +
          `Only ${distinctColors} distinct colours present. Check for camera-order ambiguity ` +
          `(two active cameras on one render target clear each other) and for failed asset loads.`
      : `Canvas is ${percent}% a single flat colour (${rgb}) — nothing appears to be drawn. ` +
          `Only ${distinctColors} distinct colours present.`,
  ).toBeLessThan(maxDominantFraction);

  return histogram;
}

/**
 * Fails when the canvas does not appear to show an imported map.
 *
 * A map is a photograph-like image, so it carries far more distinct
 * colours than the handful a grid, a few token squares and some wall lines
 * produce. `minDistinctColors` separates those two cases: it is well above
 * what an empty-but-working scene yields and well below what any real
 * battlemap does.
 */
export async function expectCanvasRendersMap(
  page: Page,
  { minDistinctColors = 40 }: { minDistinctColors?: number } = {},
): Promise<CanvasHistogram> {
  const histogram = await expectCanvasRendersContent(page);

  expect(
    histogram.distinctColors,
    `Canvas has only ${histogram.distinctColors} distinct colours — too flat to be an imported map. ` +
      `The scene's background sprite is probably missing (asset load failed, or the scene's ` +
      `width/height left the sprite sized to nothing).`,
  ).toBeGreaterThanOrEqual(minDistinctColors);

  return histogram;
}
