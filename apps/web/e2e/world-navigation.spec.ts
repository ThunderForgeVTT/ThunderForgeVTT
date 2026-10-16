import type { Page } from "@playwright/test";
import { test, expect } from "./fixtures/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Spec 101: moving between a world's pages keeps the page on screen. The
 * app remembers what it learned about the world, shows it at once on the
 * next page, and refreshes it behind the page.
 */

/**
 * Records, from now on, every time the full-screen loader enters the page
 * and every time the world's sidebar is drawn with or without the Game
 * Master's group. A MutationObserver rather than a poll: a loader or a late
 * control can come and go between two polls, and that blink is exactly
 * what this spec is about.
 */
async function watchPaints(page: Page) {
  await page.evaluate(() => {
    const seen = { loader: 0, sidebarWithoutAdmin: 0, admin: 0 };
    (window as unknown as { __seen: typeof seen }).__seen = seen;
    const look = () => {
      if (document.querySelector('[data-testid="page-loader"]')) {
        seen.loader += 1;
      }
      const sidebar = document.querySelector(
        '[data-testid="world-sidebar-nav"]',
      );
      const admin = document.querySelector('[data-testid="world-nav-admin"]');
      if (admin) seen.admin += 1;
      if (sidebar && !admin) seen.sidebarWithoutAdmin += 1;
    };
    new MutationObserver(look).observe(document.body, {
      childList: true,
      subtree: true,
    });
  });
}

const seen = (page: Page) =>
  page.evaluate(
    () =>
      (
        window as unknown as {
          __seen: {
            loader: number;
            sidebarWithoutAdmin: number;
            admin: number;
          };
        }
      ).__seen,
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
    await expect(page.getByTestId("world-nav-admin")).toBeVisible();
    // A client-side route change only: the observer lives in this document.
    await watchPaints(page);

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
      const now = await seen(page);
      expect(now.loader, `loader shown on the way to ${name}`).toBe(0);
      // SC-002: the GM's group is there on the first paint, not added late.
      expect(
        now.sidebarWithoutAdmin,
        `sidebar drawn without GM controls on the way to ${name}`,
      ).toBe(0);
    }
  });

  test("a player never sees the Game Master's controls, even for a moment (FR-006)", async ({
    page,
    browser,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Navigation Player ${uniqueSuffix()}`,
    );
    const player = await inviteAndJoinAsPlayer(browser, page, worldId);
    await player.goto(`/world/${worldId}/players`);
    await expect(player.getByTestId("players-search-input")).toBeVisible();
    await watchPaints(player);

    await goBySidebar(player, "Scenes", /\/scenes$/);
    await goBySidebar(player, "NPCs", /\/compendium\?tab=npcs$/);
    await goBySidebar(player, "Players", /\/players$/);
    await expect(player.getByTestId("players-search-input")).toBeVisible();
    const now = await seen(player);
    expect(now.admin).toBe(0);
    expect(now.loader).toBe(0);
    await player.context().close();
  });

  test("a promotion shows on the promoted player's next page (US2.3)", async ({
    page,
    browser,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Navigation Promote ${uniqueSuffix()}`,
    );
    const player = await inviteAndJoinAsPlayer(browser, page, worldId);
    await player.goto(`/world/${worldId}/players`);
    await expect(player.getByTestId("players-search-input")).toBeVisible();
    await expect(player.getByTestId("world-nav-admin")).toHaveCount(0);

    await page.goto(`/world/${worldId}/players`);
    const roleSelect = page
      .getByTestId("players-list")
      .locator('select[data-testid^="player-role-select-"]')
      .first();
    await roleSelect.selectOption("GM");
    await expect(roleSelect).toHaveValue("GM", { timeout: 10_000 });

    await goBySidebar(player, "Scenes", /\/scenes$/);
    await expect(player.getByTestId("world-nav-admin")).toBeVisible();
    await player.context().close();
  });

  test("the overview remembers a cleared character check, and a lost claim still redirects (SC-003)", async ({
    page,
    browser,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Navigation Claim ${uniqueSuffix()}`,
    );
    const created = await graphql<{
      data?: { createActor?: { id: string } };
    }>(
      page,
      `
        mutation ($input: CreateActorInput!) {
          createActor(input: $input) {
            id
          }
        }
      `,
      { input: { worldId, label: `Wren ${uniqueSuffix()}`, isNpc: false } },
    );
    const actorId = created.data?.createActor?.id;
    expect(actorId).toBeTruthy();
    await graphql(
      page,
      `
        mutation ($actorId: UUID!) {
          setActorAvailability(actorId: $actorId, available: true) {
            id
          }
        }
      `,
      { actorId },
    );
    const player = await inviteAndJoinAsPlayer(browser, page, worldId);
    const claimed = await graphql<{
      data?: { claimActor?: { actorId: string } };
      errors?: unknown;
    }>(
      player,
      `
        mutation ($worldId: UUID!, $actorId: UUID!) {
          claimActor(worldId: $worldId, actorId: $actorId) {
            actorId
          }
        }
      `,
      { worldId, actorId },
    );
    expect(claimed.errors).toBeUndefined();

    await player.goto(`/world/${worldId}`);
    await expect(player.getByTestId("world-staging-page")).toBeVisible();
    await watchPaints(player);
    await goBySidebar(player, "Players", /\/players$/);
    await goBySidebar(player, "Overview", new RegExp(`/world/${worldId}$`));
    await expect(player.getByTestId("world-staging-page")).toBeVisible();
    expect((await seen(player)).loader).toBe(0);

    // The check still runs behind the page: a claim lost while the player
    // is elsewhere sends them back to choose on their next visit, even
    // though the overview starts from the remembered clearance.
    await goBySidebar(player, "Players", /\/players$/);
    const released = await graphql<{ errors?: unknown }>(
      page,
      `
        mutation ($actorId: UUID!) {
          unclaimActor(actorId: $actorId) {
            id
          }
        }
      `,
      { actorId },
    );
    expect(released.errors).toBeUndefined();
    await goBySidebar(player, "Overview", new RegExp(`/world/${worldId}$`));
    await player.waitForURL(new RegExp(`/world/${worldId}/actor-select$`));
    await player.context().close();
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
