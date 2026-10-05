import { expect, test, type Page } from "./fixtures/test";
import { boardToScreen, rightClickBoard } from "./fixtures/boardPointer";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
  waitForWallsLoaded,
} from "./fixtures/helpers";
import { camera } from "./fixtures/lightingProbe";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 071 — a door is worked from a right-click on it.
 *
 * Two browsers at one table. The Game Master makes a wall a door, locks it,
 * unlocks it, and with a double right-click locks it as a wall; a player at
 * the same door is told it is locked, opens and shuts it once it is not, and
 * is offered nothing at all once it is wall.
 *
 * Every step is a right-click on the board and a press on the menu, and what
 * is asserted is the door the server holds afterwards — a menu that changed
 * only what this browser drew would pass a look at the screen.
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

type Door = { doorState: string; locked: boolean; secret: boolean };

/** The door as the server holds it. */
async function doorOf(
  page: Page,
  sceneId: string,
  wallId: string,
): Promise<Door | null> {
  const data = await gql<{ walls: (Door & { wallId: string })[] }>(
    page,
    `query ($sceneId: UUID!) {
      walls(sceneId: $sceneId) { wallId doorState locked secret }
    }`,
    { sceneId },
  );
  const wall = data.walls.find((w) => w.wallId === wallId);
  return wall
    ? { doorState: wall.doorState, locked: wall.locked, secret: wall.secret }
    : null;
}

/** The door as this browser's board holds it. */
async function boardDoorOf(page: Page, wallId: string): Promise<Door | null> {
  return page.evaluate(async (id) => {
    const bevy = (await import(
      /* @vite-ignore */ "/src/engine/bevy/index.ts"
    )) as typeof import("../src/engine/bevy/index");
    const wall = bevy.getBoundWorldStore()?.getState().walls[id];
    return wall
      ? {
          doorState: wall.doorState.toUpperCase(),
          locked: wall.locked === true,
          secret: wall.secret === true,
        }
      : null;
  }, wallId);
}

/** Right-click the door and press one item of the menu that opens. */
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

test("a door is made, locked, opened and locked as a wall from a right-click", async ({
  page,
  browser,
}) => {
  test.setTimeout(6 * 60_000);

  const worldId = await registerAndCreateWorld(
    page,
    `Door menu ${uniqueSuffix()}`,
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

  // Where the Game Master is looking, so it is on screen for both of them.
  const looking = (await camera(page))!;
  const at = { x: looking.x, y: looking.y };
  const created = await gql<{ createWall: { wallId: string } }>(
    page,
    `mutation ($input: GraphQLCreateWallInput!) {
      createWall(input: $input) { wallId }
    }`,
    {
      input: {
        sceneId,
        x1: at.x - 100,
        y1: at.y,
        x2: at.x + 100,
        y2: at.y,
        blocksVision: true,
        blocksMovement: true,
      },
    },
  );
  const wallId = created.createWall.wallId;
  await waitForWallsLoaded(page);

  const server = () => doorOf(page, sceneId, wallId);

  await test.step("the Game Master makes a wall a door", async () => {
    await rightClickBoard(page, at);
    await expect(page.getByTestId("canvas-menu")).toContainText("Wall");
    await page.getByTestId("canvas-menu-door-designate").click();
    await expect
      .poll(server, { timeout: 15_000 })
      .toEqual({ doorState: "CLOSED", locked: false, secret: false });
    await expect
      .poll(() => boardDoorOf(page, wallId), { timeout: 15_000 })
      .toMatchObject({ doorState: "CLOSED" });
  });

  const playerPage = await inviteAndJoinAsPlayer(browser, page, worldId);
  await playerPage.goto(`/world/${worldId}/play`);
  await waitForEngineReady(playerPage);
  await waitForWallsLoaded(playerPage);
  const playerBoard = () => boardDoorOf(playerPage, wallId);
  const onScreen = await boardToScreen(playerPage, at);
  const view = playerPage.viewportSize()!;
  expect(
    onScreen.x > 0 && onScreen.x < view.width && onScreen.y > 0,
    "the door is on the player's screen",
  ).toBe(true);

  await test.step("a right-click locks it, and a player is told so", async () => {
    await choose(page, at, "door-lock", "Lock");
    await expect.poll(server, { timeout: 15_000 }).toMatchObject({
      locked: true,
    });
    await expect
      .poll(playerBoard, { timeout: 15_000 })
      .toMatchObject({ locked: true });

    await rightClickBoard(playerPage, at);
    const told = playerPage.getByTestId("canvas-menu-door-locked");
    await expect(told).toHaveText("Locked", { timeout: 10_000 });
    await expect(told).toHaveAttribute("data-disabled", "");
    // And nothing a player could press instead.
    await expect(
      playerPage.getByTestId("canvas-menu").getByRole("menuitem"),
    ).toHaveCount(1);
    await playerPage.keyboard.press("Escape");
    await expect(playerPage.getByTestId("canvas-menu")).toBeHidden();
  });

  await test.step("a right-click unlocks it, and the player opens and shuts it", async () => {
    await choose(page, at, "door-lock", "Unlock");
    await expect.poll(server, { timeout: 15_000 }).toMatchObject({
      locked: false,
    });
    await expect
      .poll(playerBoard, { timeout: 15_000 })
      .toMatchObject({ locked: false });

    await choose(playerPage, at, "door-state", "Open");
    await expect.poll(server, { timeout: 15_000 }).toMatchObject({
      doorState: "OPEN",
    });
    await expect
      .poll(playerBoard, { timeout: 15_000 })
      .toMatchObject({ doorState: "OPEN" });
  });

  await test.step("a double right-click locks an open door as a wall", async () => {
    await expect
      .poll(() => boardDoorOf(page, wallId), { timeout: 15_000 })
      .toMatchObject({ doorState: "OPEN" });

    await rightClickBoard(page, at);
    await page.waitForTimeout(80);
    await rightClickBoard(page, at);

    // Shut, locked and hidden: all three, from the one gesture.
    await expect
      .poll(server, { timeout: 15_000 })
      .toEqual({ doorState: "CLOSED", locked: true, secret: true });
    await expect(page.getByTestId("canvas-menu")).toBeHidden();
  });

  await test.step("to the player it is now wall, with nothing to offer", async () => {
    await expect
      .poll(playerBoard, { timeout: 15_000 })
      .toEqual({ doorState: "CLOSED", locked: true, secret: true });
    await rightClickBoard(playerPage, at);
    await playerPage.waitForTimeout(1_500);
    await expect(playerPage.getByTestId("canvas-menu")).toBeHidden();
  });

  await test.step("the Game Master shows it to the table again", async () => {
    await expect
      .poll(() => boardDoorOf(page, wallId), { timeout: 15_000 })
      .toMatchObject({ secret: true });
    await rightClickBoard(page, at);
    await expect(page.getByTestId("canvas-menu")).toContainText("Hidden door");
    // Already a wall to the table, so that is not offered twice.
    await expect(page.getByTestId("canvas-menu-door-gm-lock")).toHaveCount(0);
    await page.getByTestId("canvas-menu-door-reveal").click();
    await expect
      .poll(server, { timeout: 15_000 })
      .toEqual({ doorState: "CLOSED", locked: true, secret: false });
    await expect
      .poll(playerBoard, { timeout: 15_000 })
      .toMatchObject({ secret: false });
  });
});
