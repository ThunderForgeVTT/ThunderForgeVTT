import { expect, test, type Page } from "@playwright/test";
import { BOOT_TIMEOUT, BUILT } from "./support/engine.ts";
import { expectClean, openLanding } from "./support/page.ts";
import { roomFrame } from "./support/room.ts";

/**
 * The tavern above the dungeon: the hero comes in by the tavern's door,
 * leaves by the back door, and follows the path down to the dungeon. One
 * engine serves both rooms; the frame in view holds the table.
 */

const tavern = (page: Page) => page.locator("#tavern");
const tavernFrame = (page: Page) => page.locator("#tavern-table");
const square = (page: Page) => tavern(page).locator(".room-layout").getAttribute("data-square");
const said = (page: Page) => tavern(page).locator(".room-said");
const goal = (page: Page, id: string) => tavern(page).locator(`[data-goal="${id}"]`);

async function step(page: Page, dir: string, to: string) {
  await tavern(page).getByRole("group", { name: "Move one square" }).getByRole("button", { name: dir }).click();
  await expect.poll(() => square(page)).toBe(to);
}

test("the tavern is walked through in words, and the path leads down to the dungeon", async ({ page }) => {
  const opened = await openLanding(page);
  await tavern(page).scrollIntoViewIfNeeded();
  await expect(said(page)).toBeAttached();

  // The tavern comes before the dungeon, and plays three goals, no chest.
  const [t, d] = await page.evaluate(() =>
    ["#tavern", "#room"].map((s) => document.querySelector(s)!.getBoundingClientRect().top),
  );
  expect(t).toBeLessThan(d!);
  await expect(tavern(page).locator("[data-goal]")).toHaveCount(3);
  await expect(tavern(page).getByRole("button", { name: /^Roll/ })).toHaveCount(0);

  expect(await square(page)).toBe("-2,3.5");
  await step(page, "East", "-1,3.5");
  await step(page, "East", "0,3.5");
  await expect(goal(page, "door-reached")).toContainText("(done)");
  await tavern(page).getByRole("button", { name: "Open the tavern's door" }).click();
  await expect(goal(page, "door-opened")).toContainText("(done)");

  for (const y of ["2.5", "1.5", "0.5", "-0.5", "-1.5", "-2.5"]) await step(page, "South", `0,${y}`);
  await expect(said(page)).toContainText("You stand in the tavern.");
  // The back door is shut until it is opened.
  await tavern(page).getByRole("group", { name: "Move one square" }).getByRole("button", { name: "South" }).click();
  await expect(said(page)).toContainText("The back door is shut.");
  await tavern(page).getByRole("button", { name: "Open the back door" }).click();
  await step(page, "South", "0,-3.5");
  await expect(goal(page, "room-left")).toContainText("(done)");
  await expect(said(page)).toContainText("You step out of the back door onto the path.");

  await tavern(page).getByRole("button", { name: "Follow the path down" }).click();
  await expect
    .poll(() => page.evaluate(() => Math.abs(document.querySelector("#room")!.getBoundingClientRect().top)))
    .toBeLessThan(100);
  await expectClean(opened);
});

test("the table follows the hero down: the tavern holds it, then the dungeon", async ({ page }, info) => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.skip(info.project.name !== "desktop", "the engine boots at the desktop width only");
  test.setTimeout(BOOT_TIMEOUT + 60_000);
  const opened = await openLanding(page); // emulates reduce
  // A frame and its controls on one screen, as `bootRoom` has it.
  await page.setViewportSize({ width: 1280, height: 1400 });
  // Under reduced motion a frame waits to be asked; this is the walk itself.
  await page.emulateMedia({ reducedMotion: "no-preference" });

  await tavernFrame(page).evaluate((el) => el.scrollIntoView({ block: "start" }));
  await expect(tavernFrame(page)).toHaveClass(/engine-frame--running/, { timeout: BOOT_TIMEOUT });
  await expect(tavernFrame(page).locator("canvas")).toHaveCount(1);

  await roomFrame(page).evaluate((el) => el.scrollIntoView({ block: "start" }));
  await expect(roomFrame(page)).toHaveClass(/engine-frame--running/, { timeout: 60_000 });
  await expect(roomFrame(page).locator("canvas")).toHaveCount(1);
  await expect(tavernFrame(page)).not.toHaveClass(/engine-frame--running/);

  // And back up: the tavern takes it again, its hero where it was left.
  await tavernFrame(page).evaluate((el) => el.scrollIntoView({ block: "start" }));
  await expect(tavernFrame(page)).toHaveClass(/engine-frame--running/, { timeout: 60_000 });
  await expectClean(opened);
});
