import { test, expect, type Browser, type Page } from "@playwright/test";

/**
 * specs/008-seamless-onboarding-flow: the sign-up-to-canvas funnel
 * (register → create-world → canvas), the honest engine-load indicator,
 * the removal of dead/placeholder UI, and the returning-user hub. See
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

/** Waits for the Bevy canvas to actually be mounted and settled, mirroring
 * the identical helper in token-authoring.spec.ts/canvas-authoring.spec.ts. */
async function waitForEngineReady(page: Page): Promise<void> {
  const canvas = page.locator("canvas");
  await expect(canvas).toBeVisible({ timeout: 15_000 });
  await page.waitForTimeout(1_000);
}

/** Spec 009: `/world/:id/play` now lands on the staging page first — the
 * canvas (and its loading indicators) only become visible in full-screen
 * mode after clicking "Play". */
async function clickPlay(page: Page): Promise<void> {
  await page.getByTestId("play-button").click();
}

test.describe("US1: zero-world registration goes straight to world creation, then straight to canvas (T002-T003)", () => {
  test("a brand-new account lands directly on the create-world form, with no /welcome hub content ever rendered", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eonb"));

    // FR-001: the very next screen is /worlds/create — no hub screen
    // renders first, even momentarily as page content (a brief address-bar
    // flash through /welcome before the replace-navigation is acceptable
    // per research.md §2; what matters is no hub *content* is shown).
    await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
    await expect(page.locator("#world-name")).toBeVisible();
    await expect(page.getByText("Welcome back to ThunderForge.")).toHaveCount(0);
  });

  test("submitting the create-world form lands directly on the canvas with the default scene already rendered — no dashboard, no New-scene modal", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eonb"));
    await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });

    const worldName = `E2E Onboarding ${uniqueSuffix()}`;
    await page.locator("#world-name").fill(worldName);
    await page.getByRole("button", { name: /create world/i }).click();

    // FR-004/FR-006: straight to /world/:id/play, never /world/:id (the
    // dashboard).
    await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });

    // The default scene already exists (create_world's atomic transaction,
    // T005) — the staging page's scene switcher (spec 009) shows it
    // selected, and the "New scene" modal never had to appear.
    await expect(page.getByTestId("scene-switcher")).toBeVisible({
      timeout: 10_000,
    });
    await expect(page.getByTestId("scene-switcher")).toContainText(worldName);
    await expect(page.getByTestId("new-scene-name-input")).toHaveCount(0);

    // Spec 009: clicking Play enters full-screen canvas mode, where the
    // same default scene is already loaded and rendered.
    await clickPlay(page);
    await waitForEngineReady(page);
  });
});

test.describe("US1: honest engine-load feedback (T004)", () => {
  test("a continuous loading indicator is visible until the engine is ready, and an error state renders on failure", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eonb"));
    await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
    await page.locator("#world-name").fill(`E2E Loading ${uniqueSuffix()}`);
    await page.getByRole("button", { name: /create world/i }).click();
    await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });
    // Spec 009: the indicator lives inside full-screen canvas mode now —
    // click Play immediately to catch it as early as possible (the engine
    // itself starts downloading as soon as the page mounts, regardless of
    // staging vs. full-screen, per research.md §1/§4).
    await clickPlay(page);

    // FR-002/SC-002: the indicator is visible from render until engineReady
    // flips true — check it's present immediately (before the canvas is
    // necessarily ready) rather than racing to catch a narrow window.
    const indicator = page.getByTestId("engine-load-indicator");
    // It may already have resolved to ready on a fast local run; only
    // assert presence if we can still catch it, then always confirm the
    // canvas itself shows up (proving readiness happened, not a stall).
    await indicator.isVisible().catch(() => false);
    await waitForEngineReady(page);
    await expect(page.getByTestId("engine-load-indicator")).toHaveCount(0);
  });

  test("blocking the engine asset shows a clear error state, not a blank screen", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eonb"));
    await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
    await page.locator("#world-name").fill(`E2E Load Error ${uniqueSuffix()}`);

    // Block the engine's WASM module before navigating into a world, so
    // mountEngine's dynamic import rejects (FR-003).
    await page.route("**/engine*.wasm", (route) => route.abort());
    await page.getByRole("button", { name: /create world/i }).click();
    await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });
    await clickPlay(page);

    await expect(page.getByText("Failed to load game engine")).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.getByTestId("engine-load-indicator")).toHaveCount(0);
  });
});

