import { test, expect, type Page } from "./fixtures/test";
import { boardToScreen, clickBoard } from "./fixtures/boardPointer";
import {
  graphql,
  launchSceneByName,
  openGmTool,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { createScene } from "./fixtures/world-cache";

/**
 * Spec 077 FR-023: snapping on the full stack.
 *
 * With snapping on, a Box drag lays one wall per cell edge along its
 * perimeter, so a 3×2 box is ten walls each one cell long (FR-012, FR-017);
 * the Door primitive turns one of them into a closed door with a click
 * (FR-016); a light placed near the box's corner lands on it (FR-018); `S`
 * turns snapping off and a diagonal drag arrives as one free wall (FR-008).
 *
 * Every assertion reads the server, not the engine: the walls and lights
 * counted here are the ones the mutation bridge sent and the server kept.
 */

const GRID = 64;

type Wall = {
  wallId: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  doorState: string;
};

async function sceneWalls(page: Page, sceneId: string): Promise<Wall[]> {
  const res = await graphql<{ data?: { walls?: Wall[] } }>(
    page,
    `
      query ($sceneId: UUID!) {
        walls(sceneId: $sceneId) {
          wallId
          x1
          y1
          x2
          y2
          doorState
        }
      }
    `,
    { sceneId },
  );
  return res.data?.walls ?? [];
}

async function wallsReach(
  page: Page,
  sceneId: string,
  count: number,
): Promise<Wall[]> {
  await expect
    .poll(async () => (await sceneWalls(page, sceneId)).length, {
      message: `the drawn walls should reach the server (expected ${count})`,
      timeout: 30_000,
    })
    .toBe(count);
  return sceneWalls(page, sceneId);
}

async function sceneLights(
  page: Page,
  sceneId: string,
): Promise<{ x: number; y: number }[]> {
  const res = await graphql<{
    data?: { lightSources?: { x: number; y: number }[] };
  }>(
    page,
    `
      query ($sceneId: UUID!) {
        lightSources(sceneId: $sceneId) {
          x
          y
        }
      }
    `,
    { sceneId },
  );
  return res.data?.lightSources ?? [];
}

/** A press-move-release between two board points. */
async function dragBoard(
  page: Page,
  from: { x: number; y: number },
  to: { x: number; y: number },
): Promise<void> {
  const a = await boardToScreen(page, from);
  const b = await boardToScreen(page, to);
  await page.mouse.move(a.x, a.y);
  await page.mouse.down();
  await page.waitForTimeout(80);
  await page.mouse.move(b.x, b.y, { steps: 12 });
  await page.waitForTimeout(80);
  await page.mouse.up();
}

const length = (w: Wall) => Math.hypot(w.x2 - w.x1, w.y2 - w.y1);
const axisAligned = (w: Wall) =>
  Math.abs(w.x1 - w.x2) < 1e-3 || Math.abs(w.y1 - w.y2) < 1e-3;

test.describe("Snapping (spec 077)", () => {
  test("a box is ten cell-edge walls, a click makes a door, a light lands on the corner, and S frees a diagonal", async ({
    page,
  }) => {
    test.setTimeout(240_000);

    await registerAndCreateWorld(page, `E2E Snapping ${uniqueSuffix()}`);
    const worldId = /\/world\/([^/]+)/.exec(new URL(page.url()).pathname)![1];
    const sceneId = await createScene(page, worldId, "Snapped Room");
    await graphql(
      page,
      `
        mutation ($sceneId: UUID!, $input: GraphQLUpdateSceneInput!) {
          updateScene(sceneId: $sceneId, input: $input) {
            sceneId
          }
        }
      `,
      { sceneId, input: { gridSize: GRID, width: 1280, height: 1280 } },
    );
    await launchSceneByName(page, worldId, "Snapped Room");
    await waitForEngineReady(page);
    await openGmTool(page, "walls");

    const snap = page.getByTestId("gm-snap-toggle");
    await expect(snap).toHaveAttribute("data-enabled", "true");

    // --- A 3×2 box, a little off the corners: snapping puts it on them -----
    // Corners at (64,64) and (256,192), aimed at from 9px inside each.
    await page.getByTestId("wall-primitive-room").click();
    await dragBoard(page, { x: 73, y: 73 }, { x: 247, y: 183 });
    const box = await wallsReach(page, sceneId, 10);
    for (const wall of box) {
      expect(length(wall), "each wall is one cell edge").toBeCloseTo(GRID, 1);
      expect(axisAligned(wall), "each wall lies on a grid line").toBe(true);
      expect(wall.doorState).toBe("NONE");
    }
    const xs = box.flatMap((w) => [w.x1, w.x2]);
    const ys = box.flatMap((w) => [w.y1, w.y2]);
    expect(Math.min(...xs)).toBeCloseTo(64, 1);
    expect(Math.max(...xs)).toBeCloseTo(256, 1);
    expect(Math.min(...ys)).toBeCloseTo(64, 1);
    expect(Math.max(...ys)).toBeCloseTo(192, 1);

    // --- The Door primitive turns one edge into a door, creating nothing ---
    const south = box.find(
      (w) =>
        Math.abs(w.y1 - 64) < 1e-3 &&
        Math.abs(w.y2 - 64) < 1e-3 &&
        Math.min(w.x1, w.x2) < 65 &&
        Math.max(w.x1, w.x2) > 127,
    )!;
    expect(south, "the first south edge is one of the ten").toBeTruthy();
    await page.getByTestId("wall-primitive-door").click();
    await clickBoard(page, { x: 96, y: 64 });
    await expect
      .poll(
        async () =>
          (await sceneWalls(page, sceneId)).find(
            (w) => w.wallId === south.wallId,
          )?.doorState,
        { message: "the clicked edge becomes a closed door", timeout: 30_000 },
      )
      .toBe("CLOSED");
    expect((await sceneWalls(page, sceneId)).length, "no wall was added").toBe(
      10,
    );

    // --- A light near the box's corner lands on the corner ----------------
    await openGmTool(page, "lights");
    await clickBoard(page, { x: 252, y: 188 });
    await expect
      .poll(async () => (await sceneLights(page, sceneId)).length, {
        message: "the light reaches the server",
        timeout: 30_000,
      })
      .toBe(1);
    const [light] = await sceneLights(page, sceneId);
    expect(Math.hypot(light.x - 256, light.y - 192)).toBeLessThan(1);

    // --- S turns snapping off; a diagonal arrives as one free wall --------
    // The camera looks at the origin at scale 1: a 1280-wide canvas shows
    // x in [-640, 640] and y in [-360, 360], so the free wall goes left.
    await openGmTool(page, "walls");
    await page.getByTestId("wall-primitive-segment").click();
    // The engine hears keys through the canvas: a click gives it focus, and
    // Escape discards the one-point chain that click began.
    await clickBoard(page, { x: -500, y: -300 });
    await page.keyboard.press("Escape");
    await page.keyboard.press("s");
    await expect(snap).toHaveAttribute("data-enabled", "false");
    await dragBoard(page, { x: -400, y: -200 }, { x: -270, y: -110 });
    const all = await wallsReach(page, sceneId, 11);
    const free = all.filter((w) => !box.some((b) => b.wallId === w.wallId));
    expect(free).toHaveLength(1);
    expect(axisAligned(free[0]), "a free wall keeps its slope").toBe(false);
    // Within a couple of world units: the pointer lands on whole pixels.
    expect(Math.hypot(free[0].x1 + 400, free[0].y1 + 200)).toBeLessThan(3);
    expect(Math.hypot(free[0].x2 + 270, free[0].y2 + 110)).toBeLessThan(3);

    // The button and the key agree, and the button flips it back.
    await snap.click();
    await expect(snap).toHaveAttribute("data-enabled", "true");
  });
});
