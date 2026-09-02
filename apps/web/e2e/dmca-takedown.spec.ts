import { test, expect, type Page } from "@playwright/test";
import { createItemViaCompendium } from "./fixtures/content";

/**
 * specs/015-dmca-notice-takedown: the public, unauthenticated DMCA notice
 * intake channel (`/legal/dmca`) and the moderation banner it produces on
 * the disabled content's own detail page. Resolver-level coverage already
 * exists in mutations_moderation.rs/queries/moderation.rs; this is the
 * missing browser-level check that the real form (TakedownNoticeForm.tsx,
 * a plain, hand-rolled form with no prior e2e coverage) actually submits
 * and that the disabled content's own page reflects it (ModeratedContentBanner).
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

/** Registers a GM, creates a world, and adds one Item via the Compendium,
 * returning its detail-page id. Mirrors world-compendium.spec.ts's own
 * addItemFromCompendium helper. */
async function createWorldWithItem(
  page: Page,
  itemName: string,
): Promise<{ worldId: string; itemId: string }> {
  await register(page, freshCredentials("e2edmcaowner"));
  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(`E2E DMCA Takedown ${uniqueSuffix()}`);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const worldMatch = /\/world\/([^/]+)\/staging$/.exec(
    new URL(page.url()).pathname,
  );
  if (!worldMatch)
    throw new Error(`Could not extract world id from URL: ${page.url()}`);
  const worldId = worldMatch[1];

  await createItemViaCompendium(page, worldId, itemName);
  await expect(page.getByTestId("item-catalog-table")).toContainText(itemName, {
    timeout: 10_000,
  });
  await page.getByTestId("item-catalog-table").getByText(itemName).click();
  await page.getByTestId("item-preview-panel-view").click();
  await page.waitForURL(/\/item\/[^/]+\/view$/, { timeout: 15_000 });
  const itemMatch = /\/item\/([^/]+)\/view$/.exec(new URL(page.url()).pathname);
  if (!itemMatch)
    throw new Error(`Could not extract item id from URL: ${page.url()}`);
  return { worldId, itemId: itemMatch[1] };
}

test.describe("US1/US2: a claimant files a DMCA takedown notice against an item", () => {
  test("a complete, statutorily-sufficient notice disables the item and shows the moderation banner", async ({
    browser,
  }) => {
    const ownerContext = await browser.newContext();
    const ownerPage = await ownerContext.newPage();
    const itemName = `Infringing Statue ${uniqueSuffix()}`;
    const { worldId, itemId } = await createWorldWithItem(ownerPage, itemName);

    // The intake channel itself requires no authentication (FR-002) — a
    // genuinely distinct, logged-out context, not the item owner's session.
    const claimantContext = await browser.newContext();
    const claimantPage = await claimantContext.newPage();
    await claimantPage.goto("/legal/dmca");
    await expect(
      claimantPage.getByTestId("takedown-notice-form"),
    ).toBeVisible();

    await claimantPage.getByLabel("Content type").click();
    await claimantPage.getByRole("option", { name: "Item" }).click();
    await claimantPage.locator("#dmca-entity-id").fill(itemId);
    await claimantPage.locator("#dmca-claimant-name").fill("Jane Claimant");
    await claimantPage
      .locator("#dmca-claimant-contact")
      .fill("jane.claimant@example.test");
    await claimantPage
      .locator("#dmca-work-description")
      .fill("An original bronze statue design, registered copyright.");
    await claimantPage
      .locator("#dmca-infringing-location")
      .fill(`Item detail page for "${itemName}" in a ThunderForge world.`);
    await claimantPage.locator("#dmca-good-faith").click();
    await claimantPage.locator("#dmca-accuracy").click();
    await claimantPage.locator("#dmca-signature").fill("Jane Claimant");

    await claimantPage.getByTestId("takedown-notice-submit").click();
    await expect(
      claimantPage.getByTestId("takedown-notice-accepted"),
    ).toBeVisible({ timeout: 10_000 });
    await expect(
      claimantPage.getByTestId("takedown-notice-accepted"),
    ).toContainText("Case reference");

    // The item's own detail page, for anyone (including its owner),
    // must now show the disabled-content banner instead of its content.
    await ownerPage.goto(`/world/${worldId}/item/${itemId}/view`);
    await expect(ownerPage.getByText("Content disabled")).toBeVisible({
      timeout: 10_000,
    });
    await expect(
      ownerPage.getByText(/disabled in response to a DMCA takedown notice/i),
    ).toBeVisible();

    await ownerContext.close();
    await claimantContext.close();
  });

  test("an incomplete notice (missing statutory elements) is rejected, not silently dropped", async ({
    page,
  }) => {
    // A valid, existing entity id is required for this test to exercise
    // the statutory-completeness check specifically: the resolver checks
    // the target entity exists before it evaluates completeness
    // (`Actor not found`-style errors take precedence — confirmed via
    // mutations_moderation.rs's `resolve_entity` helper), so a
    // fabricated/non-existent id would hit that unrelated error path
    // instead of the one this test is verifying.
    const itemName = `Incomplete Notice Target ${uniqueSuffix()}`;
    const { itemId } = await createWorldWithItem(page, itemName);

    await page.goto("/legal/dmca");
    await page.getByLabel("Content type").click();
    await page.getByRole("option", { name: "Item" }).click();
    // Leave the required good-faith/accuracy statements unchecked, but
    // fill just enough to get past HTML5 `required` validation on the
    // text fields so the submit actually reaches the server (FR-003: an
    // incomplete notice must still be logged, not client-side-blocked
    // before ever reaching the resolver).
    await page.locator("#dmca-entity-id").fill(itemId);
    await page.locator("#dmca-claimant-name").fill("Incomplete Claimant");
    await page
      .locator("#dmca-claimant-contact")
      .fill("incomplete@example.test");
    await page.locator("#dmca-work-description").fill("Some work.");
    await page.locator("#dmca-infringing-location").fill("Somewhere.");
    await page.locator("#dmca-signature").fill("Incomplete Claimant");
    // Deliberately do NOT check good-faith/accuracy — the statutory
    // elements this test is verifying get flagged, not silently accepted.

    await page.getByTestId("takedown-notice-submit").click();
    await expect(page.getByTestId("takedown-notice-rejected")).toBeVisible({
      timeout: 10_000,
    });
    await expect(page.getByTestId("takedown-notice-rejected")).toContainText(
      /notice incomplete/i,
    );
  });
});
