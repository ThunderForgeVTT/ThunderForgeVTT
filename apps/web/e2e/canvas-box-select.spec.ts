import { expect, test, type Page } from "./fixtures/test";
import { boardToScreen, clickBoard } from "./fixtures/boardPointer";
import {
  graphql,
  inviteAndJoinAsPlayer,
  openGmTool,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
  waitForWallsLoaded,
} from "./fixtures/helpers";
import { sightAt } from "./fixtures/lightingProbe";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 085 — hidden walls, and selecting several things at once.
 *
 * What is asserted is what each board draws and what the server holds. A
 * hidden wall is a claim about drawing — the geometry still reaches every
 * client, so it still blocks there — so the player's engine is asked which
 * walls it draws, and the player's own drag and sight are what prove it still
 * blocks.
 */

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

type ServerWall = { wallId: string; secret: boolean; doorState: string };

async function serverWall(
  page: Page,
  sceneId: string,
  wallId: string,
): Promise<ServerWall | undefined> {
  const data = await gql<{ walls: ServerWall[] }>(
    page,
    `query ($sceneId: UUID!) { walls(sceneId: $sceneId) { wallId secret doorState } }`,
    { sceneId },
  );
  return data.walls.find((w) => w.wallId === wallId);
}

async function serverToken(
  page: Page,
  sceneId: string,
  tokenId: string,
): Promise<{ x: number; y: number }> {
  const data = await gql<{
    tokens: { tokenId: string; x: number; y: number }[];
  }>(
    page,
    `query ($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId x y } }`,
    { sceneId },
  );
  const token = data.tokens.find((t) => t.tokenId === tokenId);
  if (!token) throw new Error(`token ${tokenId} is not on the scene`);
  return { x: token.x, y: token.y };
}

/** The walls this board draws a sprite for (`__engineProbe.drawnWalls`). */
async function drawnWalls(page: Page): Promise<string[]> {
  return page.evaluate(
    () =>
      (
        window as unknown as {
          __engineProbe?: { drawnWalls?: () => string[] };
        }
      ).__engineProbe?.drawnWalls?.() ?? [],
  );
}

/** Where this board's store holds a token. */
async function boardToken(
  page: Page,
  tokenId: string,
): Promise<{ x: number; y: number } | undefined> {
  return page.evaluate(async (id) => {
    const bevy = (await import(
      /* @vite-ignore */ "/src/engine/bevy/index.ts"
    )) as typeof import("../src/engine/bevy/index");
    const token = bevy.getBoundWorldStore()?.getState().tokens[id];
    return token ? { x: token.x, y: token.y } : undefined;
  }, tokenId);
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
  await page.mouse.move(end.x, end.y, { steps: 8 });
  await page.waitForTimeout(120);
  await page.mouse.up();
}

/**
 * Select a wall in the Walls tool by clicking its body. Retried as a whole:
 * a click that lands before the engine has the wall selects nothing.
 */
async function selectWall(
  page: Page,
  at: { x: number; y: number },
): Promise<void> {
  await expect(async () => {
    await clickBoard(page, at);
    await expect(page.getByTestId("wall-hidden-toggle")).toBeVisible({
      timeout: 3_000,
    });
  }).toPass({ timeout: 30_000 });
}

