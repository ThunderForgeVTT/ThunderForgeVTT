import { test, expect, type Page } from "@playwright/test";
import { createNpcViaCompendium } from "./fixtures/content";

/**
 * specs/010-world-staging-actors (US5): share an actor via a link, view
 * it read-only as a completely unrelated user, copy it into one of that
 * user's own worlds as a fully independent actor, and revoke the link.
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
  await register(page, freshCredentials("e2eshare"));
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

async function createNpcAndOpenEdit(page: Page, worldId: string, npcName: string): Promise<string> {
  // Spec 011: NPC creation moved from staging to the dedicated
  // /compendium route.
  await page.goto(`/world/${worldId}/compendium`);
  // Spec 031 FR-035 moved NPC authoring onto its own page, so the
  // fixture creates it and hands back the id — no row to find, no View
  // link to follow.
  const seededActorId = await createNpcViaCompendium(page, worldId, npcName);
  await page.goto(`/world/${worldId}/actor/${seededActorId}/view`);
  await page.waitForURL(new RegExp(`/world/${worldId}/actor/[^/]+/view$`), { timeout: 15_000 });
  await page.getByRole("button", { name: "Edit" }).click();
  await page.waitForURL(new RegExp(`/world/${worldId}/actor/([^/]+)/edit$`), { timeout: 15_000 });
  const match = new RegExp(`/world/${worldId}/actor/([^/]+)/edit$`).exec(
    new URL(page.url()).pathname,
  );
  if (!match) throw new Error(`Could not extract actor id from URL: ${page.url()}`);
  return match[1];
}

test.describe("US5: share an actor, copy it independently, then revoke", () => {
  test("an unrelated user previews, copies, edits independently; revoke then blocks the link", async ({
    page,
    browser,
  }) => {
    const npcName = `Bo Jangles ${uniqueSuffix()}`;
    const sourceWorldId = await registerAndCreateWorld(page, `E2E Share Source ${uniqueSuffix()}`);
    const actorId = await createNpcAndOpenEdit(page, sourceWorldId, npcName);

    await page.getByRole("button", { name: "Share" }).click();
    const shareUrl = await page.getByTestId("share-link-input").inputValue();
    expect(shareUrl).toContain("/shared/actor/");

    const otherContext = await browser.newContext();
    const otherPage = await otherContext.newPage();
    let destinationWorldId: string;
    const destinationWorldName = `E2E Share Destination ${uniqueSuffix()}`;
    try {
      destinationWorldId = await registerAndCreateWorld(otherPage, destinationWorldName);

      await otherPage.goto(new URL(shareUrl).pathname);
      await expect(otherPage.getByRole("heading", { name: npcName })).toBeVisible({
        timeout: 10_000,
      });
      // Read-only: no edit/save controls, no indication of the source world.
      await expect(otherPage.getByRole("button", { name: "Save" })).toHaveCount(0);
      await expect(otherPage.getByTestId("actor-ownership-block")).toHaveCount(0);

      await otherPage.getByRole("button", { name: "Copy to World" }).click();
      await otherPage.locator("select").selectOption({ label: destinationWorldName });
      await otherPage.getByRole("button", { name: "Confirm copy" }).click();
      await expect(otherPage.getByRole("heading", { name: "Copied!" })).toBeVisible({
        timeout: 10_000,
      });

      // The copy is a real, independent actor in the destination world's
      // roster (spec 011: viewed via /compendium, not /staging).
      await otherPage.goto(`/world/${destinationWorldId}/compendium`);
      await expect(otherPage.getByTestId("npc-catalog-table")).toContainText(npcName, {
        timeout: 10_000,
      });
      await otherPage
        .getByTestId("npc-catalog-table")
        .locator("tr", { hasText: npcName })
        .getByRole("link", { name: "View" })
        .click();
      await otherPage.waitForURL(/\/actor\/[^/]+\/view$/, { timeout: 15_000 });
      await otherPage.getByRole("button", { name: "Edit" }).click();
      await otherPage.locator("#actor-label").fill("Renamed Copy");
      await otherPage.getByRole("button", { name: "Save" }).click();
      await expect(otherPage.getByText("Saved.")).toBeVisible({ timeout: 10_000 });
    } finally {
      await otherContext.close();
    }

    // Revoke the link now, on the same (never-reloaded) `page` whose local
    // state still holds the "Revoke link" control from generating it above
    // — the product doesn't yet offer a way to re-discover/manage an
    // existing share link after navigating away and back (no "list this
    // actor's share links" query exists), so the revoke action is only
    // reachable in the same session that created it. It must no longer
    // resolve for anyone afterward.
    await page.getByRole("button", { name: "Revoke link" }).click();
    await expect(page.getByText("Share link revoked.")).toBeVisible({ timeout: 10_000 });

    // Editing the copy must not have touched the source actor.
    await page.goto(`/world/${sourceWorldId}/actor/${actorId}/view`);
    await expect(page.getByRole("heading", { name: npcName })).toBeVisible({ timeout: 10_000 });

    const laterContext = await browser.newContext();
    const laterPage = await laterContext.newPage();
    try {
      await register(laterPage, freshCredentials("e2eshareafter"));
      await laterPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
      await laterPage.goto(new URL(shareUrl).pathname);
      await expect(laterPage.getByText("no longer available")).toBeVisible({ timeout: 10_000 });
    } finally {
      await laterContext.close();
    }
  });

  test("a user with no DM-level world sees an explanatory state instead of a picker", async ({
    page,
    browser,
  }) => {
    const npcName = `Bo Jangles ${uniqueSuffix()}`;
    const sourceWorldId = await registerAndCreateWorld(page, `E2E Share NoDest ${uniqueSuffix()}`);
    await createNpcAndOpenEdit(page, sourceWorldId, npcName);
    await page.getByRole("button", { name: "Share" }).click();
    const shareUrl = await page.getByTestId("share-link-input").inputValue();

    const outsiderContext = await browser.newContext();
    const outsiderPage = await outsiderContext.newPage();
    try {
      await register(outsiderPage, freshCredentials("e2esharenodm"));
      await outsiderPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
      // Deliberately does not create a world — has zero DM-level access anywhere.
      await outsiderPage.goto(new URL(shareUrl).pathname);
      await expect(outsiderPage.getByRole("heading", { name: npcName })).toBeVisible({
        timeout: 10_000,
      });
      await outsiderPage.getByRole("button", { name: "Copy to World" }).click();
      await expect(
        outsiderPage.getByText("You don't have DM-level access to any world yet"),
      ).toBeVisible({ timeout: 10_000 });
    } finally {
      await outsiderContext.close();
    }
  });
});
