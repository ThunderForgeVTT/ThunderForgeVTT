import { expect, test, type Page } from "./fixtures/test";
import { boardToScreen } from "./fixtures/boardPointer";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { camera } from "./fixtures/lightingProbe";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 082 — players draw on the board, and touch only what they drew.
 *
 * A Game Master and two players at one table, with no grant rows: a player
 * holds Select and Shapes by default. Player A draws with the rail like any
 * Game Master would, everybody's board shows it, and the server says A drew
 * it. A's writes to the Game Master's drawing and to player B's are refused
 * and change nothing; A's move of their own drawing lands.
 *
 * What is asserted is what the server holds — a board that only drew the
 * change locally would pass a look at the screen.
 */

type Shape = {
  shapeId: string;
  geometry: { x: number; y: number; w: number; h: number };
  visibleToPlayers: boolean;
  createdBy: string | null;
};

type Answer<T> = { data?: T; errors?: { message: string }[] };

async function gql<T>(
  page: Page,
  query: string,
  variables: Record<string, unknown>,
): Promise<T> {
  const res = await graphql<Answer<T>>(page, query, variables);
  if (res.errors?.length || !res.data) {
    throw new Error(`GraphQL failed: ${JSON.stringify(res.errors ?? res)}`);
  }
  return res.data;
}

async function userIdOf(page: Page): Promise<string> {
  return (await gql<{ me: { id: string } }>(page, `query { me { id } }`, {})).me
    .id;
}

/** The scene's drawings, as `page`'s viewer is shown them. */
async function shapesOf(page: Page, sceneId: string): Promise<Shape[]> {
  const data = await gql<{ shapes: Shape[] }>(
    page,
    `query ($sceneId: UUID!) {
      shapes(sceneId: $sceneId) { shapeId geometry visibleToPlayers createdBy }
    }`,
    { sceneId },
  );
  return data.shapes;
}

