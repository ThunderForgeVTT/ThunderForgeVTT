import { test, expect, type Page } from "@playwright/test";

/**
 * specs/011-world-compendium (Foundational + US1/US2 read paths): the
 * dedicated `/world/:id/compendium` route — a tabbed shell (NPCs real,
 * Items/Abilities placeholder) with a searchable NPC table and a
 * row-select preview panel docked to its right.
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
  await register(page, freshCredentials("e2ecompendium"));
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

/** Seeds an NPC via the Compendium's own "Add NPC" control (US1, T010). */
async function addNpcFromStaging(page: Page, worldId: string, name: string): Promise<void> {
  await page.goto(`/world/${worldId}/compendium`);
  await page.getByPlaceholder("New NPC name").fill(name);
  await page.getByRole("button", { name: "Add NPC" }).click();
  await expect(page.getByText(name)).toBeVisible({ timeout: 10_000 });
}

test.describe("Compendium shell: tabs, NPC browse/search, row-select preview", () => {
  test("loads inside app chrome with NPCs tab selected, real data, search, and preview", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(page, `E2E Compendium ${uniqueSuffix()}`);
    const npcName = `Bo Jangles ${uniqueSuffix()}`;
    await addNpcFromStaging(page, worldId, npcName);

    await page.goto(`/world/${worldId}/compendium`);
    await expect(page.getByTestId("world-compendium-page")).toBeVisible({ timeout: 15_000 });
    // App chrome (header nav), not the full-screen canvas.
    await expect(page.getByRole("link", { name: "Welcome" })).toBeVisible();
    await expect(page.locator("canvas")).toHaveCount(0);

    // NPCs tab selected by default, real data visible.
    await expect(page.getByRole("tab", { name: "NPCs" })).toHaveAttribute(
      "data-state",
      "active",
    );
    await expect(page.getByTestId("npc-catalog-table")).toContainText(npcName);

    // Search narrows results instantly.
    await page.getByTestId("npc-catalog-search-input").fill("zzz-no-match-zzz");
    await expect(page.getByText(`No NPCs match "zzz-no-match-zzz"`)).toBeVisible();
    await page.getByTestId("npc-catalog-search-input").fill("");
    await expect(page.getByTestId("npc-catalog-table")).toContainText(npcName);

    // Selecting a row opens the preview panel with correct detail.
    await expect(page.getByTestId("actor-preview-panel-empty")).toBeVisible();
    await page.getByTestId("npc-catalog-table").getByText(npcName).click();
    await expect(page.getByTestId("actor-preview-panel")).toBeVisible();
    await expect(page.getByTestId("actor-preview-panel")).toContainText(npcName);
    await expect(page.getByTestId("actor-preview-panel")).toContainText("Non-Player Character");

    // Placeholder tabs: no table, no search, clearly-labeled "coming soon".
    await page.getByRole("tab", { name: "Items" }).click();
    await expect(page.getByTestId("compendium-coming-soon")).toContainText("Items — coming soon");
    await expect(page.getByTestId("npc-catalog-table")).toHaveCount(0);
    await expect(page.getByTestId("npc-catalog-search-input")).toHaveCount(0);

    await page.getByRole("tab", { name: "Abilities" }).click();
    await expect(page.getByTestId("compendium-coming-soon")).toContainText(
      "Abilities — coming soon",
    );
  });

  test("a non-member of the world cannot see its NPC data via the Compendium", async ({
    page,
    browser,
  }) => {
    const worldId = await registerAndCreateWorld(page, `E2E Compendium Private ${uniqueSuffix()}`);
    await addNpcFromStaging(page, worldId, `Secret NPC ${uniqueSuffix()}`);

    const outsiderContext = await browser.newContext();
    const outsiderPage = await outsiderContext.newPage();
    try {
      await register(outsiderPage, freshCredentials("e2ecompendiumout"));
      await outsiderPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });

      await outsiderPage.goto(`/world/${worldId}/compendium`);
      // The route renders (matching the existing staging-route precedent
      // of no dedicated route guard), but the underlying world-scoped
      // queries reject a non-member, surfacing as a load failure rather
      // than any real NPC data.
      await expect(outsiderPage.getByText(/Failed to load NPCs/i)).toBeVisible({
        timeout: 10_000,
      });
      await expect(outsiderPage.getByTestId("npc-catalog-table")).toHaveCount(0);
    } finally {
      await outsiderContext.close();
    }
  });
});

/** Registers a brand-new account, creates a world it isn't the DM of by
 * joining someone else's invite — used for US2's Player-parity test. */
async function joinWorldAsPlayer(
  page: Page,
  worldId: string,
  inviteUrl: string,
  prefix: string,
): Promise<void> {
  await register(page, freshCredentials(prefix));
  await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
  await page.goto(inviteUrl);
  await page.getByRole("button", { name: "Join Campaign" }).click();
  await page.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 });
}

