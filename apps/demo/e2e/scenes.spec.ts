import { expect, test, type Page } from "@playwright/test";
import {
  WORLD_ID,
  activeSceneId,
  data,
  enterPlay,
  openDemo,
  scenes,
} from "./support";

/**
 * Spec 074: the Game Master makes a scene from the Scenes page and adds a
 * level to the board in play, and both are still there after a reload.
 */

let page: Page;
let outside: string[];

test.describe.configure({ mode: "serial" });

test.beforeAll(async ({ browser, baseURL }) => {
  ({ page, outside } = await openDemo(browser, baseURL));
});

test.afterAll(async () => {
  await page.context().close();
});

test.afterEach(() => {
  expect(outside, "nothing leaves the demo's own static files").toEqual([]);
});

test("the Game Master makes a scene, and it is listed after a reload", async () => {
  await page.goto(`/demo/world/${WORLD_ID}/scenes`);
  await page.getByTestId("new-scene-name-input").fill("The Sunken Vault");
  await page.getByTestId("add-scene-button").click();
  const made = async () =>
    (await scenes(page)).find((s) => s.name === "The Sunken Vault");
  await expect.poll(async () => (await made()) !== undefined).toBe(true);
  const scene = await made();

  await page.reload();
  await expect(
    page.getByTestId(`scene-row-${scene?.sceneId ?? ""}`),
  ).toBeVisible();
});

test("the Game Master adds a level, and it is a tab after a reload", async () => {
  await enterPlay(page);
  const sceneId = await activeSceneId(page);
  await page.getByTestId("level-manage-toggle").click();
  await page.getByTestId("level-add-input").fill("Cellar");
  await page.getByTestId("level-add-submit").click();
  await expect(
    page.getByTestId("level-tab").filter({ hasText: "Cellar" }),
  ).toBeVisible();
  const { sceneLevels } = await data<{
    sceneLevels: { name: string; isEntry: boolean }[];
  }>(page, "query ($s: UUID!) { sceneLevels(sceneId: $s) { name isEntry } }", {
    s: sceneId,
  });
  expect(sceneLevels.map((l) => l.name)).toEqual(["Ground", "Cellar"]);

  await enterPlay(page);
  await expect(
    page.getByTestId("level-tab").filter({ hasText: "Cellar" }),
  ).toBeVisible();
});