async function drawRect(
  page: Page,
  sceneId: string,
  geometry: Shape["geometry"],
): Promise<string> {
  const drawn = await gql<{ createShape: { shapeId: string } }>(
    page,
    `mutation ($input: GraphQLCreateShapeInput!) {
      createShape(input: $input) { shapeId }
    }`,
    { input: { sceneId, kind: "RECT", geometry, visibleToPlayers: true } },
  );
  return drawn.createShape.shapeId;
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

/** A left drag between two board points, held across frames at each end. */
async function dragBoard(
  page: Page,
  from: { x: number; y: number },
  to: { x: number; y: number },
): Promise<void> {
  const start = await boardToScreen(page, from);
  const end = await boardToScreen(page, to);
  await page.mouse.move(start.x, start.y);
  await page.mouse.down();
  await page.waitForTimeout(120);
  await page.mouse.move(end.x, end.y, { steps: 6 });
  await page.waitForTimeout(120);
  await page.mouse.up();
}

test("a player draws, everybody sees it, and they touch only their own", async ({
  page: gm,
  browser,
}) => {
  test.setTimeout(6 * 60_000);

  const worldId = await registerAndCreateWorld(
    gm,
    `Players draw ${uniqueSuffix()}`,
  );
  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    gm,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(gm, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;

  const playerA = await inviteAndJoinAsPlayer(browser, gm, worldId, "e2edrawa");
  const playerB = await inviteAndJoinAsPlayer(browser, gm, worldId, "e2edrawb");
  const aId = await userIdOf(playerA);

  try {
    for (const page of [gm, playerA, playerB]) {
      await page.goto(`/world/${worldId}/play`);
      await waitForEngineReady(page);
    }

    // Away from where A will draw, so no drag lands on them.
    const looking = (await camera(playerA))!;
    const gmShape = await drawRect(gm, sceneId, {
      x: looking.x - 260,
      y: looking.y - 40,
      w: 60,
      h: 60,
    });
    const bShape = await drawRect(playerB, sceneId, {
      x: looking.x + 200,
      y: looking.y - 40,
      w: 60,
      h: 60,
    });

    await test.step("a player's rail holds Select and Shapes, nothing else", async () => {
      await expect(playerA.getByTestId("gm-tool-rail")).toBeVisible({
        timeout: 15_000,
      });
      await expect(playerA.getByTestId("gm-tool-select")).toBeVisible();
      await expect(playerA.getByTestId("gm-tool-shapes")).toBeVisible();
      for (const tool of ["walls", "lights", "tokens", "interactions"]) {
        await expect(playerA.getByTestId(`gm-tool-${tool}`)).toHaveCount(0);
      }
    });

    let aShape = "";
    await test.step("player A draws, and every board shows it", async () => {
      await playerA.getByTestId("gm-tool-shapes").click();
      await expect(playerA.getByTestId("shape-tool")).toBeVisible();
      // A player is not offered hiding a drawing (FR-015).
      await expect(playerA.getByLabel("Visible to players")).toHaveCount(0);
      const rectangle = playerA.getByRole("button", { name: "Rectangle" });
      await rectangle.click();
      await expect(rectangle).toHaveAttribute("aria-pressed", "true");
      await dragBoard(
        playerA,
        { x: looking.x - 40, y: looking.y + 40 },
        { x: looking.x + 40, y: looking.y - 40 },
      );

      await expect
        .poll(
          async () =>
            (await shapesOf(gm, sceneId)).filter((s) => s.createdBy === aId)
              .length,
          { timeout: 15_000, message: "the drag must create A's drawing" },
        )
        .toBe(1);
      const mine = (await shapesOf(gm, sceneId)).find(
        (s) => s.createdBy === aId,
      )!;
      aShape = mine.shapeId;
      expect(mine.visibleToPlayers).toBe(true);
      for (const page of [gm, playerA, playerB]) {
        await expect
          .poll(() => boardHasDrawing(page, aShape), { timeout: 15_000 })
          .toBe(true);
      }
    });

    await test.step("player A's writes to the GM's and B's drawings are refused", async () => {
      const before = await shapesOf(gm, sceneId);
      for (const shapeId of [gmShape, bShape]) {
        const moved = await graphql<Answer<unknown>>(
          playerA,
          `
            mutation ($shapeId: UUID!, $input: GraphQLUpdateShapeInput!) {
              updateShape(shapeId: $shapeId, input: $input) {
                shapeId
              }
            }
          `,
          { shapeId, input: { geometry: { x: 0, y: 0, w: 5, h: 5 } } },
        );
        expect(moved.errors?.length ?? 0, "an update must be refused").toBe(1);
        const removed = await gql<{ deleteShape: boolean }>(
          playerA,
          `mutation ($shapeId: UUID!) { deleteShape(shapeId: $shapeId) }`,
          { shapeId },
        );
        expect(removed.deleteShape).toBe(false);
      }
      expect(await shapesOf(gm, sceneId)).toEqual(before);
    });

    await test.step("player A moves their own drawing", async () => {
      const rectangle = playerA.getByRole("button", { name: "Rectangle" });
      await rectangle.click();
      await expect(rectangle).toHaveAttribute("aria-pressed", "false");
      const before = (await shapesOf(gm, sceneId)).find(
        (s) => s.shapeId === aShape,
      )!;
      const centre = {
        x: before.geometry.x + before.geometry.w / 2,
        y: before.geometry.y + before.geometry.h / 2,
      };
      await dragBoard(playerA, centre, { x: centre.x, y: centre.y - 120 });
      await expect
        .poll(
          async () =>
            (await shapesOf(gm, sceneId)).find((s) => s.shapeId === aShape)
              ?.geometry.y,
          { timeout: 15_000, message: "A's own drawing must move" },
        )
        .toBeCloseTo(before.geometry.y - 120, -1);
    });
  } finally {
    await playerA.context().close();
    await playerB.context().close();
  }
});
