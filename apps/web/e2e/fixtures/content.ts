import { type Page } from "@playwright/test";

/**
 * Creating world content that a test needs but is not about.
 *
 * # Why this exists
 *
 * Several specs create an NPC because they need *an actor*, not because they
 * are testing the compendium: `players-section.spec.ts` needs someone to put in
 * a players list, `actor-claim.spec.ts` needs a character to claim. Each had
 * grown its own copy of the same flow, reaching straight for
 * `new-npc-name-input` and `add-npc-button` — locators that no longer exist.
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
 * actor existing the way a Game Master would have made it, and the id is
 * stated plainly in the route the editor redirects to on save. If that ever becomes a burden, the
 * body can move to `createActor` over GraphQL without any caller changing.
 */
export async function createNpcViaCompendium(
  page: Page,
  worldId: string,
  label: string,
  description?: string,
): Promise<string> {
  // The editor page, not the tab. Spec 031 FR-035 removed the inline form:
  // creating an NPC meant filling a cramped block inside a list, so both
  // compendium tabs became list-plus-link with a real page behind them.
  await page.goto(`/world/${worldId}/compendium/npc/new`);
  await page.locator('[data-testid="npc-editor-name-input"]').fill(label);
  if (description !== undefined) {
    await page
      .locator('[data-testid="npc-editor-description-input"]')
      .fill(description);
  }
  await page.locator('[data-testid="npc-editor-save"]').click();

  // Saving redirects to the new NPC's own edit route, which states the id
  // plainly — so there is no longer any need to find a row and follow a
  // View link to learn it.
  await page.waitForURL(/\/compendium\/npc\/[^/]+\/edit$/, {
    timeout: 15_000,
  });

  const match = /\/compendium\/npc\/([^/]+)\/edit$/.exec(
    new URL(page.url()).pathname,
  );
  if (!match) {
    throw new Error(`Could not extract actor id from URL: ${page.url()}`);
  }
  return match[1];
}

/**
 * Create an item through the compendium.
 *
 * The same move as `createNpcViaCompendium` and for the same reason: spec 031
 * FR-035 replaced both inline forms with editor pages, so every spec that
 * created an item had its own copy of a flow that has now changed once and
 * would otherwise have to change in five places again next time.
 *
 * Unlike the NPC helper this returns nothing. Saving an item redirects to the
 * items *list*, not to the item's own route, so the id is not stated anywhere
 * a test can read it. The two callers that need an id already take it from the
 * `createItem` GraphQL response, which is unaffected — inventing a URL round
 * trip here to recover something they already have would be worse.
 */
export async function createItemViaCompendium(
  page: Page,
  worldId: string,
  name: string,
  description?: string,
): Promise<void> {
  await page.goto(`/world/${worldId}/compendium/item/new`);
  await page.locator('[data-testid="item-editor-name-input"]').fill(name);
  if (description !== undefined) {
    await page
      .locator('[data-testid="item-editor-description-input"]')
      .fill(description);
  }
  await page.locator('[data-testid="item-editor-save"]').click();

  // Back on the items tab is how the save reports success.
  await page.waitForURL(/\/compendium\?tab=items$/, { timeout: 15_000 });
}
