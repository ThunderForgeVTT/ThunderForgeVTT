import { expect, test, type Page } from "@playwright/test";
import {
  activeSceneId,
  ask,
  data,
  enterPlay,
  openDemo,
  WORLD_ID,
} from "./support";

/**
 * Spec 082 US6: the demo's player draws as a server's does. Two tabs of one
 * browser, one switched to the player: the player's rail holds Select and
 * Shapes, a drag draws a rectangle both tabs hold, the player's write to the
 * Game Master's drawing is refused, and the GM clears the player's drawing
 * while their own stays.
 */

const PLAYER_ID = "d0000000-0000-4000-0001-000000000002";

type Shape = { shapeId: string; createdBy: string };

async function shapesOf(page: Page, sceneId: string): Promise<Shape[]> {
  const answer = await data<{ shapes: Shape[] }>(
    page,
    "query ($sceneId: UUID!) { shapes(sceneId: $sceneId) { shapeId createdBy } }",
    { sceneId },
  );
  return answer.shapes;
}

/** A left drag across the middle of the board. */
async function dragAcrossBoard(page: Page): Promise<void> {
  const box = (await page.locator("canvas").boundingBox())!;
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  await page.mouse.move(x - 60, y - 40);
  await page.mouse.down();
  await page.waitForTimeout(120);
  await page.mouse.move(x + 60, y + 40, { steps: 6 });
  await page.waitForTimeout(120);
  await page.mouse.up();
}

test("a player draws in the demo, and the GM clears their drawing", async ({
  browser,
  baseURL,
}) => {
  test.setTimeout(240_000);
  const { page: gm, outside } = await openDemo(browser, baseURL);
  const context = gm.context();
  try {
    await enterPlay(gm);
    const sceneId = await activeSceneId(gm);
    const gmShape = (
      await data<{ createShape: { shapeId: string } }>(
        gm,
        `mutation ($input: GraphQLCreateShapeInput!) {
          createShape(input: $input) { shapeId }
        }`,
        {
          input: {
            sceneId,
            kind: "RECT",
            geometry: { x: 100, y: 100, w: 80, h: 80 },
            visibleToPlayers: true,
          },
        },
      )
    ).createShape.shapeId;

    const player = await context.newPage();
    await player.goto(`/demo/world/${WORLD_ID}/play`);
    await expect(player.getByTestId("demo-viewer")).toBeVisible({
      timeout: 60_000,
    });
    await player.getByRole("button", { name: "View as player" }).click();
    await expect(player.getByTestId("demo-viewer")).toContainText(
      "Viewing as a player",
    );
    await expect(player.locator("canvas")).toBeVisible({ timeout: 60_000 });

    await test.step("the player's rail holds Select and Shapes", async () => {
      await expect(player.getByTestId("gm-tool-shapes")).toBeVisible({
        timeout: 60_000,
      });
      await expect(player.getByTestId("gm-tool-select")).toBeVisible();
      for (const tool of ["walls", "lights", "tokens", "interactions"]) {
        await expect(player.getByTestId(`gm-tool-${tool}`)).toHaveCount(0);
      }
    });

    await test.step("the player draws, and it is theirs", async () => {
      await player.getByTestId("gm-tool-shapes").click();
      await expect(player.getByTestId("shape-tool")).toBeVisible();
      const rectangle = player.getByRole("button", { name: "Rectangle" });
      await rectangle.click();
      await expect(rectangle).toHaveAttribute("aria-pressed", "true");
      await dragAcrossBoard(player);
      await expect
        .poll(
          async () =>
            (await shapesOf(gm, sceneId)).filter(
              (shape) => shape.createdBy === PLAYER_ID,
            ).length,
          { timeout: 15_000, message: "the drag must draw the player's shape" },
        )
        .toBe(1);
    });

    await test.step("the player cannot move the GM's drawing", async () => {
      const moved = await ask(
        player,
        `mutation ($id: UUID!, $input: GraphQLUpdateShapeInput!) {
          updateShape(shapeId: $id, input: $input) { shapeId }
        }`,
        { id: gmShape, input: { geometry: { x: 0, y: 0, w: 1, h: 1 } } },
      );
      expect(moved.body.errors?.[0]?.message).toBe(
        "Failed to update shape (not found or not owned by you)",
      );
    });

    await test.step("the GM clears the player's drawing, and keeps their own", async () => {
      await gm.getByTestId("gm-tool-shapes").click();
      await expect(gm.getByTestId("shape-tool")).toBeVisible();
      await gm.getByTestId("shape-clear-player").click();
      const option = gm.locator(
        `[data-testid="shape-clear-player-option"][data-user-id="${PLAYER_ID}"]`,
      );
      await expect(option).toBeVisible({ timeout: 15_000 });
      await option.click();
      await gm.getByTestId("shape-clear-player-confirm").click();
      for (const page of [gm, player]) {
        await expect
          .poll(
            async () => (await shapesOf(page, sceneId)).map((s) => s.shapeId),
            {
              timeout: 15_000,
            },
          )
          .toEqual([gmShape]);
      }
    });

    expect(outside).toEqual([]);
  } finally {
    await context.close();
  }
});
