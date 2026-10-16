import { test, expect, type Page } from "./fixtures/test";
import {
  graphql,
  openDockTab,
  registerAndCreateWorld,
  setWorldSystem,
} from "./fixtures/helpers";
import { claimFor } from "../playtest/combat";
import { closeTable, openTable, placeCast, sitDown } from "../playtest/table";

/**
 * Spec 100 US4 (SC-005): every world page fills the screen it is on. At
 * 390, 1440 and 3440 px nothing scrolls sideways, and at 3440 the content
 * spans at least 75% of the viewport, lined up with the header above it.
 * One describe per page, so each runs alone.
 *
 * SC-006: the 5e sheet lays out by its own width. Its ability inputs stay in
 * their cells from 320 to 3440 px, it has at least two columns at 1440 and
 * four at 3440, and in the play dock it reads in one.
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

/** The sheet's column count, read from its computed template. */
function sheetColumns(page: Page): Promise<number> {
  return page
    .getByTestId("dnd5e-actor-sheet")
    .first()
    .evaluate(
      (grid) =>
        getComputedStyle(grid)
          .gridTemplateColumns.split(" ")
          .filter((track) => track.trim() !== "").length,
    );
}

const ABILITIES = [
  "strength",
  "dexterity",
  "constitution",
  "intelligence",
  "wisdom",
  "charisma",
];

test.describe("character sheet", () => {
  test("ability inputs stay in their cells, and the sheet widens to four columns", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    const worldId = await registerAndCreateWorld(page, "Wide sheet", "e2ewide");
    await setWorldSystem(page, worldId, "dnd5e");
    const actorId = await createCharacter(page, worldId);
    await page.goto(`/world/${worldId}/actor/${actorId}/edit`);

    for (const width of [320, 390, 1024, 1440, 1920, 3440]) {
      await test.step(`${width} px`, async () => {
        await page.setViewportSize({ width, height: 1000 });
        await expect(page.getByTestId("dnd5e-actor-sheet")).toBeVisible({
          timeout: 15_000,
        });
        for (const ability of ABILITIES) {
          const cell = await page
            .getByTestId(`dnd5e-score-${ability}`)
            .boundingBox();
          const input = await page
            .getByTestId(`dnd5e-score-${ability}-input`)
            .boundingBox();
          expect(cell, ability).not.toBeNull();
          expect(input, ability).not.toBeNull();
          expect(input!.x, `${ability} left edge`).toBeGreaterThanOrEqual(
            cell!.x - 0.5,
          );
          expect(
            input!.x + input!.width,
            `${ability} right edge`,
          ).toBeLessThanOrEqual(cell!.x + cell!.width + 0.5);
        }
        if (width === 1440) {
          expect(await sheetColumns(page)).toBeGreaterThanOrEqual(2);
        }
        if (width === 3440) {
          expect(await sheetColumns(page)).toBeGreaterThanOrEqual(4);
        }
      });
    }
  });
});

test.describe("play dock sheet", () => {
  test("reads in one column beside the board", async ({
    page,
    browser,
  }, testInfo) => {
    test.setTimeout(5 * 60_000);
    const table = await openTable({
      browser,
      gm: page,
      testInfo,
      system: "dnd5e",
      players: ["Pip"],
    });
    const [pip] = table.players;
    try {
      const hero = await placeCast(table, {
        label: "Pip",
        at: { x: -192, y: 0 },
        seat: pip,
      });
      await claimFor(table, pip, hero.actorId);
      await pip.page.setViewportSize({ width: 3440, height: 1440 });
      await sitDown(table, pip.page);
      await openDockTab(pip.page, "actors");
      if (
        !(await pip.page.getByTestId("in-pane-character-sheet").isVisible())
      ) {
        await pip.page.getByTestId(`actor-view-${hero.actorId}`).click();
      }
      const sheet = pip.page.getByTestId("in-pane-character-sheet");
      await expect(sheet.getByTestId("dnd5e-actor-sheet")).toBeVisible({
        timeout: 15_000,
      });
      expect(await sheetColumns(pip.page)).toBe(1);
    } finally {
      await closeTable(table);
    }
  });
});
