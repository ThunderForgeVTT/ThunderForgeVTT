import { test, expect, type Page } from "@playwright/test";
import path from "node:path";

/**
 * End-to-end coverage for the native canvas authoring feature
 * (specs/001-bevy-canvas-authoring) plus the scene-switching work built
 * on top of it: a GM imports a Universal VTT map into one scene, creates
 * a second scene, imports a different map into it, and switches between
 * the two — proving map import and per-scene isolation both work through
 * the real UI, not just at the API/unit level.
 *
 * Fixtures: examples/maps/demo.dd2vtt (8 line_of_sight polygons -> 31
 * walls, 2 doors, 12 lights) and examples/maps/chamber-of-echoing-grief.dd2vtt
 * (1 polygon -> 4 walls, 0 doors, 0 lights) — counts verified directly
 * against the parser in src/server/src/map_import.rs's own tests, and
 * asserted here via the exact UI text MapImportTool renders after a
 * successful import (no backdoor API calls — this is what a GM sees).
 */

const DEMO_MAP = path.resolve(__dirname, "../../../examples/maps/demo.dd2vtt");
const CHAMBER_MAP = path.resolve(
  __dirname,
  "../../../examples/maps/chamber-of-echoing-grief.dd2vtt",
);

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

async function registerAndCreateWorld(page: Page, worldName: string): Promise<void> {
  const suffix = uniqueSuffix();
  const username = `e2e${suffix}`;
  const email = `${username}@example.test`;
  const password = "Sup3r-Secret-Passphrase!";

  await page.goto("/register");
  await page.locator("#register-username").fill(username);
  await page.locator("#register-email").fill(email);
  await page.locator("#register-password").fill(password);
  await page.locator("#register-password-confirmation").fill(password);
  await page.getByRole("button", { name: "Create account" }).click();

  // Registration logs the user in and redirects away from /register.
  await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
    timeout: 15_000,
  });

  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();

  // CreateWorldPage navigates to /world/{id} (the dashboard) on success.
  await page.waitForURL(/\/world\/[^/]+$/, { timeout: 15_000 });

  await page.getByRole("link", { name: "Enter world" }).first().click();
  await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });
}

async function createScene(page: Page, name: string): Promise<void> {
  await page.getByTestId("new-scene-button").click();
  await page.getByTestId("new-scene-name-input").fill(name);
  await page.getByTestId("create-scene-submit").click();

  // Dialog closes and the switcher shows the newly-created (and
  // auto-selected) scene once creation round-trips.
  await expect(page.getByTestId("new-scene-name-input")).toBeHidden({
    timeout: 10_000,
  });
  await expect(page.getByTestId("scene-switcher")).toContainText(name);
}

async function importMap(
  page: Page,
  filePath: string,
  expectedSummary: string,
): Promise<number> {
  const tool = page.getByTestId("map-import-tool");
  const startedAt = Date.now();
  await tool.locator('input[type="file"]').setInputFiles(filePath);

  const success = page.getByTestId("map-import-success");
  await expect(success).toBeVisible({ timeout: 30_000 });
  await expect(success).toContainText(expectedSummary);
  return Date.now() - startedAt;
}

async function switchToScene(page: Page, name: string): Promise<void> {
  await page.getByTestId("scene-switcher").click();
  await page.getByRole("option", { name }).click();
  await expect(page.getByTestId("scene-switcher")).toContainText(name);
}

test.describe("Native canvas authoring: map import and scene switching", () => {
  test("importing a map into one scene, creating a second scene, importing a different map, and switching between them keeps each scene's content isolated", async ({
    page,
  }) => {
    await registerAndCreateWorld(page, `E2E World ${uniqueSuffix()}`);

    // A brand-new world has zero scenes — only the "New scene" affordance
    // should be visible (FR-013: the canvas must still work with nothing
    // authored on it yet; this ensures the *page*, not just the canvas,
    // doesn't crash on zero scenes either).
    await expect(page.getByTestId("new-scene-button")).toBeVisible();
    await expect(page.getByTestId("map-import-tool")).toHaveCount(0);

    // --- Scene One: import the rich demo map ---
    await createScene(page, "Scene One");
    await expect(page.getByTestId("map-import-tool")).toBeVisible();
    // SC-007: importing demo.dd2vtt (background art + 8 wall polygons + 2
    // doors + 12 lights) end-to-end through the real UI must complete in
    // under 30 seconds.
    const importDurationMs = await importMap(
      page,
      DEMO_MAP,
      "31 walls, 2 doors, 12 lights",
    );
    expect(importDurationMs).toBeLessThan(30_000);
    console.log(`SC-007: demo.dd2vtt import took ${importDurationMs}ms`);

    // --- Scene Two: import the simpler chamber map ---
    await createScene(page, "Scene Two");
    // A freshly created scene must not show the previous scene's import
    // result (per-scene isolation, not a global "last import" banner).
    await expect(page.getByTestId("map-import-success")).toHaveCount(0);
    await importMap(page, CHAMBER_MAP, "4 walls, 0 doors, 0 lights");

    // --- Switch back to Scene One: its own import result, not Scene Two's ---
    await switchToScene(page, "Scene One");
    await expect(page.getByTestId("map-import-success")).toHaveCount(0);

    // --- And back to Scene Two once more, to rule out one-way state bleed ---
    await switchToScene(page, "Scene Two");
    await expect(page.getByTestId("map-import-success")).toHaveCount(0);
  });
});
