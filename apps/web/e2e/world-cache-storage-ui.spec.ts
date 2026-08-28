import { expect, test, type Page } from "@playwright/test";
import {
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import {
  assetFingerprint,
  createCanvasAsset,
  holdsFingerprint,
  openWorldAndSync,
  sceneIds,
  watchCacheSync,
  worldBlobs,
} from "./fixtures/world-cache";

/**
 * Seeing what is stored and taking it back (spec 028, US5, FR-025/FR-026,
 * T067).
 *
 * US5 scenario 2 is the whole test: clearing one world zeroes its figure,
 * leaves the others intact, and the cleared world still loads. The last
 * clause is the one that makes the feature safe to use — a "clear" that
 * quietly cost you a world would be worse than a full disk — so it is
 * asserted by actually reopening the world, not by reasoning that nothing
 * server-side was touched.
 *
 * # The figures are read from the panel, and checked against the disk
 *
 * A storage screen can be wrong in a way nothing else notices: it can report
 * numbers that are internally consistent and describe nothing. So each
 * assertion about what the panel *says* is paired with a look at what OPFS
 * actually holds. A panel claiming a world was cleared while its blobs are
 * still on disk is the failure this pairing exists to catch.
 */

/** The per-world rows the panel is showing, as world id to reported size. */
async function panelRows(page: Page): Promise<Map<string, string>> {
  const rows = page.getByTestId("storage-world-row");
  const entries = new Map<string, string>();
  for (const row of await rows.all()) {
    const worldId = await row.getAttribute("data-world-id");
    const bytes = await row.getByTestId("storage-world-bytes").textContent();
    if (worldId) entries.set(worldId, (bytes ?? "").trim());
  }
  return entries;
}

async function openStoragePanel(page: Page): Promise<void> {
  await page.goto("/settings/storage");
  await expect(page.getByTestId("storage-panel")).toBeVisible({ timeout: 30_000 });
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

test.describe("Client world cache — seeing and reclaiming storage (US5)", () => {
  test.setTimeout(300_000);

  test("clearing one world zeroes its figure, leaves the other, and it still loads (US5 scenario 2)", async ({
    page,
  }) => {
    const sync = watchCacheSync(page);

    const worldA = await registerAndCreateWorld(
      page,
      `E2E Storage A ${uniqueSuffix()}`,
      "e2estorage",
    );
    const scenesA = await sceneIds(page, worldA);
    const assetA = await createCanvasAsset(page, worldA, scenesA[0], 3210);
    const fingerprintA = await assetFingerprint(page, assetA);
    await openWorldAndSync(page, worldA, sync);
    await expect
      .poll(() => holdsFingerprint(page, worldA, fingerprintA), {
        timeout: 90_000,
        message: "world A must be cached before the panel can report it",
      })
      .toBe(true);

    const worldB = await createAnotherWorld(page, `E2E Storage B ${uniqueSuffix()}`);
    const scenesB = await sceneIds(page, worldB);
    const assetB = await createCanvasAsset(page, worldB, scenesB[0], 6540);
    const fingerprintB = await assetFingerprint(page, assetB);
    await openWorldAndSync(page, worldB, sync);
    await expect
      .poll(() => holdsFingerprint(page, worldB, fingerprintB), {
        timeout: 90_000,
        message: "world B must be cached too, or 'the others are intact' proves nothing",
      })
      .toBe(true);

    // FR-025: a total and a per-world breakdown.
    await openStoragePanel(page);
    const total = await page.getByTestId("storage-total").textContent();
    console.log(`[storage] total reported: ${total?.trim()}`);
    expect(total, "the panel should report a non-zero total").not.toMatch(/^0 B/);

    const before = await panelRows(page);
    console.log(`[storage] rows: ${JSON.stringify([...before])}`);
    expect(
      before.has(worldA),
      "both cached worlds should appear in the breakdown",
    ).toBe(true);
    expect(before.has(worldB), "both cached worlds should appear").toBe(true);

    // FR-026: clear one.
    const rowA = page.locator(`[data-testid="storage-world-row"][data-world-id="${worldA}"]`);
    await rowA.getByTestId("storage-clear-world").click();
    await expect(page.getByTestId("storage-note")).toBeVisible({ timeout: 30_000 });

    const after = await panelRows(page);
    console.log(`[storage] rows after clearing A: ${JSON.stringify([...after])}`);
    expect(
      after.has(worldA),
      "the cleared world should no longer be occupying space",
    ).toBe(false);
    expect(
      after.get(worldB),
      "the other world's figure must be untouched",
    ).toBe(before.get(worldB));

    // What the panel says, checked against what the disk holds. A panel that
    // reported a clear it did not perform would pass every assertion above.
    expect(
      await worldBlobs(page, worldA),
      "clearing must actually remove the blobs, not just the row",
    ).toHaveLength(0);
    expect(
      await holdsFingerprint(page, worldB, fingerprintB),
      "the untouched world's blobs must still be on disk",
    ).toBe(true);

    // The clause that makes the feature safe to use: nothing server-side was
    // touched, so the cleared world opens exactly as an unvisited one does.
    const beforeReopen = sync.count();
    await page.goto(`/world/${worldA}/play`);
    await waitForEngineReady(page);
    const resync = await sync.next(beforeReopen);
    expect(resync.status, JSON.stringify(resync)).toBe("synced");
    await expect(page.locator("canvas")).toBeVisible({ timeout: 30_000 });

    // And it caches again — a cleared world is an unvisited world, not a
    // permanently degraded one.
    await expect
      .poll(() => holdsFingerprint(page, worldA, fingerprintA), {
        timeout: 90_000,
        message: "a cleared world should cache again on its next visit",
      })
      .toBe(true);

    sync.stop();
  });

  test("clear all empties every world, and the account is untouched (FR-026)", async ({
    page,
  }) => {
    const sync = watchCacheSync(page);
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Storage All ${uniqueSuffix()}`,
      "e2estorage",
    );
    const scenes = await sceneIds(page, worldId);
    const assetId = await createCanvasAsset(page, worldId, scenes[0], 1234);
    const fingerprint = await assetFingerprint(page, assetId);
    await openWorldAndSync(page, worldId, sync);
    await expect
      .poll(() => holdsFingerprint(page, worldId, fingerprint), { timeout: 90_000 })
      .toBe(true);

    await openStoragePanel(page);
    await page.getByTestId("storage-clear-all").click();
    await expect(page.getByTestId("storage-note")).toBeVisible({ timeout: 30_000 });

    await expect
      .poll(() => page.getByTestId("storage-empty").isVisible().catch(() => false), {
        timeout: 30_000,
        message: "with everything cleared the panel should say so",
      })
      .toBe(true);
    expect(await worldBlobs(page, worldId)).toHaveLength(0);

    // "Without affecting their account or server-side data" — the world is
    // still listed, still openable, and still has its content.
    await page.goto("/worlds");
    await expect(page.getByText(/E2E Storage All/).first()).toBeVisible({
      timeout: 30_000,
    });
    expect(
      await assetFingerprint(page, assetId),
      "the asset must still be served by the server",
    ).toBe(fingerprint);

    sync.stop();
  });
});
