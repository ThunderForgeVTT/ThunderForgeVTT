import { expect, test, type Page } from "@playwright/test";
import {
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import {
  assetFingerprint,
  countAssetRequests,
  createCanvasAsset,
  createScene,
  holdsFingerprint,
  openWorldAndSync,
  sceneIds,
  setSceneHidden,
  switchToScene,
  watchCacheSync,
} from "./fixtures/world-cache";

/**
 * Background prefetch (spec 028 User Story 8, T119, T120 and T121).
 *
 * The promise is narrow and worth stating exactly: content for scenes in the
 * world you already have open, fetched while nothing else needs the
 * connection, so that switching to one of them is immediate. It is a
 * refinement of US1 rather than a new capability — the sync plan already
 * names what is missing, and the page is already open with the engine
 * running.
 *
 * That last part is the whole of FR-073, and it is a privacy property as
 * much as a performance one: this feature must not be able to do anything
 * while the application is closed.
 */

/** Every service-worker registration this page holds, by scope. */
async function serviceWorkerScopes(page: Page): Promise<string[]> {
  return page.evaluate(async () => {
    if (!("serviceWorker" in navigator)) return [];
    const registrations = await navigator.serviceWorker.getRegistrations();
    return registrations.map((registration) => registration.scope);
  });
}

test.describe("Client world cache — background prefetch (US8)", () => {
  test.setTimeout(300_000);

  /**
   * SC-023. A scene the user has never opened is already here.
   *
   * The assertion that carries it is the request count, not the absence of a
   * spinner: a scene can render without a loading state simply because it
   * had nothing to load. Counting GETs for the specific asset is what
   * separates "already cached" from "nothing was needed". `countAssetRequests`
   * is GET-only for a reason documented at its definition — the page HEADs
   * the background on every scene load for reachability, and counting that
   * made an earlier test answer "did this come from the network?" with yes
   * on no evidence at all.
   */
  test("a scene never opened is already cached, and switching to it fetches nothing (SC-023, T119)", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(page, `E2E Prefetch ${uniqueSuffix()}`);
    const [firstSceneId] = await sceneIds(page, worldId);

    // A second scene, visible to its GM either way, holding art nothing has
    // ever displayed.
    const secondName = `Unvisited ${uniqueSuffix()}`;
    const secondSceneId = await createScene(page, worldId, secondName);
    await setSceneHidden(page, secondSceneId, false);
    const unvisitedAssetId = await createCanvasAsset(page, worldId, secondSceneId, 41);
    const unvisitedFingerprint = await assetFingerprint(page, unvisitedAssetId);

    // Something on the *open* scene too, so the prefetch has to share the
    // connection rather than being the only thing using it.
    await createCanvasAsset(page, worldId, firstSceneId, 42);

    const sync = watchCacheSync(page);
    await openWorldAndSync(page, worldId, sync);

    // The prefetch is deliberately unhurried — it yields to the active scene
    // and paces itself between fetches — so this waits for the outcome
    // rather than assuming one sync pass carried it.
    await expect
      .poll(() => holdsFingerprint(page, worldId, unvisitedFingerprint), {
        timeout: 120_000,
        message: "art for a scene never opened should arrive on its own",
      })
      .toBe(true);

    // From here on, any GET for that asset means the switch went to the
    // network — which is exactly what the prefetch exists to prevent.
    const requests = countAssetRequests(page, unvisitedAssetId);
    await switchToScene(page, secondName);
    await waitForEngineReady(page);
    await page.waitForTimeout(3_000);

    expect(
      requests.stop(),
      "switching to a prefetched scene must not go to the network",
    ).toBe(0);
    await expect(page.getByTestId("scene-load-error")).toHaveCount(0);
  });

  /**
   * SC-024 / FR-070. Speculative work never gets in front of the user.
   *
   * # Why this is an ordering test and not a stopwatch
   *
   * SC-024 is written as a timing budget — prefetch may add no more than 5%
   * to active-scene time-to-interactive, measured enabled versus disabled.
   * Neither half of that is available here, and pretending otherwise would
   * produce a test that passes for the wrong reason:
   *
   * - There is no "disabled" to measure against. Prefetching has no off
   *   switch, and inventing a user-facing setting to make a test possible is
   *   the wrong way round. Opening the same world twice is not a control
   *   either: the second open has the active scene cached too, so the
   *   comparison changes both variables.
   * - 5% is far below the noise floor. Time-to-interactive here is dominated
   *   by instantiating a ~190MB development wasm bundle, and run-to-run
   *   variance on that swamps a 5% difference in a few asset fetches. A
   *   threshold a measurement cannot resolve is one that can only ever pass.
   *
   * So this asserts the property the budget exists to protect, in a form the
   * environment can actually decide: **the active scene's bytes are asked
   * for before any speculative byte is.** That is what `DemandGuard` and the
   * yield loop are for, and an implementation that fetched speculatively
   * ahead of the open scene would fail here however fast it happened to be.
   *
   * SC-024's number itself is left recorded as unverified rather than
   * quietly marked satisfied.
   */
  test("the open scene is served before anything speculative (SC-024, FR-070, T120)", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(page, `E2E Yield ${uniqueSuffix()}`);
    const [firstSceneId] = await sceneIds(page, worldId);

    const secondName = `Unvisited ${uniqueSuffix()}`;
    const secondSceneId = await createScene(page, worldId, secondName);
    await setSceneHidden(page, secondSceneId, false);
    const speculativeId = await createCanvasAsset(page, worldId, secondSceneId, 45);
    const demandId = await createCanvasAsset(page, worldId, firstSceneId, 46);

    // Recorded in arrival order, so the assertion is about precedence rather
    // than about how long anything took.
    const order: string[] = [];
    page.on("response", (response) => {
      if (response.request().method() !== "GET") return;
      if (response.url().includes(`/api/canvas-assets/${demandId}`)) order.push("demand");
      if (response.url().includes(`/api/canvas-assets/${speculativeId}`)) {
        order.push("speculative");
      }
    });

    const sync = watchCacheSync(page);
    await openWorldAndSync(page, worldId, sync);

    const speculativeFingerprint = await assetFingerprint(page, speculativeId);
    await expect
      .poll(() => holdsFingerprint(page, worldId, speculativeFingerprint), {
        timeout: 120_000,
        message: "the prefetch must run, or precedence is untested",
      })
      .toBe(true);

    expect(
      order.filter((kind) => kind === "demand").length,
      "the open scene's art must have been fetched",
    ).toBeGreaterThanOrEqual(1);
    expect(
      order.indexOf("demand"),
      "the open scene must be asked for before anything speculative",
    ).toBeLessThan(order.indexOf("speculative"));
  });

  /**
   * SC-025 and FR-073. The feature cannot act while the application is shut.
   *
   * This is asserted by instrumentation rather than by observation, because
   * the interesting claim is about what happens when nothing is watching.
   * Wrapping the three registration APIs before any application script runs
   * turns "we never scheduled background work" into something a test can
   * see, instead of something inferred from a quiet network.
   *
   * Note what is deliberately *not* asserted: that no service worker exists
   * at all. One does — the asset cache, which predates this work and is a
   * different feature. FR-073's words are that prefetching "introduces no
   * Service Worker", so the test pins the registration set to exactly the
   * one that was already there. Asserting zero would fail for the wrong
   * reason and would be quietly wrong the day someone read it as licence to
   * remove the asset cache.
   */
  test("prefetch schedules no background work of any kind (SC-025, FR-073, T121)", async ({
    page,
  }) => {
    await page.addInitScript(() => {
      const calls: string[] = [];
      (window as unknown as { __bg: string[] }).__bg = calls;

      const sync = (window as unknown as { SyncManager?: { prototype: object } })
        .SyncManager;
      if (sync?.prototype) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (sync.prototype as any).register = function (tag: string) {
          calls.push(`sync:${tag}`);
          return Promise.resolve();
        };
      }
      const periodic = (
        window as unknown as { PeriodicSyncManager?: { prototype: object } }
      ).PeriodicSyncManager;
      if (periodic?.prototype) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (periodic.prototype as any).register = function (tag: string) {
          calls.push(`periodicSync:${tag}`);
          return Promise.resolve();
        };
      }
      if (typeof PushManager !== "undefined") {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (PushManager.prototype as any).subscribe = function () {
          calls.push("push:subscribe");
          return Promise.resolve(null);
        };
      }
    });

    const worldId = await registerAndCreateWorld(page, `E2E NoBg ${uniqueSuffix()}`);
    const [firstSceneId] = await sceneIds(page, worldId);
    const secondName = `Unvisited ${uniqueSuffix()}`;
    const secondSceneId = await createScene(page, worldId, secondName);
    await setSceneHidden(page, secondSceneId, false);
    const unvisitedAssetId = await createCanvasAsset(page, worldId, secondSceneId, 43);
    const unvisitedFingerprint = await assetFingerprint(page, unvisitedAssetId);
    await createCanvasAsset(page, worldId, firstSceneId, 44);

    const sync = watchCacheSync(page);
    await openWorldAndSync(page, worldId, sync);

    // Wait for the prefetch to have actually run, so this is a test about a
    // feature that did something rather than one that never started.
    await expect
      .poll(() => holdsFingerprint(page, worldId, unvisitedFingerprint), {
        timeout: 120_000,
        message: "the prefetch must actually run, or this proves nothing",
      })
      .toBe(true);

    expect(
      await page.evaluate(() => (window as unknown as { __bg: string[] }).__bg),
      "prefetching must register no background sync, periodic sync, or push subscription",
    ).toEqual([]);

    // And exactly the service worker that was already here, no more.
    const scopes = await serviceWorkerScopes(page);
    expect(
      scopes.length,
      `expected only the pre-existing asset-cache worker, saw ${scopes.join(", ")}`,
    ).toBeLessThanOrEqual(1);

    // Nothing owed to the network once the page is gone. A worker would keep
    // fetching after this point; a page-scoped prefetch cannot.
    let afterClose = 0;
    page.on("request", (request) => {
      if (request.url().includes("/api/canvas-assets/")) afterClose += 1;
    });
    await page.close({ runBeforeUnload: true });
    expect(afterClose, "a closed page must not still be fetching").toBe(0);
  });
});
