import { expect, test, type Page } from "./fixtures/test";
import { expectNoAxeViolations } from "./fixtures/axe";
import {
  clickPlay,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Owner, 2026-09-15: entering a world "sort of just freezes and waits for an
 * interaction point", and the map image arriving shows nothing at all.
 *
 * The failure and retry path, the progress bar and "exactly one indicator"
 * are `engine-loading.spec.ts`'s, which runs in the measured lane. This proves
 * the part that is new: the wait is a list of named steps that moves, it is
 * announced, and it stands still for someone who asked for less movement.
 */

/**
 * Give the engine's arrival a floor, so the loading window is observable.
 *
 * Held and then let through, not fetched and re-served as
 * `engine-loading.spec.ts`'s `serveEngineSlowly` does. That one runs in the
 * measured lane against a release engine; in the ordinary lane the engine is
 * the dev build, some 270MB, and `route.fetch` carries the whole body across
 * the DevTools protocol — which closes the browser session long before the
 * test's own timeout, and reads as the loader never finishing. A fresh
 * context has nothing cached, so delaying the request is enough.
 */
async function serveEngineSlowly(page: Page, delayMs = 2_000): Promise<void> {
  await page.route("**/*.wasm", async (route) => {
    await new Promise((resolve) => setTimeout(resolve, delayMs));
    await route.continue();
  });
}

test.describe("Bringing a table up", () => {
  test.afterEach(async ({ page }) => {
    await page.unrouteAll({ behavior: "ignoreErrors" });
  });

  test("the wait names its steps, is announced, and is still for reduced motion", async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await serveEngineSlowly(page);
    await registerAndCreateWorld(
      page,
      `E2E Board Loading ${uniqueSuffix()}`,
      "e2eboard",
    );
    await clickPlay(page);

    const indicator = page.getByTestId("engine-load-indicator");
    await expect(
      indicator,
      "no engine loader appeared despite a slowed wasm — check the route " +
        "still matches the engine's URL before suspecting the loader",
    ).toBeVisible({ timeout: 5_000 });

    // The world is fetched by the time the engine is being woken, and the
    // steps after it are still to come — the list says where the wait is.
    const board = indicator.getByTestId("board-loading");
    await expect(board).toHaveAttribute("data-step", "engine");
    await expect(
      indicator.getByTestId("board-loading-step-world"),
    ).toHaveAttribute("data-state", "done");
    await expect(
      indicator.getByTestId("board-loading-step-engine"),
    ).toHaveAttribute("data-state", "current");
    await expect(
      indicator.getByTestId("board-loading-step-map"),
    ).toHaveAttribute("data-state", "waiting");
    await expect(indicator.getByTestId("board-loading-detail")).not.toBeEmpty();

    // Asked for less movement: nothing animates, and the words are all there.
    const animation = await indicator
      .getByTestId("board-loading-mark")
      .evaluate((node) => getComputedStyle(node).animationName);
    expect(animation).toBe("none");

    await expectNoAxeViolations(page, '[data-testid="engine-load-indicator"]');

    // And it gets out of the way: the board comes up and no loader is left.
    await expect(indicator).toBeHidden({ timeout: 120_000 });
    await expect(page.getByTestId("scene-load-indicator")).toBeHidden({
      timeout: 60_000,
    });
  });
});
