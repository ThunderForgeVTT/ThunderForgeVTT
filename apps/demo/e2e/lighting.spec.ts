import { expect, test, type Page } from "@playwright/test";
import { activeSceneId, data, enterPlay, openDemo } from "./support";

/**
 * Spec 074 / spec 045: the Lights panel's two scene controls answer in the
 * demo as on a server. The scene's light is set and stays set across a
 * reload; exploration is turned on, reset, and stays on.
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

async function openLights(): Promise<void> {
  await page.getByTestId("gm-tool-lights").click();
  await expect(page.getByTestId("gm-tool-panel-lights")).toBeVisible();
}

async function sceneLight(sceneId: string): Promise<string> {
  const { scene } = await data<{ scene: { ambientLight: string } }>(
    page,
    "query ($id: UUID!) { scene(sceneId: $id) { ambientLight } }",
    { id: sceneId },
  );
  return scene.ambientLight;
}

test("the Game Master darkens the scene, and it is still dark after a reload", async () => {
  await enterPlay(page);
  const sceneId = await activeSceneId(page);
  await openLights();
  const next = (await sceneLight(sceneId)) === "dark" ? "dim" : "dark";
  await page.getByTestId(`scene-ambient-${next}`).click();
  await expect(page.getByTestId(`scene-ambient-${next}`)).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  await expect.poll(() => sceneLight(sceneId)).toBe(next);

  await enterPlay(page);
  await openLights();
  await expect(page.getByTestId(`scene-ambient-${next}`)).toHaveAttribute(
    "aria-pressed",
    "true",
  );
});

test("the Game Master turns exploration on and resets it, and it stays on", async () => {
  const sceneId = await activeSceneId(page);
  const read = async () =>
    (
      await data<{
        sceneExploration: { enabled: boolean; epoch: number };
      }>(
        page,
        "query ($s: UUID!) { sceneExploration(sceneId: $s) { enabled epoch } }",
        { s: sceneId },
      )
    ).sceneExploration;
  expect((await read()).enabled).toBe(false);

  const toggle = page.getByTestId("scene-exploration-toggle");
  await expect(toggle).toHaveText("Not remembering");
  await toggle.click();
  await expect(toggle).toHaveText("Remembering");
  await expect.poll(async () => (await read()).enabled).toBe(true);

  await page.getByTestId("scene-exploration-reset-all").click();
  await expect.poll(async () => (await read()).epoch).toBe(1);

  await enterPlay(page);
  await openLights();
  await expect(page.getByTestId("scene-exploration-toggle")).toHaveText(
    "Remembering",
  );
});
