import { test, expect, type Page } from "@playwright/test";
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
 * Playtest 2026-09-10 P9: "walls aren't shading properly".
 *
 * They could not. The engine's darkness layer — the sheet over the map, the
 * light pools cut out of it, and the wall shadows cast back into those pools —
 * draws nothing in daylight, and nothing in the product ever told the engine a
 * scene was anything else. `engine-lighting-limits.spec.ts` pinned that as
 * "zero shadow quads at every level". This drives the real control: the Game
 * Master sets the scene's light in the Lights panel, and the shadows appear —
 * for them, for a player who only heard about it on the event channel, and
 * again after a reload.
 */

async function shadowQuads(page: Page): Promise<number> {
  return page.evaluate(async () => {
    const mod = (await import(
      /* @vite-ignore */ "/src/engine/bevy/stats.ts"
    )) as typeof import("../src/engine/bevy/stats");
    const stats = await mod.readEngineStats();
    return stats?.shadowQuads ?? 0;
  });
}

async function wallCount(page: Page): Promise<number> {
  return page.evaluate(() => window.__worldProbe?.state()?.counts.walls ?? 0);
}

test.describe("Scene light (playtest 2026-09-10 P9)", () => {
  test("a Game Master dims the scene, its walls cast shadows for everyone, and it stays set", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    await registerAndCreateWorld(page, `E2E Scene Light ${uniqueSuffix()}`);
    const worldId = /\/world\/([^/]+)/.exec(new URL(page.url()).pathname)![1];
    const sceneId = await createScene(page, worldId, "Lit Scene");
    // Visible to players, so the second client below reads the same scene.
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
    await launchSceneByName(page, worldId, "Lit Scene");
    await page.goto(`/world/${worldId}/play`);
    await waitForEngineReady(page);

    // One shadow-casting light, and one vision-blocking wall inside its pool.
    for (const [query, input] of [
      [
        `mutation ($input: GraphQLCreateLightSourceInput!) { createLightSource(input: $input) { lightId } }`,
        { sceneId, x: 0, y: 0, radius: 600, intensity: 1, castsShadows: true },
      ],
      [
        `mutation ($input: GraphQLCreateWallInput!) { createWall(input: $input) { wallId } }`,
        { sceneId, x1: 150, y1: -200, x2: 150, y2: 200, blocksVision: true },
      ],
    ] as const) {
      const result = await graphql<{ errors?: unknown }>(page, query, {
        input,
      });
      expect(result.errors).toBeUndefined();
    }
    await expect.poll(() => wallCount(page), { timeout: 15_000 }).toBe(1);

    // Daylight draws no darkness, so no shadow: where every scene was stuck.
    await page.waitForTimeout(1_000);
    expect(await shadowQuads(page)).toBe(0);

    await page.getByTestId("gm-tool-lights").click();
    const bright = page.getByTestId("scene-ambient-bright");
    const dim = page.getByTestId("scene-ambient-dim");
    const dark = page.getByTestId("scene-ambient-dark");
    await expect(bright).toHaveAttribute("aria-pressed", "true");
    await dim.click();
    await expect(dim).toHaveAttribute("aria-pressed", "true");
    await expect
      .poll(() => shadowQuads(page), {
        timeout: 15_000,
        message: "a dim scene draws the wall's shadow",
      })
      .toBeGreaterThan(0);

    // A player joining now loads the scene as the Game Master left it.
    const player = await inviteAndJoinAsPlayer(browser, page, worldId);
    await player.goto(`/world/${worldId}/play`);
    await waitForEngineReady(player);
    await expect.poll(() => wallCount(player), { timeout: 15_000 }).toBe(1);
    await expect
      .poll(() => shadowQuads(player), {
        timeout: 30_000,
        message: "the player's canvas loads the scene dim",
      })
      .toBeGreaterThan(0);

    // And follows a change live, without reloading — the event channel.
    await bright.click();
    await expect
      .poll(() => shadowQuads(player), {
        timeout: 15_000,
        message: "back to daylight, the player's shadows go with it",
      })
      .toBe(0);
    await dark.click();
    await expect
      .poll(() => shadowQuads(player), { timeout: 15_000 })
      .toBeGreaterThan(0);

    // It is the scene's setting, not this session's: a reload comes back dark.
    await page.reload();
    await waitForEngineReady(page);
    await expect
      .poll(() => shadowQuads(page), { timeout: 30_000 })
      .toBeGreaterThan(0);
    await page.getByTestId("gm-tool-lights").click();
    await expect(page.getByTestId("scene-ambient-dark")).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    await player.context().close();
  });
});
