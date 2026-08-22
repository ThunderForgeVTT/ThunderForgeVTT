import { test, expect, type Browser, type Page } from "@playwright/test";

/**
 * specs/009-gm-staging-page: the GM staging page and full-screen play
 * canvas that replace the old `WorldLayout.tsx` placeholder shell. See
 * quickstart.md for the full Given/When/Then this file exercises.
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
  await register(page, freshCredentials("e2estage"));
  await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });
  const match = /\/world\/([^/]+)\/play$/.exec(new URL(page.url()).pathname);
  if (!match) {
    throw new Error(`Could not extract world id from URL: ${page.url()}`);
  }
  return match[1];
}

async function waitForEngineReady(page: Page): Promise<void> {
  const canvas = page.locator("canvas");
  await expect(canvas).toBeVisible({ timeout: 15_000 });
  await page.waitForTimeout(1_000);
}

test.describe("US1: GM sees a real staging page, not the old placeholder shell", () => {
  test("staging page shows real scene/player/NPC data with no dead links, and Play enters full-screen canvas", async ({
    page,
  }) => {
    const worldName = `E2E Staging ${uniqueSuffix()}`;
    await registerAndCreateWorld(page, worldName);

    // The staging page, not the canvas, is the first thing shown. The
    // canvas container stays mounted in the DOM even while staging
    // (research.md §1 — Bevy's engine boot starts immediately regardless
    // of playView, so its <canvas> element may already exist), so this
    // checks it isn't *visible*, not that it doesn't exist.
    await expect(page.getByTestId("world-staging-page")).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.locator("canvas")).toBeHidden();

    // Real scene data (the auto-created default scene), real player list
    // (at least the GM), real NPC roster (empty state, not placeholder).
    await expect(page.getByTestId("scene-switcher")).toContainText(worldName);
    await expect(page.getByText("Players")).toBeVisible();
    await expect(page.getByTestId("staging-player-list")).toHaveCount(1);
    await expect(page.getByText("No NPCs yet.")).toBeVisible({ timeout: 10_000 });

    // No dead "Return to dashboard" link pointing at /counter.
    await expect(page.getByRole("link", { name: "Return to dashboard" })).toHaveCount(0);

    // Play enters full-screen canvas mode.
    await page.getByTestId("play-button").click();
    await expect(page.getByTestId("world-staging-page")).toBeHidden();
    await waitForEngineReady(page);
  });
});

test.describe("US1: back-to-staging preserves engine state", () => {
  test("returning to staging and clicking Play again does not repeat the engine-load sequence", async ({
    page,
  }) => {
    await registerAndCreateWorld(page, `E2E Preserve ${uniqueSuffix()}`);
    await page.getByTestId("play-button").click();
    await waitForEngineReady(page);

    await page.getByTestId("back-to-staging-button").click();
    await expect(page.getByTestId("world-staging-page")).toBeVisible();

    await page.getByTestId("play-button").click();
    // The canvas must already be visible immediately — no repeat of the
    // "Downloading engine…" indicator (the engine never unmounted).
    await expect(page.locator("canvas")).toBeVisible({ timeout: 2_000 });
    await expect(page.getByTestId("engine-load-indicator")).toHaveCount(0);
  });
});

test.describe("US2: sidebar exposes scenes/NPC/trackers without losing canvas space", () => {
  test("sidebar opens with real data and collapses back to a full-viewport canvas", async ({
    page,
  }) => {
    await registerAndCreateWorld(page, `E2E Sidebar ${uniqueSuffix()}`);
    await page.getByTestId("play-button").click();
    await waitForEngineReady(page);

    await expect(page.getByTestId("world-sidebar")).toHaveCount(0);
    await page.getByTestId("sidebar-toggle-button").click();
    const sidebar = page.getByTestId("world-sidebar");
    await expect(sidebar).toBeVisible();
    await expect(sidebar.getByTestId("scene-switcher")).toBeVisible();
    await expect(sidebar.getByText("No NPCs yet.")).toBeVisible({ timeout: 10_000 });

    await page.getByTestId("sidebar-toggle-button").click();
    await expect(page.getByTestId("world-sidebar")).toHaveCount(0);
  });
});

test.describe("US3: players get the same shell, read-only and independent of the GM", () => {
  async function secondSessionSameLogin(
    browser: Browser,
    sourcePage: Page,
  ): Promise<{ context: Awaited<ReturnType<Browser["newContext"]>>; page: Page }> {
    const storageState = await sourcePage.context().storageState();
    const context = await browser.newContext({ storageState });
    const page = await context.newPage();
    return { context, page };
  }

  test("an invited player sees the staging page with GM controls hidden, and can enter full-screen independently", async ({
    page,
    browser,
  }) => {
    const worldId = await registerAndCreateWorld(page, `E2E Player Staging ${uniqueSuffix()}`);

    await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.goto(`/world/${worldId}`);
    await page.getByRole("button", { name: "Generate Invite Code" }).click();
    const inviteCode = (await page.locator("code").first().textContent())?.trim();
    if (!inviteCode) throw new Error("Invite code did not render");

    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    try {
      await register(playerPage, freshCredentials("e2estageplayer"));
      await playerPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
      // Give the player their own world first so they land on the hub, not
      // the zero-world create path, then redeem the GM's invite.
      await playerPage.locator("#world-name").fill(`E2E Player Own ${uniqueSuffix()}`);
      await playerPage.getByRole("button", { name: /create world/i }).click();
      await playerPage.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });

      await playerPage.goto(`/join/${inviteCode}`);
      await expect(
        playerPage.getByRole("button", { name: "Join Campaign" }),
      ).toBeVisible({ timeout: 10_000 });
      await playerPage.getByRole("button", { name: "Join Campaign" }).click();
      await playerPage.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 });

      await playerPage.goto(`/world/${worldId}/play`);
      await expect(playerPage.getByTestId("world-staging-page")).toBeVisible({
        timeout: 15_000,
      });
      // GM-only controls are absent for a Player.
      await expect(playerPage.getByTestId("new-scene-button")).toHaveCount(0);

      // The player enters full-screen independently — the GM's own,
      // separate browser session is unaffected by the player's navigation.
      await playerPage.getByTestId("play-button").click();
      await waitForEngineReady(playerPage);
      await page.goto(`/world/${worldId}/play`);
      await expect(page.getByTestId("world-staging-page")).toBeVisible({
        timeout: 15_000,
      });
    } finally {
      await playerContext.close();
    }
  });

  test("a non-member cannot see the staging page's real world data", async ({
    page,
    browser,
  }) => {
    const worldName = `E2E Non-Member ${uniqueSuffix()}`;
    const worldId = await registerAndCreateWorld(page, worldName);

    const outsiderContext = await browser.newContext();
    const outsiderPage = await outsiderContext.newPage();
    try {
      await register(outsiderPage, freshCredentials("e2estageoutsider"));
      await outsiderPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });

      await outsiderPage.goto(`/world/${worldId}/play`);
      // The staging shell itself still renders, but never with the real
      // world's data — `getWorld`/`worldActors` both enforce the same
      // visibility rule as `scenes`, so a non-member never sees the real
      // name, and the NPC roster surfaces a load failure rather than
      // silently showing real (or fabricated) content.
      await expect(outsiderPage.getByTestId("world-staging-page")).toBeVisible({
        timeout: 15_000,
      });
      await expect(outsiderPage.getByText(worldName)).toHaveCount(0);
    } finally {
      await outsiderContext.close();
    }
  });
});
