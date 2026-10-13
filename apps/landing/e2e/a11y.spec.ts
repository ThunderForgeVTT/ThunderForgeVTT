import { expect, test, type Page } from "@playwright/test";
import { axeReport, focusStop, overflowReport, stopProblems } from "./support/a11y.ts";
import { BOOT_TIMEOUT, BUILT } from "./support/engine.ts";
import { expectClean, openLanding, SCHEMES } from "./support/page.ts";
import { bootRoom, FIXED_SEEDS, playThrough, room, roomFrame } from "./support/room.ts";

/**
 * Spec 090 US1 (FR-003, FR-004): the whole front page, at this project's
 * width, in both colour schemes. Spec 091 SC-005 adds the room's states:
 * at rest (the page as it loads), completed in words, and with the engine
 * running in the room's frame.
 */

async function expectAccessible(page: Page, what: string) {
  const axe = await axeReport(page);
  expect(axe, `${what}: axe\n${axe.join("\n")}`).toEqual([]);
  const overflow = await overflowReport(page);
  expect(overflow, `${what}: overflow\n${overflow.join("\n")}`).toEqual([]);
}

for (const scheme of SCHEMES) {
  test.describe(`${scheme} scheme`, () => {
    test("axe finds nothing on the whole page", async ({ page }) => {
      const opened = await openLanding(page, { scheme });
      const report = await axeReport(page);
      expect(report, report.join("\n")).toEqual([]);
      expectClean(opened);
    });

    test("nothing runs past the right edge, and no text is clipped", async ({ page }) => {
      const opened = await openLanding(page, { scheme });
      const found = await overflowReport(page);
      expect(found, found.join("\n")).toEqual([]);
      expectClean(opened);
    });

    test("with the room completed in words, axe and the edges still find nothing", async ({ page }) => {
      await page.addInitScript(FIXED_SEEDS);
      const opened = await openLanding(page, { scheme });
      await playThrough(page);
      await expectAccessible(page, `completed, ${scheme}`);
      expectClean(opened);
    });
  });
}

test.describe("with the engine running in the room", () => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.beforeEach(({}, info) => {
    test.skip(info.project.name !== "desktop", "the engine boots at the desktop width only");
    test.setTimeout(BOOT_TIMEOUT + 300_000);
  });

  // One boot, then each width and scheme: the frame follows both live.
  test("axe, the edges and the room's focus stops, at three widths and two schemes (US2 scenarios 4 and 5)", async ({
    page,
  }) => {
    const opened = await openLanding(page);
    await bootRoom(page);
    const canvas = roomFrame(page).locator("canvas");
    await expect(canvas).toHaveAttribute("aria-hidden", "true");
    await expect(canvas).toHaveAttribute("tabindex", "-1");

    for (const width of [1440, 768, 390]) {
      for (const scheme of SCHEMES) {
        await page.setViewportSize({ width, height: 900 });
        await page.emulateMedia({ colorScheme: scheme });
        await expectAccessible(page, `booted, ${width}px, ${scheme}`);
      }

      // The room's stops, from the hero picker to Start over, over the
      // running frame: each one shown, on screen and uncovered.
      // Arrive by keyboard, as a visitor would: a focus from script after
      // the click on "Play the room" is not :focus-visible.
      await room(page).getByRole("radio").first().focus();
      await page.keyboard.press("Shift+Tab");
      await page.keyboard.press("Tab");
      const problems: string[] = [];
      for (let i = 0; i < 30; i += 1) {
        const stop = await focusStop(page);
        if (!stop || !(await page.evaluate(() => !!document.activeElement?.closest("#room")))) break;
        problems.push(...stopProblems(stop).map((p) => `${width}px ${p}`));
        await page.keyboard.press("Tab");
      }
      expect(problems, problems.join("\n")).toEqual([]);
    }
    expectClean(opened);
  });
});

test("the keyboard walk starts with a skip link, and every stop is shown and on screen", async ({
  page,
}) => {
  const opened = await openLanding(page);
  await page.keyboard.press("Tab");
  const first = page.locator(":focus");
  await expect(first).toHaveText(/skip to content/i);
  await expect(first).toBeInViewport();
  await page.keyboard.press("Enter");
  await expect(page.locator(":focus")).toHaveAttribute("id", "content");

  // From the top again: every stop, in order, until focus leaves the page
  // or comes round to the start.
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.locator("body").focus();
  const problems: string[] = [];
  const seen = new Set<string>();
  for (let i = 0; i < 200; i++) {
    await page.keyboard.press("Tab");
    const stop = await focusStop(page);
    if (!stop || seen.has(stop.key)) break;
    seen.add(stop.key);
    problems.push(...stopProblems(stop));
  }
  expect(seen.size).toBeGreaterThan(10);
  expect(problems, problems.join("\n")).toEqual([]);
  expectClean(opened);
});

test("the GitHub link in the nav keeps its name when the star count fails", async ({ page }) => {
  await page.route("**/gh/repo", (route) => route.fulfill({ status: 503, body: "" }));
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/");
  await expect(
    page.locator("nav").getByRole("link", { name: /ThunderForge on GitHub/ }),
  ).toBeVisible();
});
