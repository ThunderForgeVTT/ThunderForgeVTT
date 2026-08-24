import { test, expect, type Page } from "@playwright/test";

/**
 * specs/014-dice-rolling-engine (US4): triggering a roll from the play
 * canvas produces a real result (server-authoritative, per US1) and, for
 * the DM, the roll shows up afterward in worldRollRecords history.
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
  await register(page, freshCredentials("e2edice"));
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

test("triggering a roll shows a result, and the DM sees it in roll history afterward", async ({ page }) => {
  const worldName = `E2E Dice ${uniqueSuffix()}`;
  const worldId = await registerAndCreateWorld(page, worldName);

  await page.getByTestId("play-button").click();
  await page.waitForURL(new RegExp(`/world/${worldId}/play$`), { timeout: 15_000 });
  await expect(page.locator("canvas")).toBeVisible({ timeout: 20_000 });

  const panel = page.getByTestId("dice-roller-panel");
  await expect(panel).toBeVisible({ timeout: 15_000 });
  await page.getByTestId("dice-formula-input").fill("1d20");
  await page.getByTestId("dice-roll-button").click();

  await expect(page.getByTestId("dice-roll-result")).toBeVisible({ timeout: 10_000 });
  const resultText = await page.getByTestId("dice-roll-result").innerText();
  expect(resultText).toMatch(/1d20:\s*-?\d+/);

  // Confirm the DM's roll history shows the matching record via GraphQL
  // (worldRollRecords, DM-only) — a lightweight check against the live
  // session's own cookies rather than adding a full history UI.
  const historyResponse = await page.evaluate(async (wid) => {
    const csrfToken = document.cookie
      .split(";")
      .map((part) => part.trim())
      .find((part) => part.startsWith("csrf_token="))
      ?.slice("csrf_token=".length);
    const res = await fetch("/api/graphql", {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "Content-Type": "application/json",
        ...(csrfToken ? { "x-csrf-token": csrfToken } : {}),
      },
      body: JSON.stringify({
        query: `query($worldId: UUID!) { worldRollRecords(worldId: $worldId, limit: 5) { id resolution { formula resultValue } } }`,
        variables: { worldId: wid },
      }),
    });
    const text = await res.text();
    try {
      return JSON.parse(text);
    } catch {
      throw new Error(`Non-JSON response (status ${res.status}): ${text.slice(0, 500)}`);
    }
  }, worldId);

  expect(historyResponse.data.worldRollRecords.length).toBeGreaterThan(0);
  expect(historyResponse.data.worldRollRecords[0].resolution.formula).toBe("1d20");
});
