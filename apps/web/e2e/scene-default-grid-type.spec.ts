import { expect, test } from "@playwright/test";
import { freshCredentials, graphql, register } from "./fixtures/helpers";

/**
 * Spec 022 (User Story 3, P3, FR-014/FR-015/FR-016): a world-level default
 * grid type seeds every newly created scene, without the GM re-selecting
 * it. Grid type isn't surfaced anywhere in the Scenes UI itself (there's
 * no reason for a GM to see it there), so this verifies the created
 * scene's `gridType` directly via GraphQL — same pattern the genie-*
 * specs already use for fields the UI doesn't render.
 */

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

async function gridTypeOfScene(
  page: import("@playwright/test").Page,
  worldId: string,
  sceneName: string,
): Promise<string | undefined> {
  const result = await graphql<{ data?: { scenes: { name: string; gridType: string }[] } }>(
    page,
    `query ($worldId: UUID!) { scenes(worldId: $worldId) { name gridType } }`,
    { worldId },
  );
  return result.data?.scenes.find((s) => s.name === sceneName)?.gridType;
}

test("a scene created after the world default changes inherits that default, without explicit selection", async ({
  page,
}) => {
  await register(page, freshCredentials("e2egridtype"));
  await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
  const worldName = `E2E Default Grid Type ${uniqueSuffix()}`;
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const worldId = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname)?.[1];
  if (!worldId) throw new Error("Could not extract world id");

  // Set the world default to Hexagons.
  await page.goto(`/world/${worldId}/settings/system`);
  await page.getByTestId("default-scene-grid-type-picker").click();
  await page.getByRole("option", { name: "Hexagons" }).click();

  // Create a scene without touching grid type anywhere.
  const hexSceneName = `Hex Scene ${uniqueSuffix()}`;
  await page.goto(`/world/${worldId}/scenes`);
  await page.getByTestId("new-scene-name-input").fill(hexSceneName);
  await page.getByTestId("add-scene-button").click();
  await expect(page.getByRole("link", { name: hexSceneName })).toBeVisible({ timeout: 10_000 });

  expect(await gridTypeOfScene(page, worldId, hexSceneName)).toBe("hex");

  // Switch the default to None and create another scene.
  await page.goto(`/world/${worldId}/settings/system`);
  await page.getByTestId("default-scene-grid-type-picker").click();
  await page.getByRole("option", { name: "None" }).click();

  const gridlessSceneName = `Gridless Scene ${uniqueSuffix()}`;
  await page.goto(`/world/${worldId}/scenes`);
  await page.getByTestId("new-scene-name-input").fill(gridlessSceneName);
  await page.getByTestId("add-scene-button").click();
  await expect(page.getByRole("link", { name: gridlessSceneName })).toBeVisible({ timeout: 10_000 });

  expect(await gridTypeOfScene(page, worldId, gridlessSceneName)).toBe("gridless");

  // The earlier Hex scene's grid type is unaffected by the later default change (FR-016).
  expect(await gridTypeOfScene(page, worldId, hexSceneName)).toBe("hex");
});
