import { expect, type Page } from "@playwright/test";

/**
 * Creating world content that a test needs but is not about.
 *
 * # Why this exists
 *
 * Several specs create an NPC because they need *an actor*, not because they
 * are testing the compendium: `players-section.spec.ts` needs someone to put in
 * a players list, `actor-claim.spec.ts` needs a character to claim. Each had
 * grown its own copy of the same flow, reaching straight for
 * `new-npc-name-input` and `add-npc-button`.
 *
 * That coupled three specs to the *shape* of one screen. Spec 031 (FR-035)
 * moves NPC creation off the list and onto a dedicated editing page, and
 * without this fixture that change would have to edit three files — two of
 * which have nothing to do with the compendium. With it, the change is one
 * edit here.
 *
 * `world-compendium.spec.ts` deliberately does *not* use this. That spec is
 * genuinely about the compendium's own creation surface, including asserting
 * the form is absent for a player who may not create. A test about a screen
 * should drive that screen.
 */

/**
 * Create an NPC through the compendium and return its actor id.
 *
 * Goes through the UI rather than the API on purpose: these specs depend on the
 * actor existing the way a Game Master would have made it, and the id is only
 * stated plainly in the detail route's path. If that ever becomes a burden, the
 * body can move to `createActor` over GraphQL without any caller changing.
 */
export async function createNpcViaCompendium(
  page: Page,
  worldId: string,
  label: string,
  description?: string,
): Promise<string> {
  await page.goto(`/world/${worldId}/compendium`);
  await page.locator('[data-testid="new-npc-name-input"]').fill(label);
  if (description !== undefined) {
    await page
      .locator('[data-testid="new-npc-description-input"]')
      .fill(description);
  }
  await page.locator('[data-testid="add-npc-button"]').click();

  const row = page.locator("tr", { hasText: label });
  await expect(row).toBeVisible({ timeout: 10_000 });
  await row.getByRole("link", { name: "View" }).click();
  await page.waitForURL(/\/actor\/[^/]+\/view$/, { timeout: 10_000 });

  const match = /\/actor\/([^/]+)\/view$/.exec(new URL(page.url()).pathname);
  if (!match) {
    throw new Error(`Could not extract actor id from URL: ${page.url()}`);
  }
  return match[1];
}
