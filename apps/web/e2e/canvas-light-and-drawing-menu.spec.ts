import { expect, test, type Page } from "./fixtures/test";
import { rightClickBoard } from "./fixtures/boardPointer";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { camera, storeCounts } from "./fixtures/lightingProbe";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 073 — a light and a drawing are worked from a right-click on them.
 *
 * Two browsers at one table. The Game Master puts a light out and lights it
 * again, lets it through walls, shows a drawing to the table, hides it and
 * removes both; a player right-clicking the same two spots is offered nothing.
 *
 * Every step is a right-click on the board and a press on the menu, and what
 * is asserted is what the server holds afterwards — a menu that changed only
 * what this browser drew would pass a look at the screen.
 */

async function gql<T>(
  page: Page,
  query: string,
  variables: Record<string, unknown>,
): Promise<T> {
  const res = await graphql<{ data?: T; errors?: { message: string }[] }>(
    page,
    query,
    variables,
  );
  if (res.errors?.length || !res.data) {
    throw new Error(`GraphQL failed: ${JSON.stringify(res.errors ?? res)}`);
  }
  return res.data;
}

type Light = { intensity: number; castsShadows: boolean };

/** The light as the server holds it. */
async function lightOf(
  page: Page,
  sceneId: string,
  lightId: string,
): Promise<Light | null> {
  const data = await gql<{ lightSources: (Light & { lightId: string })[] }>(
    page,
    `query ($sceneId: UUID!) {
      lightSources(sceneId: $sceneId) { lightId intensity castsShadows }
    }`,
    { sceneId },
  );
  const light = data.lightSources.find((l) => l.lightId === lightId);
  return light
    ? { intensity: light.intensity, castsShadows: light.castsShadows }
    : null;
}

/** Whether the server shows the drawing to players, or `null` once it is gone. */
async function drawingShown(
  page: Page,
  sceneId: string,
  shapeId: string,
): Promise<boolean | null> {
  const data = await gql<{
    shapes: { shapeId: string; visibleToPlayers: boolean }[];
  }>(
    page,
    `query ($sceneId: UUID!) {
      shapes(sceneId: $sceneId) { shapeId visibleToPlayers }
    }`,
    { sceneId },
  );
  return (
    data.shapes.find((s) => s.shapeId === shapeId)?.visibleToPlayers ?? null
  );
}

/** Whether this browser's board holds the drawing. */
async function boardHasDrawing(page: Page, shapeId: string): Promise<boolean> {
  return page.evaluate(async (id) => {
    const bevy = (await import(
      /* @vite-ignore */ "/src/engine/bevy/index.ts"
    )) as typeof import("../src/engine/bevy/index");
    return bevy.getBoundWorldStore()?.getState().shapes[id] !== undefined;
  }, shapeId);
}

/** Right-click the board and press one item of the menu that opens. */
async function choose(
  page: Page,
  at: { x: number; y: number },
  item: string,
  label: string,
): Promise<void> {
  await rightClickBoard(page, at);
  const entry = page.getByTestId(`canvas-menu-${item}`);
  await expect(entry).toHaveText(label, { timeout: 10_000 });
  await entry.click();
  await expect(page.getByTestId("canvas-menu")).toBeHidden();
}

