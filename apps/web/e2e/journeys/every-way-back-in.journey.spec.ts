import { expect, test, type Page, type TestInfo } from "@playwright/test";
import { openAnotherClient } from "../fixtures/clients";
import { ensureSidebarOpen, uniqueSuffix } from "../fixtures/helpers";
import {
  enterPlayFromTheWorld,
  expectOnlyTheNotice,
  pausedOnServer,
  pauseThroughThePortal,
  seatATable,
  signInTheOperator,
} from "../fixtures/journeyTable";

/**
 * Spec 051 T069, US2 as people live it: after a pause, every way a person
 * tries to get back to the table ends on the notice — never on a playfield
 * that is blank, half-loaded, or live.
 *
 * The ways in, all through the product:
 *
 * - **refresh** on the notice;
 * - the browser's **Back** button, both *into* the playfield (the Game Master
 *   had stepped out to setup before the pause landed, so Back returns to
 *   `/play`) and back *to* the notice from the world list;
 * - a **bookmarked** `/world/:id/play` opened in a new tab;
 * - a **second tab** that was already on the playfield when the pause landed;
 * - the **same account in a second browser context**, standing in for a
 *   second device, signed in through the login page;
 * - **the world list**: the card's Enter world, then Play.
 *
 * Then the notice at a phone (375×812) and at a room screen (1920×1080):
 * readable, nothing overlapping, nothing off screen, with screenshots attached.
 * And a world that is not paused still refreshes into its playfield.
 */

/** The notice's parts a reader needs, in reading order. */
const NOTICE_PARTS = {
  heading: '[data-testid="play-paused-notice"] h1',
  status: '[data-testid="play-paused-status"]',
  guidance: '[data-testid="play-paused-notice"] > div:last-child > p',
  actions: '[data-testid="play-paused-notice"] > div:last-child > div',
} as const;

interface PartBox {
  name: string;
  left: number;
  top: number;
  right: number;
  bottom: number;
  fontSizePx: number;
  clipped: boolean;
}

async function measureNotice(page: Page): Promise<{
  parts: PartBox[];
  viewport: { width: number; height: number };
  scrollWidth: number;
}> {
  return page.evaluate((selectors) => {
    const parts = Object.entries(selectors).map(([name, selector]) => {
      const elements = Array.from(document.querySelectorAll(selector));
      if (elements.length !== 1) {
        throw new Error(
          `${name}: expected one element for ${selector}, found ${elements.length}`,
        );
      }
      const el = elements[0] as HTMLElement;
      const rect = el.getBoundingClientRect();
      return {
        name,
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
        fontSizePx: parseFloat(getComputedStyle(el).fontSize),
        clipped:
          el.scrollWidth > el.clientWidth + 1 ||
          el.scrollHeight > el.clientHeight + 1,
      };
    });
    return {
      parts,
      viewport: { width: window.innerWidth, height: window.innerHeight },
      scrollWidth: document.documentElement.scrollWidth,
    };
  }, NOTICE_PARTS);
}

function intersects(a: PartBox, b: PartBox): boolean {
  return (
    a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
  );
}

async function expectReadableAt(
  page: Page,
  testInfo: TestInfo,
  width: number,
  height: number,
): Promise<void> {
  await page.setViewportSize({ width, height });
  // Let layout settle at the new width (fonts, wrapping) before measuring.
  await expect(page.locator(NOTICE_PARTS.heading)).toBeVisible();
  await page.waitForTimeout(500);
  await page.evaluate(() => window.scrollTo(0, 0));

  // Written beside the run's other artifacts and attached by path, so the
  // two screenshots can be opened from the report without unpacking it.
  const shot = testInfo.outputPath(`notice-${width}x${height}.png`);
  await page.screenshot({ path: shot });
  await testInfo.attach(`notice-${width}x${height}.png`, {
    path: shot,
    contentType: "image/png",
  });

  const { parts, viewport, scrollWidth } = await measureNotice(page);
  testInfo.annotations.push({
    type: `notice ${width}x${height}`,
    description: JSON.stringify(parts),
  });

  expect(viewport).toEqual({ width, height });
  expect(scrollWidth, "no sideways scrolling").toBeLessThanOrEqual(width);
  for (const part of parts) {
    expect(part.left, `${part.name} starts on screen`).toBeGreaterThanOrEqual(
      0,
    );
    expect(part.top, `${part.name} starts on screen`).toBeGreaterThanOrEqual(0);
    expect(part.right, `${part.name} ends on screen`).toBeLessThanOrEqual(
      width,
    );
    expect(part.bottom, `${part.name} ends on screen`).toBeLessThanOrEqual(
      height,
    );
    expect(part.clipped, `${part.name} is not clipped`).toBe(false);
  }
  for (let i = 0; i < parts.length; i += 1) {
    for (let j = i + 1; j < parts.length; j += 1) {
      expect(
        intersects(parts[i], parts[j]),
        `${parts[i].name} and ${parts[j].name} must not overlap`,
      ).toBe(false);
    }
  }
  // Readable: the page's own type scale, not shrunk to fit. Body text is at
  // least the 18px that `text-xl`/`text-lg` give, the heading well above it.
  const byName = Object.fromEntries(parts.map((part) => [part.name, part]));
  expect(byName.heading.fontSizePx).toBeGreaterThanOrEqual(32);
  expect(byName.status.fontSizePx).toBeGreaterThanOrEqual(18);
  expect(byName.guidance.fontSizePx).toBeGreaterThanOrEqual(18);
}

