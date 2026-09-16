import { test, expect } from "./fixtures/test";
import { expectNoAxeViolations } from "./fixtures/axe";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import { createNpcViaCompendium } from "./fixtures/content";
import { createScene } from "./fixtures/world-cache";

/**
 * The world dashboard leads with figures and names only the scenes touched
 * last.
 *
 * The owner: "imagine 40 scenes we've been running for five years. Showing
 * all the scenes would be chaotic." So the world here has forty, and the
 * assertions are that the dashboard says forty-one (the starter scene too)
 * and lists five.
 */

const STATS = `
  query DashboardStats($worldId: UUID!) {
    worldStatistics(worldId: $worldId) {
      scenes
      members
      characters
      npcs
      tokens
    }
  }
`;

type StatsResponse = {
  data?: {
    worldStatistics: {
      scenes: number;
      members: number;
      characters: number;
      npcs: number | null;
      tokens: number;
    };
  };
  errors?: unknown;
};

const SCENE_COUNT = 40;

test.describe("The world dashboard shows figures, not every scene", () => {
  test("forty scenes read as a figure and a short list, at desktop and phone widths", async ({
    page,
    browser,
  }) => {
    test.setTimeout(240_000);
    const worldId = await registerAndCreateWorld(
      page,
      `Five Year Campaign ${uniqueSuffix()}`,
      "e2edash",
    );
    for (let index = 1; index <= SCENE_COUNT; index += 1) {
      await createScene(page, worldId, `Session ${index} ${uniqueSuffix()}`);
    }
    await createNpcViaCompendium(page, worldId, "A Goblin Nobody Has Met");

    await page.goto(`/world/${worldId}`);
    const figures = page.getByTestId("world-statistics");
    await expect(figures).toHaveAttribute("aria-busy", "false", {
      timeout: 20_000,
    });

    // The figure, read as a term and its value.
    const scenesFigure = page.getByTestId("world-stat-scenes");
    await expect(scenesFigure.locator("dt")).toHaveText("Scenes");
    await expect(scenesFigure.locator("dd").first()).toHaveText(
      String(SCENE_COUNT + 1),
    );
    // A Game Master is told about NPCs.
    await expect(page.getByTestId("world-stat-npcs")).toBeVisible();
    await expect(page.getByTestId("world-stat-players")).toBeVisible();
    await expect(page.getByTestId("world-stat-tokens")).toBeVisible();

    // Five names, not forty-one, and a way to the rest.
    await expect(page.getByTestId("recent-scene")).toHaveCount(5);
    await expect(page.getByTestId("recent-scenes-all-link")).toHaveText(
      `All ${SCENE_COUNT + 1} scenes`,
    );
    // The most recently made scene is the first named.
    await expect(page.getByTestId("recent-scene").first()).toContainText(
      `Session ${SCENE_COUNT} `,
    );

    // Same speed at forty scenes as at three: the figures are counted on the
    // server, so the request's cost is a handful of indexed COUNTs whatever
    // the world holds. Measured rather than asserted tightly — a shared test
    // machine is noisy — but a regression to "fetch everything and count in
    // the browser" would blow well past this.
    const started = Date.now();
    const stats = await graphql<StatsResponse>(page, STATS, { worldId });
    const elapsed = Date.now() - started;
    console.log(
      `[world-dashboard] worldStatistics at ${SCENE_COUNT + 1} scenes: ${elapsed}ms`,
    );
    expect(stats.errors).toBeUndefined();
    expect(stats.data?.worldStatistics.scenes).toBe(SCENE_COUNT + 1);
    expect(stats.data?.worldStatistics.npcs).toBeGreaterThanOrEqual(1);
    expect(elapsed).toBeLessThan(2_000);

    for (const width of [1440, 375]) {
      await page.setViewportSize({ width, height: 900 });
      await page.goto(`/world/${worldId}`);
      await expect(figures).toHaveAttribute("aria-busy", "false", {
        timeout: 20_000,
      });
      await expectNoAxeViolations(page, "[data-testid=world-at-a-glance]");
      const overflows = await page.evaluate(
        () =>
          document.documentElement.scrollWidth >
          document.documentElement.clientWidth + 1,
      );
      expect(overflows, `page scrolls sideways at ${width}px`).toBe(false);
    }
    await page.setViewportSize({ width: 1280, height: 900 });

    // A player asking the same question is told fewer figures, and never
    // one a hidden NPC could move. Asked through the real server as the
    // player: the dashboard sends a player without a character to Actor
    // Selection first, which is not what this spec is about.
    const player = await inviteAndJoinAsPlayer(
      browser,
      page,
      worldId,
      "e2edashp",
    );
    const asPlayer = await graphql<StatsResponse>(player, STATS, { worldId });
    expect(asPlayer.errors).toBeUndefined();
    expect(asPlayer.data?.worldStatistics.npcs).toBeNull();
    // New scenes are hidden from players until a Game Master shows them.
    expect(asPlayer.data?.worldStatistics.scenes).toBeLessThan(SCENE_COUNT + 1);
    await player.context().close();
  });
});