test("a light and a drawing are changed and removed from a right-click", async ({
  page,
  browser,
}) => {
  test.setTimeout(6 * 60_000);

  const worldId = await registerAndCreateWorld(
    page,
    `Light menu ${uniqueSuffix()}`,
  );
  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    page,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(page, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;

  await page.goto(`/world/${worldId}/play`);
  await waitForEngineReady(page);

  // Either side of where the Game Master is looking, so both are on screen
  // and neither is under the other.
  const looking = (await camera(page))!;
  const lightAt = { x: looking.x - 150, y: looking.y };
  const drawing = { x: looking.x + 100, y: looking.y - 40, w: 80, h: 80 };
  const drawingAt = { x: drawing.x + 40, y: drawing.y + 40 };

  const lit = await gql<{ createLightSource: { lightId: string } }>(
    page,
    `mutation ($input: GraphQLCreateLightSourceInput!) {
      createLightSource(input: $input) { lightId }
    }`,
    {
      input: {
        sceneId,
        ...lightAt,
        radius: 200,
        intensity: 1,
        castsShadows: true,
      },
    },
  );
  const lightId = lit.createLightSource.lightId;
  const drawn = await gql<{ createShape: { shapeId: string } }>(
    page,
    `mutation ($input: GraphQLCreateShapeInput!) {
      createShape(input: $input) { shapeId }
    }`,
    {
      input: {
        sceneId,
        kind: "RECT",
        geometry: drawing,
        visibleToPlayers: false,
      },
    },
  );
  const shapeId = drawn.createShape.shapeId;
  await expect
    .poll(() => storeCounts(page), { timeout: 15_000 })
    .toMatchObject({ lights: 1, shapes: 1 });

  const light = () => lightOf(page, sceneId, lightId);
  const shown = () => drawingShown(page, sceneId, shapeId);

  const playerPage = await inviteAndJoinAsPlayer(browser, page, worldId);
  await playerPage.goto(`/world/${worldId}/play`);
  await waitForEngineReady(playerPage);

  await test.step("a right-click puts the light out and lights it again", async () => {
    await rightClickBoard(page, lightAt);
    await expect(page.getByTestId("canvas-menu")).toContainText("Light");
    await expect(page.getByTestId("canvas-menu-light-power")).toHaveText(
      "Put it out",
    );
    await page.getByTestId("canvas-menu-light-power").click();
    await expect
      .poll(light, { timeout: 15_000 })
      .toMatchObject({ intensity: 0 });

    await choose(page, lightAt, "light-power", "Light it");
    await expect
      .poll(light, { timeout: 15_000 })
      .toMatchObject({ intensity: 1 });
  });

  await test.step("a right-click lets the light through walls", async () => {
    await choose(page, lightAt, "light-shadows", "Shine through walls");
    await expect
      .poll(light, { timeout: 15_000 })
      .toMatchObject({ castsShadows: false });
  });

  await test.step("a right-click shows the drawing to the table", async () => {
    expect(await boardHasDrawing(playerPage, shapeId)).toBe(false);

    await rightClickBoard(page, drawingAt);
    await expect(page.getByTestId("canvas-menu")).toContainText("Rectangle");
    await expect(page.getByTestId("canvas-menu-shape-visibility")).toHaveText(
      "Show to players",
    );
    await page.getByTestId("canvas-menu-shape-visibility").click();
    await expect.poll(shown, { timeout: 15_000 }).toBe(true);
    await expect
      .poll(() => boardHasDrawing(playerPage, shapeId), { timeout: 15_000 })
      .toBe(true);
  });

  await test.step("a player is offered nothing on either", async () => {
    for (const at of [lightAt, drawingAt]) {
      await rightClickBoard(playerPage, at);
      await playerPage.waitForTimeout(1_500);
      await expect(playerPage.getByTestId("canvas-menu")).toBeHidden();
    }
  });

  await test.step("a right-click hides the drawing again", async () => {
    await choose(page, drawingAt, "shape-visibility", "Hide from players");
    await expect.poll(shown, { timeout: 15_000 }).toBe(false);
    await expect
      .poll(() => boardHasDrawing(playerPage, shapeId), { timeout: 15_000 })
      .toBe(false);
  });

  await test.step("a right-click removes each, and the board is board again", async () => {
    await choose(page, drawingAt, "shape-remove", "Remove from the board");
    await expect.poll(shown, { timeout: 15_000 }).toBeNull();
    await choose(page, lightAt, "light-remove", "Remove this light");
    await expect.poll(light, { timeout: 15_000 }).toBeNull();
    await expect
      .poll(() => storeCounts(page), { timeout: 15_000 })
      .toMatchObject({ lights: 0, shapes: 0 });

    await rightClickBoard(page, lightAt);
    await expect(page.getByTestId("canvas-menu-add-light")).toBeVisible({
      timeout: 10_000,
    });
    await page.keyboard.press("Escape");
  });
});