test.describe("US1: DM adds and edits an NPC from the Compendium", () => {
  test("DM adds an NPC via the Compendium, edits its description, and sees the update", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(page, `E2E Compendium DM ${uniqueSuffix()}`);
    const npcName = `Compendium-Added NPC ${uniqueSuffix()}`;

    await page.goto(`/world/${worldId}/compendium`);
    await expect(page.getByTestId("world-compendium-page")).toBeVisible({ timeout: 15_000 });

    // Add NPC control is present for the DM, directly on the Compendium.
    await page.getByTestId("new-npc-name-input").fill(npcName);
    await page.getByTestId("new-npc-description-input").fill("Original description");
    await page.getByTestId("add-npc-button").click();
    await expect(page.getByTestId("npc-catalog-table")).toContainText(npcName, { timeout: 10_000 });

    // Select it, follow Edit from the preview panel, change its description.
    await page.getByTestId("npc-catalog-table").getByText(npcName).click();
    await expect(page.getByTestId("actor-preview-panel")).toContainText("Original description");
    await page.getByTestId("actor-preview-panel-edit").click();
    await page.waitForURL(/\/actor\/[^/]+\/edit$/, { timeout: 15_000 });
    const descriptionField = page.getByLabel("Description");
    await descriptionField.fill("Updated via Compendium edit flow");
    await page.getByRole("button", { name: "Save" }).click();
    await expect(page.getByText("Saved.")).toBeVisible({ timeout: 10_000 });

    // Back on the Compendium, the update is visible in the table and preview.
    await page.goto(`/world/${worldId}/compendium`);
    await expect(page.getByTestId("npc-catalog-table")).toContainText(
      "Updated via Compendium edit flow",
    );
    await page.getByTestId("npc-catalog-table").getByText(npcName).click();
    await expect(page.getByTestId("actor-preview-panel")).toContainText(
      "Updated via Compendium edit flow",
    );
  });
});

test.describe("US2: a Player browses the Compendium with the same read access, minus DM controls", () => {
  test("Player sees identical browse/search/preview, no Add NPC, and no Edit on a Viewer-only NPC", async ({
    page,
    browser,
  }) => {
    const worldName = `E2E Compendium Player ${uniqueSuffix()}`;
    const worldId = await registerAndCreateWorld(page, worldName);
    const npcName = `Player-Visible NPC ${uniqueSuffix()}`;
    await addNpcFromStaging(page, worldId, npcName);

    // Generate an invite from the world dashboard, mirroring the pattern
    // established in invite-membership.spec.ts / gm-staging-page.spec.ts.
    // Clipboard permission avoids invite-membership.spec.ts's documented
    // flakiness around the copy-to-clipboard side effect of this click.
    await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
    await page.goto(`/world/${worldId}`);
    await page.getByRole("button", { name: "Generate Join Link" }).click();
    const inviteInput = page.locator("input[readonly]");
    await expect(inviteInput).toBeVisible({ timeout: 10_000 });
    const inviteUrl = await inviteInput.inputValue();
    const inviteUrlObj = new URL(inviteUrl);
    const inviteRelativePath = inviteUrlObj.pathname;

    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    try {
      await joinWorldAsPlayer(playerPage, worldId, inviteRelativePath, "e2ecompendiumplyr");

      await playerPage.goto(`/world/${worldId}/compendium`);
      await expect(playerPage.getByTestId("world-compendium-page")).toBeVisible({
        timeout: 15_000,
      });

      // Same browse/search/preview as the DM.
      await expect(playerPage.getByTestId("npc-catalog-table")).toContainText(npcName);
      await playerPage.getByTestId("npc-catalog-search-input").fill(npcName.slice(0, 6));
      await expect(playerPage.getByTestId("npc-catalog-table")).toContainText(npcName);
      await playerPage.getByTestId("npc-catalog-table").getByText(npcName).click();
      await expect(playerPage.getByTestId("actor-preview-panel")).toContainText(npcName);

      // No Add NPC control for a non-DM.
      await expect(playerPage.getByTestId("new-npc-name-input")).toHaveCount(0);
      await expect(playerPage.getByTestId("add-npc-button")).toHaveCount(0);

      // Default-Viewer access on this NPC (DM never granted the Player
      // Editor/Owner) — no Edit action in the preview panel, and the
      // inline row Edit link is also absent (both gated on the same
      // myPermissionLevel field, spec 010).
      await expect(playerPage.getByTestId("actor-preview-panel-view")).toBeVisible();
      await expect(playerPage.getByTestId("actor-preview-panel-edit")).toHaveCount(0);
    } finally {
      await playerContext.close();
    }
  });
});
