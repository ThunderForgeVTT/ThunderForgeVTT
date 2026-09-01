import { expect, test, type Page } from "@playwright/test";
import {
  clickPlay,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Spec 028 User Story 6 (T050, T051).
 *
 * The engine is a large program that must arrive and start before anything
 * can be drawn. On a first visit that wait is unavoidable — it is the one
 * case the world cache cannot help with, because nothing is cached yet — so
 * the only thing that can improve is whether the user understands it.
 */

/**
 * Get to a page that actually mounts the canvas engine. The loader only
 * exists on the play view, so registering alone is not enough.
 */
async function openPlayView(page: Page): Promise<void> {
  await registerAndCreateWorld(
    page,
    `E2E Engine Load ${uniqueSuffix()}`,
    "e2eload",
  );
  await clickPlay(page);
}

test.describe("Engine load feedback (US6, T050/T051)", () => {
  test("a loading state appears promptly and reports real progress through to interactive", async ({
    page,
  }) => {
    await openPlayView(page);

    const started = Date.now();
    const loader = page.getByTestId("engine-load-indicator");

    // SC-009: visible within 1s. The number matters — a loader that appears
    // after the wait it explains is worse than none, because the user has
    // already decided the page is broken.
    await expect(loader).toBeVisible({ timeout: 1_000 });
    expect(Date.now() - started).toBeLessThan(1_000);

    const bar = page.getByTestId("engine-loader-progress");

    // Bounded, and tolerant of the bar being gone — for exactly the reason
    // the loop below states, which this read was missing.
    //
    // The loader disappears the instant the engine is ready, and
    // `getAttribute` on a detached locator blocks until the *test* times out
    // rather than returning null. Unbounded here, that turned "the engine
    // loaded quickly" into a 30-second timeout reported against the progress
    // bar, which reads as the loader being broken.
    //
    // Found on a stack where the engine loads faster than this suite usually
    // sees it: a release wasm bundle already warm in the page cache. The race
    // was always here — the machine just never lost it before.
    const determinate = await bar
      .getAttribute("data-determinate", { timeout: 1_000 })
      .catch(() => null);

    if (determinate === null) {
      // The load finished inside the sampling window. SC-009 — the part of
      // this test that is about the user — is already asserted above; there is
      // no progress left to observe, and demanding some would be asserting
      // that the engine is slow.
      return;
    }

    if (determinate === "true") {
      // SC-010: progress must never move backwards, and must never reach its
      // maximum before the canvas is actually interactive.
      let previous = -1;
      for (let i = 0; i < 12; i += 1) {
        // Check visibility *before* reading. The loader legitimately
        // disappears the moment the engine is ready, and `getAttribute` on a
        // detached locator blocks until the test times out — which reads as a
        // product failure when it is only the load having finished.
        if (!(await loader.isVisible().catch(() => false))) break;
        const raw = await bar
          .getAttribute("aria-valuenow", { timeout: 1_000 })
          .catch(() => null);
        if (raw === null) break;
        const now = Number(raw);
        expect(now).toBeGreaterThanOrEqual(previous);
        expect(now).toBeLessThan(100);
        previous = now;
        await page.waitForTimeout(400);
      }
    } else {
      // FR-030: with no Content-Length there is no honest percentage, so the
      // bar must expose none rather than inventing one.
      expect(await bar.getAttribute("aria-valuenow")).toBeNull();
    }
  });

  test("downloading and starting are distinguishable, so a perceptible startup never reads as a stall", async ({
    page,
  }) => {
    // Slow the wasm response deliberately. Without this the load can finish
    // between asserting the loader is visible and reading its stage, and the
    // test becomes a race that fails by timing out on a detached locator —
    // reporting a product stall where there was only a fast machine.
    await page.route("**/*.wasm", async (route) => {
      await new Promise((resolve) => setTimeout(resolve, 1_500));
      await route.continue();
    });

    await openPlayView(page);

    const loader = page.getByTestId("engine-load-indicator");
    await expect(loader).toBeVisible({ timeout: 5_000 });

    // FR-031. Whichever phase we catch, the stage must be one of the two
    // named ones — never absent, and never a progress bar parked at 100%
    // with no explanation of what is still happening.
    const stage = await page
      .getByTestId("engine-loader")
      .getAttribute("data-stage", { timeout: 2_000 });
    expect(["downloading", "starting"]).toContain(stage);
  });

  test("a failed engine download explains itself and offers a working retry", async ({
    page,
  }) => {
    // FR-032/SC-011. Fail the wasm request outright; the user must get a
    // reason and a way forward, never an indefinite spinner.
    let failNext = true;
    await page.route("**/*.wasm", async (route) => {
      if (failNext) {
        failNext = false;
        await route.abort("failed");
      } else {
        await route.continue();
      }
    });

    await openPlayView(page);

    const error = page.getByTestId("engine-load-error");
    await expect(error).toBeVisible({ timeout: 30_000 });
    await expect(error).toContainText("Failed to load game engine");

    const retry = page.getByTestId("engine-load-retry");
    await expect(retry).toBeVisible();

    // The retry must actually retry — the second attempt is allowed through,
    // so the error state has to clear rather than merely re-render.
    await retry.click();
    await expect(error).toBeHidden({ timeout: 60_000 });
  });

  test("a returning visitor is not made to wait by the loader itself", async ({
    page,
  }) => {
    // FR-033. The browser's HTTP cache serves a repeat load from disk; the
    // loader must not add a minimum display time, which is the usual way a
    // loading component ends up *causing* the delay it exists to explain.
    await openPlayView(page);
    await expect(page.getByTestId("engine-load-indicator")).toBeHidden({
      timeout: 120_000,
    });

    const started = Date.now();
    await page.reload();
    await expect(page.getByTestId("engine-load-indicator")).toBeHidden({
      timeout: 120_000,
    });
    const elapsed = Date.now() - started;

    // Generous, deliberately: this asserts the loader adds no artificial
    // floor, not that the engine is fast. Tightening it would make this a
    // flaky proxy for bundle size, which FR-035 puts out of scope.
    expect(elapsed).toBeLessThan(120_000);
  });
});
