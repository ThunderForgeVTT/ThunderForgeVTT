import path from "node:path";
import { expect, test } from "@playwright/test";
import { freshCredentials, register } from "./fixtures/helpers";

/**
 * Spec 022 (User Story 1, P1/MVP): the Scenes section's GM workflow —
 * create a scene, import a dd2vtt map, write and save a Markdown summary,
 * toggle its visibility, and launch it — entirely without visiting
 * Session Setup, which no longer hosts any scene controls (FR-002).
 */

const DEMO_MAP = path.resolve(__dirname, "../../../examples/maps/demo.dd2vtt");

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

test("GM creates a scene, imports a map, writes a summary, toggles hidden, and launches it — all from the Scenes section", async ({
  page,
}) => {
  // Playwright's 30-second default cannot cover a real dd2vtt import (see
  // the `map-import-success` wait below), let alone the four other server
  // round trips this walk-through makes after it.
  test.setTimeout(180_000);

  await register(page, freshCredentials("e2escenemgmt"));
  await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
  const worldName = `E2E Scene Management ${uniqueSuffix()}`;
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const worldId = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname)?.[1];
  if (!worldId) throw new Error("Could not extract world id");

  // Session Setup no longer has any scene-management/selection controls
  // (FR-002) — the only thing left touching "scene" is the sidebar nav
  // link into the Scenes section itself.
  await expect(page.getByTestId("scene-switcher")).toHaveCount(0);
  await expect(page.getByTestId("new-scene-name-input")).toHaveCount(0);

  // Create a scene from the Scenes section.
  await page.getByTestId("world-nav-scenes").click();
  await page.waitForURL(`**/world/${worldId}/scenes`, { timeout: 10_000 });
  const sceneName = `Ambush at the Bridge ${uniqueSuffix()}`;
  await page.getByTestId("new-scene-name-input").fill(sceneName);
  await page.getByTestId("add-scene-button").click();
  await expect(page.getByRole("link", { name: sceneName })).toBeVisible({ timeout: 10_000 });

  // New scenes start hidden by default (Clarifications).
  await expect(page.getByText("Hidden").first()).toBeVisible();

  // Open its detail gateway.
  await page.getByRole("link", { name: sceneName }).click();
  await page.waitForURL(new RegExp(`/world/${worldId}/scenes/[^/]+$`), { timeout: 10_000 });
  await expect(page.getByRole("heading", { name: sceneName })).toBeVisible();

  // Import a dd2vtt map.
  await page.getByRole("button", { name: "Import map" }).click();
  await page.setInputFiles('input[type="file"]', DEMO_MAP);
  // 120s, not 20s: a dd2vtt import is a real upload plus wall/door/light
  // extraction, and under suite load it routinely runs past twenty seconds
  // while still succeeding — the failure then reads as "import is broken"
  // when the panel was simply still spinning. canvas-authoring.spec.ts's
  // `importMap` settled on the same budget for the same reason; SC-007's
  // 30-second requirement is asserted there, on the measured duration,
  // rather than smuggled into a locator timeout here.
  await expect(page.getByTestId("map-import-success")).toBeVisible({ timeout: 120_000 });

  // Write and save a Markdown summary.
  const summaryEditor = page.getByTestId("scene-summary-editor").locator(".cm-content");
  await summaryEditor.click();
  await summaryEditor.fill("A rope bridge over a chasm, fog rolling in.");
  await page.getByRole("button", { name: "Save summary" }).click();
  await expect(page.getByText("Summary saved.")).toBeVisible({ timeout: 10_000 });

  // Un-hide it.
  await page.getByTestId("scene-hidden-toggle").click();
  await expect(page.getByTestId("scene-hidden-toggle")).toBeChecked({ timeout: 10_000 });

  // Launch it. Launch moves the table here *and* takes the GM there
  // (spec 031 FR-021), so the navigation is the confirmation.
  await page.getByTestId("launch-scene-button").click();
  await page.waitForURL(/\/world\/[^/]+\/play/, { timeout: 15_000 });
});
