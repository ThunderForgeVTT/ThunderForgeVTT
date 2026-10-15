import { expect, test, type Page } from "@playwright/test";
import { BOOT_TIMEOUT, BUILT } from "./support/engine.ts";
import { expectClean, openLanding } from "./support/page.ts";

/**
 * A wide page opens in the tavern, beside the headline: the hero starts
 * inside with the rest of the party, leaves by the back door, and follows
 * the path down to the dungeon. Narrower pages keep the marker room.
 */

const inn = (page: Page) => page.locator(".hero-tavern");
const innFrame = (page: Page) => page.locator("#inn-table");
const square = (page: Page) => inn(page).locator(".room-layout").getAttribute("data-square");
const said = (page: Page) => inn(page).locator(".room-said");
const goal = (page: Page, id: string) => inn(page).locator(`[data-goal="${id}"]`);

async function step(page: Page, dir: string, to: string) {
  await inn(page).getByRole("group", { name: "Move one square" }).getByRole("button", { name: dir }).click();
  await expect.poll(() => square(page)).toBe(to);
}

test("a wide page starts in the tavern, and the hero leaves by the back door", async ({ page }, info) => {
  test.skip(info.project.name !== "desktop", "the tavern opens the page from 1200px");
  const opened = await openLanding(page);
  await expect(inn(page)).toBeVisible();
  // Beside the headline, not under it; and the tavern is not met again below.
  const [h1, frame] = await page.evaluate(() =>
    ["h1", ".hero-tavern"].map((s) => document.querySelector(s)!.getBoundingClientRect().left),
  );
  expect(frame).toBeGreaterThan(h1! + 300);
  await expect(page.locator("#tavern")).toHaveCount(0);
  await expect(page.locator(".field, .door-toggle, .token")).toHaveCount(0);

  await expect(inn(page).locator("[data-goal]")).toHaveCount(3);
  expect(await square(page)).toBe("-1,1.5");
  await step(page, "East", "0,1.5");
  for (const y of ["0.5", "-0.5", "-1.5", "-2.5"]) await step(page, "South", `0,${y}`);
  await expect(goal(page, "door-reached")).toContainText("(done)");
  await inn(page).getByRole("button", { name: "Open the back door" }).click();
  await expect(goal(page, "door-opened")).toContainText("(done)");
  await step(page, "South", "0,-3.5");
  await expect(goal(page, "room-left")).toContainText("(done)");
  await expect(said(page)).toContainText("You step out of the back door onto the path.");

  await inn(page).getByRole("button", { name: "Follow the path down" }).click();
  await expect
    .poll(() => page.evaluate(() => Math.abs(document.querySelector("#room")!.getBoundingClientRect().top)))
    .toBeLessThan(100);
  await expectClean(opened);
});

test("a narrower page keeps the marker room, and the tavern further down", async ({ page }, info) => {
  test.skip(info.project.name !== "desktop", "one narrow width is enough");
  await page.setViewportSize({ width: 1100, height: 900 });
  const opened = await openLanding(page);
  await expect(page.locator(".field")).toHaveCount(1);
  await expect(inn(page)).toHaveCount(0);
  await expect(page.locator("#tavern")).toHaveCount(1);
  await expectClean(opened);
});

test("the tavern holds the table at the top, and gives it to the page further down", async ({ page }, info) => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.skip(info.project.name !== "desktop", "the engine boots at the desktop width only");
  test.setTimeout(BOOT_TIMEOUT + 60_000);
  const opened = await openLanding(page); // emulates reduce
  // The tavern is in view from the start: full motion, then load again, so
  // the frame boots on its own as a visitor's would.
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.reload();

  await expect(innFrame(page)).toHaveClass(/engine-frame--running/, { timeout: BOOT_TIMEOUT });
  await expect(innFrame(page).locator("canvas")).toHaveCount(1);

  // Well past the tavern, the page's own table takes it.
  await page.evaluate(() => scrollTo(0, innerHeight * 3));
  await expect(page.locator("#page-table canvas")).toHaveCount(1, { timeout: 60_000 });
  await expect(innFrame(page)).not.toHaveClass(/engine-frame--running/);

  // And back up: the tavern takes it again.
  await page.evaluate(() => scrollTo(0, 0));
  await expect(innFrame(page)).toHaveClass(/engine-frame--running/, { timeout: 60_000 });
  await expectClean(opened);
});
