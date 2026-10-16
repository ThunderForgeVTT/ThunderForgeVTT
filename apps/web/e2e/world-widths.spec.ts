import { test, expect, type Page } from "./fixtures/test";
import {
  graphql,
  registerAndCreateWorld,
  setWorldSystem,
} from "./fixtures/helpers";

/**
 * Spec 100 US4 (SC-005): every world page fills the screen it is on. At
 * 390, 1440 and 3440 px nothing scrolls sideways, and at 3440 the content
 * spans at least 75% of the viewport, lined up with the header above it.
 * One describe per page, so each runs alone.
 */

const WIDTHS = [
  { width: 390, height: 844 },
  { width: 1440, height: 900 },
  { width: 3440, height: 1440 },
];

function scrollsSideways(page: Page): Promise<boolean> {
  return page.evaluate(
    () => document.documentElement.scrollWidth > window.innerWidth,
  );
}

async function createCharacter(page: Page, worldId: string): Promise<string> {
  const result = await graphql<{
    data?: { createActor?: { id: string } };
    errors?: { message: string }[];
  }>(
    page,
    `
      mutation ($input: CreateActorInput!) {
        createActor(input: $input) {
          id
        }
      }
    `,
    {
      input: {
        worldId,
        label: "Wide Hero",
        isNpc: false,
        gameSystemId: "dnd5e",
      },
    },
  );
  const id = result.data?.createActor?.id;
  if (!id) {
    throw new Error(`createActor: ${JSON.stringify(result.errors ?? result)}`);
  }
  return id;
}

/** Walks the widths on one page: `path` from the world, `ready` shows it
 * loaded, `content` is the box that must fill the ultrawide screen. */
async function fillsEveryWidth(
  page: Page,
  path: string,
  ready: string,
  content: string,
) {
  for (const { width, height } of WIDTHS) {
    await test.step(`${width} px`, async () => {
      await page.setViewportSize({ width, height });
      await page.goto(path);
      await expect(page.getByTestId(ready)).toBeVisible({ timeout: 15_000 });
      expect(await scrollsSideways(page), "no sideways scroll").toBe(false);
      if (width < 3440) return;

      const box = await page.getByTestId(content).boundingBox();
      expect(box).not.toBeNull();
      expect(
        box!.width,
        "content spans 75% of the screen",
      ).toBeGreaterThanOrEqual(width * 0.75);
      const header = page.getByRole("banner").locator(":scope > div").first();
      if (await header.isVisible()) {
        const headerBox = await header.boundingBox();
        expect(
          Math.abs(headerBox!.x - box!.x),
          "the header lines up with the content",
        ).toBeLessThanOrEqual(1);
      }
    });
  }
}

test.describe("overview", () => {
  test("fills phone, desktop and ultrawide", async ({ page }) => {
    test.setTimeout(120_000);
    const worldId = await registerAndCreateWorld(page, "Wide hall", "e2ewide");
    await fillsEveryWidth(
      page,
      `/world/${worldId}`,
      "world-dashboard-grid",
      "world-section-shell",
    );
  });
});

test.describe("players", () => {
  test("fills phone, desktop and ultrawide", async ({ page }) => {
    test.setTimeout(120_000);
    const worldId = await registerAndCreateWorld(page, "Wide roll", "e2ewide");
    await fillsEveryWidth(
      page,
      `/world/${worldId}/players`,
      "players-list",
      "world-section-shell",
    );
  });
});

test.describe("scenes", () => {
  test("fills phone, desktop and ultrawide", async ({ page }) => {
    test.setTimeout(120_000);
    const worldId = await registerAndCreateWorld(page, "Wide maps", "e2ewide");
    await fillsEveryWidth(
      page,
      `/world/${worldId}/scenes`,
      "scenes-table",
      "world-section-shell",
    );
  });
});

test.describe("character", () => {
  test("fills phone, desktop and ultrawide", async ({ page }) => {
    test.setTimeout(120_000);
    const worldId = await registerAndCreateWorld(page, "Wide hero", "e2ewide");
    await setWorldSystem(page, worldId, "dnd5e");
    const actorId = await createCharacter(page, worldId);
    await fillsEveryWidth(
      page,
      `/world/${worldId}/actor/${actorId}/edit`,
      "actor-view-grid",
      "actor-view",
    );
  });
});
