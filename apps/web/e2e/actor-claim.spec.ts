import { test, expect, type Page } from "@playwright/test";
import { createNpcViaCompendium } from "./fixtures/content";

/**
 * Spec 017 (Player Onboarding — Invite-to-Actor Selection): US1 (GM-
 * designated claiming), US2 (player-created characters), and US3 (GM
 * availability/un-claim management), end-to-end through the real app.
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
  await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
    timeout: 15_000,
  });
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
  await register(page, freshCredentials("e2eclaimgm"));
  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const match = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname);
  if (!match) {
    throw new Error(`Could not extract world id from URL: ${page.url()}`);
  }
  return match[1];
}

async function generateInviteCode(gmPage: Page, worldId: string): Promise<string> {
  await gmPage.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await gmPage.goto(`/world/${worldId}`);
  await gmPage.getByRole("button", { name: "Generate Join Link" }).click();
  return extractInviteCode(gmPage);
}

/**
 * The Compendium's NPC tab (default tab, always `isNpc: true` at
 * creation — no PC-creation UI exists) is the only actor-creation
 * surface; a fresh actor is then flipped to PC via its own edit route's
 * "This is a player character" checkbox, matching how a real GM would
 * prepare a claimable character today.
 */
async function createPcActor(gmPage: Page, worldId: string, label: string): Promise<string> {
  // Through the shared fixture: this spec needs a claimable character, it is
  // not about how one is made. See `fixtures/content.ts`.
  const actorId = await createNpcViaCompendium(gmPage, worldId, label);

  await gmPage.goto(`/world/${worldId}/actor/${actorId}/edit`);
  await gmPage.getByLabel(/this is a player character/i).check();
  await gmPage.getByRole("button", { name: "Save" }).click();
  await expect(gmPage.getByText(/^saved\.?$/i)).toBeVisible({ timeout: 10_000 });

  return actorId;
}

async function markAvailable(gmPage: Page, worldId: string, actorId: string): Promise<void> {
  await gmPage.goto(`/world/${worldId}/actor/${actorId}/view`);
  const checkbox = gmPage.getByTestId("actor-claim-block").locator('input[type="checkbox"]');
  await checkbox.click();
  await expect(checkbox).toBeChecked({ timeout: 10_000 });
}

test.describe("Spec 017 US1: a joining player picks a GM-designated character", () => {
  test("player lands on Actor Selection, sees exactly the available characters, and claiming removes it from the list", async ({
    browser,
  }) => {
    const gmContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const gmPage = await gmContext.newPage();
    const worldId = await registerAndCreateWorld(gmPage, `E2E Actor Claim ${uniqueSuffix()}`);

    const ariaId = await createPcActor(gmPage, worldId, `Aria ${uniqueSuffix()}`);
    await markAvailable(gmPage, worldId, ariaId);
    const borinId = await createPcActor(gmPage, worldId, `Borin ${uniqueSuffix()}`);
    await markAvailable(gmPage, worldId, borinId);

    const inviteCode = await generateInviteCode(gmPage, worldId);

    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    await register(playerPage, freshCredentials("e2eclaimplyr"));
    await playerPage.goto(`/join/${inviteCode}`);
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();

    await playerPage.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), {
      timeout: 15_000,
    });
    const rows = playerPage.getByTestId("available-actor-row");
    await expect(rows).toHaveCount(2);

    await rows.first().getByRole("button", { name: "Select" }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 });

    // Revisiting later does not show Actor Selection again (FR-002).
    await playerPage.goto(`/world/${worldId}/actor-select`);
    await playerPage.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 });

    // A second joining player only sees the one character left.
    const secondContext = await browser.newContext();
    const secondPage = await secondContext.newPage();
    await register(secondPage, freshCredentials("e2eclaimplyr2"));
    const secondInvite = await generateInviteCode(gmPage, worldId);
    await secondPage.goto(`/join/${secondInvite}`);
    await secondPage.getByRole("button", { name: "Join Campaign" }).click();
    await secondPage.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), {
      timeout: 15_000,
    });
    await expect(secondPage.getByTestId("available-actor-row")).toHaveCount(1);

    await gmContext.close();
    await playerContext.close();
    await secondContext.close();
  });

  test("two players racing to claim the same character: exactly one wins", async ({ browser }) => {
    test.setTimeout(120_000);
    const gmContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const gmPage = await gmContext.newPage();
    const worldId = await registerAndCreateWorld(gmPage, `E2E Claim Race ${uniqueSuffix()}`);
    const actorId = await createPcActor(gmPage, worldId, `Contested ${uniqueSuffix()}`);
    await markAvailable(gmPage, worldId, actorId);

    const inviteA = await generateInviteCode(gmPage, worldId);
    const contextA = await browser.newContext();
    const pageA = await contextA.newPage();
    await register(pageA, freshCredentials("e2eracea"));
    await pageA.goto(`/join/${inviteA}`);
    await pageA.getByRole("button", { name: "Join Campaign" }).click();
    await pageA.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), { timeout: 15_000 });

    const inviteB = await generateInviteCode(gmPage, worldId);
    const contextB = await browser.newContext();
    const pageB = await contextB.newPage();
    await register(pageB, freshCredentials("e2eraceb"));
    await pageB.goto(`/join/${inviteB}`);
    await pageB.getByRole("button", { name: "Join Campaign" }).click();
    await pageB.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), { timeout: 15_000 });

    const selectA = pageA.getByTestId("available-actor-row").getByRole("button", { name: "Select" });
    const selectB = pageB.getByTestId("available-actor-row").getByRole("button", { name: "Select" });
    await expect(selectA).toBeVisible({ timeout: 15_000 });
    await expect(selectB).toBeVisible({ timeout: 15_000 });

    const [resultA, resultB] = await Promise.allSettled([
      selectA.click().then(() =>
        pageA.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 }),
      ),
      selectB.click().then(() =>
        pageB.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 }),
      ),
    ]);

    const successes = [resultA, resultB].filter((r) => r.status === "fulfilled").length;
    expect(successes).toBe(1);

    await gmContext.close().catch(() => {});
    await contextA.close().catch(() => {});
    await contextB.close().catch(() => {});
  });
});

