import { expect, test, type Page } from "./fixtures/test";
import {
  boardToScreen,
  clickBoard,
  rightClickBoard,
} from "./fixtures/boardPointer";
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
 * Spec 071 User Story 3 — a door shows what it can do (SC-003).
 *
 * Two browsers at one table. The Game Master's pointer over a shut door shows
 * the open icon, and pressing it opens the door; the open door then shows the
 * close icon. Locked, it shows the padlock to the Game Master and to the
 * player alike; the player's press changes nothing and the Game Master's
 * unlocks. Made secret, it shows nothing to either.
 *
 * Which icon is drawn is read from the engine's probe, because a sprite
 * cannot be read back from a canvas; what a press did is read from the
 * server, because a door that only looked opened has not opened.
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

/** The icon this board draws on the door, or `null` for none. */
async function iconOf(page: Page, wallId: string): Promise<string | null> {
  return page.evaluate((id) => {
    const probe = (
      window as unknown as {
        __engineProbe?: {
          doorIcons?: () => { wallId: string; icon: string }[];
        };
      }
    ).__engineProbe;
    return probe?.doorIcons?.().find((row) => row.wallId === id)?.icon ?? null;
  }, wallId);
}

/** Rest the pointer on a board point, as a hover. */
async function hover(page: Page, point: { x: number; y: number }) {
  const at = await boardToScreen(page, point);
  await page.mouse.move(at.x, at.y);
}

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

test("a door shows open, close and padlock icons on hover, and pressing one does what the menu does", async ({
  page,
  browser,
}) => {
  test.setTimeout(6 * 60_000);

  const worldId = await registerAndCreateWorld(
    page,
    `Door icons ${uniqueSuffix()}`,
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

  const looking = (await camera(page))!;
  const at = { x: looking.x, y: looking.y };
  // Far from the door, and from any token: where the pointer goes to rest.
  const away = { x: at.x + 300, y: at.y + 200 };
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

  await rightClickBoard(page, at);
  await page.getByTestId("canvas-menu-door-designate").click();
  await expect
    .poll(() => boardDoorOf(page, wallId), { timeout: 15_000 })
    .toMatchObject({ doorState: "CLOSED" });

  await test.step("a plain door shows nothing until the pointer is near", async () => {
    await hover(page, away);
    await page.waitForTimeout(300);
    expect(await iconOf(page, wallId)).toBeNull();
    // Along the door, off its middle: the door's reach, not the icon's.
    await hover(page, { x: at.x - 80, y: at.y + 2 });
    await expect.poll(() => iconOf(page, wallId)).toBe("open");
    await hover(page, away);
    await expect.poll(() => iconOf(page, wallId)).toBeNull();
  });

  await test.step("pressing the open icon opens the door, which then shows close", async () => {
    await hover(page, at);
    await expect.poll(() => iconOf(page, wallId)).toBe("open");
    await clickBoard(page, at);
    await expect
      .poll(server, { timeout: 15_000 })
      .toMatchObject({ doorState: "OPEN" });
    await expect.poll(() => iconOf(page, wallId)).toBe("close");
    // The press was the icon's: no menu, and nothing picked up.
    await expect(page.getByTestId("canvas-menu")).toHaveCount(0);
  });

  await test.step("pressing the close icon shuts it", async () => {
    await clickBoard(page, at);
    await expect
      .poll(server, { timeout: 15_000 })
      .toMatchObject({ doorState: "CLOSED" });
    await expect.poll(() => iconOf(page, wallId)).toBe("open");
  });

  const playerPage = await inviteAndJoinAsPlayer(browser, page, worldId);
  await playerPage.goto(`/world/${worldId}/play`);
  await waitForEngineReady(playerPage);
  await waitForWallsLoaded(playerPage);
  const playerBoard = () => boardDoorOf(playerPage, wallId);

  await test.step("locked, the padlock shows to the Game Master and the player", async () => {
    await choose(page, at, "door-lock", "Lock");
    await expect
      .poll(server, { timeout: 15_000 })
      .toMatchObject({ locked: true });
    await hover(page, at);
    await expect.poll(() => iconOf(page, wallId)).toBe("padlock");

    await expect
      .poll(playerBoard, { timeout: 15_000 })
      .toMatchObject({ locked: true });
    await hover(playerPage, at);
    await expect.poll(() => iconOf(playerPage, wallId)).toBe("padlock");
  });

  await test.step("the player's padlock changes nothing, and says it is locked", async () => {
    await clickBoard(playerPage, at);
    await expect(playerPage.getByTestId("canvas-menu-notice")).toContainText(
      "It is locked.",
    );
    await playerPage.waitForTimeout(1_500);
    expect(await server()).toEqual({
      doorState: "CLOSED",
      locked: true,
      secret: false,
    });
  });

  await test.step("the Game Master's padlock unlocks it", async () => {
    await hover(page, at);
    await expect.poll(() => iconOf(page, wallId)).toBe("padlock");
    await clickBoard(page, at);
    await expect
      .poll(server, { timeout: 15_000 })
      .toMatchObject({ doorState: "CLOSED", locked: false });
    await expect.poll(() => iconOf(page, wallId)).toBe("open");
    await expect
      .poll(playerBoard, { timeout: 15_000 })
      .toMatchObject({ locked: false });
    await hover(playerPage, away);
    await hover(playerPage, at);
    await expect.poll(() => iconOf(playerPage, wallId)).toBe("open");
  });

  await test.step("made secret, it shows no icon to either", async () => {
    await choose(page, at, "door-gm-lock", "Lock as a wall");
    await expect
      .poll(server, { timeout: 15_000 })
      .toEqual({ doorState: "CLOSED", locked: true, secret: true });
    await expect
      .poll(playerBoard, { timeout: 15_000 })
      .toMatchObject({ secret: true });
    await hover(page, away);
    await hover(page, at);
    await hover(playerPage, away);
    await hover(playerPage, at);
    await page.waitForTimeout(500);
    expect(await iconOf(page, wallId)).toBeNull();
    expect(await iconOf(playerPage, wallId)).toBeNull();
  });
});
