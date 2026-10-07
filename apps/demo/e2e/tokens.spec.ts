import { expect, test, type Page } from "@playwright/test";
import { activeSceneId, data, enterPlay, openDemo } from "./support";

/**
 * Spec 074: a token takes its size and its sight from its sheet, in the demo
 * as on a server. The Large dire wolf stands on four squares and the wizard's
 * 60 ft of darkvision reaches the engine as twelve of the scene's squares.
 */

let page: Page;
let outside: string[];

test.describe.configure({ mode: "serial" });

test.beforeAll(async ({ browser, baseURL }) => {
  ({ page, outside } = await openDemo(browser, baseURL));
  await enterPlay(page);
});

test.afterAll(async () => {
  await page.context().close();
});

test.afterEach(() => {
  expect(outside, "nothing leaves the demo's own static files").toEqual([]);
});

interface Probe {
  tokenFootprints?: () => { tokenId: string; footprint: number }[];
  tokenVision?: (tokenId: string) => number | null;
}

test("the Large dire wolf covers two squares by two", async () => {
  const sceneId = await activeSceneId(page);
  const { tokenGrid } = await data<{
    tokenGrid: { tokenId: string; footprint: number }[];
  }>(
    page,
    "query ($s: UUID!) { tokenGrid(sceneId: $s) { tokenId footprint } }",
    { s: sceneId },
  );
  expect(tokenGrid).toHaveLength(1);
  const wolf = tokenGrid[0].tokenId;
  await expect
    .poll(() =>
      page.evaluate(
        (id) =>
          (window as unknown as { __engineProbe?: Probe }).__engineProbe
            ?.tokenFootprints?.()
            .find((t) => t.tokenId === id)?.footprint ?? null,
        wolf,
      ),
    )
    .toBe(2);
});

test("the wizard's darkvision reaches the engine as twelve squares", async () => {
  const sceneId = await activeSceneId(page);
  const { tokenVision, scene } = await data<{
    tokenVision: { tokenId: string; darkvision: number }[];
    scene: { gridSize: number };
  }>(
    page,
    `query ($s: UUID!) {
      tokenVision(sceneId: $s) { tokenId darkvision }
      scene(sceneId: $s) { gridSize }
    }`,
    { s: sceneId },
  );
  const sixtyFeet = 12 * scene.gridSize;
  const wizard = tokenVision.find((v) => v.darkvision === sixtyFeet);
  expect(wizard, "a token sees 60 ft in the dark").toBeDefined();
  const wizardId = wizard?.tokenId ?? "";
  await expect
    .poll(() =>
      page.evaluate(
        (id) =>
          (
            window as unknown as { __engineProbe?: Probe }
          ).__engineProbe?.tokenVision?.(id) ?? null,
        wizardId,
      ),
    )
    .toBe(sixtyFeet);
});
