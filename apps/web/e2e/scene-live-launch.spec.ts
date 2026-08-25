import { expect, test } from "@playwright/test";
import {
  ensureSidebarOpen,
  freshCredentials,
  inviteAndJoinAsPlayer,
  launchSceneByName,
  register,
} from "./fixtures/helpers";

/**
 * Spec 022 (User Story 1, FR-002a/FR-002b, ADR-046, SC-006): launching a
 * scene from the Scenes section live-switches everyone already in Play —
 * the actual point of making the active scene server-authoritative
 * instead of purely client-local. Two independent browser contexts (GM +
 * player) stand in for two real people at the table.
 */

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

test("launching a different scene live-switches every member already in Play, with no manual rejoin", async ({
  browser,
}) => {
  // `CampaignSettingsPanel`'s invite-generation flow also attempts a
  // clipboard write (see system-settings.spec.ts's identical setup) — the
  // default context has no clipboard permission, which throws and stops
  // that handler before it stores the new invite in state.
  const gmContext = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const gmPage = await gmContext.newPage();
  await register(gmPage, freshCredentials("e2elivelaunch"));
  await gmPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
  const worldName = `E2E Live Launch ${uniqueSuffix()}`;
  await gmPage.locator("#world-name").fill(worldName);
  await gmPage.getByRole("button", { name: /create world/i }).click();
  await gmPage.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const worldId = /\/world\/([^/]+)\/staging$/.exec(new URL(gmPage.url()).pathname)?.[1];
  if (!worldId) throw new Error("Could not extract world id");

  const playerPage = await inviteAndJoinAsPlayer(browser, gmPage, worldId);

  // Create and un-hide a second scene the GM will launch.
  const secondSceneName = `Second Scene ${uniqueSuffix()}`;
  await gmPage.goto(`/world/${worldId}/scenes`);
  await gmPage.getByTestId("new-scene-name-input").fill(secondSceneName);
  await gmPage.getByTestId("add-scene-button").click();
  await expect(gmPage.getByRole("link", { name: secondSceneName })).toBeVisible({ timeout: 10_000 });
  await gmPage.getByRole("link", { name: secondSceneName }).click();
  await gmPage.waitForURL(new RegExp(`/world/${worldId}/scenes/[^/]+$`), { timeout: 10_000 });
  await gmPage.getByTestId("scene-hidden-toggle").click();
  await expect(gmPage.getByTestId("scene-hidden-toggle")).toBeChecked({ timeout: 10_000 });

  // Both GM and player enter Play — world creation already auto-launched
  // the world's default scene (spec 010 FR-004 reconciled with FR-002d),
  // so both land on that scene, not an empty canvas.
  await gmPage.goto(`/world/${worldId}/play`);
  await playerPage.goto(`/world/${worldId}/play`);
  await expect(gmPage.locator("canvas")).toBeVisible({ timeout: 15_000 });
  await expect(playerPage.locator("canvas")).toBeVisible({ timeout: 15_000 });

  // GM launches the second scene from the Scenes section — not from
  // within Play itself — while both tabs are sitting in Play.
  await launchSceneByName(gmPage, worldId, secondSceneName);

  // The player's tab never navigated away from Play — this is the actual
  // live-broadcast assertion (FR-002b/SC-006): it must reflect the switch
  // via the open WebSocket subscription, with no manual refresh/rejoin.
  await ensureSidebarOpen(playerPage);
  await expect(playerPage.getByTestId("scene-switcher")).toContainText(secondSceneName, {
    timeout: 15_000,
  });

  // Sanity check that Launch itself actually persisted (a fresh visit,
  // not a live-sync assertion — the GM necessarily left Play to launch).
  await gmPage.goto(`/world/${worldId}/play`);
  await expect(gmPage.locator("canvas")).toBeVisible({ timeout: 15_000 });
  await ensureSidebarOpen(gmPage);
  await expect(gmPage.getByTestId("scene-switcher")).toContainText(secondSceneName, {
    timeout: 15_000,
  });
});
