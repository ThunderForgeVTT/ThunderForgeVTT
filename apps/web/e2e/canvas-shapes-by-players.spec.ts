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

test("the GM clears every drawing, and a player cannot", async ({
  page: gm,
  browser,
}) => {
  test.setTimeout(6 * 60_000);

  const worldId = await registerAndCreateWorld(
    gm,
    `Clear drawings ${uniqueSuffix()}`,
  );
  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    gm,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(gm, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;

  const playerA = await inviteAndJoinAsPlayer(browser, gm, worldId, "e2eclra");
  const playerB = await inviteAndJoinAsPlayer(browser, gm, worldId, "e2eclrb");

  try {
    for (const page of [gm, playerA, playerB]) {
      await page.goto(`/world/${worldId}/play`);
      await waitForEngineReady(page);
    }
    const drawn = [
      await drawRect(gm, sceneId, { x: 0, y: 0, w: 40, h: 40 }),
      await drawRect(playerA, sceneId, { x: 80, y: 0, w: 40, h: 40 }),
      await drawRect(playerB, sceneId, { x: 160, y: 0, w: 40, h: 40 }),
    ];
    for (const page of [gm, playerA, playerB]) {
      for (const shapeId of drawn) {
        await expect
          .poll(() => boardHasDrawing(page, shapeId), { timeout: 15_000 })
          .toBe(true);
      }
    }

    await test.step("a player is offered no clear, and the server refuses theirs", async () => {
      await playerA.getByTestId("gm-tool-shapes").click();
      await expect(playerA.getByTestId("shape-tool")).toBeVisible();
      await expect(playerA.getByTestId("shape-clear-all")).toHaveCount(0);
      const refused = await graphql<Answer<unknown>>(
        playerA,
        `
          mutation ($sceneId: UUID!) {
            clearShapes(sceneId: $sceneId)
          }
        `,
        { sceneId },
      );
      expect(refused.errors?.length ?? 0, "a player's clear is refused").toBe(
        1,
      );
      expect(await shapesOf(gm, sceneId)).toHaveLength(3);
    });

    await gm.getByTestId("gm-tool-shapes").click();
    const dialog = gm.getByRole("dialog", { name: /Clear every drawing on/ });

    await test.step("cancelling leaves every drawing", async () => {
      await gm.getByTestId("shape-clear-all").click();
      await expect(dialog).toBeVisible();
      await dialog.getByTestId("shape-clear-cancel").click();
      await expect(dialog).toBeHidden();
      expect(await shapesOf(gm, sceneId)).toHaveLength(3);
    });

    await test.step("confirming empties the server and every board", async () => {
      await gm.getByTestId("shape-clear-all").click();
      await dialog.getByTestId("shape-clear-confirm").click();
      await expect
        .poll(async () => (await shapesOf(gm, sceneId)).length, {
          timeout: 15_000,
        })
        .toBe(0);
      for (const page of [gm, playerA, playerB]) {
        for (const shapeId of drawn) {
          await expect
            .poll(() => boardHasDrawing(page, shapeId), { timeout: 15_000 })
            .toBe(false);
        }
      }
    });
  } finally {
    await playerA.context().close();
    await playerB.context().close();
  }
});

test("the GM clears one player's drawings, and the rest stay", async ({
  page: gm,
  browser,
}) => {
  test.setTimeout(6 * 60_000);

  const worldId = await registerAndCreateWorld(
    gm,
    `Clear a player ${uniqueSuffix()}`,
  );
  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    gm,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(gm, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;

  const playerA = await inviteAndJoinAsPlayer(browser, gm, worldId, "e2eclpa");
  const playerB = await inviteAndJoinAsPlayer(browser, gm, worldId, "e2eclpb");

  try {
    for (const page of [gm, playerA, playerB]) {
      await page.goto(`/world/${worldId}/play`);
      await waitForEngineReady(page);
    }
    const aId = await userIdOf(playerA);
    const gmShape = await drawRect(gm, sceneId, { x: 0, y: 0, w: 40, h: 40 });
    const aShapes = [
      await drawRect(playerA, sceneId, { x: 80, y: 0, w: 40, h: 40 }),
      await drawRect(playerA, sceneId, { x: 80, y: 80, w: 40, h: 40 }),
    ];
    const bShape = await drawRect(playerB, sceneId, {
      x: 160,
      y: 0,
      w: 40,
      h: 40,
    });
    for (const shapeId of [gmShape, ...aShapes, bShape]) {
      await expect
        .poll(() => boardHasDrawing(gm, shapeId), { timeout: 15_000 })
        .toBe(true);
    }

    await gm.getByTestId("gm-tool-shapes").click();
    await gm.getByTestId("shape-clear-player").click();
    const dialog = gm.getByRole("dialog", {
      name: /Clear a player's drawings on/,
    });
    const options = dialog.getByTestId("shape-clear-player-option");

    await test.step("the picker lists the two players with their counts", async () => {
      await expect(options).toHaveCount(2);
      await expect(dialog).toContainText("2 shapes");
      await expect(dialog).toContainText("1 shape");
      await expect(
        dialog.getByTestId("shape-clear-player-confirm"),
      ).toBeDisabled();
    });

    await test.step("clearing A leaves the GM's and B's drawings", async () => {
      await dialog.locator(`[data-user-id="${aId}"]`).click();
      await dialog.getByTestId("shape-clear-player-confirm").click();
      await expect(dialog).toBeHidden();
      await expect
        .poll(
          async () =>
            (await shapesOf(gm, sceneId)).map((shape) => shape.shapeId).sort(),
          { timeout: 15_000 },
        )
        .toEqual([gmShape, bShape].sort());
      for (const page of [gm, playerA, playerB]) {
        for (const shapeId of aShapes) {
          await expect
            .poll(() => boardHasDrawing(page, shapeId), { timeout: 15_000 })
            .toBe(false);
        }
        for (const shapeId of [gmShape, bShape]) {
          expect(await boardHasDrawing(page, shapeId)).toBe(true);
        }
      }
    });
  } finally {
    await playerA.context().close();
    await playerB.context().close();
  }
});

test("the GM takes Shapes away and gives it back, without a reload", async ({
  page: gm,
  browser,
}) => {
  test.setTimeout(6 * 60_000);

  const worldId = await registerAndCreateWorld(
    gm,
    `Take the pen ${uniqueSuffix()}`,
  );
  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    gm,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(gm, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;

  const player = await inviteAndJoinAsPlayer(browser, gm, worldId, "e2etake");

  try {
    const playerId = await userIdOf(player);
    const members = await gql<{
      worldMembers: { id: string; userId: string }[];
    }>(
      gm,
      `query ($worldId: UUID!) { worldMembers(worldId: $worldId) { id userId } }`,
      { worldId },
    );
    const memberId = members.worldMembers.find(
      (member) => member.userId === playerId,
    )?.id;
    expect(memberId, "the player is a member").toBeTruthy();

    await player.goto(`/world/${worldId}/play`);
    await waitForEngineReady(player);
    const earlier = await drawRect(player, sceneId, {
      x: 0,
      y: 0,
      w: 40,
      h: 40,
    });
    await expect(player.getByTestId("gm-tool-shapes")).toBeVisible();

    await gm.goto(`/world/${worldId}/settings/system`);
    const toggle = gm.getByTestId(`authoring-tool-toggle-${memberId}-shapes`);
    await expect(gm.getByTestId("authoring-tool-grants-card")).toContainText(
      "Players select and draw by default",
    );
    await expect(toggle).toBeChecked();

    await test.step("unticking Shapes takes it from the rail and the server", async () => {
      // The card shows the server's answer, not the click (a grant is never
      // guessed), so `uncheck()`'s own "did it change" check is too early.
      await toggle.click();
      await expect(toggle).not.toBeChecked();
      await expect(toggle).toBeEnabled();
      await expect(player.getByTestId("gm-tool-shapes")).toHaveCount(0, {
        timeout: 15_000,
      });
      const refused = await graphql<Answer<unknown>>(
        player,
        `
          mutation ($input: GraphQLCreateShapeInput!) {
            createShape(input: $input) {
              shapeId
            }
          }
        `,
        {
          input: {
            sceneId,
            kind: "RECT",
            geometry: { x: 80, y: 0, w: 40, h: 40 },
            visibleToPlayers: true,
          },
        },
      );
      expect(refused.errors?.length ?? 0, "drawing is refused").toBe(1);
    });

    await test.step("ticking it again gives both back, and the old drawing stands", async () => {
      await toggle.click();
      await expect(toggle).toBeChecked();
      await expect(player.getByTestId("gm-tool-shapes")).toBeVisible({
        timeout: 15_000,
      });
      await drawRect(player, sceneId, { x: 80, y: 0, w: 40, h: 40 });
      const shapes = await shapesOf(gm, sceneId);
      const kept = shapes.find((shape) => shape.shapeId === earlier);
      expect(kept?.geometry).toEqual({ x: 0, y: 0, w: 40, h: 40 });
      expect(kept?.createdBy).toBe(playerId);
      expect(shapes).toHaveLength(2);
    });
  } finally {
    await player.context().close();
  }
});
