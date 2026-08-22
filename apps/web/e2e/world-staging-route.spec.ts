import { test, expect, type Page } from "@playwright/test";

/**
 * specs/010-world-staging-actors (US1, US2): the dedicated `/world/:id/staging`
 * route reached from `/welcome`'s "Enter" link, and the DM-only "add NPC"
 * catalog action. `gm-staging-page.spec.ts` covers the staging-vs-canvas
 * route split and role gating in depth (spec 009 lineage); this file
 * focuses on what's new in spec 010: the welcome-hub entry point and NPC
 * creation actually persisting and appearing in the roster.
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

async function extractInviteCode(page: Page): Promise<string> {
  const input = page.locator("input[readonly]").first();
  await expect(input).toBeVisible({ timeout: 10_000 });
  const url = await input.inputValue();
  const code = new URL(url).pathname.split("/").pop();
  if (!code) throw new Error(`Could not extract invite code from URL: ${url}`);
  return code;
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

test.describe("US1: DM can add an NPC from the staging roster", () => {
  test("a newly created NPC appears in the roster without a page reload", async ({ page }) => {
    await registerAndCreateWorld(page, `E2E Add NPC ${uniqueSuffix()}`);

    await expect(page.getByText("No NPCs yet.")).toBeVisible({ timeout: 10_000 });

    const npcName = `Bo Jangles ${uniqueSuffix()}`;
    await page.getByPlaceholder("New NPC name").fill(npcName);
    await page.getByRole("button", { name: "Add NPC" }).click();

    await expect(page.getByText(npcName)).toBeVisible({ timeout: 10_000 });
    // Confirms this was a live re-fetch, not a full navigation/reload.
    await expect(page.getByTestId("world-staging-page")).toBeVisible();
  });

  test("the add-NPC control is not shown to a non-DM player", async ({ page, browser }) => {
    const worldName = `E2E No Add NPC ${uniqueSuffix()}`;
    const worldId = await registerAndCreateWorld(page, worldName);

    await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.goto(`/world/${worldId}`);
    await page.getByRole("button", { name: "Generate Join Link" }).click();
    const inviteCode = await extractInviteCode(page);
    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    try {
      await register(playerPage, freshCredentials("e2estagingnoadd"));
      await playerPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
      await playerPage.locator("#world-name").fill(`E2E Player Own ${uniqueSuffix()}`);
      await playerPage.getByRole("button", { name: /create world/i }).click();
      await playerPage.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });

      await playerPage.goto(`/join/${inviteCode}`);
      await playerPage.getByRole("button", { name: "Join Campaign" }).click();
      await playerPage.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 });

      await playerPage.goto(`/world/${worldId}/staging`);
      await expect(playerPage.getByTestId("world-staging-page")).toBeVisible({
        timeout: 15_000,
      });
      await expect(playerPage.getByPlaceholder("New NPC name")).toHaveCount(0);
      await expect(playerPage.getByRole("button", { name: "Add NPC" })).toHaveCount(0);
    } finally {
      await playerContext.close();
    }
  });
});