test("the GM hides a plain wall: the table does not see it, and it still blocks", async ({
  page: gm,
  browser,
}) => {
  test.setTimeout(6 * 60_000);

  const worldId = await registerAndCreateWorld(
    gm,
    `Hidden wall ${uniqueSuffix()}`,
  );
  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    gm,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(gm, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;

  const player = await inviteAndJoinAsPlayer(browser, gm, worldId);
  const playerUserId = (
    await gql<{ me: { id: string } }>(player, `query { me { id } }`, {})
  ).me.id;

  // A plain wall across the origin, and the player's token west of it.
  const wallId = (
    await gql<{ createWall: { wallId: string } }>(
      gm,
      `mutation ($input: GraphQLCreateWallInput!) {
        createWall(input: $input) { wallId }
      }`,
      {
        input: {
          sceneId,
          x1: 0,
          y1: -300,
          x2: 0,
          y2: 300,
          blocksVision: true,
          blocksMovement: true,
        },
      },
    )
  ).createWall.wallId;
  const tokenId = (
    await gql<{ createToken: { tokenId: string } }>(
      gm,
      `mutation ($input: GraphQLCreateTokenInput!) {
        createToken(input: $input) { tokenId }
      }`,
      { input: { sceneId, x: -200, y: 0, tokenType: "character" } },
    )
  ).createToken.tokenId;
  await gql(
    gm,
    `mutation ($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
      updateToken(tokenId: $tokenId, input: $input) { tokenId }
    }`,
    { tokenId, input: { ownerUserId: playerUserId, isPrimary: true } },
  );

  try {
    for (const page of [gm, player]) {
      await page.goto(`/world/${worldId}/play`);
      await waitForEngineReady(page);
      await waitForWallsLoaded(page);
    }
    await expect
      .poll(() => drawnWalls(player), { timeout: 15_000 })
      .toContain(wallId);

    await test.step("the GM hides it from the Walls panel", async () => {
      await openGmTool(gm, "walls");
      await selectWall(gm, { x: 0, y: 100 });
      const toggle = gm.getByTestId("wall-hidden-toggle");
      await expect(toggle).not.toBeChecked();
      await toggle.click();
      await expect(toggle).toBeChecked();
      await expect
        .poll(async () => (await serverWall(gm, sceneId, wallId))?.secret, {
          timeout: 15_000,
          message: "the server must hold the wall hidden",
        })
        .toBe(true);
    });

    await test.step("the player's board does not draw it; the GM's does", async () => {
      await expect
        .poll(() => drawnWalls(player), {
          timeout: 15_000,
          message: "a hidden wall is not drawn for the table",
        })
        .not.toContain(wallId);
      expect(await drawnWalls(gm)).toContain(wallId);
    });

    await test.step("it still blocks the player's sight and the player's drag", async () => {
      await expect
        .poll(() => sightAt(player, 200, 0), {
          timeout: 15_000,
          message: "the player cannot see past a hidden wall",
        })
        .toEqual({ looking: true, seen: false });

      const before = await serverToken(gm, sceneId, tokenId);
      const onBoard = (await boardToken(player, tokenId))!;
      await dragBoard(player, onBoard, { x: 200, y: onBoard.y });
      // Long enough for a move that was going to be sent to have landed.
      await player.waitForTimeout(1_500);
      expect(
        await serverToken(gm, sceneId, tokenId),
        "the drag across the hidden wall is not sent",
      ).toEqual(before);
      await expect
        .poll(async () => (await boardToken(player, tokenId))?.x, {
          timeout: 10_000,
          message: "the token ends where it began",
        })
        .toBeCloseTo(onBoard.x, 0);
    });

    await test.step("made a door, it stays hidden", async () => {
      await gm.getByTestId("wall-primitive-door").click();
      await clickBoard(gm, { x: 0, y: -100 });
      await expect
        .poll(async () => await serverWall(gm, sceneId, wallId), {
          timeout: 15_000,
          message: "the click makes the hidden wall a closed door",
        })
        .toMatchObject({ doorState: "CLOSED", secret: true });
    });

    await test.step("a reload keeps all of it", async () => {
      for (const page of [gm, player]) {
        await page.reload();
        await waitForEngineReady(page);
        await waitForWallsLoaded(page);
      }
      // Given time for a wall that was going to be drawn to have been.
      await expect
        .poll(() => drawnWalls(gm), { timeout: 15_000 })
        .toContain(wallId);
      expect(await drawnWalls(player)).not.toContain(wallId);
      await expect
        .poll(() => sightAt(player, 200, 0), { timeout: 15_000 })
        .toEqual({ looking: true, seen: false });
      expect(await serverWall(gm, sceneId, wallId)).toMatchObject({
        doorState: "CLOSED",
        secret: true,
      });
    });
  } finally {
    await player.context().close();
  }
});
