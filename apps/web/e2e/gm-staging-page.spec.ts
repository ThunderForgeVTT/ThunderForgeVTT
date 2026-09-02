import { test, expect, type Page } from "@playwright/test";
import { openDockTab } from "./fixtures/helpers";

/**
 * specs/009-gm-staging-page: the GM staging page and full-screen play
 * canvas that replace the old `WorldLayout.tsx` placeholder shell.
 *
 * Spec 010 update: staging moved from a UI state inside `/world/:id/play`
 * to its own routed `/world/:id/staging` page — `registerAndCreateWorld`
 * now lands there, and "Play" is a real route navigation to `/play`
 * rather than a same-page visibility toggle. One consequence (documented,
 * not silently dropped): since `/staging` and `/play` are now separate
 * route components, navigating back to staging unmounts the canvas
 * container, so the "no engine reload on the second Play" guarantee spec
 * 009 built no longer holds across a staging round-trip — only within a
 * single continuous `/play` visit. See quickstart.md for the full
 * Given/When/Then this file exercises.
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
  await register(page, freshCredentials("e2estage"));
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
    const worldId = await registerAndCreateWorld(page, worldName);

    // Spec 010: staging is its own route now — the canvas container isn't
    // mounted at all here (not merely hidden), since `/staging` and
    // `/play` are separate route components.
    await expect(page.getByTestId("world-staging-page")).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.locator("canvas")).toHaveCount(0);

    // Spec 022: scene management (incl. the auto-created default scene)
    // moved to the dedicated Scenes section. Spec 011: NPC management
    // similarly moved to /compendium (see world-compendium.spec.ts for
    // NPC-roster coverage). Spec 023: the player roster itself moved to
    // its own dedicated Players sidebar section — Overview now only
    // shows session notes and the invite link.
    await expect(page.getByTestId("scene-switcher")).toHaveCount(0);
    await expect(page.getByTestId("world-nav-scenes")).toBeVisible();
    await page.getByTestId("world-nav-scenes").click();
    await page.waitForURL(`**/world/${worldId}/scenes`, { timeout: 10_000 });
    await expect(page.getByRole("link", { name: worldName })).toBeVisible({
      timeout: 10_000,
    });
    await page.goBack();
    await page.waitForURL(`**/world/${worldId}/staging`, { timeout: 10_000 });

    await expect(page.getByTestId("world-nav-players")).toBeVisible();
    await expect(page.getByTestId("world-nav-npcs")).toBeVisible();
    await expect(page.getByTestId("session-notes-panel")).toBeVisible();

    // Play navigates to the full-screen canvas route.
    await page.getByTestId("play-button").click();
    await page.waitForURL(new RegExp(`/world/${worldId}/play$`), { timeout: 15_000 });
    await expect(page.getByTestId("world-staging-page")).toHaveCount(0);
    await waitForEngineReady(page);
  });
});

test.describe("US1: back-to-staging navigates to the dedicated staging route", () => {
  // This was a `test.fail()` for a real bug: after /play -> back-to-staging
  // -> /staging, the Bevy canvas from the first /play visit stayed pinned
  // over the whole viewport (it is a child of <body> with `position: fixed`,
  // so React unmounting `WorldPage` never touched it) and intercepted the
  // second "Play" click. `WorldPage.tsx` now hides it from the one callback
  // that reliably runs on unmount — see `hideEngineCanvas` there — so the
  // round trip works and this is an ordinary passing test again. The engine
  // itself deliberately stays booted (spec 009 research.md §1); what was
  // missing was only hiding its canvas.
  test("the on-screen back control returns to /staging (spec 010 route split)", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(page, `E2E BackToStaging ${uniqueSuffix()}`);
    await page.getByTestId("play-button").click();
    await waitForEngineReady(page);

    // "Back to setup" lives in the play dock's Settings section now, not on
    // the canvas chrome — it does not exist in the DOM until that section is
    // open (see WorldDock/SettingsPanel).
    await openDockTab(page, "settings");
    await page.getByTestId("back-to-staging-button").click();
    await page.waitForURL(new RegExp(`/world/${worldId}/staging$`), { timeout: 15_000 });
    await expect(page.getByTestId("world-staging-page")).toBeVisible();

    // The stray canvas used to intercept this click.
    await page.getByTestId("play-button").click();
    await page.waitForURL(new RegExp(`/world/${worldId}/play$`), { timeout: 15_000 });
    await waitForEngineReady(page);
  });
});

test.describe("US2: the play dock exposes scenes/actors/settings without losing canvas space", () => {
  test("a dock section opens with real data and collapses back to a full-viewport canvas", async ({
    page,
  }) => {
    await registerAndCreateWorld(page, `E2E Dock ${uniqueSuffix()}`);
    await page.getByTestId("play-button").click();
    await waitForEngineReady(page);

    // The icon rail is always present; no section panel is open yet.
    await expect(page.getByTestId("world-dock")).toBeVisible();
    await expect(page.getByTestId("world-dock-panel-settings")).toHaveCount(0);

    await page.getByTestId("world-dock-tab-settings").click();
    const settings = page.getByTestId("world-dock-panel-settings");
    await expect(settings).toBeVisible();
    await expect(settings.getByTestId("scene-switcher")).toBeVisible();

    // Actors moved to their own section, foldered into PCs and NPCs.
    await page.getByTestId("world-dock-tab-actors").click();
    const actors = page.getByTestId("world-dock-panel-actors");
    await expect(actors).toBeVisible();
    await expect(actors.getByText("No NPCs yet.")).toBeVisible({ timeout: 10_000 });

    await page.getByTestId("world-dock-tab-actors").click();
    await expect(page.getByTestId("world-dock-panel-actors")).toHaveCount(0);
  });
});

test.describe("US3: players get the same shell, read-only and independent of the GM", () => {
  test("an invited player sees the staging page with GM controls hidden, and can enter full-screen independently", async ({
    page,
    browser,
  }) => {
    const worldId = await registerAndCreateWorld(page, `E2E Player Staging ${uniqueSuffix()}`);

    await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.goto(`/world/${worldId}`);
    await page.getByRole("button", { name: "Generate Join Link" }).click();
    const inviteCode = await extractInviteCode(page);
    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    try {
      await register(playerPage, freshCredentials("e2estageplayer"));
      await playerPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
      // Give the player their own world first so they land on the hub, not
      // the zero-world create path, then redeem the GM's invite.
      await playerPage.locator("#world-name").fill(`E2E Player Own ${uniqueSuffix()}`);
      await playerPage.getByRole("button", { name: /create world/i }).click();
      await playerPage.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });

      await playerPage.goto(`/join/${inviteCode}`);
      await expect(
        playerPage.getByRole("button", { name: "Join Campaign" }),
      ).toBeVisible({ timeout: 10_000 });
      await playerPage.getByRole("button", { name: "Join Campaign" }).click();
      await playerPage.waitForURL(new RegExp(`/world/${worldId}(/actor-select)?$`), { timeout: 15_000 });

      await playerPage.goto(`/world/${worldId}/staging`);
      await expect(playerPage.getByTestId("world-staging-page")).toBeVisible({
        timeout: 15_000,
      });
      // GM-only controls are absent for a Player — Session Setup has no
      // scene controls at all now (spec 022), and the Scenes section
      // itself hides the "New scene" creation form from non-GM members.
      await expect(playerPage.getByTestId("scene-switcher")).toHaveCount(0);
      await playerPage.goto(`/world/${worldId}/scenes`);
      await expect(playerPage.getByTestId("new-scene-name-input")).toHaveCount(0);
      await playerPage.goto(`/world/${worldId}/staging`);

      // The player enters full-screen independently — the GM's own,
      // separate browser session is unaffected by the player's navigation.
      await playerPage.getByTestId("play-button").click();
      await waitForEngineReady(playerPage);
      await page.goto(`/world/${worldId}/staging`);
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

      await outsiderPage.goto(`/world/${worldId}/staging`);
      // The staging shell itself still renders, but never with the real
      // world's data — `getWorld` enforces the same visibility rule as
      // `scenes`, so a non-member never sees the real world name.
      await expect(outsiderPage.getByTestId("world-staging-page")).toBeVisible({
        timeout: 15_000,
      });
      await expect(outsiderPage.getByText(worldName)).toHaveCount(0);
    } finally {
      await outsiderContext.close();
    }
  });
});
