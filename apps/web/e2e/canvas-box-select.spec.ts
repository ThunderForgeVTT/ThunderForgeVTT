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

type Selection = {
  tokens: string[];
  walls: string[];
  lights: string[];
  shapes: string[];
};

/** What this board's engine holds selected (`__engineProbe.selection`). */
async function selection(page: Page): Promise<Selection> {
  return page.evaluate(
    () =>
      (
        window as unknown as {
          __engineProbe?: { selection?: () => Selection };
        }
      ).__engineProbe?.selection?.() ?? {
        tokens: [],
        walls: [],
        lights: [],
        shapes: [],
      },
  );
}

/** A box: a press on empty board, a move, a release. */
/**
 * Press Shift where the engine hears it, and hold it across a frame. The
 * engine reads keys from the canvas, and reads Shift on the frame of the
 * press and again on the frame of the release: a key that changes in the
 * same frame as the mouse is a plain gesture to it.
 */
async function holdShiftOnBoard(page: Page): Promise<void> {
  await page.locator("canvas").focus();
  await page.keyboard.down("Shift");
  await page.waitForTimeout(120);
}

/** Let go of Shift a frame after the mouse, so the release still sees it. */
async function releaseShift(page: Page): Promise<void> {
  await page.waitForTimeout(120);
  await page.keyboard.up("Shift");
}

async function boxBoard(
  page: Page,
  from: { x: number; y: number },
  to: { x: number; y: number },
  shift = false,
): Promise<void> {
  if (shift) await holdShiftOnBoard(page);
  try {
    await dragBoard(page, from, to);
  } finally {
    if (shift) await releaseShift(page);
  }
}

type ServerWallAt = {
  wallId: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  secret: boolean;
};

async function serverWalls(
  page: Page,
  sceneId: string,
): Promise<ServerWallAt[]> {
  return (
    await gql<{ walls: ServerWallAt[] }>(
      page,
      `query ($sceneId: UUID!) { walls(sceneId: $sceneId) { wallId x1 y1 x2 y2 secret } }`,
      { sceneId },
    )
  ).walls;
}

async function serverTokens(
  page: Page,
  sceneId: string,
): Promise<{ tokenId: string; x: number; y: number }[]> {
  return (
    await gql<{ tokens: { tokenId: string; x: number; y: number }[] }>(
      page,
      `query ($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId x y } }`,
      { sceneId },
    )
  ).tokens;
}

const sorted = (ids: string[]) => [...ids].sort();

/**
 * Wait for the board to hold `expected` (only the kinds named, compared
 * unordered). Polled rather than read once: a box's release lands on the
 * engine's next frame, and until then the board still shows the press,
 * which drops the tokens (a press on empty board deselects them).
 */
async function expectHeld(
  page: Page,
  expected: Partial<Selection>,
  timeout = 5_000,
): Promise<void> {
  const kinds = Object.keys(expected) as (keyof Selection)[];
  const pick = (held: Selection) =>
    Object.fromEntries(kinds.map((kind) => [kind, sorted(held[kind])]));
  await expect
    .poll(async () => pick(await selection(page)), { timeout })
    .toEqual(
      pick({ tokens: [], walls: [], lights: [], shapes: [], ...expected }),
    );
}

