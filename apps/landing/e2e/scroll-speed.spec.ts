import { expect, test, type Page } from "@playwright/test";
import { BOOT_TIMEOUT, BUILT } from "./support/engine.ts";
import { expectClean, openLanding } from "./support/page.ts";

/**
 * A visitor flicks, not strolls. A frame takes the table only once the
 * scroll rests on it: a take swaps the scene, and a flick that swapped two
 * scenes on the way past stuttered on a phone. The tavern and the dungeon
 * lie on the page's own table at every width, so neither takes it.
 */

// Under 1200px, where both rooms once had frames of their own.
test.beforeEach(async ({ page }, info) => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.skip(info.project.name !== "desktop", "the engine boots at the desktop width only");
  test.setTimeout(BOOT_TIMEOUT + 90_000);
  await page.setViewportSize({ width: 1100, height: 900 });
});

/** Every frame that held the table, sampled each animation frame. */
async function watchHolders(page: Page) {
  await page.evaluate(() => {
    const held = new Set<string>();
    (window as unknown as { held: Set<string> }).held = held;
    const tick = () => {
      const c = document.querySelector("#thunderforge-table");
      const frame = c?.closest("#page-table, #tavern-table, #room-table");
      if (frame) held.add(frame.id);
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
}
const holders = (page: Page) =>
  page.evaluate(() => [...(window as unknown as { held: Set<string> }).held].sort());

async function bootPage(page: Page) {
  const opened = await openLanding(page); // emulates reduce
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.reload();
  await expect(page.locator("#page-table canvas")).toHaveCount(1, { timeout: BOOT_TIMEOUT });
  return opened;
}

/** Where the page ends below the dungeon, in scroll pixels. */
const pastRoom = (page: Page) =>
  page.evaluate(() => Math.round(document.querySelector("#room")!.getBoundingClientRect().bottom + scrollY + innerHeight));

test("a fast flick past the tavern and the dungeon hands the table to neither", async ({ page }) => {
  const opened = await bootPage(page);
  await watchHolders(page);
  const end = await pastRoom(page);
  await page.mouse.move(550, 450);
  // A thumb's flick: a screen and a half each frame or two.
  while ((await page.evaluate(() => scrollY)) < end) {
    await page.mouse.wheel(0, 1400);
    await page.waitForTimeout(30);
  }
  // At rest, well below both rooms, the page still has its table.
  await page.waitForTimeout(1500);
  await expect(page.locator("#page-table canvas")).toHaveCount(1);
  expect(await holders(page)).toEqual(["page-table"]);
  await expectClean(opened);
});

test("a slow scroll keeps the table on the page, past the tavern and the dungeon", async ({ page }) => {
  const opened = await bootPage(page);
  await watchHolders(page);
  const end = await pastRoom(page);
  await page.mouse.move(550, 450);
  // A reader's pace: a line or two at a time.
  while ((await page.evaluate(() => scrollY)) < end) {
    await page.mouse.wheel(0, 120);
    await page.waitForTimeout(90);
  }
  await page.waitForTimeout(1500);
  expect(await holders(page)).toEqual(["page-table"]);
  await expectClean(opened);
});
