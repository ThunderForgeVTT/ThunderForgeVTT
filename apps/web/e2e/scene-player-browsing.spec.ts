import { expect, test } from "@playwright/test";
import { freshCredentials, inviteAndJoinAsPlayer, register } from "./fixtures/helpers";

/**
 * Spec 022 (User Story 2, P2): players see a table of only the non-hidden
 * scenes, and can open one's detail gateway to read the GM's summary.
 */

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

test("player sees only non-hidden scenes, can preview one, and it disappears again once re-hidden", async ({
  browser,
}) => {
  const gmContext = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const gmPage = await gmContext.newPage();
  await register(gmPage, freshCredentials("e2esceneplayer"));
  await gmPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
  const worldName = `E2E Scene Player Browsing ${uniqueSuffix()}`;
  await gmPage.locator("#world-name").fill(worldName);
  await gmPage.getByRole("button", { name: /create world/i }).click();
  await gmPage.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const worldId = /\/world\/([^/]+)\/staging$/.exec(new URL(gmPage.url()).pathname)?.[1];
  if (!worldId) throw new Error("Could not extract world id");

  const playerPage = await inviteAndJoinAsPlayer(browser, gmPage, worldId);

  // Visible scene: create it, write a summary, un-hide it.
  const visibleName = `Visible Scene ${uniqueSuffix()}`;
  await gmPage.goto(`/world/${worldId}/scenes`);
  await gmPage.getByTestId("new-scene-name-input").fill(visibleName);
  await gmPage.getByTestId("add-scene-button").click();
  await expect(gmPage.getByRole("link", { name: visibleName })).toBeVisible({ timeout: 10_000 });
  await gmPage.getByRole("link", { name: visibleName }).click();
  await gmPage.waitForURL(new RegExp(`/world/${worldId}/scenes/[^/]+$`), { timeout: 10_000 });

  const summaryEditor = gmPage.getByTestId("scene-summary-editor").locator(".cm-content");
  await summaryEditor.click();
  await summaryEditor.fill("A quiet tavern, safe for now.");
  await gmPage.getByRole("button", { name: "Save summary" }).click();
  await expect(gmPage.getByText("Summary saved.")).toBeVisible({ timeout: 10_000 });

  await gmPage.getByTestId("scene-hidden-toggle").click();
  await expect(gmPage.getByTestId("scene-hidden-toggle")).toBeChecked({ timeout: 10_000 });

  // Hidden scene: create it, leave it hidden (the default).
  const hiddenName = `Hidden Scene ${uniqueSuffix()}`;
  await gmPage.goto(`/world/${worldId}/scenes`);
  await gmPage.getByTestId("new-scene-name-input").fill(hiddenName);
  await gmPage.getByTestId("add-scene-button").click();
  await expect(gmPage.getByRole("link", { name: hiddenName })).toBeVisible({ timeout: 10_000 });

  // Player: only the visible scene appears.
  await playerPage.goto(`/world/${worldId}/scenes`);
  await expect(playerPage.getByRole("link", { name: visibleName })).toBeVisible({ timeout: 10_000 });
  await expect(playerPage.getByRole("link", { name: hiddenName })).toHaveCount(0);

  // Player opens the visible scene's detail gateway — read-only summary,
  // no GM controls (hidden toggle, Launch, import).
  await playerPage.getByRole("link", { name: visibleName }).click();
  await playerPage.waitForURL(new RegExp(`/world/${worldId}/scenes/[^/]+$`), { timeout: 10_000 });
  await expect(playerPage.getByTestId("scene-summary-view")).toContainText("A quiet tavern, safe for now.");
  await expect(playerPage.getByTestId("scene-hidden-toggle")).toHaveCount(0);
  await expect(playerPage.getByTestId("launch-scene-button")).toHaveCount(0);
  await expect(playerPage.getByTestId("scene-import-card")).toHaveCount(0);

  // GM re-hides the visible scene.
  await gmPage.goto(`/world/${worldId}/scenes`);
  await gmPage.getByRole("link", { name: visibleName }).click();
  await gmPage.waitForURL(new RegExp(`/world/${worldId}/scenes/[^/]+$`), { timeout: 10_000 });
  await gmPage.getByTestId("scene-hidden-toggle").click();
  await expect(gmPage.getByTestId("scene-hidden-toggle")).not.toBeChecked({ timeout: 10_000 });

  // Player's refreshed table no longer shows it.
  await playerPage.goto(`/world/${worldId}/scenes`);
  await expect(playerPage.getByRole("link", { name: visibleName })).toHaveCount(0);
});
