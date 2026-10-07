import { expect, test, type Page } from "@playwright/test";
import { activeSceneId, data, enterPlay, openDemo } from "./support";

/**
 * Spec 074: the visitor's own map. The Game Master gives the board a PNG; it
 * is transcoded and kept in the browser, the level's background becomes it,
 * and after a reload the page still serves it at the address the engine
 * reads every background from.
 */

let page: Page;
let outside: string[];

test.describe.configure({ mode: "serial" });

test.beforeAll(async ({ browser, baseURL }) => {
  ({ page, outside } = await openDemo(browser, baseURL));
  await enterPlay(page);
});

test.afterAll(async () => {
  await page.context().close();
});

test.afterEach(() => {
  expect(outside, "nothing leaves the demo's own static files").toEqual([]);
});

interface Level {
  levelId: string;
  isEntry: boolean;
  backgroundAssetId: string | null;
  backgroundUrl: string | null;
  width: number;
  height: number;
}

async function entryLevel(page: Page, sceneId: string): Promise<Level> {
  const { sceneLevels } = await data<{ sceneLevels: Level[] }>(
    page,
    `query ($s: UUID!) {
      sceneLevels(sceneId: $s) {
        levelId isEntry backgroundAssetId backgroundUrl width height
      }
    }`,
    { s: sceneId },
  );
  return sceneLevels.find((l) => l.isEntry) as Level;
}

/** What the page answers at `url`: its type and the picture's size. */
function served(page: Page, url: string) {
  return page.evaluate(async (address) => {
    const response = await fetch(address);
    if (!response.ok) return { status: response.status };
    const bitmap = await createImageBitmap(await response.blob());
    return {
      status: response.status,
      type: response.headers.get("content-type"),
      size: [bitmap.width, bitmap.height],
    };
  }, url);
}

test("a PNG map becomes the board's background, and is there after a reload", async () => {
  const sceneId = await activeSceneId(page);
  const before = await entryLevel(page, sceneId);

  const png = await page.evaluate(async () => {
    const canvas = new OffscreenCanvas(640, 480);
    const context = canvas.getContext(
      "2d",
    ) as OffscreenCanvasRenderingContext2D;
    context.fillStyle = "#2a6f4e";
    context.fillRect(0, 0, 640, 480);
    const blob = await canvas.convertToBlob({ type: "image/png" });
    const bytes = new Uint8Array(await blob.arrayBuffer());
    return Array.from(bytes);
  });

  await page.getByTestId("level-manage-toggle").click();
  await page
    .getByTestId("level-background-input")
    .first()
    .setInputFiles({
      name: "my-map.png",
      mimeType: "image/png",
      buffer: Buffer.from(png),
    });

  await expect
    .poll(async () => (await entryLevel(page, sceneId)).backgroundAssetId)
    .not.toBe(before.backgroundAssetId);
  const after = await entryLevel(page, sceneId);
  expect([after.width, after.height]).toEqual([640, 480]);
  expect(after.backgroundUrl).toBe(
    `/api/canvas-assets/${after.backgroundAssetId}.webp`,
  );
  expect(await served(page, after.backgroundUrl as string)).toEqual({
    status: 200,
    type: "image/webp",
    size: [640, 480],
  });

  await page.reload();
  await enterPlay(page);
  const reloaded = await entryLevel(page, sceneId);
  expect(reloaded.backgroundAssetId).toBe(after.backgroundAssetId);
  expect(await served(page, reloaded.backgroundUrl as string)).toEqual({
    status: 200,
    type: "image/webp",
    size: [640, 480],
  });
});

test("a Universal VTT map brings its art, walls, door and light, as on a server", async () => {
  const sceneId = await activeSceneId(page);
  const answer = await page.evaluate(async (scene) => {
    const canvas = new OffscreenCanvas(1280, 768);
    const context = canvas.getContext(
      "2d",
    ) as OffscreenCanvasRenderingContext2D;
    context.fillStyle = "#30304a";
    context.fillRect(0, 0, 1280, 768);
    const png = await canvas.convertToBlob({ type: "image/png" });
    const bytes = new Uint8Array(await png.arrayBuffer());
    let binary = "";
    for (const byte of bytes) binary += String.fromCharCode(byte);
    const file = {
      format: 0.3,
      resolution: { map_size: { x: 10, y: 6 }, pixels_per_grid: 128 },
      line_of_sight: [
        [
          { x: 0, y: 0 },
          { x: 10, y: 0 },
        ],
      ],
      portals: [
        {
          bounds: [
            { x: 5, y: 3 },
            { x: 6, y: 3 },
          ],
          closed: true,
        },
      ],
      environment: { ambient_light: "ff8a7a60" },
      lights: [
        {
          position: { x: 5, y: 3 },
          range: 2,
          color: "ffffaa00",
          shadows: true,
        },
      ],
      image: btoa(binary),
    };
    const form = new FormData();
    form.append("file", new Blob([JSON.stringify(file)]), "map.dd2vtt");
    const response = await fetch(`/api/scenes/${scene}/import/uvtt`, {
      method: "POST",
      body: form,
    });
    return { status: response.status, body: await response.json() };
  }, sceneId);
  expect(answer).toEqual({
    status: 200,
    body: {
      wallsCreated: 1,
      doorsCreated: 1,
      lightsCreated: 1,
      backgroundImageSet: true,
      skippedDegeneratePolygons: 0,
      warnings: [],
    },
  });

  await enterPlay(page);
  const { scene } = await data<{
    scene: { gridSize: number; width: number; ambientLight: string };
  }>(
    page,
    "query ($s: UUID!) { scene(sceneId: $s) { gridSize width ambientLight } }",
    { s: sceneId },
  );
  expect(scene).toEqual({ gridSize: 128, width: 1280, ambientLight: "dim" });
  const level = await entryLevel(page, sceneId);
  expect(await served(page, level.backgroundUrl as string)).toEqual({
    status: 200,
    type: "image/webp",
    size: [1280, 768],
  });
});
