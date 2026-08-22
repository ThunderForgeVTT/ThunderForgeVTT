import { test, expect, type Page } from "@playwright/test";

/**
 * specs/010-world-staging-actors (US4): dedicated `/world/:id/actor/:actorId/view`
 * and `.../edit` routes — reachable by anyone with at least Viewer access,
 * edit gated to Editor/Owner, and denied entirely to non-members.
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
  await register(page, freshCredentials("e2earoutes"));
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

async function currentUserId(page: Page): Promise<string> {
  const cookies = await page.context().cookies();
  const csrfToken = cookies.find((c) => c.name === "csrf_token")?.value;
  const response = await page.request.post("/api/graphql", {
    headers: csrfToken ? { "x-csrf-token": csrfToken } : {},
    data: { query: "query { me { id } }" },
  });
  const payload = (await response.json()) as { data?: { me?: { id?: string } } };
  const id = payload.data?.me?.id;
  if (!id) throw new Error("Could not resolve current user id via /api/graphql");
  return id;
}

test.describe("US4: Viewer can view but is redirected away from /edit", () => {
  test("a default-Viewer world member sees /view but bounces off /edit", async ({
    page,
    browser,
  }) => {
    const worldName = `E2E Actor Routes ${uniqueSuffix()}`;
    const worldId = await registerAndCreateWorld(page, worldName);

    const npcName = `Bo Jangles ${uniqueSuffix()}`;
    // Spec 011: NPC creation moved from staging to /compendium.
    await page.goto(`/world/${worldId}/compendium`);
    await page.getByPlaceholder("New NPC name").fill(npcName);
    await page.getByRole("button", { name: "Add NPC" }).click();
    await page
      .getByTestId("npc-catalog-table")
      .locator("tr", { hasText: npcName })
      .getByRole("link", { name: "View" })
      .click();
    await page.waitForURL(new RegExp(`/world/${worldId}/actor/([^/]+)/view$`), { timeout: 15_000 });
    const actorMatch = new RegExp(`/world/${worldId}/actor/([^/]+)/view$`).exec(
      new URL(page.url()).pathname,
    );
    if (!actorMatch) throw new Error(`Could not extract actor id from URL: ${page.url()}`);
    const actorId = actorMatch[1];

    await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.goto(`/world/${worldId}`);
    await page.getByRole("button", { name: "Generate Join Link" }).click();
    const inviteCode = await extractInviteCode(page);
    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    try {
      await register(playerPage, freshCredentials("e2earoutesplyr"));
      await playerPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
      await playerPage.locator("#world-name").fill(`E2E Player Own ${uniqueSuffix()}`);
      await playerPage.getByRole("button", { name: /create world/i }).click();
      await playerPage.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
      const playerId = await currentUserId(playerPage);

      await playerPage.goto(`/join/${inviteCode}`);
      await playerPage.getByRole("button", { name: "Join Campaign" }).click();
      await playerPage.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 });

      // Default Viewer: /view renders, /edit redirects to /view.
      await playerPage.goto(`/world/${worldId}/actor/${actorId}/view`);
      await expect(playerPage.getByRole("heading", { level: 1 })).toBeVisible({ timeout: 10_000 });
      await expect(playerPage.getByRole("button", { name: "Edit" })).toHaveCount(0);

      await playerPage.goto(`/world/${worldId}/actor/${actorId}/edit`);
      await playerPage.waitForURL(new RegExp(`/actor/${actorId}/view$`), { timeout: 10_000 });

      // DM grants Editor; the player can now reach /edit and save.
      await page.goto(`/world/${worldId}/actor/${actorId}/edit`);
      await expect(page.getByTestId("actor-ownership-block")).toBeVisible({ timeout: 10_000 });
      await page.getByTestId(`ownership-select-${playerId}`).selectOption("EDITOR");
      await expect(page.getByTestId(`ownership-select-${playerId}`)).toHaveValue("EDITOR");

      await playerPage.goto(`/world/${worldId}/actor/${actorId}/edit`);
      await expect(playerPage).toHaveURL(new RegExp(`/actor/${actorId}/edit$`));
      await playerPage.locator("#actor-label").fill("Renamed By Editor");
      await playerPage.getByRole("button", { name: "Save" }).click();
      await expect(playerPage.getByText("Saved.")).toBeVisible({ timeout: 10_000 });
    } finally {
      await playerContext.close();
    }
  });

  test("a non-member is denied both routes", async ({ page, browser }) => {
    const worldName = `E2E Actor Non-Member ${uniqueSuffix()}`;
    const worldId = await registerAndCreateWorld(page, worldName);

    const npcName = `Bo Jangles ${uniqueSuffix()}`;
    // Spec 011: NPC creation moved from staging to /compendium.
    await page.goto(`/world/${worldId}/compendium`);
    await page.getByPlaceholder("New NPC name").fill(npcName);
    await page.getByRole("button", { name: "Add NPC" }).click();
    await page
      .getByTestId("npc-catalog-table")
      .locator("tr", { hasText: npcName })
      .getByRole("link", { name: "View" })
      .click();
    await page.waitForURL(new RegExp(`/world/${worldId}/actor/([^/]+)/view$`), { timeout: 15_000 });
    const actorMatch = new RegExp(`/world/${worldId}/actor/([^/]+)/view$`).exec(
      new URL(page.url()).pathname,
    );
    if (!actorMatch) throw new Error(`Could not extract actor id from URL: ${page.url()}`);
    const actorId = actorMatch[1];

    const outsiderContext = await browser.newContext();
    const outsiderPage = await outsiderContext.newPage();
    try {
      await register(outsiderPage, freshCredentials("e2earoutesout"));
      await outsiderPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });

      await outsiderPage.goto(`/world/${worldId}/actor/${actorId}/view`);
      await expect(outsiderPage.getByText("Actor not found")).toBeVisible({ timeout: 10_000 });

      await outsiderPage.goto(`/world/${worldId}/actor/${actorId}/edit`);
      await expect(outsiderPage.getByText("Actor not found")).toBeVisible({ timeout: 10_000 });
    } finally {
      await outsiderContext.close();
    }
  });
});
