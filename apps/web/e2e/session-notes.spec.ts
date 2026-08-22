import { test, expect, type Page } from "@playwright/test";

/**
 * specs/011-world-compendium (US3): Session Setup's "Last Session Notes"
 * panel — a single freeform per-world recap, DM/GM-editable, read-only for
 * everyone else. Also confirms Session Setup's simplified shape (Play,
 * Players, Last Session Notes only — no NPC list, no Lore placeholder).
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
  await register(page, freshCredentials("e2esessnotes"));
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

test("Session Setup shows exactly Play, Players, and Last Session Notes", async ({ page }) => {
  await registerAndCreateWorld(page, `E2E Session Shape ${uniqueSuffix()}`);

  await expect(page.getByTestId("play-button")).toBeVisible();
  await expect(page.getByText("Players")).toBeVisible();
  await expect(page.getByTestId("session-notes-panel")).toBeVisible();
  await expect(page.getByText("Lore — coming soon")).toHaveCount(0);
  await expect(page.getByPlaceholder("New NPC name")).toHaveCount(0);
});

test("DM edits and saves Last Session Notes; a Player sees it read-only", async ({
  page,
  browser,
}) => {
  const worldName = `E2E Session Notes ${uniqueSuffix()}`;
  const worldId = await registerAndCreateWorld(page, worldName);

  const notesText = `We defeated the goblin ambush ${uniqueSuffix()}`;
  await page.getByTestId("session-notes-textarea").fill(notesText);
  await page.getByTestId("session-notes-save-button").click();
  await expect(page.getByText("Saved.")).toBeVisible({ timeout: 10_000 });

  // Persists across reload.
  await page.reload();
  await expect(page.getByTestId("session-notes-textarea")).toHaveValue(notesText);

  // A Player sees the same text, read-only (no textarea/save control).
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto(`/world/${worldId}`);
  await page.getByRole("button", { name: "Generate Join Link" }).click();
  const inviteInput = page.locator("input[readonly]");
  await expect(inviteInput).toBeVisible({ timeout: 10_000 });
  const inviteRelativePath = new URL(await inviteInput.inputValue()).pathname;

  const playerContext = await browser.newContext();
  const playerPage = await playerContext.newPage();
  try {
    await register(playerPage, freshCredentials("e2esessnotesplyr"));
    await playerPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
    await playerPage.goto(inviteRelativePath);
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 });

    await playerPage.goto(`/world/${worldId}/staging`);
    await expect(playerPage.getByTestId("session-notes-readonly")).toContainText(notesText);
    await expect(playerPage.getByTestId("session-notes-textarea")).toHaveCount(0);
    await expect(playerPage.getByTestId("session-notes-save-button")).toHaveCount(0);
  } finally {
    await playerContext.close();
  }
});

test("saving an empty value is a valid save, not an error", async ({ page }) => {
  await registerAndCreateWorld(page, `E2E Session Notes Empty ${uniqueSuffix()}`);

  // First, save some text so there's something to clear.
  await page.getByTestId("session-notes-textarea").fill("Something to clear");
  await page.getByTestId("session-notes-save-button").click();
  await expect(page.getByText("Saved.")).toBeVisible({ timeout: 10_000 });

  // Now clear it and save again.
  await page.getByTestId("session-notes-textarea").fill("");
  await page.getByTestId("session-notes-save-button").click();
  await expect(page.getByText("Saved.")).toBeVisible({ timeout: 10_000 });

  await page.reload();
  await expect(page.getByTestId("session-notes-textarea")).toHaveValue("");
});
