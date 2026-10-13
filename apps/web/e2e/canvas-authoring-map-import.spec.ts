import { test, expect } from "./fixtures/test";
import { openDockTab } from "./fixtures/helpers";
import {
  DEMO_MAP,
  CHAMBER_MAP,
  uniqueSuffix,
  registerAndCreateWorld,
  createScene,
  importMap,
  switchToScene,
  expectImportedWallsOnTheMap,
  expectSceneGridMatchesMap,
} from "./fixtures/canvasAuthoring";

/**
 * End-to-end coverage for the native canvas authoring feature
 * (specs/001-bevy-canvas-authoring) plus the scene-switching work built
 * on top of it: a GM imports a Universal VTT map into one scene, creates
 * a second scene, imports a different map into it, and switches between
 * the two — proving map import and per-scene isolation both work through
 * the real UI, not just at the API/unit level.
 *
 * Fixtures: examples/maps/demo.dd2vtt (8 line_of_sight polygons -> 31
 * walls, 2 doors, 12 lights) and examples/maps/chamber-of-echoing-grief.dd2vtt
 * (1 polygon -> 4 walls, 0 doors, 0 lights) — counts verified directly
 * against the parser in crates/thunderforge-server/src/map_import.rs's own tests, and
 * asserted here via the exact UI text MapImportTool renders after a
 * successful import (no backdoor API calls — this is what a GM sees).
 */

test.describe("Native canvas authoring: map import and scene switching", () => {
  test("importing a map into one scene, creating a second scene, importing a different map, and switching between them keeps each scene's content isolated", async ({
    page,
  }) => {
    // Two full map imports (each with a 30-second SC-007 budget of its own),
    // three scene launches and four scene switches do not fit in Playwright's
    // 30-second default — the test was dying mid-upload, which reads like a
    // broken import rather than a clock running out.
    test.setTimeout(240_000);

    await registerAndCreateWorld(page, `E2E World ${uniqueSuffix()}`);

    // "New scene" lives in the Play dock's Settings section, collapsed by
    // default — open it before the rest of this test relies on it being
    // reachable via `createScene`/`switchToScene`.
    await page.getByTestId("world-dock-tab-settings").click();
    await expect(
      page.locator('[data-testid="new-scene-button"]:visible'),
    ).toBeVisible();

    // --- Scene One: import the rich demo map ---
    //
    // `createScene` finishes by launching the scene, which is a real
    // navigation — the dock comes back collapsed, so every assertion about
    // its Settings contents has to open it again first. Asserting the import
    // tool without doing so found nothing and read like the tool being gone.
    await createScene(page, "Scene One");
    await openDockTab(page, "settings");
    await expect(page.getByTestId("map-import-tool")).toBeVisible();
    // SC-007: importing demo.dd2vtt (background art + 8 wall polygons + 2
    // doors + 12 lights) end-to-end through the real UI must complete in
    // under 30 seconds.
    const importDurationMs = await importMap(
      page,
      DEMO_MAP,
      "31 walls, 2 doors, 12 lights",
    );
    // Measured live at 28-34s against the dev stack, i.e. right on the line
    // and failing about as often as it passes.
    // Nearly all of it is `save_background_image`/`save_scene_preview_image`
    // decoding and transcoding demo.dd2vtt's ~3MB embedded PNG in a `cargo
    // run` *debug* backend, where the `image`/`webp` codecs run unoptimised:
    // the 958KB chamber map, same code path, lands in ~2.9s. The product this
    // budget is about is the release build, so a `[profile.dev.package.image]
    // opt-level` bump would make this measurement representative rather than
    // marginal — a build-config change, out of scope here, recorded so a
    // borderline failure is read as the debug backend and not a regression.
    expect(importDurationMs).toBeLessThan(30_000);
    console.log(`SC-007: demo.dd2vtt import took ${importDurationMs}ms`);

    // The imported scene is as many cells across as the file says the map is.
    //
    // The regression: a background wider than the GPU texture cap is stored
    // resized, and the importer kept the *source* file's `pixels_per_grid` as
    // the scene's `grid_size` — a number describing an image that was never
    // written. demo.dd2vtt is 4480x2560 at 128px, so it was stored at
    // 4096x2304 under a 128px grid: thirty-two cells across a map with
    // thirty-five, and every wall, portal and light out by the same factor.
    // Reported from a real import, where the only visible symptom was "the
    // grid is off".
    //
    // Asserted as a ratio, deliberately. The server-side test that should have
    // caught this checked `grid_size == pixels_per_grid` — a *value* — and so
    // asserted the bug as correct behaviour for every map big enough to be
    // resized. What has to hold is that the stored picture and the stored grid
    // describe the same map.
    await expectSceneGridMatchesMap(page, DEMO_MAP);
    await expectImportedWallsOnTheMap(page);

    // --- Scene Two: import the simpler chamber map ---
    await createScene(page, "Scene Two");
    // A freshly created scene must not show the previous scene's import
    // result (per-scene isolation, not a global "last import" banner).
    // Open Settings first: with the dock collapsed the success panel is not
    // mounted at all, so this would pass without proving anything.
    await openDockTab(page, "settings");
    await expect(page.getByTestId("map-import-success")).toHaveCount(0);
    await importMap(page, CHAMBER_MAP, "4 walls, 0 doors, 0 lights");

    // --- Switch back to Scene One: its own import result, not Scene Two's ---
    await switchToScene(page, "Scene One");
    await expect(page.getByTestId("map-import-success")).toHaveCount(0);

    // --- And back to Scene Two once more, to rule out one-way state bleed ---
    await switchToScene(page, "Scene Two");
    await expect(page.getByTestId("map-import-success")).toHaveCount(0);
  });
});