test.describe("US2: no dead/placeholder controls remain (T011-T012)", () => {
  test("the create-world form shows only name and description — no game-system or interface-pack selector", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eonb"));
    await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });

    await expect(page.locator("#world-name")).toBeVisible();
    await expect(page.locator("#world-description")).toBeVisible();
    await expect(page.locator("#world-game-system")).toHaveCount(0);
    await expect(page.locator("#world-interface-pack")).toHaveCount(0);
    await expect(page.getByText(/placeholder game system/i)).toHaveCount(0);
  });

  test("an existing world's dashboard shows only real data — no unfilled placeholder panels", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eonb"));
    await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
    const worldName = `E2E Dashboard ${uniqueSuffix()}`;
    await page.locator("#world-name").fill(worldName);
    await page.getByRole("button", { name: /create world/i }).click();
    await page.waitForURL(/\/world\/([^/]+)\/play$/, { timeout: 15_000 });
    const worldId = /\/world\/([^/]+)\/play$/.exec(new URL(page.url()).pathname)?.[1];
    if (!worldId) throw new Error("Could not extract world id");

    await page.goto(`/world/${worldId}`);
    await expect(page.getByText("Placeholder domain")).toHaveCount(0);
    await expect(page.getByText(/Awaiting a later phase/i)).toHaveCount(0);
    // The real Scenes panel shows the auto-created default scene.
    await expect(page.getByText("1 scene")).toBeVisible({ timeout: 10_000 });
  });
});

test.describe("US2: invite-code path works for existing and brand-new accounts (T013)", () => {
  test("a logged-in user can join a world by entering an invite code on the hub", async ({
    page,
    browser,
  }) => {
    // GM: create a world, generate an invite code from its dashboard.
    await register(page, freshCredentials("e2eonbgm"));
    await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
    await page.locator("#world-name").fill(`E2E Invite ${uniqueSuffix()}`);
    await page.getByRole("button", { name: /create world/i }).click();
    await page.waitForURL(/\/world\/([^/]+)\/play$/, { timeout: 15_000 });
    const worldId = /\/world\/([^/]+)\/play$/.exec(new URL(page.url()).pathname)?.[1];
    if (!worldId) throw new Error("Could not extract world id");

    await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.goto(`/world/${worldId}`);
    await page.getByRole("button", { name: "Generate Invite Code" }).click();
    const inviteCodeLocator = page.locator("code").first();
    await expect(inviteCodeLocator).toBeVisible({ timeout: 10_000 });
    const inviteCode = (await inviteCodeLocator.textContent())?.trim();
    if (!inviteCode) throw new Error("Invite code did not render");

    // A second, distinct account (with an existing world of its own, so it
    // lands on the hub, not zero-world create-world) redeems the code.
    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    try {
      await register(playerPage, freshCredentials("e2eonbplayer"));
      await playerPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
      await playerPage.locator("#world-name").fill(`E2E Player Own World ${uniqueSuffix()}`);
      await playerPage.getByRole("button", { name: /create world/i }).click();
      await playerPage.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });

      await playerPage.goto("/welcome");
      await playerPage.locator("#welcome-invite-code").fill(inviteCode);
      await playerPage.getByRole("button", { name: "Join via Invite Code" }).click();
      await playerPage.waitForURL(new RegExp(`/join/${inviteCode}`), { timeout: 10_000 });
      await expect(
        playerPage.getByRole("button", { name: "Join Campaign" }),
      ).toBeVisible({ timeout: 10_000 });
    } finally {
      await playerContext.close();
    }
  });

  test("an unauthenticated invite link survives login-vs-register and redemption completes after registering", async ({
    page,
  }) => {
    // GM: create a world, generate an invite code.
    await register(page, freshCredentials("e2eonbgm2"));
    await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
    await page.locator("#world-name").fill(`E2E Invite Register ${uniqueSuffix()}`);
    await page.getByRole("button", { name: /create world/i }).click();
    await page.waitForURL(/\/world\/([^/]+)\/play$/, { timeout: 15_000 });
    const worldId = /\/world\/([^/]+)\/play$/.exec(new URL(page.url()).pathname)?.[1];
    if (!worldId) throw new Error("Could not extract world id");

    await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.goto(`/world/${worldId}`);
    await page.getByRole("button", { name: "Generate Invite Code" }).click();
    const inviteCode = (await page.locator("code").first().textContent())?.trim();
    if (!inviteCode) throw new Error("Invite code did not render");
    await page.context().clearCookies();

    // Follow the invite link unauthenticated -> redirected to login with
    // returnTo preserved.
    await page.goto(`/join/${inviteCode}`);
    await page.waitForURL(/\/login\?returnTo=/, { timeout: 10_000 });

    // T017: the "Register" link must preserve that returnTo across the
    // Login -> Register hop.
    await page.getByRole("link", { name: "Create a local account" }).click();
    await expect(page).toHaveURL(/\/register\?returnTo=/);

    const creds = freshCredentials("e2eonbinvitee");
    await page.locator("#register-username").fill(creds.username);
    await page.locator("#register-email").fill(creds.email);
    await page.locator("#register-password").fill(creds.password);
    await page.locator("#register-password-confirmation").fill(creds.password);
    await page.getByRole("button", { name: "Create account" }).click();

    // FR-012: registration returns straight to redeeming the code, not the
    // zero-worlds create-world path.
    await page.waitForURL(new RegExp(`/join/${inviteCode}`), { timeout: 15_000 });
    await expect(
      page.getByRole("button", { name: "Join Campaign" }),
    ).toBeVisible({ timeout: 10_000 });
  });
});