test.describe("Spec 017 US2: a joining player creates their own character", () => {
  test("create-your-own is offered only when the GM has turned the setting on, and is server-enforced regardless", async ({
    browser,
  }) => {
    const gmContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const gmPage = await gmContext.newPage();
    const worldId = await registerAndCreateWorld(gmPage, `E2E Player Created ${uniqueSuffix()}`);

    await gmPage.goto(`/world/${worldId}`);
    await gmPage.getByTestId("allow-player-created-actors-toggle").click();
    await expect(
      gmPage.getByTestId("allow-player-created-actors-toggle").locator("input"),
    ).toBeChecked({ timeout: 10_000 });

    const inviteCode = await generateInviteCode(gmPage, worldId);
    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    await register(playerPage, freshCredentials("e2ecreateown"));
    await playerPage.goto(`/join/${inviteCode}`);
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), {
      timeout: 15_000,
    });

    await expect(playerPage.getByTestId("create-own-actor-form")).toBeVisible();
    await playerPage.locator("#new-character-name").fill(`Homebrew Hero ${uniqueSuffix()}`);
    await playerPage.getByRole("button", { name: /create and play as this character/i }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 });

    // A second player, with the setting off, sees no create option.
    await gmPage.goto(`/world/${worldId}`);
    await gmPage.getByTestId("allow-player-created-actors-toggle").click();
    await expect(
      gmPage.getByTestId("allow-player-created-actors-toggle").locator("input"),
    ).not.toBeChecked({ timeout: 10_000 });

    const secondInvite = await generateInviteCode(gmPage, worldId);
    const secondContext = await browser.newContext();
    const secondPage = await secondContext.newPage();
    await register(secondPage, freshCredentials("e2ecreateoff"));
    await secondPage.goto(`/join/${secondInvite}`);
    await secondPage.getByRole("button", { name: "Join Campaign" }).click();
    await secondPage.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), {
      timeout: 15_000,
    });
    await expect(secondPage.getByTestId("create-own-actor-form")).toHaveCount(0);

    await gmContext.close();
    await playerContext.close();
    await secondContext.close();
  });
});

test.describe("Spec 017 US3: the GM manages availability and claims", () => {
  test("GM sees who claimed a character and can un-claim it, making it available again", async ({
    browser,
  }) => {
    const gmContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const gmPage = await gmContext.newPage();
    const worldId = await registerAndCreateWorld(gmPage, `E2E Unclaim ${uniqueSuffix()}`);
    const actorId = await createPcActor(gmPage, worldId, `Reclaimable ${uniqueSuffix()}`);
    await markAvailable(gmPage, worldId, actorId);

    const inviteCode = await generateInviteCode(gmPage, worldId);
    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    await register(playerPage, freshCredentials("e2eunclaimplyr"));
    await playerPage.goto(`/join/${inviteCode}`);
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), {
      timeout: 15_000,
    });
    await playerPage.getByTestId("available-actor-row").getByRole("button", { name: "Select" }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 });

    await gmPage.goto(`/world/${worldId}/actor/${actorId}/view`);
    const claimBlock = gmPage.getByTestId("actor-claim-block");
    await expect(claimBlock.getByText(/claimed by/i)).toBeVisible({ timeout: 10_000 });

    await claimBlock.getByRole("button", { name: "Un-claim" }).click();
    await expect(claimBlock.locator('input[type="checkbox"]')).toBeVisible({ timeout: 10_000 });

    // The character is available again without re-flagging.
    await playerPage.goto(`/world/${worldId}/actor-select`);
    await expect(playerPage.getByTestId("available-actor-row")).toHaveCount(1);

    await gmContext.close();
    await playerContext.close();
  });
});
