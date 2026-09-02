import { test, expect, type Page } from "@playwright/test";

/**
 * specs/010-world-staging-actors (US1): the dedicated `/world/:id/staging`
 * route reached from `/welcome`'s "Enter" link. `gm-staging-page.spec.ts`
 * covers the staging-vs-canvas route split and role gating in depth (spec
 * 009 lineage); this file focuses on what was new in spec 010: the
 * welcome-hub entry point.
 *
 * Spec 011 relocated the NPC catalog (add-NPC control, roster) off this
 * page entirely, onto the dedicated `/world/:id/compendium` route — see
 * `world-compendium.spec.ts` for that coverage (including the DM-only
 * add-NPC gate this file used to test here).
 */

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

interface Credentials {
  username: string;
  email: string;
  password: string;
}

function freshCredentials(prefix: string): Credentials {
  const suffix = uniqueSuffix();
  const username = `${prefix}${suffix}`;
  return {
    username,
    email: `${username}@example.test`,
    password: "Sup3r-Secret-Passphrase!",
  };
}

async function register(page: Page, creds: Credentials): Promise<void> {
  await page.goto("/register");
  await page.locator("#register-username").fill(creds.username);
  await page.locator("#register-email").fill(creds.email);
  await page.locator("#register-password").fill(creds.password);
  await page.locator("#register-password-confirmation").fill(creds.password);
  await page.getByRole("button", { name: "Create account" }).click();
}

async function registerAndCreateWorld(page: Page, worldName: string): Promise<string> {
  await register(page, freshCredentials("e2estagingroute"));
  await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const match = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname);
  if (!match) {
    throw new Error(`Could not extract world id from URL: ${page.url()}`);
  }
  return match[1];
}

test.describe("US1: welcome hub 'Enter' goes to staging, not straight to canvas", () => {
  test("clicking Enter for a world lands on /world/:id/staging", async ({ page }) => {
    const worldName = `E2E Welcome Enter ${uniqueSuffix()}`;
    const worldId = await registerAndCreateWorld(page, worldName);

    await page.goto("/welcome");
    await page.getByRole("link", { name: `Enter ${worldName}` }).click();
    await page.waitForURL(new RegExp(`/world/${worldId}/staging$`), { timeout: 15_000 });
    await expect(page.getByTestId("world-staging-page")).toBeVisible();
  });
});

test.describe("Spec 011 regression: staging no longer embeds the NPC catalog", () => {
  test("staging shows a Compendium link instead of an inline NPC form", async ({ page }) => {
    await registerAndCreateWorld(page, `E2E Staging No NPC ${uniqueSuffix()}`);

    await expect(page.getByTestId("world-nav-npcs")).toBeVisible({ timeout: 10_000 });
    // The authoring entry point, not the old inline form: that form no
    // longer exists anywhere, so asserting its absence would pass vacuously
    // and stop testing that staging has no NPC authoring (spec 031 FR-035).
    await expect(page.getByTestId("new-npc-link")).toHaveCount(0);
    await expect(page.getByText("Lore — coming soon")).toHaveCount(0);
  });
});
