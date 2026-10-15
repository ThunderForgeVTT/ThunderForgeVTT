import { expect, test, type Page } from "@playwright/test";
import { BOOT_TIMEOUT, BUILT, engine } from "./support/engine.ts";
import { expectClean, openLanding } from "./support/page.ts";

/**
 * The page opens in the tavern: beside the headline on a wide page, under it
 * on a narrower one. The map lies on the page's own grid with the party
 * inside (on a phone it fits between the gutters), and the page's table
 * draws it.
 */

const inn = (page: Page) => page.locator("[data-testid=hero-tavern]");

test("a wide page starts in the tavern, on the page's grid", async ({ page }, info) => {
  test.skip(info.project.name !== "desktop", "the tavern opens the page from 1200px");
  const opened = await openLanding(page);
  await expect(inn(page)).toBeVisible();
  // The map, the party of four, and three goblins on the grass below.
  await expect(inn(page).locator("img")).toHaveCount(8);
  // Beside the headline, its corner on a grid line, one square to a cell.
  const box = await page.evaluate(() => {
    const cell = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--cell"));
    const h1 = document.querySelector("h1")!.getBoundingClientRect();
    const r = document.querySelector("[data-testid=hero-tavern]")!.getBoundingClientRect();
    return { cell, h1: h1.right, x: r.left + scrollX, y: r.top + scrollY, w: r.width, h: r.height };
  });
  expect(box.x).toBeGreaterThanOrEqual(box.h1);
  expect(box.x % box.cell).toBeCloseTo(0, 1);
  expect(box.y % box.cell).toBeCloseTo(0, 1);
  expect([box.w, box.h]).toEqual([11 * box.cell, 8 * box.cell]);
  // The tavern is not met again below, and the marker room is gone.
  await expect(page.locator("#tavern")).toHaveCount(0);
  await expect(page.locator(".field, .door-toggle, .token")).toHaveCount(0);
  await expect(page.locator(".hero-tavern-credit")).toContainText("MBRound18");
  await expectClean(opened);
});

test("a narrower page has the tavern under the headline, inside the screen", async ({ page }, info) => {
  if (info.project.name === "desktop") await page.setViewportSize({ width: 1100, height: 900 });
  const opened = await openLanding(page);
  await expect(inn(page)).toBeVisible();
  await expect(inn(page).locator("img")).toHaveCount(8);
  const box = await page.evaluate(() => {
    const cell = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--cell"));
    const h1 = document.querySelector("h1")!.getBoundingClientRect();
    const r = document.querySelector("[data-testid=hero-tavern]")!.getBoundingClientRect();
    return {
      cell,
      h1: h1.bottom,
      top: r.top,
      left: r.left,
      right: innerWidth - r.right,
      w: r.width,
      h: r.height,
      scroll: document.documentElement.scrollWidth - innerWidth,
    };
  });
  expect(box.top).toBeGreaterThan(box.h1);
  // Eleven squares by eight, a cell a square at most, with a gutter each side.
  expect(box.w / box.h).toBeCloseTo(11 / 8, 2);
  expect(box.w).toBeLessThanOrEqual(11 * box.cell + 0.5);
  expect(box.left).toBeGreaterThanOrEqual(16);
  expect(box.right).toBeGreaterThanOrEqual(15.5);
  expect(box.scroll).toBe(0);
  await expect(page.locator("#tavern")).toHaveCount(0);
  await expectClean(opened);
});

test("the page's table holds the tavern from the start, the company in it", async ({ page }, info) => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.skip(info.project.name !== "desktop", "the engine boots at the desktop width only");
  test.setTimeout(BOOT_TIMEOUT + 60_000);
  const opened = await openLanding(page); // emulates reduce
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.reload();

  await expect(page.locator("body.in-engine")).toBeAttached({ timeout: BOOT_TIMEOUT });
  await expect(page.locator("#page-table canvas")).toHaveCount(1);
  // Everyone but the walker stands in the tavern as an engine token, and
  // the three goblins on the grass below.
  await expect
    .poll(async () => {
      const all = await engine<{ tokenId: string }[]>(page, "token_footprints");
      return all.filter((t) => t.tokenId.startsWith("mat-")).length;
    })
    .toBe(6);
  await expect(inn(page).locator(".hero-tavern-art")).toHaveCSS("visibility", "hidden");
  await expectClean(opened);
});
