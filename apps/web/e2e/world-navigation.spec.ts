import type { Page } from "@playwright/test";
import { test, expect } from "./fixtures/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Spec 101: moving between a world's pages keeps the page on screen. The
 * app remembers what it learned about the world, shows it at once on the
 * next page, and refreshes it behind the page.
 */

/**
 * Counts every time the full-screen loader enters the page from now on.
 * A MutationObserver rather than a poll: the loader can come and go between
 * two polls, and that blink is exactly what this spec is about.
 */
async function watchForLoader(page: Page) {
  await page.evaluate(() => {
    const w = window as unknown as { __loaderSeen: number };
    w.__loaderSeen = 0;
    new MutationObserver(() => {
      if (document.querySelector('[data-testid="page-loader"]')) {
        w.__loaderSeen += 1;
      }
    }).observe(document.body, { childList: true, subtree: true });
  });
}

const loaderSeen = (page: Page) =>
  page.evaluate(
    () => (window as unknown as { __loaderSeen: number }).__loaderSeen,
  );

async function goBySidebar(page: Page, name: string, url: RegExp) {
  await page
    .getByTestId("world-sidebar-nav")
    .getByRole("link", { name, exact: true })
    .click();
  await page.waitForURL(url);
}

test.describe("Spec 101: smooth world navigation", () => {
  test("a Game Master moves between a world's pages without a loader (SC-001)", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Navigation ${uniqueSuffix()}`,
    );
    await page.goto(`/world/${worldId}`);
    await expect(page.getByTestId("world-staging-page")).toBeVisible();
    // A client-side route change only: the observer lives in this document.
    await watchForLoader(page);

    const stops: [string, RegExp, string][] = [
      ["Players", /\/players$/, "players-search-input"],
      ["Scenes", /\/scenes$/, "new-scene-name-input"],
      ["NPCs", /\/compendium\?tab=npcs$/, "world-compendium-page"],
      [
        "System settings & license",
        /\/settings\/system$/,
        "world-system-settings-page",
      ],
      ["Overview", new RegExp(`/world/${worldId}$`), "world-staging-page"],
    ];
    for (const [name, url, testId] of stops) {
      await goBySidebar(page, name, url);
      await expect(page.getByTestId(testId)).toBeVisible();
      expect(await loaderSeen(page), `loader shown on the way to ${name}`).toBe(
        0,
      );
    }
  });

  test("a change made elsewhere shows on the next page (FR-003)", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Navigation Notes ${uniqueSuffix()}`,
    );
    await page.goto(`/world/${worldId}`);
    await expect(page.getByTestId("world-staging-page")).toBeVisible();
    await goBySidebar(page, "Players", /\/players$/);

    const notes = `Left off at the crypt door ${uniqueSuffix()}`;
    const res = await graphql<{ errors?: unknown }>(
      page,
      `
        mutation ($input: UpdateWorldSessionNotesInput!) {
          updateWorldSessionNotes(input: $input) {
            id
          }
        }
      `,
      { input: { worldId, notes } },
    );
    expect(res.errors).toBeUndefined();

    await goBySidebar(page, "Overview", new RegExp(`/world/${worldId}$`));
    await expect(page.getByTestId("session-notes-rendered")).toContainText(
      notes,
    );
  });

  test("a save on settings is what the next visit shows (FR-004)", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Navigation Grid ${uniqueSuffix()}`,
    );
    await page.goto(`/world/${worldId}/settings/system`);
    const picker = page.getByTestId("default-scene-grid-type-picker");
    await picker.click();
    await page.getByRole("option", { name: "Hexagons" }).click();
    await expect(picker).toContainText("Hexagons");

    await goBySidebar(page, "Scenes", /\/scenes$/);
    await goBySidebar(page, "System settings & license", /\/settings\/system$/);
    await expect(picker).toContainText("Hexagons");
  });

  test("a world deleted elsewhere is not shown from memory (FR-005)", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Navigation Gone ${uniqueSuffix()}`,
    );
    await page.goto(`/world/${worldId}`);
    await expect(page.getByTestId("world-staging-page")).toBeVisible();
    await goBySidebar(page, "Players", /\/players$/);

    const res = await graphql<{ errors?: unknown }>(
      page,
      `
        mutation ($id: UUID!) {
          deleteWorld(id: $id) {
            id
          }
        }
      `,
      { id: worldId },
    );
    expect(res.errors).toBeUndefined();

    await goBySidebar(page, "Overview", new RegExp(`/world/${worldId}$`));
    await expect(
      page.getByRole("heading", { name: "This world could not be opened." }),
    ).toBeVisible();
  });
});
