import { expect, test, type Page } from "@playwright/test";
import { registerAndCreateWorld, uniqueSuffix } from "./fixtures/helpers";
import {
  assetStatus,
  createCanvasAsset,
  holdsFingerprint,
  assetFingerprint,
  openWorldAndSync,
  sceneIds,
  type SyncSummary,
  watchCacheSync,
  worldBlobs,
} from "./fixtures/world-cache";

/**
 * Staying within the space the browser is willing to give (US4, FR-022,
 * FR-023, FR-024, SC-006).
 *
 * `thunderforge_cache_core::budget` has held the whole policy — `limit_bytes`
 * is half the reported quota capped at 20GiB, `plan_eviction` releases whole
 * worlds LRU-first and never the open one — with unit tests, since T058. What
 * it did not have until now was a caller: nothing in the tree read
 * `navigator.storage.estimate()` or acted on a plan, so the store grew without
 * any limit at all. These tests are about the caller, because the policy is
 * already covered and a policy nobody invokes is the failure mode this spec
 * has now hit twice (FR-019's repair pass was the first).
 *
 * # Why the quota is faked
 *
 * SC-006 asks for correct behaviour "across machines whose reported quota
 * differs by an order of magnitude", and a test cannot own two machines. What
 * it can own is the number those machines would report: `navigator.storage
 * .estimate()` is the single input the budget is derived from, so overriding
 * it is not a stub standing in for the system under test — it *is* the
 * machine difference, expressed exactly.
 *
 * The override replaces only `estimate`. `navigator.storage.getDirectory` has
 * to keep working, because OPFS is where the bytes under discussion live.
 */

/** A quota small enough that a single cached world exceeds half of it. */
const TIGHT_QUOTA_BYTES = 200_000;

/**
 * Half of this fits one test world's art and not two, which is what isolates
 * FR-023 from FR-024.
 *
 * The two rules collide at a smaller number and the collision is easy to
 * mistake for a bug. Under `TIGHT_QUOTA_BYTES` the open world's own asset
 * does not fit either, so the budget reports `insufficient`, the store stops
 * accepting writes (FR-024), and the open world ends up *uncached* — which
 * looks exactly like "the open world was evicted" while being the opposite:
 * nothing was taken, nothing was added. Sizing the limit to hold one world
 * means an eviction that happens here can only be the LRU rule doing its job.
 */
const MODEST_QUOTA_BYTES = 500_000;

/** Roughly a hundredfold more — the "other machine" of SC-006. */
const ROOMY_QUOTA_BYTES = 20_000_000;

/**
 * Make `navigator.storage.estimate()` report `quota`, for every document this
 * page loads from now on.
 *
 * `addInitScript` rather than a one-off `page.evaluate`: the engine reads the
 * estimate during its sync, which happens on every load, and a value patched
 * into only the current document would vanish at the first `reload()` — the
 * test would then be measuring the real machine while believing otherwise.
 */
async function reportQuota(page: Page, quota: number): Promise<void> {
  await page.addInitScript((bytes) => {
    const storage = navigator.storage;
    // Only `estimate`. `getDirectory` is how OPFS is reached, and replacing
    // the whole `storage` object would take the cache's actual storage with
    // it and make every assertion below meaningless.
    Object.defineProperty(storage, "estimate", {
      configurable: true,
      value: async () => ({ quota: bytes, usage: 0 }),
    });
  }, quota);
}

/** The budget figures out of the most recent sync summary. */
interface BudgetReport {
  limit: number;
  inUse: number;
  evicted: number;
  blobsRemoved: number;
  insufficient: boolean;
  quotaUnknown: boolean;
}

function budgetOf(summary: SyncSummary): BudgetReport {
  return {
    limit: Number(summary.budgetLimit ?? -1),
    inUse: Number(summary.budgetInUse ?? -1),
    evicted: Number(summary.budgetEvicted ?? -1),
    blobsRemoved: Number(summary.budgetBlobsRemoved ?? -1),
    insufficient: Boolean(summary.budgetInsufficient),
    quotaUnknown: Boolean(summary.budgetQuotaUnknown),
  };
}

