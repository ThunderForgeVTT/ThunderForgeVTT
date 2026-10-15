import { expect, test, type Page } from "@playwright/test";
import { BOOT_TIMEOUT, BUILT, engine } from "./support/engine.ts";
import { openLanding } from "./support/page.ts";
import { roomFrame } from "./support/room.ts";

/**
 * The first screen's mat on the page's table. Once the page holds the engine
 * the mat's own tokens are hidden and the engine draws them, so a token the
 * engine never got is a token gone from the screen.
 */

interface Footprint {
  tokenId: string;
  footprint: number;
}

/** The mat's tokens on the table, by id, with their size in squares. */
async function matTokens(page: Page) {
  const all = await engine<Footprint[]>(page, "token_footprints");
  return Object.fromEntries(
    all.filter((t) => t.tokenId.startsWith("mat-")).map((t) => [t.tokenId, t.footprint]),
  );
}

test.describe("the mat on the page's table", () => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.beforeEach(({}, info) => {
    test.skip(info.project.name !== "desktop", "the engine boots at the desktop width only");
    test.setTimeout(BOOT_TIMEOUT + 120_000);
  });

  test("the mat's tokens come back whole after the room had the table", async ({ page }) => {
    // The marker room's mat: from 1200px the first screen is the tavern instead.
    await page.setViewportSize({ width: 1100, height: 1400 });
    await openLanding(page, { settle: false });
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await page.reload();
    await expect(page.locator("body.in-engine")).toBeAttached({ timeout: BOOT_TIMEOUT });

    await expect.poll(async () => Object.keys(await matTokens(page)).length).toBeGreaterThan(0);
    const first = await matTokens(page);

    await roomFrame(page).scrollIntoViewIfNeeded();
    await expect(roomFrame(page).locator("#thunderforge-table")).toBeAttached({ timeout: 60_000 });
    await expect.poll(async () => Object.keys(await matTokens(page)).length).toBe(0);

    await page.evaluate(() => window.scrollTo(0, 0));
    await expect(page.locator("body.in-engine")).toBeAttached({ timeout: 60_000 });
    await expect.poll(() => matTokens(page)).toEqual(first);
  });
});
