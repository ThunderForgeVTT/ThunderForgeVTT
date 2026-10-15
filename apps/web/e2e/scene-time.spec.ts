import { test, expect, type Page } from "./fixtures/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  launchSceneByName,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { createScene } from "./fixtures/world-cache";

/**
 * Spec 099 US5 (SC-006): a Game Master sets the table's time, and every
 * client's engine runs the same clock. The hour is asked of each engine
 * (`__engineProbe.timeOfDay`), not read off the page: the clock lives in the
 * engine, and the page only ever hears it.
 */

type Clock = { hour: number; darkness: number } | null;

function clockOf(page: Page): Promise<Clock> {
  return page.evaluate(
    () =>
      (
        window as unknown as {
          __engineProbe?: { timeOfDay?: () => Clock };
        }
      ).__engineProbe?.timeOfDay?.() ?? null,
  );
}

test.describe("Scene time of day (spec 099 US5)", () => {
  test("a Game Master sets 22:00 over 2s, and a player's engine reaches 22:00 too", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    await registerAndCreateWorld(page, `E2E Scene Time ${uniqueSuffix()}`);
    const worldId = /\/world\/([^/]+)/.exec(new URL(page.url()).pathname)![1];
    const sceneId = await createScene(page, worldId, "Clock Scene");
    await graphql(
      page,
      `
        mutation ($sceneId: UUID!) {
          updateSceneHidden(sceneId: $sceneId, hidden: false) {
            sceneId
          }
        }
      `,
      { sceneId },
    );
    await launchSceneByName(page, worldId, "Clock Scene");
    await page.goto(`/world/${worldId}/play`);
    await waitForEngineReady(page);

    const player = await inviteAndJoinAsPlayer(browser, page, worldId);
    await player.goto(`/world/${worldId}/play`);
    await waitForEngineReady(player);

    // A scene made before the clock has none, and draws as it did.
    await player.waitForTimeout(1_000);
    expect(await clockOf(player)).toBeNull();

    await page.getByTestId("gm-tool-lights").click();
    await page.getByTestId("scene-time-hour").fill("12:00");
    await page.getByTestId("scene-time-set").click();
    await expect
      .poll(async () => (await clockOf(player))?.hour, {
        timeout: 15_000,
        message: "the player's clock jumps to noon",
      })
      .toBeCloseTo(12, 2);

    // Passing time: both engines run the transition, not a jump.
    await page.getByTestId("scene-time-hour").fill("22:00");
    await page.getByTestId("scene-time-over").fill("2");
    await page.getByTestId("scene-time-set").click();
    const seen: number[] = [];
    await expect
      .poll(
        async () => {
          const hour = (await clockOf(player))?.hour ?? 0;
          seen.push(hour);
          return hour;
        },
        {
          timeout: 15_000,
          intervals: [100],
          message: "the player's clock reaches 22:00",
        },
      )
      .toBeCloseTo(22, 2);
    expect(
      seen.some((hour) => hour > 12.05 && hour < 21.95),
      `the player's clock passed through the evening: ${seen.join(", ")}`,
    ).toBe(true);
    await expect
      .poll(async () => (await clockOf(page))?.hour, { timeout: 15_000 })
      .toBeCloseTo(22, 2);
    await expect(page.getByTestId("scene-time")).toContainText("22:00");

    // The hour is the scene's: a player who reloads lands at it at once.
    await player.reload();
    await waitForEngineReady(player);
    await expect
      .poll(async () => (await clockOf(player))?.hour, { timeout: 30_000 })
      .toBeCloseTo(22, 2);
    await expect
      .poll(async () => (await clockOf(player))?.darkness ?? 0, {
        timeout: 15_000,
        message: "22:00 is night",
      })
      .toBeGreaterThan(0.5);

    // No clock: the scene's light rules again, for everyone.
    await page.getByTestId("scene-time-clear").click();
    await expect.poll(() => clockOf(player), { timeout: 15_000 }).toBeNull();

    await player.context().close();
  });
});
