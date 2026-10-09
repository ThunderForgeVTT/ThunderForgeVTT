import path from "node:path";
import type { Page, Response } from "@playwright/test";
import { expect, test } from "./fixtures/test";
import type { WorldProbe } from "../src/engine/world/probe";
import {
  freshCredentials,
  inviteAndJoinAsPlayer,
  register,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { expectCanvasRendersMap } from "./fixtures/canvasPixels";
import {
  importMapBackground,
  sceneBackgroundAssetId,
  sceneIds,
} from "./fixtures/world-cache";

declare global {
  interface Window {
    __worldProbe?: WorldProbe;
  }
}

const DEMO_MAP = path.resolve(__dirname, "../../../examples/maps/demo.dd2vtt");

/**
 * A map the Game Master imports from Settings reaches everyone at the table.
 *
 * Owner report, 2026-10-09: "i loaded a map from settings as a gm but it didnt
 * load the map for anyone". The server persisted the art and broadcast a
 * map-imported world event (code 13), but no client acted on it: the
 * importing tab re-read the scene list (not the scene's levels, which is
 * where the board takes its art from now), and every other tab ignored the
 * event outright. So nobody's board changed until something unrelated made it
 * re-read.
 *
 * The world's auto-created scene is left hidden on purpose: that is the scene
 * a new table plays, and a player is only allowed its art because it is the
 * one being played.
 */

/**
 * Every response for the asset's image, so reachability is read, not
 * assumed. The engine asks for `<id>.webp.meta` before each image, and the
 * server answers that with a 404 on purpose (see `parse_asset_id`), so the
 * `.meta` probe is not the image and is left out.
 */
function watchAsset(page: Page): (assetId: string) => number[] {
  const seen: { url: string; status: number }[] = [];
  page.on("response", (response: Response) => {
    const url = response.url();
    if (url.includes("/api/canvas-assets/") && !url.endsWith(".meta")) {
      seen.push({ url: response.url(), status: response.status() });
    }
  });
  return (assetId) =>
    seen.filter((each) => each.url.includes(assetId)).map((e) => e.status);
}

async function sceneLoaded(page: Page): Promise<void> {
  await expect(page.locator("canvas")).toBeVisible({ timeout: 30_000 });
  await expect
    .poll(
      () =>
        page.evaluate(
          () =>
            window.__worldProbe
              ?.commands()
              .some((command) => command.type === "set_scene_grid") ?? false,
        ),
      { timeout: 60_000, message: "the scene must reach this client first" },
    )
    .toBe(true);
  await expect(page.getByTestId("scene-load-indicator")).toHaveCount(0, {
    timeout: 60_000,
  });
}

async function wallCount(page: Page): Promise<number> {
  return page.evaluate(() => window.__worldProbe?.state().counts.walls ?? -1);
}

test("a map the GM imports from Settings appears on every player's board without a reload", async ({
  browser,
}) => {
  test.setTimeout(360_000);

  const gmContext = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const gmPage = await gmContext.newPage();

  await register(gmPage, freshCredentials("e2egmmapsync"));
  await gmPage.goto("/worlds/create");
  await gmPage.locator("#world-name").fill(`E2E Map Sync ${uniqueSuffix()}`);
  await gmPage.getByRole("button", { name: /create world/i }).click();
  await gmPage.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 20_000 });
  const worldId = /\/world\/([^/]+)\/staging$/.exec(
    new URL(gmPage.url()).pathname,
  )![1];
  const [sceneId] = await sceneIds(gmPage, worldId);
  expect(sceneId, "a new world has its Starting Scene").toBeTruthy();

  // A player already at the table before the map arrives.
  const playerPage = await inviteAndJoinAsPlayer(
    browser,
    gmPage,
    worldId,
    "e2eplmapsync",
  );
  const playerAsset = watchAsset(playerPage);
  await playerPage.goto(`/world/${worldId}/play`);
  await sceneLoaded(playerPage);
  expect(await wallCount(playerPage)).toBe(0);

  // The act under test: the GM imports a map from the Settings dock.
  await gmPage.goto(`/world/${worldId}/play`);
  await waitForEngineReady(gmPage);
  await sceneLoaded(gmPage);
  await importMapBackground(gmPage, DEMO_MAP);
  const assetId = await sceneBackgroundAssetId(gmPage, worldId, sceneId);

  // The GM's own board shows it.
  await expect(async () => {
    await expectCanvasRendersMap(gmPage);
  }).toPass({ timeout: 60_000 });

  // The connected player's board shows it, with no reload: the art was
  // fetched (and allowed), the imported walls arrived, and the canvas draws
  // a map rather than an empty grid.
  await expect
    .poll(() => playerAsset(assetId).some((status) => status < 400), {
      timeout: 60_000,
      message: "the connected player must fetch the imported map's art",
    })
    .toBe(true);
  expect(
    playerAsset(assetId).filter((status) => status >= 400),
    "a player of the world must be allowed the art of the scene being played",
  ).toEqual([]);
  await expect
    .poll(() => wallCount(playerPage), {
      timeout: 30_000,
      message: "the imported walls must reach the connected player",
    })
    .toBeGreaterThan(0);
  await expect(async () => {
    await expectCanvasRendersMap(playerPage);
  }).toPass({ timeout: 60_000 });
  await expect(playerPage.getByTestId("scene-load-error")).toHaveCount(0);

  // A player who joins afterwards gets it on their first load.
  const latePage = await inviteAndJoinAsPlayer(
    browser,
    gmPage,
    worldId,
    "e2eplmaplate",
  );
  const lateAsset = watchAsset(latePage);
  await latePage.goto(`/world/${worldId}/play`);
  await sceneLoaded(latePage);
  await expect
    .poll(() => lateAsset(assetId).some((status) => status < 400), {
      timeout: 60_000,
      message: "a player joining later must fetch the map's art",
    })
    .toBe(true);
  await expect(async () => {
    await expectCanvasRendersMap(latePage);
  }).toPass({ timeout: 60_000 });

  await latePage.context().close();
  await playerPage.context().close();
  await gmContext.close();
});