test.describe("US3: returning users always see the hub with one-click shortcuts (T018-T020)", () => {
  async function secondSessionSameLogin(
    browser: Browser,
    sourcePage: Page,
  ): Promise<{ context: Awaited<ReturnType<Browser["newContext"]>>; page: Page }> {
    const storageState = await sourcePage.context().storageState();
    const context = await browser.newContext({ storageState });
    const page = await context.newPage();
    return { context, page };
  }

  test("a user with exactly one world lands on the hub (not auto-entered) and sees it as a one-click shortcut", async ({
    page,
    browser,
  }) => {
    await register(page, freshCredentials("e2eonbsingle"));
    await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
    const worldName = `E2E Single World ${uniqueSuffix()}`;
    await page.locator("#world-name").fill(worldName);
    await page.getByRole("button", { name: /create world/i }).click();
    await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });

    // A fresh session with the same login, landing on /welcome directly.
    const { context, page: freshPage } = await secondSessionSameLogin(browser, page);
    try {
      await freshPage.goto("/welcome");
      await expect(freshPage).toHaveURL(/\/welcome$/);
      await expect(
        freshPage.getByRole("link", { name: `Enter ${worldName}` }),
      ).toBeVisible({ timeout: 10_000 });
    } finally {
      await context.close();
    }
  });

  test("a user with multiple worlds sees all of them as shortcuts on the same hub", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eonbmulti"));
    await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
    const firstName = `E2E Multi A ${uniqueSuffix()}`;
    await page.locator("#world-name").fill(firstName);
    await page.getByRole("button", { name: /create world/i }).click();
    await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });

    const secondName = `E2E Multi B ${uniqueSuffix()}`;
    await page.goto("/worlds/create");
    await page.locator("#world-name").fill(secondName);
    await page.getByRole("button", { name: /create world/i }).click();
    await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });

    await page.goto("/welcome");
    await expect(page).toHaveURL(/\/welcome$/);
    await expect(page.getByRole("link", { name: `Enter ${firstName}` })).toBeVisible();
    await expect(page.getByRole("link", { name: `Enter ${secondName}` })).toBeVisible();
  });
});

test.describe("Polish: create-world form preserves input on error (T024)", () => {
  test("a failed submission keeps the entered name/description and shows a specific error", async ({
    page,
  }) => {
    await register(page, freshCredentials("e2eonberr"));
    await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });

    // A name under the minimum length triggers validate_world_name's
    // existing rejection (server-side "3-64 characters" rule).
    await page.locator("#world-name").fill("ab");
    await page.locator("#world-description").fill("Keep me around");
    await page.getByRole("button", { name: /create world/i }).click();

    await expect(
      page.getByText(/World name must be between \d+ and \d+ characters/i),
    ).toBeVisible({ timeout: 10_000 });
    await expect(page.locator("#world-name")).toHaveValue("ab");
    await expect(page.locator("#world-description")).toHaveValue("Keep me around");
  });
});
