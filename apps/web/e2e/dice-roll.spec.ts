import { test, expect, type Page } from "./fixtures/test";
import { rollD20IntoHistory } from "./fixtures/diceRollProof";

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

async function registerAndCreateWorld(
  page: Page,
  worldName: string,
): Promise<string> {
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

test("triggering a roll shows a result, and the DM sees it in roll history afterward", async ({
  page,
}) => {
  const worldName = `E2E Dice ${uniqueSuffix()}`;
  const worldId = await registerAndCreateWorld(page, worldName);

  await page.getByTestId("play-button").click();
  await page.waitForURL(new RegExp(`/world/${worldId}/play$`), {
    timeout: 15_000,
  });
  await expect(page.locator("canvas")).toBeVisible({ timeout: 20_000 });

  // The proof itself is `fixtures/diceRollProof.ts`, so that the demo can
  // run the same one (spec 074 SC-008).
  await rollD20IntoHistory(page, expect, worldId);
});
