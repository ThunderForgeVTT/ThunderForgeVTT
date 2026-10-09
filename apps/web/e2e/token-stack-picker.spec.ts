import { expect, test, type Page } from "./fixtures/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * A stack of tokens can be taken apart: a click picks up the whole pile, a
 * double-click asks which one, and the token chosen there is the one the
 * next drag moves. The rest stay where they were.
 *
 * Owner's report: a double-click opened the picker, but picking a token did
 * not take. The picker narrowed the store's selection and the engine never
 * heard, so the next press picked up the whole pile again.
 *
 * # Why this is an e2e and not a unit test
 *
 * The defect sat between layers. The picker is React, the selection is the
 * world store, and the press that picks tokens up is the engine. Each one was
 * right on its own. Only a browser shows the store's answer reaching the
 * engine. Positions are read back from the server, because a token that only
 * looked moved has not moved.
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

/** The canvas centre, in page pixels — where the world origin is drawn. */
async function canvasCentre(page: Page): Promise<{ x: number; y: number }> {
  const box = await page.locator("canvas").boundingBox();
  if (!box) throw new Error("the canvas must be laid out before it is used");
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

/** A click with a frame between press and release; see
 * `status-placement.spec.ts` for why `page.mouse.click()` is not enough. */
async function clickCanvasAt(
  page: Page,
  dx: number,
  dy: number,
): Promise<void> {
  const centre = await canvasCentre(page);
  await page.mouse.move(centre.x + dx, centre.y + dy);
  await page.mouse.down();
  await page.waitForTimeout(80);
  await page.mouse.up();
}

/** The store's selection, topmost first. */
async function selectedIds(page: Page): Promise<string[]> {
  return page.evaluate(
    () =>
      (
        window as unknown as {
          __worldProbe?: { state: () => { selectedTokenIds: string[] } };
        }
      ).__worldProbe?.state().selectedTokenIds ?? [],
  );
}

async function storeTokenCount(page: Page): Promise<number> {
  return page.evaluate(
    () =>
      (
        window as unknown as {
          __worldProbe?: { state: () => { tokenIds: string[] } };
        }
      ).__worldProbe?.state().tokenIds.length ?? 0,
  );
}

interface ServerToken {
  tokenId: string;
  x: number;
  y: number;
}

async function serverTokens(
  page: Page,
  sceneId: string,
): Promise<ServerToken[]> {
  const data = await gql<{ tokens: ServerToken[] }>(
    page,
    `query ($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId x y } }`,
    { sceneId },
  );
  return data.tokens;
}

/** A table open on the board with two tokens stacked on the origin. */
async function boardWithAStack(
  page: Page,
): Promise<{ sceneId: string; tokenIds: string[] }> {
  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Stack ${suffix}`);
  const active = await gql<{ world: { activeSceneId: string | null } }>(
    page,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(page, worldId);
  const sceneId = active.world.activeSceneId ?? firstScene;

  const tokenIds: string[] = [];
  for (let i = 0; i < 2; i += 1) {
    const created = await gql<{ createToken: { tokenId: string } }>(
      page,
      `mutation ($input: GraphQLCreateTokenInput!) {
        createToken(input: $input) { tokenId }
      }`,
      { input: { sceneId, x: 0, y: 0, tokenType: "npc" } },
    );
    tokenIds.push(created.createToken.tokenId);
  }

  await page.goto(`/world/${worldId}/play`);
  await waitForEngineReady(page);
  return { sceneId, tokenIds };
}

test("a token picked from a stack is the one the next drag moves, the pile can be asked again, and the count says how many are selected", async ({
  page,
}) => {
  test.setTimeout(4 * 60_000);
  page.on("pageerror", (error) => {
    console.log(`[browser] uncaught: ${error.message}`);
  });

  const { sceneId, tokenIds } = await boardWithAStack(page);
  await expect.poll(() => storeTokenCount(page), { timeout: 60_000 }).toBe(2);

  // A click on the pile picks up all of it. Each attempt starts from an
  // empty selection: a press on a pile holding the one selected token takes
  // only that token, so a click that landed while only one token had loaded
  // would otherwise stick.
  await expect
    .poll(
      async () => {
        await clickCanvasAt(page, 320, 240);
        await page.waitForTimeout(150);
        await clickCanvasAt(page, 0, 0);
        return (await selectedIds(page)).length;
      },
      {
        message: "a click on the stack should select both tokens",
        timeout: 60_000,
        intervals: [1_000],
      },
    )
    .toBe(2);
  const stack = await selectedIds(page);
  expect([...stack].sort()).toEqual([...tokenIds].sort());

  const count = page.getByTestId("selection-count");
  await expect(count).toHaveText("2 selected");

  // A double-click asks which one. Take the lower one.
  const centre = await canvasCentre(page);
  await page.mouse.dblclick(centre.x, centre.y);
  await expect(page.getByTestId("token-stack-picker")).toBeVisible({
    timeout: 10_000,
  });
  const [top, lower] = stack;
  await page.getByTestId(`token-stack-option-${lower}`).click();
  await expect(page.getByTestId("token-stack-picker")).toHaveCount(0);
  await expect.poll(() => selectedIds(page)).toEqual([lower]);
  await expect(count, "one token selected needs no count").toHaveCount(0);

  // A second double-click on the same pile asks again, rather than pinning
  // the status panel of the one token the picker chose. The owner's report:
  // after a pick, the pile could only be asked about again once empty board
  // had been clicked. Change the answer to the other token.
  await page.mouse.dblclick(centre.x, centre.y);
  await expect(
    page.getByTestId("token-stack-picker"),
    "a double-click on a pile of two should open the picker again",
  ).toBeVisible({ timeout: 10_000 });
  await expect(page.getByTestId(`token-stack-option-${top}`)).toBeVisible();
  await expect(page.getByTestId(`token-stack-option-${lower}`)).toBeVisible();
  await page.getByTestId(`token-stack-option-${top}`).click();
  await expect(page.getByTestId("token-stack-picker")).toHaveCount(0);
  await expect.poll(() => selectedIds(page)).toEqual([top]);

  // Where the server has both before the drag. Read rather than assumed:
  // the clicks that picked the pile up may have snapped it into its cell.
  const position = (tokens: ServerToken[], id: string) => {
    const token = tokens.find((candidate) => candidate.tokenId === id);
    return token ? { x: Math.round(token.x), y: Math.round(token.y) } : null;
  };
  const before = await serverTokens(page, sceneId);
  const pickedBefore = position(before, top);
  const leftBefore = position(before, lower);
  expect(
    leftBefore,
    "the token left on the pile is on the server",
  ).not.toBeNull();
  expect(pickedBefore, "the picked token is on the server").not.toBeNull();

  // Drag the pile. Only the picked token should come away.
  await page.mouse.move(centre.x, centre.y);
  await page.mouse.down();
  await page.waitForTimeout(80);
  await page.mouse.move(centre.x + 150, centre.y, { steps: 12 });
  await page.waitForTimeout(80);
  await page.mouse.up();

  await expect
    .poll(
      async () =>
        (position(await serverTokens(page, sceneId), top)?.x ?? 0) -
        (pickedBefore?.x ?? 0),
      {
        message: "the picked token should have moved, on the server",
        timeout: 30_000,
      },
    )
    .toBeGreaterThan(20);

  // And the one left behind has not: not with the drag, and not a moment
  // later, as a second move of a stack arriving late would.
  await page.waitForTimeout(1_500);
  expect(
    position(await serverTokens(page, sceneId), lower),
    "the token left on the pile should not have moved",
  ).toEqual(leftBefore);
});

/**
 * Which of a character (blue) and an NPC (red) the canvas shows at a page
 * point, read from a screenshot rather than from the engine: the claim is
 * about what the player sees. Pixels that are neither — grid lines, the
 * board — are left out, and the colour more of the rest shows wins.
 */
async function kindShownAt(
  page: Page,
  at: { x: number; y: number },
): Promise<"character" | "npc" | null> {
  const half = 8;
  const png = await page.screenshot({
    clip: { x: at.x - half, y: at.y - half, width: half * 2, height: half * 2 },
  });
  return page.evaluate(async (base64: string) => {
    const response = await fetch(`data:image/png;base64,${base64}`);
    const bitmap = await createImageBitmap(await response.blob());
    const surface = new OffscreenCanvas(bitmap.width, bitmap.height);
    const context = surface.getContext("2d");
    if (!context) throw new Error("OffscreenCanvas 2D context unavailable");
    context.drawImage(bitmap, 0, 0);
    const { data } = context.getImageData(0, 0, bitmap.width, bitmap.height);
    let red = 0;
    let blue = 0;
    for (let i = 0; i < data.length; i += 4) {
      const lean = data[i] - data[i + 2];
      if (lean > 60) red += 1;
      else if (lean < -60) blue += 1;
    }
    if (red === 0 && blue === 0) return null;
    return red > blue ? "npc" : "character";
  }, png.toString("base64"));
}

/** A world point's offset from the canvas centre, in page pixels. One
 * screen pixel is `scale` world units, and world y grows upward. */
async function pageOffsetOf(
  page: Page,
  world: { x: number; y: number },
): Promise<{ dx: number; dy: number }> {
  const camera = await page.evaluate(
    () =>
      (
        window as unknown as {
          __engineProbe?: {
            camera: () => { x: number; y: number; scale: number } | null;
          };
        }
      ).__engineProbe?.camera() ?? null,
  );
  if (!camera) throw new Error("the engine probe should report the camera");
  return {
    dx: (world.x - camera.x) / camera.scale,
    dy: -(world.y - camera.y) / camera.scale,
  };
}

test("a click on a pile takes the token drawn on top, not the one with the lowest id", async ({
  page,
}) => {
  test.setTimeout(4 * 60_000);

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Pile ${suffix}`);
  const active = await gql<{ world: { activeSceneId: string | null } }>(
    page,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(page, worldId);
  const sceneId = active.world.activeSceneId ?? firstScene;

  const create = async (
    tokenType: string,
    at: { x: number; y: number },
  ): Promise<string> =>
    (
      await gql<{ createToken: { tokenId: string } }>(
        page,
        `mutation ($input: GraphQLCreateTokenInput!) {
          createToken(input: $input) { tokenId }
        }`,
        { input: { sceneId, ...at, tokenType } },
      )
    ).createToken.tokenId;

  // A pile of a character (blue) and an NPC (red). The character is on the
  // board when it opens; the NPC is added while the table watches, the way
  // a Game Master drops one onto a square that is already taken. Ids are
  // time-ordered, so the NPC's sorts last. The picker used to break a tie on
  // z by id and so took the character, while the NPC, added last, was drawn
  // over it.
  const at = { x: 0, y: 0 };
  const character = await create("character", at);

  await page.goto(`/world/${worldId}/play`);
  await waitForEngineReady(page);
  await expect.poll(() => storeTokenCount(page), { timeout: 60_000 }).toBe(1);

  const npc = await create("npc", at);
  expect(npc > character, "a later token's id sorts after").toBe(true);
  await expect.poll(() => storeTokenCount(page), { timeout: 60_000 }).toBe(2);

  // What the player sees on the pile: the NPC, added last, on top.
  const centre = await canvasCentre(page);
  const { dx, dy } = await pageOffsetOf(page, at);
  await expect
    .poll(
      () => kindShownAt(page, { x: centre.x + dx + 4, y: centre.y + dy + 4 }),
      { message: "the token added last is drawn on top", timeout: 30_000 },
    )
    .toBe("npc");

  // A click takes the pile, the one on top first.
  await expect
    .poll(
      async () => {
        await clickCanvasAt(page, 320, 240);
        await page.waitForTimeout(150);
        await clickCanvasAt(page, dx, dy);
        return [...(await selectedIds(page))].sort();
      },
      {
        message: "a click on the pile should take both its tokens",
        timeout: 60_000,
        intervals: [1_000],
      },
    )
    .toEqual([character, npc].sort());
  expect(
    (await selectedIds(page))[0],
    "the token taken first should be the NPC the player sees on top",
  ).toBe(npc);
});