test.describe("spec 051 US2 journey: every way back in ends on the notice", () => {
  test("refresh, Back, a bookmark, a second tab, a second device and the world list", async ({
    browser,
  }, testInfo) => {
    test.setTimeout(600_000);
    const table = await seatATable(browser, "WayBack");
    const operator = await signInTheOperator(browser);
    const [gmContext, playerContext] = table.contexts;
    const extraContexts = [operator.context()];
    const { worldId, worldName } = table;

    try {
      let secondTab: Page | undefined;
      await test.step("before the pause: a second Game Master tab is on the playfield", async () => {
        secondTab = await gmContext.newPage();
        await secondTab.goto(`/world/${worldId}/play`);
        await expect(secondTab.locator("canvas")).toBeVisible({
          timeout: 90_000,
        });
      });

      await test.step("before the pause: the Game Master's first tab steps out to setup", async () => {
        await ensureSidebarOpen(table.gmPage);
        await table.gmPage.getByTestId("back-to-staging-button").click();
        await table.gmPage.waitForURL(
          new RegExp(`/world/${worldId}/staging$`),
          {
            timeout: 15_000,
          },
        );
      });

      await test.step("the operator pauses the world through the portal", async () => {
        await pauseThroughThePortal(
          operator,
          worldId,
          worldName,
          `Journey grounds ${uniqueSuffix()}: every way back in`,
        );
        expect(pausedOnServer(worldId)).toBe(true);
      });

      await test.step("a second tab already on the playfield lands on the notice", async () => {
        await expectOnlyTheNotice(secondTab!, worldId, worldName);
      });

      await test.step("the player at the table lands on the notice", async () => {
        await expectOnlyTheNotice(table.playerPage, worldId, worldName);
      });

      await test.step("refresh: the notice again", async () => {
        await table.playerPage.reload();
        await expectOnlyTheNotice(table.playerPage, worldId, worldName);
      });

      await test.step("Back into the playfield: the notice", async () => {
        const gm = table.gmPage;
        // The tab stepped out to setup before the pause, so the entry behind
        // it is `/play`. Setup may itself have heard of the pause and moved
        // to the notice (replacing its own entry); Back reaches `/play`
        // either way.
        testInfo.annotations.push({
          type: "setup tab when the pause landed",
          description: new URL(gm.url()).pathname,
        });
        await gm.goBack();
        await expect(gm).not.toHaveURL(/\/staging$/);
        await expectOnlyTheNotice(gm, worldId, worldName);
      });

      await test.step("Back from the world list to the notice: the notice", async () => {
        const player = table.playerPage;
        await player.getByRole("link", { name: "Go to your worlds" }).click();
        await player.waitForURL(/\/worlds$/, { timeout: 15_000 });
        // The world list actually on screen, not only in the address bar: the
        // router keeps the notice rendered while the list's code loads, and a
        // Back inside that window never leaves the notice at all, which would
        // prove nothing about coming back to it.
        await expect(player.getByTestId("play-paused-notice")).toHaveCount(0, {
          timeout: 15_000,
        });
        // The list's own heading. Not this world's card: the list shows the
        // worlds a person owns, and a player owns none.
        await expect(
          player.getByRole("heading", {
            name: "Every world in your library.",
            level: 1,
          }),
        ).toBeVisible({ timeout: 20_000 });
        await player.goBack();
        await expectOnlyTheNotice(player, worldId, worldName);
      });

      await test.step("a bookmarked /world/:id/play in a new tab: the notice", async () => {
        const bookmark = await playerContext.newPage();
        await bookmark.goto(`/world/${worldId}/play`);
        await expectOnlyTheNotice(bookmark, worldId, worldName);
        await bookmark.close();
      });

      await test.step("the same account on a second device: the notice", async () => {
        const device = await openAnotherClient(
          browser,
          table.player,
          "context",
        );
        extraContexts.push(device.context());
        await device.waitForURL((url) => !url.pathname.startsWith("/login"), {
          timeout: 20_000,
          waitUntil: "commit",
        });
        await device.goto(`/world/${worldId}/play`);
        await expectOnlyTheNotice(device, worldId, worldName);
      });

      await test.step("the world list, Enter world, Play: the notice", async () => {
        const gm = secondTab!;
        await gm.getByRole("link", { name: "Go to your worlds" }).click();
        await gm.waitForURL(/\/worlds$/, { timeout: 15_000 });
        await expect(gm.getByTestId("play-paused-notice")).toHaveCount(0, {
          timeout: 15_000,
        });
        // The card is a plain surface with no role of its own; its Enter
        // world link is the one that points at this world.
        const enter = gm
          .getByRole("link", { name: "Enter world" })
          .and(gm.locator(`a[href="/world/${worldId}/staging"]`));
        await expect(enter).toBeVisible({ timeout: 20_000 });
        await expect(
          gm.getByRole("heading", { name: worldName, level: 2 }),
        ).toBeVisible();
        await enter.click();
        await gm.waitForURL(new RegExp(`/world/${worldId}/(staging|paused)$`), {
          timeout: 15_000,
        });
        // Setup can hear of the pause on its own and move to the notice
        // before Play is pressed: seen in practice, the button detaching
        // under the press. Either way is a way in that ends on the notice, so
        // Play is pressed if it is still there, and which way it went is
        // recorded rather than guessed at.
        const play = gm.getByTestId("play-button");
        const notice = gm.getByTestId("play-paused-notice");
        await expect(play.or(notice)).toBeVisible({ timeout: 20_000 });
        let pressed = false;
        if (await play.isVisible()) {
          pressed = await play
            .click({ timeout: 5_000 })
            .then(() => true)
            .catch(async (error: unknown) => {
              // Only a press the notice pre-empted is excused.
              await expect(notice).toBeVisible({ timeout: 10_000 });
              testInfo.annotations.push({
                type: "world list: Play pre-empted",
                description: String(error).split("\n")[0],
              });
              return false;
            });
        }
        testInfo.annotations.push({
          type: "world list path",
          description: pressed
            ? "setup, Play pressed, then the notice"
            : "setup moved to the notice by itself",
        });
        await expectOnlyTheNotice(gm, worldId, worldName);
      });

      await test.step("the notice at 375×812 and at 1920×1080: readable, nothing overlapping", async () => {
        const player = table.playerPage;
        await expectOnlyTheNotice(player, worldId, worldName);
        await expectReadableAt(player, testInfo, 375, 812);
        await expectReadableAt(player, testInfo, 1920, 1080);
      });

      await test.step("a world that is not paused still refreshes into its playfield", async () => {
        const gm = table.gmPage;
        const otherName = `Journey Unpaused ${uniqueSuffix()}`;
        await gm.goto("/worlds/create");
        await gm.locator("#world-name").fill(otherName);
        await gm.getByRole("button", { name: /create world/i }).click();
        await gm.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
        const otherId = /\/world\/([^/]+)\/staging$/.exec(
          new URL(gm.url()).pathname,
        )![1];
        await enterPlayFromTheWorld(gm, otherId);
        await expect(gm.locator("canvas")).toBeVisible({ timeout: 90_000 });
        await gm.reload();
        await expect(gm).toHaveURL(new RegExp(`/world/${otherId}/play$`));
        await expect(gm.locator("canvas")).toBeVisible({ timeout: 90_000 });
        await expect(gm.getByTestId("play-paused-notice")).toHaveCount(0);
        expect(pausedOnServer(otherId)).toBe(false);
      });
    } finally {
      for (const context of extraContexts) await context.close();
      for (const context of table.contexts) await context.close();
    }
  });
});
