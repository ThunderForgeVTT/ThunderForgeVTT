import path from "node:path";
import { expect, test, type Route } from "./fixtures/test";
import { registerAndCreateWorld, uniqueSuffix } from "./fixtures/helpers";
import {
  assetFingerprint,
  holdsFingerprint,
  importMapBackground,
  openWorldAndSync,
  sceneBackgroundAssetId,
  sceneIds,
  watchCacheSync,
} from "./fixtures/world-cache";

/**
 * Spec 080 T018 (US1): a scene's background, fetched into the world cache in
 * parts, survives one of those parts being cut off halfway.
 *
 * # Why the numbers are overridden
 *
 * A real map's background is a few hundred KB and the default threshold is
 * 16 MB, so with the defaults it is one plain request and nothing here would
 * be exercised. A dev build takes the downloader's numbers from
 * `__thunderforgeDownloadSettings` (`services/downloads.ts`); a production
 * build never reads it.
 *
 * # Why the proof is the cache, not a screenshot
 *
 * The world cache stores an asset only when its bytes hash to the
 * fingerprint the server published for it. A resume that skipped a byte or
 * repeated one would fail that check and be dropped. So "the cache holds the
 * fingerprint" says the parts were joined exactly right.
 */

const CHAMBER_MAP = path.resolve(
  __dirname,
  "../../../examples/maps/chamber-of-echoing-grief.dd2vtt",
);

const PARTS = {
  threshold: 4 * 1024,
  partSize: 16 * 1024,
  concurrency: 2,
  retryDelayMs: 50,
};

/** The start offset of a `Range: bytes=a-b` header. */
const startOf = (range: string) => Number(/^bytes=(\d+)-/.exec(range)?.[1]);

test("a background part cut halfway is asked for again from where it stopped", async ({
  page,
}) => {
  test.setTimeout(300_000);
  const sync = watchCacheSync(page);

  const worldId = await registerAndCreateWorld(
    page,
    `E2E Resume ${uniqueSuffix()}`,
    "e2eresume",
  );
  const sceneId = (await sceneIds(page, worldId))[0];
  await openWorldAndSync(page, worldId, sync);
  await importMapBackground(page, CHAMBER_MAP);
  const assetId = await sceneBackgroundAssetId(page, worldId, sceneId);
  const fingerprint = await assetFingerprint(page, assetId);

  await page.addInitScript((settings) => {
    (
      globalThis as { __thunderforgeDownloadSettings?: unknown }
    ).__thunderforgeDownloadSettings = settings;
  }, PARTS);

  // Cut the first ranged request after the first part, halfway, and pass
  // everything else through untouched. Routing also turns off the browser's
  // HTTP cache, so every part really goes to the server.
  const ranges: { range: string; status: number }[] = [];
  let cut: string | null = null;
  await page.route(`**/api/canvas-assets/${assetId}*`, async (route: Route) => {
    const range = route.request().headers()["range"];
    if (!range) return route.fallback();
    const response = await route.fetch();
    ranges.push({ range, status: response.status() });
    const body = await response.body();
    if (cut === null && startOf(range) > 0 && body.length > 1) {
      cut = range;
      const headers = { ...response.headers() };
      delete headers["content-length"];
      return route.fulfill({
        status: response.status(),
        headers,
        body: body.subarray(0, Math.floor(body.length / 2)),
      });
    }
    return route.fulfill({ response, body });
  });

  // An asset imported after a sync is planned and prefetched on the next
  // open; that prefetch is the world cache's download.
  await openWorldAndSync(page, worldId, sync);

  await expect
    .poll(() => holdsFingerprint(page, worldId, fingerprint), {
      timeout: 90_000,
      message: "the background must be stored, its bytes matching its hash",
    })
    .toBe(true);

  expect(
    cut,
    `no part was cut; ranges seen: ${JSON.stringify(ranges)}`,
  ).not.toBeNull();
  const cutStart = startOf(cut!);
  const resumed = ranges.find(
    ({ range }) =>
      startOf(range) > cutStart && startOf(range) < cutStart + PARTS.partSize,
  );
  expect(
    resumed,
    `the cut part must be asked for again from inside it: ${JSON.stringify(ranges)}`,
  ).toBeTruthy();
  expect(resumed!.status).toBe(206);
  expect(
    ranges.filter(({ range }) => startOf(range) === cutStart),
    "the cut part must not be fetched from its start a second time",
  ).toHaveLength(1);

  await page.unrouteAll({ behavior: "ignoreErrors" });
});
