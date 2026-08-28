import { expect, test } from "@playwright/test";
import type { WorldProbe } from "../src/engine/world/probe";
import {
  freshCredentials,
  inviteAndJoinAsPlayer,
  register,
  uniqueSuffix,
} from "./fixtures/helpers";

declare global {
  interface Window {
    __worldProbe?: WorldProbe;
  }
}

/**
 * A player is given the scene they are playing (spec 022 FR-008's edge).
 *
 * `hidden` is a rule about the Scenes *table* — it keeps a GM's unfinished
 * prep out of the players' scene browser. It was also filtering the scene a
 * world is actively playing, and since a newly created scene is hidden by
 * default (FR-003), that was every player in every new world: they had a
 * scene id from the world's unfiltered `activeSceneId` and no scene record
 * to go with it.
 *
 * What that costs is not cosmetic. The scene record is what carries the map
 * art and the grid, so a player's canvas silently had neither — and the
 * grid is what the engine sizes a token's hit area from, so it also changed
 * what a click could pick up. The tokens, walls and lights were all there,
 * which is exactly why nothing looked broken enough to report.
 */
test("a player receives the map and grid of the scene their world is playing", async ({
  browser,
}) => {
  test.setTimeout(180_000);

  // The invite flow writes to the clipboard; a context without that
  // permission throws before the invite is stored (see
  // scene-live-launch.spec.ts).
  const gmContext = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const gmPage = await gmContext.newPage();

  await register(gmPage, freshCredentials("e2egmscene"));
  await gmPage.goto("/worlds/create");
  await gmPage.locator("#world-name").fill(`E2E Active Scene ${uniqueSuffix()}`);
  await gmPage.getByRole("button", { name: /create world/i }).click();
  await gmPage.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 20_000 });
  const worldId = /\/world\/([^/]+)\/staging$/.exec(
    new URL(gmPage.url()).pathname,
  )![1];

  const playerPage = await inviteAndJoinAsPlayer(
    browser,
    gmPage,
    worldId,
    "e2eplscene",
  );

  // The world's auto-created scene is never un-hidden — the point of the
  // test is that a player can play it anyway.
  await playerPage.goto(`/world/${worldId}/play`);
  await expect(playerPage.locator("canvas")).toBeVisible({ timeout: 30_000 });

  // The scene record reached the player: the store was told about the grid.
  await expect
    .poll(
      () =>
        playerPage.evaluate(
          () =>
            window.__worldProbe
              ?.commands()
              .some((command) => command.type === "set_scene_grid") ?? false,
        ),
      {
        timeout: 60_000,
        message: "a player must be given the grid of the scene they are playing",
      },
    )
    .toBe(true);

  // And the scene-load state can finish, which it cannot while the
  // background resource has nothing to resolve it.
  await expect(playerPage.getByTestId("scene-load-indicator")).toHaveCount(0, {
    timeout: 60_000,
  });
  await expect(playerPage.getByTestId("scene-load-error")).toHaveCount(0);

  await playerPage.context().close();
  await gmContext.close();
});
