import { test, expect, type Page } from "./fixtures/test";
import { registerAndCreateWorld } from "./fixtures/helpers";

/**
 * Spec 088 US4 (SC-004, FR-033 to FR-036, contracts/layouts.md): the world
 * page uses the width it has. One column on a phone, two from 1024 px, three
 * from 1536 px, the content capped at 1800 px and centred, and no sideways
 * scroll from 320 px to 3840 px. The Players card counts the members and the
 * open links, and points to the players page. The actor view's half is
 * `actor-layout-widths.spec.ts`, in the actors slice.
 */

function columns(page: Page): Promise<number> {
  return page.getByTestId("world-dashboard-grid").evaluate(
    (grid) =>
      getComputedStyle(grid)
        .gridTemplateColumns.split(" ")
        .filter((track) => track.trim() !== "").length,
  );
}

function scrollsSideways(page: Page): Promise<boolean> {
  return page.evaluate(
    () => document.documentElement.scrollWidth > window.innerWidth,
  );
}

const WIDTHS: { width: number; columns: number; capped: boolean }[] = [
  { width: 320, columns: 1, capped: false },
  { width: 375, columns: 1, capped: false },
  { width: 1280, columns: 2, capped: false },
  { width: 2560, columns: 3, capped: true },
  { width: 3840, columns: 3, capped: true },
];

test("the world page takes one, two or three columns, capped at 1800 px, with a Players card", async ({
  page,
}) => {
  test.setTimeout(120_000);
  const worldId = await registerAndCreateWorld(page, "Wide world", "e2ewworld");

  for (const { width, columns: expected, capped } of WIDTHS) {
    await test.step(`${width} px`, async () => {
      await page.setViewportSize({ width, height: 1000 });
      await page.goto(`/world/${worldId}`);
      await expect(page.getByTestId("world-dashboard-grid")).toBeVisible({
        timeout: 15_000,
      });
      await expect.poll(() => columns(page)).toBe(expected);
      expect(await scrollsSideways(page), "no sideways scroll").toBe(false);
      if (capped) {
        const box = await page.getByTestId("world-dashboard").boundingBox();
        expect(box).not.toBeNull();
        expect(Math.abs(box!.width - 1800)).toBeLessThanOrEqual(1);
      }
    });
  }

  // The Players card: the GM alone, no link yet, and the way to make one.
  const card = page.getByTestId("world-players-card");
  await expect(card.getByTestId("world-players-members")).toHaveText(
    "1 member",
  );
  await expect(card.getByTestId("world-players-links")).toHaveText(
    "0 active links",
  );
  await card.getByTestId("world-players-page-link").click();
  await expect(page).toHaveURL(new RegExp(`/world/${worldId}/players$`));
});