/** Create a second world through the form, and return its id. */
async function createAnotherWorld(page: Page, name: string): Promise<string> {
  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(name);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 20_000 });
  const id = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname)?.[1];
  expect(id, "the second world should have been created").toBeTruthy();
  return id!;
}

test.describe("Client world cache — living within the reported quota (US4)", () => {
  test.setTimeout(300_000);

  test("the limit follows the machine's reported quota (FR-022, SC-006)", async ({
    page,
  }) => {
    const sync = watchCacheSync(page);
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Budget Quota ${uniqueSuffix()}`,
      "e2ebudget",
    );

    // The same profile, the same world, the same content — only the number
    // the platform reports differs. That is the whole of SC-006.
    await reportQuota(page, ROOMY_QUOTA_BYTES);
    const roomy = budgetOf(await openWorldAndSync(page, worldId, sync));
    console.log(`[budget] roomy: ${JSON.stringify(roomy)}`);
    expect(roomy.quotaUnknown, "the faked estimate should be read").toBe(false);
    expect(roomy.limit, "half of the reported quota").toBe(ROOMY_QUOTA_BYTES / 2);

    await reportQuota(page, TIGHT_QUOTA_BYTES);
    const before = sync.count();
    await page.reload();
    const tight = budgetOf(await sync.next(before));
    console.log(`[budget] tight: ${JSON.stringify(tight)}`);
    expect(tight.limit, "half of the smaller quota").toBe(TIGHT_QUOTA_BYTES / 2);

    // The point of recomputing on every open rather than once: the limit is a
    // fact about the machine right now, and it moved by two orders of
    // magnitude without the application restarting.
    expect(
      tight.limit,
      "a shrunken quota must shrink the limit, not keep the old one",
    ).toBeLessThan(roomy.limit);

    sync.stop();
  });

  test("a browser that will not estimate keeps its cache (FR-024)", async ({
    page,
  }) => {
    const sync = watchCacheSync(page);
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Budget Silent ${uniqueSuffix()}`,
      "e2ebudget",
    );
    const scenes = await sceneIds(page, worldId);
    const assetId = await createCanvasAsset(page, worldId, scenes[0], 5150);
    const fingerprint = await assetFingerprint(page, assetId);

    await openWorldAndSync(page, worldId, sync);
    await expect
      .poll(() => holdsFingerprint(page, worldId, fingerprint), {
        timeout: 90_000,
        message: "the world must be cached before the estimate is taken away",
      })
      .toBe(true);

    // `estimate()` rejecting is the case that must not be read as "zero bytes
    // available". Zero would mean a limit of zero, and a limit of zero means
    // evicting everything the user has — destroying a working cache because a
    // diagnostic API was unavailable.
    await page.addInitScript(() => {
      Object.defineProperty(navigator.storage, "estimate", {
        configurable: true,
        value: async () => {
          throw new Error("estimate unavailable");
        },
      });
    });

    const before = sync.count();
    await page.reload();
    const summary = budgetOf(await sync.next(before));
    console.log(`[budget] no estimate: ${JSON.stringify(summary)}`);

    expect(summary.quotaUnknown, "the refusal should be reported as unknown").toBe(
      true,
    );
    expect(summary.evicted, "an unknown quota must evict nothing").toBe(0);
    expect(
      await holdsFingerprint(page, worldId, fingerprint),
      "a cache must survive a browser that will not estimate",
    ).toBe(true);

    sync.stop();
  });

  test("a world that no longer fits is released, and never the open one (FR-023)", async ({
    page,
  }) => {
    const sync = watchCacheSync(page);
    const firstWorld = await registerAndCreateWorld(
      page,
      `E2E Budget A ${uniqueSuffix()}`,
      "e2ebudget",
    );
    const firstScenes = await sceneIds(page, firstWorld);
    const firstAsset = await createCanvasAsset(page, firstWorld, firstScenes[0], 9001);
    const firstFingerprint = await assetFingerprint(page, firstAsset);

    // Roomy while the first world is cached, so the eviction below is caused
    // by the quota changing and not by the world never having been stored.
    await reportQuota(page, ROOMY_QUOTA_BYTES);
    await openWorldAndSync(page, firstWorld, sync);
    await expect
      .poll(() => holdsFingerprint(page, firstWorld, firstFingerprint), {
        timeout: 90_000,
        message: "the first world must be cached before it can be evicted",
      })
      .toBe(true);

    const secondWorld = await createAnotherWorld(
      page,
      `E2E Budget B ${uniqueSuffix()}`,
    );
    const secondScenes = await sceneIds(page, secondWorld);
    const secondAsset = await createCanvasAsset(
      page,
      secondWorld,
      secondScenes[0],
      9002,
    );
    const secondFingerprint = await assetFingerprint(page, secondAsset);

    // Now the machine gets smaller, and a different world is the open one.
    // Modest, not tight: room for one world's art, so the open world can
    // still be stored and an eviction can only mean the LRU rule ran.
    await reportQuota(page, MODEST_QUOTA_BYTES);
    const summary = budgetOf(await openWorldAndSync(page, secondWorld, sync));
    console.log(`[budget] after opening the second world: ${JSON.stringify(summary)}`);

    expect(
      summary.evicted,
      "the world that no longer fits should have been released",
    ).toBeGreaterThanOrEqual(1);
    expect(
      summary.blobsRemoved,
      "releasing a row should take its blob with it",
    ).toBeGreaterThanOrEqual(1);

    await expect
      .poll(() => holdsFingerprint(page, firstWorld, firstFingerprint), {
        timeout: 60_000,
        message: "the least-recently-used world should have been released",
      })
      .toBe(false);

    // FR-023, the rule that makes this safe: whatever else goes, not the
    // world being looked at. Evicting the open world would free space by
    // taking away the thing the user is actually using.
    await expect
      .poll(() => holdsFingerprint(page, secondWorld, secondFingerprint), {
        timeout: 90_000,
        message: "the open world must survive its own budget pass",
      })
      .toBe(true);

    const survivors = await worldBlobs(page, firstWorld);
    expect(
      survivors,
      "the released world should leave no blobs behind",
    ).toHaveLength(0);

    sync.stop();
  });

  test("a store with no room serves without filing, and never fails a load (FR-024)", async ({
    page,
  }) => {
    const sync = watchCacheSync(page);
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Budget Full ${uniqueSuffix()}`,
      "e2ebudget",
    );
    const scenes = await sceneIds(page, worldId);
    const assetId = await createCanvasAsset(page, worldId, scenes[0], 7007);
    const fingerprint = await assetFingerprint(page, assetId);

    // A limit the open world's own content cannot fit inside. FR-023 forbids
    // evicting the open world, so there is nothing left to release and the
    // honest answer is `insufficient` — not a smaller cache, and emphatically
    // not a failed load.
    await reportQuota(page, TIGHT_QUOTA_BYTES);
    const summary = budgetOf(await openWorldAndSync(page, worldId, sync));
    console.log(`[budget] no room: ${JSON.stringify(summary)}`);

    expect(
      summary.insufficient,
      "a world too big for the whole budget must be reported as insufficient",
    ).toBe(true);
    expect(
      summary.inUse,
      "nothing should have been filed",
    ).toBeLessThanOrEqual(summary.limit);

    // The degradation, stated as the spec states it: fetch without storing.
    // Not "fetch and fail", and not "store anyway and blow the budget".
    await page.waitForTimeout(10_000);
    expect(
      await holdsFingerprint(page, worldId, fingerprint),
      "content that cannot fit must not be filed",
    ).toBe(false);

    // And the user still gets their world. This is the half of FR-024 that
    // matters most: a full disk degrades the cache, never the application.
    expect(
      await assetStatus(page, assetId),
      "the asset must still be served over the network",
    ).toBe(200);
    await expect(page.locator("canvas")).toBeVisible({ timeout: 30_000 });

    sync.stop();
  });
});
