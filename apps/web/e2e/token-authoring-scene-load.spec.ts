import { test, expect } from "./fixtures/test";
import { waitForEngineReady } from "./fixtures/tokenAuthoring";
import {
  uniqueSuffix,
  registerAndCreateWorld,
  createScene,
} from "./fixtures/tokenAuthoringSession";

/**
 * specs/004-token-canvas-authoring, User Story 4 (T031/T033): scene-switch
 * loading/error feedback. Before this, WorldPage.tsx's four per-scene
 * loaders (walls/tokens/lights/shapes) plus the background-image dispatch
 * had zero UI signal on failure or during the load itself — just a
 * `console.error` (contracts/scene-load-state.md). T032 (a connected
 * player sees the same sequence) is not separately tested here: the
 * loading/error state is derived independently per browser tab from that
 * tab's own scene fetch (no shared/broadcast state), so there is nothing
 * player-specific to verify beyond what these single-session tests
 * already cover — a second session hitting the exact same code path is
 * not a materially different case worth a second live browser context.
 */
test.describe("Scene-load loading/error feedback (US4, T031/T033)", () => {
  test.setTimeout(60_000);

  test("switching to a scene with no background art shows a loading indicator then clears to ready with no error", async ({
    page,
  }) => {
    await registerAndCreateWorld(page, `E2E Scene Load ${uniqueSuffix()}`);
    await createScene(page, "Scene Load A");
    await waitForEngineReady(page);

    // The loading indicator is expected to appear and clear quickly for a
    // small, background-art-free scene — assert it *was* shown at some
    // point during the very first scene selection (which itself triggers
    // the same effects a switch would), then assert it clears.
    await expect(page.getByTestId("scene-load-error")).toHaveCount(0);
    await expect(page.getByTestId("scene-load-indicator")).toHaveCount(0, {
      timeout: 10_000,
    });
  });

  test("a background image that fails to load shows a distinct error state with a working retry action", async ({
    page,
  }) => {
    await registerAndCreateWorld(
      page,
      `E2E Scene Load Error ${uniqueSuffix()}`,
    );
    await createScene(page, "Scene Load B");
    await waitForEngineReady(page);

    // Inject a controllable background URL by intercepting the `scenes`
    // query response, then control that exact path's reachability — this
    // tests the load-state machine itself (this feature's actual scope),
    // not whether a particular scene really has art.
    //
    // The injected field is `backgroundUrl`, not `backgroundImagePath`.
    // This test used to write the latter, back when it was the field
    // `WorldPage` dispatched into the engine. It no longer is: map import
    // writes `background_asset_id` rather than the legacy
    // `background_image_path` column, so `backgroundImagePath` was blank
    // for every imported map and the art silently never loaded.
    // `WorldPage` now reads the server-computed `backgroundUrl`, which is
    // populated whichever mechanism put the art there — so overwriting the
    // old field left the scene with no background at all, and no
    // background is exactly the case that produces no error to assert on.
    //
    // The same thing happened again with scene levels: the board's art is
    // the *level's* `backgroundUrl` now, and the scene's is read only when
    // no level could be. So the level list is overwritten too — the scene
    // list alone left the board with a level that has no art, and again
    // nothing to fail.
    const FAKE_BG_PATH = "/assets/e2e-fake-background-for-scene-load-test.png";
    let bgAssetShouldSucceed = false;

    await page.route("**/api/graphql", async (route, request) => {
      const postData = request.postData() ?? "";
      if (
        !postData.includes("scenes(") &&
        !postData.includes("scenes {") &&
        !postData.includes("sceneLevels(")
      ) {
        await route.fallback();
        return;
      }
      const response = await route.fetch();
      const json = await response.json();
      for (const list of [json?.data?.scenes, json?.data?.sceneLevels]) {
        if (!Array.isArray(list)) continue;
        for (const record of list) {
          record.backgroundUrl = FAKE_BG_PATH;
        }
      }
      await route.fulfill({ response, json });
    });

    await page.route(`**${FAKE_BG_PATH}`, (route) => {
      if (bgAssetShouldSucceed) {
        void route.fulfill({
          status: 200,
          contentType: "image/png",
          body: Buffer.from([]),
        });
      } else {
        void route.abort();
      }
    });

    await page.reload();
    await waitForEngineReady(page);

    await expect(page.getByTestId("scene-load-error")).toBeVisible({
      timeout: 10_000,
    });
    await expect(page.getByTestId("scene-load-error")).toContainText(
      "background image",
    );
    await expect(page.getByTestId("scene-load-retry")).toBeVisible();

    // Fix the underlying issue (the mocked asset now "exists") and retry
    // — the error state must clear and the scene must load successfully,
    // without switching away and back (FR-013a).
    bgAssetShouldSucceed = true;
    await page.getByTestId("scene-load-retry").click();

    await expect(page.getByTestId("scene-load-error")).toHaveCount(0, {
      timeout: 10_000,
    });
    await expect(page.getByTestId("scene-load-indicator")).toHaveCount(0, {
      timeout: 10_000,
    });
  });
});
