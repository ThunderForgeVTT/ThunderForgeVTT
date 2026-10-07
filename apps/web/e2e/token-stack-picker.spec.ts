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
  expect(leftBefore, "the token left on the pile is on the server").not.toBeNull();
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