test("the GM boxes a group, moves it as one, trims it, hides its walls and deletes it", async ({
  page: gm,
}) => {
  test.setTimeout(6 * 60_000);

  // A 50-unit grid keeps the whole layout in the middle of the board, clear
  // of the tool rail on the left and the dock on the right.
  const GRID = 50;
  const worldId = await registerAndCreateWorld(
    gm,
    `Box select ${uniqueSuffix()}`,
  );
  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    gm,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(gm, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;
  await gql(
    gm,
    `mutation ($sceneId: UUID!, $input: GraphQLUpdateSceneInput!) {
      updateScene(sceneId: $sceneId, input: $input) { sceneId }
    }`,
    { sceneId, input: { gridSize: GRID } },
  );

  // Three one-cell tokens on cell centres, and a wall below them.
  const at = [
    { x: -125, y: 75 },
    { x: -25, y: 75 },
    { x: -125, y: -25 },
  ];
  const tokenIds: string[] = [];
  for (const point of at) {
    tokenIds.push(
      (
        await gql<{ createToken: { tokenId: string } }>(
          gm,
          `mutation ($input: GraphQLCreateTokenInput!) {
            createToken(input: $input) { tokenId }
          }`,
          {
            input: { sceneId, x: point.x, y: point.y, tokenType: "character" },
          },
        )
      ).createToken.tokenId,
    );
  }
  const [, t2] = tokenIds;
  const wallId = (
    await gql<{ createWall: { wallId: string } }>(
      gm,
      `mutation ($input: GraphQLCreateWallInput!) {
        createWall(input: $input) { wallId }
      }`,
      {
        input: {
          sceneId,
          x1: -150,
          y1: -100,
          x2: -50,
          y2: -100,
          blocksVision: true,
          blocksMovement: true,
        },
      },
    )
  ).createWall.wallId;

  await gm.goto(`/world/${worldId}/play`);
  await waitForEngineReady(gm);
  await waitForWallsLoaded(gm);
  for (const id of tokenIds) {
    await expect
      .poll(() => boardToken(gm, id), { timeout: 15_000 })
      .toBeDefined();
  }

  await test.step("a box over the three tokens and the wall takes all four", async () => {
    await openGmTool(gm, "select");
    await expect(async () => {
      await boxBoard(gm, { x: -170, y: 120 }, { x: 10, y: -120 });
      await expectHeld(gm, { tokens: tokenIds, walls: [wallId] });
    }).toPass({ timeout: 30_000 });
    await expect(gm.getByTestId("selection-bar-counts")).toHaveText(
      "3 tokens, 1 wall",
    );
  });

  await test.step("dragging one token two cells moves all four by the same offset (SC-002)", async () => {
    const tokensBefore = await serverTokens(gm, sceneId);
    const [wallBefore] = (await serverWalls(gm, sceneId)).filter(
      (w) => w.wallId === wallId,
    );
    await dragBoard(gm, at[0], { x: at[0].x + 2 * GRID, y: at[0].y });
    await expect
      .poll(
        async () =>
          (await serverWalls(gm, sceneId)).find((w) => w.wallId === wallId)?.x1,
        { timeout: 15_000, message: "the wall's move reaches the server" },
      )
      .toBe(wallBefore.x1 + 2 * GRID);

    await gm.reload();
    await waitForEngineReady(gm);
    await waitForWallsLoaded(gm);
    const tokensAfter = await serverTokens(gm, sceneId);
    for (const before of tokensBefore) {
      const after = tokensAfter.find((t) => t.tokenId === before.tokenId)!;
      expect(after.x - before.x, `${before.tokenId} x`).toBe(2 * GRID);
      expect(after.y - before.y, `${before.tokenId} y`).toBe(0);
    }
    const wallAfter = (await serverWalls(gm, sceneId)).find(
      (w) => w.wallId === wallId,
    )!;
    expect([
      wallAfter.x1 - wallBefore.x1,
      wallAfter.y1 - wallBefore.y1,
      wallAfter.x2 - wallBefore.x2,
      wallAfter.y2 - wallBefore.y2,
    ]).toEqual([2 * GRID, 0, 2 * GRID, 0]);
    for (const id of tokenIds) {
      await expect
        .poll(() => boardToken(gm, id), { timeout: 15_000 })
        .toBeDefined();
    }
  });

  // Where everything is now: two cells to the right.
  const moved = at.map((p) => ({ x: p.x + 2 * GRID, y: p.y }));

  await test.step("shift-box takes one token out; shift-click puts it back", async () => {
    await expect(async () => {
      await boxBoard(gm, { x: -70, y: 120 }, { x: 120, y: -120 });
      await expectHeld(gm, { tokens: tokenIds, walls: [wallId] });
    }).toPass({ timeout: 30_000 });

    // Retried from the whole group: the engine reads shift from the
    // keyboard, and a key press it missed leaves a plain box, which takes
    // only the one token.
    await expect(async () => {
      if (sorted((await selection(gm)).tokens).length !== tokenIds.length) {
        await boxBoard(gm, { x: -70, y: 120 }, { x: 120, y: -120 });
      }
      await boxBoard(gm, { x: 120, y: 120 }, { x: 45, y: 30 }, true);
      await expect
        .poll(async () => sorted((await selection(gm)).tokens), {
          timeout: 5_000,
        })
        .toEqual(sorted(tokenIds.filter((id) => id !== t2)));
    }).toPass({ timeout: 30_000 });
    expect((await selection(gm)).walls).toEqual([wallId]);

    const withoutT2 = sorted(tokenIds.filter((id) => id !== t2));
    await expect(async () => {
      // A missed shift makes the click a plain one, which leaves only t2:
      // put the group back as the step above left it, then click again.
      if (
        JSON.stringify(sorted((await selection(gm)).tokens)) !==
        JSON.stringify(withoutT2)
      ) {
        await boxBoard(gm, { x: -70, y: 120 }, { x: 120, y: -120 });
        await boxBoard(gm, { x: 120, y: 120 }, { x: 45, y: 30 }, true);
        await expectHeld(gm, { tokens: withoutT2 });
      }
      await holdShiftOnBoard(gm);
      try {
        await clickBoard(gm, moved[1]);
      } finally {
        await releaseShift(gm);
      }
      await expect
        .poll(async () => sorted((await selection(gm)).tokens), {
          timeout: 5_000,
        })
        .toEqual(sorted(tokenIds));
    }).toPass({ timeout: 30_000 });
    expect((await selection(gm)).walls).toEqual([wallId]);
  });

  await test.step("with walls unticked, a box does not take the wall", async () => {
    await openGmTool(gm, "select");
    const expand = gm.getByTestId("selection-filter-expand");
    if (await expand.isVisible().catch(() => false)) await expand.click();
    await gm.getByTestId("selection-filter-walls").uncheck();
    await expect(async () => {
      await boxBoard(gm, { x: -70, y: 120 }, { x: 120, y: -120 });
      await expectHeld(gm, { tokens: tokenIds, walls: [] });
    }).toPass({ timeout: 30_000 });

    await gm.getByTestId("selection-filter-walls").check();
    await expect(async () => {
      await boxBoard(gm, { x: -70, y: 120 }, { x: 120, y: -120 });
      await expectHeld(gm, { walls: [wallId] });
    }).toPass({ timeout: 30_000 });
  });

  await test.step("the Select bar hides the group's walls", async () => {
    const hidden = gm.getByTestId("selection-bar-hidden");
    await expect(hidden).not.toBeChecked();
    await hidden.click();
    await expect
      .poll(
        async () =>
          (await serverWalls(gm, sceneId)).find((w) => w.wallId === wallId)
            ?.secret,
        { timeout: 15_000, message: "the server holds the wall hidden" },
      )
      .toBe(true);
    await expect(hidden).toBeChecked();
  });

  await test.step("Delete removes the group from the board and the server", async () => {
    await gm.getByTestId("selection-bar-delete").click();
    await expect
      .poll(
        async () => {
          const tokens = (await serverTokens(gm, sceneId)).map(
            (t) => t.tokenId,
          );
          const walls = (await serverWalls(gm, sceneId)).map((w) => w.wallId);
          return [...tokens, ...walls].filter(
            (id) => tokenIds.includes(id) || id === wallId,
          );
        },
        { timeout: 15_000, message: "the server holds none of the group" },
      )
      .toEqual([]);
    for (const id of tokenIds) {
      await expect
        .poll(() => boardToken(gm, id), { timeout: 15_000 })
        .toBeUndefined();
    }
    await expect
      .poll(() => drawnWalls(gm), { timeout: 15_000 })
      .not.toContain(wallId);
    await expect(gm.getByTestId("selection-bar")).toHaveCount(0);
  });
});

test("a player's box takes only what is theirs, and a group drag leaves a token at the wall", async ({
  page: gm,
  browser,
}) => {
  test.setTimeout(6 * 60_000);

  const GRID = 50;
  const worldId = await registerAndCreateWorld(
    gm,
    `Player box ${uniqueSuffix()}`,
  );
  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    gm,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(gm, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;
  await gql(
    gm,
    `mutation ($sceneId: UUID!, $input: GraphQLUpdateSceneInput!) {
      updateScene(sceneId: $sceneId, input: $input) { sceneId }
    }`,
    { sceneId, input: { gridSize: GRID } },
  );

  const player = await inviteAndJoinAsPlayer(browser, gm, worldId);
  const playerUserId = (
    await gql<{ me: { id: string } }>(player, `query { me { id } }`, {})
  ).me.id;

  const token = async (x: number, y: number): Promise<string> =>
    (
      await gql<{ createToken: { tokenId: string } }>(
        gm,
        `mutation ($input: GraphQLCreateTokenInput!) {
          createToken(input: $input) { tokenId }
        }`,
        { input: { sceneId, x, y, tokenType: "character" } },
      )
    ).createToken.tokenId;
  const give = async (tokenId: string, isPrimary: boolean) =>
    gql(
      gm,
      `mutation ($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
        updateToken(tokenId: $tokenId, input: $input) { tokenId }
      }`,
      { tokenId, input: { ownerUserId: playerUserId, isPrimary } },
    );
  const wall = async (x1: number, y1: number, x2: number, y2: number) =>
    (
      await gql<{ createWall: { wallId: string } }>(
        gm,
        `mutation ($input: GraphQLCreateWallInput!) {
          createWall(input: $input) { wallId }
        }`,
        {
          input: {
            sceneId,
            x1,
            y1,
            x2,
            y2,
            blocksVision: true,
            blocksMovement: true,
          },
        },
      )
    ).createWall.wallId;
  const shape = async (page: Page, x: number): Promise<string> =>
    (
      await gql<{ createShape: { shapeId: string } }>(
        page,
        `mutation ($input: GraphQLCreateShapeInput!) {
          createShape(input: $input) { shapeId }
        }`,
        {
          input: {
            sceneId,
            kind: "RECT",
            geometry: { x, y: -150, w: 30, h: 30 },
            visibleToPlayers: true,
          },
        },
      )
    ).createShape.shapeId;

  // A (their primary) and B side by side; C is theirs too, but behind W1
  // where A cannot see it. An NPC, a GM's light, and W2 just below B.
  const a = { x: -125, y: 75 };
  const b = { x: -25, y: 75 };
  const tokenA = await token(a.x, a.y);
  const tokenB = await token(b.x, b.y);
  const tokenC = await token(175, 75);
  const npc = await token(-75, -75);
  await give(tokenA, true);
  await give(tokenB, false);
  await give(tokenC, false);
  await wall(100, -150, 100, 150);
  await wall(-60, 25, 10, 25);
  await gql(
    gm,
    `mutation ($input: GraphQLCreateLightSourceInput!) {
      createLightSource(input: $input) { lightId }
    }`,
    {
      input: {
        sceneId,
        x: -125,
        y: -75,
        radius: 60,
        intensity: 0.8,
        castsShadows: false,
      },
    },
  );
  const theirShape = await shape(player, -40);
  await shape(gm, 20);

  try {
    for (const page of [gm, player]) {
      await page.goto(`/world/${worldId}/play`);
      await waitForEngineReady(page);
      await waitForWallsLoaded(page);
    }
    for (const id of [tokenA, tokenB, npc]) {
      await expect
        .poll(() => boardToken(player, id), { timeout: 15_000 })
        .toBeDefined();
    }

    await test.step("a box over everything takes their two seen tokens and their shape (SC-003)", async () => {
      await expect(async () => {
        await boxBoard(player, { x: -170, y: 160 }, { x: 220, y: -160 });
        await expectHeld(player, {
          tokens: [tokenA, tokenB],
          shapes: [theirShape],
          walls: [],
          lights: [],
        });
      }).toPass({ timeout: 30_000 });
    });

    await test.step("a drag where B's path crosses a wall moves A and leaves B (SC-004)", async () => {
      await expect(async () => {
        await boxBoard(player, { x: -170, y: 120 }, { x: 10, y: 40 });
        await expectHeld(player, { tokens: [tokenA, tokenB], shapes: [] });
      }).toPass({ timeout: 30_000 });

      const before = await serverTokens(gm, sceneId);
      const bBefore = before.find((t) => t.tokenId === tokenB)!;
      await dragBoard(player, a, { x: a.x, y: a.y - 2 * GRID });

      await expect(player.getByText("1 of 2 could not be moved.")).toBeVisible({
        timeout: 15_000,
      });
      await expect
        .poll(async () => await serverToken(gm, sceneId, tokenA), {
          timeout: 15_000,
          message: "A's move reaches the server",
        })
        .toEqual({ x: a.x, y: a.y - 2 * GRID });
      expect(
        await serverToken(gm, sceneId, tokenB),
        "B's move is never sent",
      ).toEqual({ x: bBefore.x, y: bBefore.y });

      for (const page of [player, gm]) {
        await expect
          .poll(async () => await boardToken(page, tokenA), {
            timeout: 15_000,
          })
          .toEqual({ x: a.x, y: a.y - 2 * GRID });
        await expect
          .poll(async () => await boardToken(page, tokenB), {
            timeout: 15_000,
          })
          .toEqual(b);
      }
    });
  } finally {
    await player.context().close();
  }
});
