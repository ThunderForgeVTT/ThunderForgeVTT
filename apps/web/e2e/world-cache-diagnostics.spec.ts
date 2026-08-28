import path from "path";
import { expect, test, type Page, type Request } from "@playwright/test";
import { registerAndCreateWorld, uniqueSuffix } from "./fixtures/helpers";
import {
  assetFingerprint,
  assetUrl,
  createScene,
  holdsFingerprint,
  importMapBackground,
  openWorldAndSync,
  sceneBackgroundAssetId,
  sceneIds,
  sceneNames,
  switchToScene,
  watchCacheSync,
  type SyncSummary,
} from "./fixtures/world-cache";

/**
 * The cache diagnostics panel (spec 028 Phase 11, T123/T124, FR-051/FR-052,
 * SC-017/SC-018).
 *
 * # What SC-017 actually demands, and why these tests read rendered text
 *
 * "The performance outcomes SC-001 through SC-003 can be confirmed from the
 * diagnostics view during an ordinary session, without developer tooling or a
 * test harness." The suites that measure SC-001 and SC-003 already exist
 * (`world-cache.spec.ts`), and they measure them the way SC-017 says a person
 * must not have to: by attaching to the browser's network events. So these
 * tests deliberately do *not* re-measure the wire. They drive the ordinary UI
 * — open a world, open the dock's Settings tab, read the panel — and assert
 * that what a person sees there is a true and sufficient account of what the
 * cache did.
 *
 * Each figure is asserted twice over: once as the sentence a person reads,
 * and once as the machine-readable `data-*` attribute beside it. The pairing
 * is the point. A panel whose prose and numbers had drifted apart would be
 * exactly the failure SC-017 cares about — a readout that looks like an
 * answer and is not one — and either assertion alone would miss it.
 *
 * # Why a scene switch, and not simply a reload
 *
 * The cached read path is only consulted mid-session. At boot the scene's
 * background sprite is loaded as soon as scene state arrives, which is before
 * `sync_world_cache` has resolved and published the fingerprints `try_cached`
 * needs, so an opening scene's art never goes through the cache — this is
 * documented at length in `world-cache-repair.spec.ts`. A **mid-session scene
 * switch** is the one moment in the application where a stored blob is
 * actually read back, so it is the only way to make the panel report a cache
 * hit at all. Every test here therefore parks on a scene with no art and
 * switches to the one that has some.
 */

/** A small real map, so the scene has art the engine genuinely loads. */
const CHAMBER_MAP = path.resolve(
  __dirname,
  "../../../examples/maps/chamber-of-echoing-grief.dd2vtt",
);

/** A second, visibly different map — the "one changed asset" of SC-003. */
const MEETING_MAP = path.resolve(
  __dirname,
  "../../../examples/maps/azheim-meeting.dd2vtt",
);

/** The figures the panel is showing, read off `cache-figures`. */
interface Figures {
  cacheItems: number;
  cacheBytes: number;
  networkItems: number;
  networkBytes: number;
  peerItems: number;
  peerBytes: number;
}

/**
 * Open the dock's Settings tab and wait for the panel.
 *
 * Two clicks from the map, with the world still open behind it — which is
 * what makes this an "ordinary session" rather than a harness.
 */
async function openCachePanel(page: Page): Promise<void> {
  const panel = page.locator('[data-testid="cache-panel"]:visible').first();
  if (await panel.isVisible().catch(() => false)) return;
  await page.getByTestId("world-dock-tab-settings").click();
  await expect(panel).toBeVisible({ timeout: 30_000 });
}

/**
 * Read the panel's figures.
 *
 * Throws rather than returning zeroes when the panel is reporting an absence:
 * "the engine is not running so there is nothing to say" and "the engine is
 * running and says nothing happened" are the two answers this whole suite is
 * built to keep apart, and silently collapsing them into `0` would let every
 * assertion below pass on a page where the cache never ran.
 */
async function figures(page: Page): Promise<Figures> {
  // The panel's first sample resolves a tick after it mounts, and until it
  // does the panel says "Reading…" rather than either answer. Waiting for a
  // settled state here — instead of throwing on the transient one — is what
  // keeps every `expect.poll` below from failing on its own first attempt.
  await expect(
    page.locator(
      '[data-testid="cache-figures"]:visible, [data-testid="cache-absent"]:visible',
    ),
  ).toHaveCount(1, { timeout: 30_000 });

  const node = page.locator('[data-testid="cache-figures"]:visible').first();
  if (!(await node.isVisible().catch(() => false))) {
    const absent = await page
      .locator('[data-testid="cache-absent"]:visible')
      .first()
      .textContent()
      .catch(() => null);
    throw new Error(
      `the cache panel is reporting no figures at all: ${absent ?? "(no panel)"}`,
    );
  }
  const read = async (attribute: string): Promise<number> =>
    Number((await node.getAttribute(attribute)) ?? NaN);
  return {
    cacheItems: await read("data-cache-items"),
    cacheBytes: await read("data-cache-bytes"),
    networkItems: await read("data-network-items"),
    networkBytes: await read("data-network-bytes"),
    peerItems: await read("data-peer-items"),
    peerBytes: await read("data-peer-bytes"),
  };
}

/** How many bytes the server actually serves for an asset. */
async function assetByteSize(page: Page, assetId: string): Promise<number> {
  const size = await page.evaluate(async (url) => {
    const res = await fetch(url, {
      credentials: "same-origin",
      cache: "no-store",
    });
    if (!res.ok) return -1;
    return (await res.arrayBuffer()).byteLength;
  }, assetUrl(assetId));
  expect(size, `asset ${assetId} should be readable`).toBeGreaterThan(1_000);
  return size;
}

async function reloadAndSync(
  page: Page,
  sync: ReturnType<typeof watchCacheSync>,
): Promise<SyncSummary> {
  const before = sync.count();
  await page.reload();
  return sync.next(before);
}

/**
 * A world parked on an art-less scene, with a second scene whose background
 * has been imported and cached.
 *
 * The import happens through the real UI because that is the only thing in
 * the application that gives a scene a *persistent* background asset — see
 * `importMapBackground`. The reload afterwards is load-bearing: an asset
 * created after a sync is not in that sync's plan, so no fingerprint is
 * published for it and the cache deliberately declines to store what it
 * fetched. The next open both plans it and prefetches it.
 */
async function worldWithCachedScene(
  page: Page,
  sync: ReturnType<typeof watchCacheSync>,
  prefix: string,
): Promise<{
  worldId: string;
  emptyScene: string;
  artScene: string;
  assetId: string;
  names: Map<string, string>;
}> {
  const worldId = await registerAndCreateWorld(
    page,
    `E2E Diagnostics ${uniqueSuffix()}`,
    prefix,
  );
  const emptyScene = (await sceneIds(page, worldId))[0];
  expect(emptyScene, "a new world should have a scene").toBeTruthy();
  const artScene = await createScene(page, worldId, "Painted Scene");

  const first = await openWorldAndSync(page, worldId, sync);
  expect(first.status, JSON.stringify(first)).toBe("synced");

  const names = await sceneNames(page, worldId);
  await switchToScene(page, names.get(artScene)!);
  await importMapBackground(page, CHAMBER_MAP);
  const assetId = await sceneBackgroundAssetId(page, worldId, artScene);
  await switchToScene(page, names.get(emptyScene)!);

  return { worldId, emptyScene, artScene, assetId, names };
}

test.describe("Client world cache — the diagnostics panel (US6, T123)", () => {
  test.setTimeout(300_000);

  test("the panel shows a first visit downloading the art and a revisit reading it off this device (SC-001, SC-017)", async ({
    page,
  }) => {
    const sync = watchCacheSync(page);
    const world = await worldWithCachedScene(page, sync, "e2ediag");
    const fingerprint = await assetFingerprint(page, world.assetId);
    const assetSize = await assetByteSize(page, world.assetId);

    // ---- First visit: the art is not held yet, so it is downloaded. ----
    //
    // This is the half of the test that makes the other half mean something.
    // "Nothing was downloaded" is only evidence of a working cache if the
    // panel is capable of saying that something *was*, on a visit where
    // something genuinely was.
    const firstOpen = await reloadAndSync(page, sync);
    expect(firstOpen.status, JSON.stringify(firstOpen)).toBe("synced");
    await openCachePanel(page);

    await expect
      .poll(async () => (await figures(page)).networkBytes, {
        timeout: 120_000,
        message:
          "the first visit must show the art being downloaded, or a later " +
          "'downloaded nothing' proves only that the panel never populated",
      })
      .toBeGreaterThan(0);

    const cold = await figures(page);
    console.log(`[diagnostics] first visit: ${JSON.stringify(cold)}`);
    expect(
      cold.cacheItems,
      "nothing can have come off this device before anything was stored",
    ).toBe(0);
    // What was downloaded is the art, not some incidental fetch: within 10%
    // of the asset's own size, the same bound SC-003 uses.
    expect(cold.networkBytes).toBeGreaterThanOrEqual(assetSize * 0.9);
    expect(cold.networkBytes).toBeLessThanOrEqual(assetSize * 1.1);

    // Checked against the disk, because a panel that reported a download it
    // never performed would satisfy everything above.
    await expect
      .poll(() => holdsFingerprint(page, world.worldId, fingerprint), {
        timeout: 120_000,
        message: "the downloaded art must actually be on disk",
      })
      .toBe(true);

    // ---- The revisit: SC-001, read off the panel. ----
    const revisit = await reloadAndSync(page, sync);
    expect(revisit.status, JSON.stringify(revisit)).toBe("synced");
    await openCachePanel(page);
    // The switch is what sends the load through the cache; see the file's
    // header. Parking on the art-less scene at boot is what keeps this
    // visit's only asset load a cached one.
    await switchToScene(page, world.names.get(world.artScene)!);

    await expect
      .poll(async () => (await figures(page)).cacheItems, {
        timeout: 120_000,
        message: "the revisit should load the art from this device",
      })
      .toBeGreaterThanOrEqual(1);

    const warm = await figures(page);
    console.log(`[diagnostics] revisit: ${JSON.stringify(warm)}`);

    // SC-001 as a person reads it: everything asked for came off the disk,
    // and the server sent none of it.
    expect(
      warm.networkBytes,
      "a revisited, unchanged world should download no asset bytes at all",
    ).toBe(0);
    expect(warm.networkItems).toBe(0);
    expect(warm.cacheBytes).toBeGreaterThanOrEqual(assetSize * 0.9);
    expect(warm.cacheBytes).toBeLessThanOrEqual(assetSize * 1.1);

    // And the sentences agree with the numbers. A panel whose prose had
    // drifted from its attributes would be the exact SC-017 failure — a
    // readout that looks like an answer without being one.
    await expect(page.getByTestId("cache-served-locally")).toContainText(
      `${warm.cacheItems} of ${warm.cacheItems}`,
    );
    await expect(page.getByTestId("cache-served-locally")).toContainText(
      /did not have to be downloaded/i,
    );
    await expect(page.getByTestId("cache-from-server")).toContainText("0 B");

    sync.stop();
  });

  test("the panel accounts for one changed asset and nothing else (SC-003, SC-017)", async ({
    page,
  }) => {
    const sync = watchCacheSync(page);
    const world = await worldWithCachedScene(page, sync, "e2ediag3");
    const firstAsset = world.assetId;
    const firstFingerprint = await assetFingerprint(page, firstAsset);

    // Settle: open once so the first map is planned, prefetched and stored.
    const settling = await reloadAndSync(page, sync);
    expect(settling.status, JSON.stringify(settling)).toBe("synced");
    await expect
      .poll(() => holdsFingerprint(page, world.worldId, firstFingerprint), {
        timeout: 120_000,
        message: "the first map must be cached before anything is changed",
      })
      .toBe(true);

    // Change exactly one thing in an otherwise-unchanged world: give the
    // other scene art of its own.
    await switchToScene(page, world.names.get(world.emptyScene)!);
    await importMapBackground(page, MEETING_MAP);
    const changedAsset = await sceneBackgroundAssetId(
      page,
      world.worldId,
      world.emptyScene,
    );
    expect(changedAsset).not.toBe(firstAsset);
    const changedSize = await assetByteSize(page, changedAsset);

    // Reopen. The world now holds one item this browser already has and one
    // it has never seen — the shape SC-003 describes.
    const delta = await reloadAndSync(page, sync);
    expect(delta.status, JSON.stringify(delta)).toBe("synced");
    await openCachePanel(page);

    await expect
      .poll(async () => (await figures(page)).networkBytes, {
        timeout: 120_000,
        message: "the changed asset should be downloaded",
      })
      .toBeGreaterThan(0);

    // Give anything else that was going to be fetched a chance to be fetched,
    // so "and nothing else" is a claim about a settled figure rather than
    // about a figure caught early.
    await page.waitForTimeout(5_000);

    const after = await figures(page);
    console.log(
      `[diagnostics] one changed asset: ${JSON.stringify(after)} for a ` +
        `${changedSize}B change — ${((after.networkBytes / changedSize) * 100).toFixed(1)}%`,
    );

    // SC-003, read off the panel: the bytes downloaded are within 10% of the
    // one changed asset's size, and the already-held map was not among them.
    expect(after.networkItems).toBe(1);
    expect(after.networkBytes).toBeGreaterThanOrEqual(changedSize * 0.9);
    expect(after.networkBytes).toBeLessThanOrEqual(changedSize * 1.1);

    // The other half of "and nothing else": the map this browser already had
    // is still served off the disk on this very visit.
    await switchToScene(page, world.names.get(world.artScene)!);
    await expect
      .poll(async () => (await figures(page)).cacheItems, {
        timeout: 120_000,
        message: "the unchanged map should still come off this device",
      })
      .toBeGreaterThanOrEqual(1);
    const settled = await figures(page);
    expect(
      settled.networkBytes,
      "serving the unchanged map must not have downloaded anything more",
    ).toBe(after.networkBytes);

    sync.stop();
  });
});

/**
 * One request the client made, reduced to the parts that could carry a
 * number out of the browser.
 *
 * Headers are deliberately absent. The client does not set any, and the ones
 * the browser sets — `Content-Length` above all — carry byte counts that have
 * nothing to do with the cache and would make a value scan meaningless.
 */
interface Outbound {
  method: string;
  url: string;
  contentType: string;
  body: string | null;
}

/**
 * Every request the page makes, from now until `stop()`.
 *
 * `page.on("request")` rather than `route`: it sees `sendBeacon` and
 * `fetch(..., {keepalive: true})` — the two shapes telemetry actually takes,
 * both of which fire during unload — and it cannot itself alter what the page
 * does.
 */
function recordOutbound(page: Page): {
  all: () => Outbound[];
  stop: () => Outbound[];
} {
  const seen: Outbound[] = [];
  const onRequest = (request: Request) => {
    seen.push({
      method: request.method(),
      url: request.url(),
      contentType: request.headers()["content-type"] ?? "",
      body: request.postData(),
    });
  };
  page.on("request", onRequest);
  return {
    all: () => [...seen],
    stop: () => {
      page.off("request", onRequest);
      return [...seen];
    },
  };
}

/**
 * Names a cache-statistics payload would have to use.
 *
 * Chosen to be *shapes of this feature's own vocabulary*, not generic words.
 * `evict` and `held` are deliberately absent even though the cache uses them,
 * because `worldSyncPlan`'s query text — which the client legitimately sends
 * on every world open — contains both, and a detector that fired on the
 * feature working correctly would be turned off within a week.
 */
const STATISTIC_KEYS = [
  "cacheitems",
  "cachebytes",
  "cachehit",
  "cachestats",
  "cacheusage",
  "hitrate",
  "bytessaved",
  "bytesavoided",
  "bytesfrompeers",
  "networkbytes",
  "networkitems",
  "peerbytes",
  "peeritems",
  "prefetchedbytes",
  "prefetcheditems",
  "unverifieditems",
  "rowsrepaired",
  "blobsreclaimed",
  "blobsremoved",
  "telemetry",
  "usagestats",
];

/**
 * Whether one request carries cache statistics.
 *
 * This is the whole substance of T124, and it is written to answer a question
 * a request count cannot. The application legitimately makes a great many
 * requests during a session — GraphQL for every panel, the session
 * heartbeat, the assets themselves — so "no requests were made" is neither
 * true nor the property under test. What SC-018 forbids is a request whose
 * *content* is the client's cache figures, and content is what this reads.
 *
 * Two independent tests, because a leak could take either form:
 *
 * 1. **By name.** A payload with a `cacheBytes` or `hitRate` field in it, no
 *    matter what the value is. This catches a leak reporting zeroes, or one
 *    added before the figures were populated.
 * 2. **By value.** Any of the session's own large figures appearing as a
 *    whole number in the url or body. This catches a payload that renamed the
 *    fields, or hid them in a generic `metrics: []`.
 *
 * Only figures above `VALUE_FLOOR` are looked for. Small numbers — an item
 * count of 1, a byte total of 0 — occur constantly and by coincidence in
 * every id, timestamp and page number the client sends, so searching for them
 * would produce a detector that fires on everything and therefore means
 * nothing. Byte totals are large and distinctive, and they are also the
 * figures that would actually be worth exfiltrating.
 */
const VALUE_FLOOR = 1_000;

/**
 * A request for the application's own source, which the dev server answers
 * by path.
 *
 * Vite serves modules by URL in development, so `GET /src/engine/bevy/
 * cacheStats.ts` puts the word "cacheStats" on the wire — and the name scan
 * below caught it, correctly by its own rules and wrongly in substance. That
 * request carries no figure; it *is* the code that computes them, and in a
 * production build it would not be a request at all.
 *
 * Excluding it does not soften the check. Only same-origin GETs into the dev
 * module graph are skipped: a beacon is a POST, a leak smuggled into a query
 * string would be on `/api/`, and anything sent to a third party is a
 * different origin. All of those are still read in full.
 */
function isDevModuleRequest(request: Outbound): boolean {
  if (request.method !== "GET") return false;
  const path = new URL(request.url, "http://localhost:5173").pathname;
  return (
    path.startsWith("/src/") ||
    path.startsWith("/node_modules/") ||
    path.startsWith("/@")
  );
}

function carriesStatistics(request: Outbound, values: number[]): string | null {
  if (isDevModuleRequest(request)) return null;
  const haystack = `${request.url}\n${request.body ?? ""}`;
  const lowered = haystack.toLowerCase();
  for (const key of STATISTIC_KEYS) {
    if (lowered.includes(key)) return `field name "${key}"`;
  }
  // Multipart bodies are raw image bytes read back as a lossy string, in
  // which a coincidental run of digits is possible. They are excluded from
  // the *value* scan only, and never from the name scan above — a real
  // telemetry payload would still be caught by its field names, whereas a
  // false positive here would be a flake that teaches everyone to ignore
  // this test.
  if (request.contentType.includes("multipart/form-data")) return null;
  for (const value of values) {
    if (value < VALUE_FLOOR) continue;
    // Delimited, so 179378 does not match inside 1793789 or a uuid's digits.
    if (new RegExp(`(?<![0-9])${value}(?![0-9])`).test(haystack)) {
      return `value ${value}`;
    }
  }
  return null;
}

test.describe("Client world cache — diagnostics stay on this machine (T124)", () => {
  test.setTimeout(300_000);

  test("no request the client makes carries the cache figures (SC-018, FR-052/FR-054)", async ({
    page,
  }) => {
    const sync = watchCacheSync(page);
    const world = await worldWithCachedScene(page, sync, "e2enotel");
    const assetSize = await assetByteSize(page, world.assetId);

    // Recording starts here, before the reload that produces the figures.
    //
    // That is the complete window, not a convenient one: a page reload
    // discards the wasm instance and with it the tally, so every figure this
    // session reports is produced after this line. A request made before a
    // number exists cannot be carrying it. Recording from here also keeps the
    // multipart map upload out of the capture entirely, so the value scan
    // never has to reason about binary bodies.
    const outbound = recordOutbound(page);

    const opened = await reloadAndSync(page, sync);
    expect(opened.status, JSON.stringify(opened)).toBe("synced");
    await openCachePanel(page);
    await switchToScene(page, world.names.get(world.artScene)!);

    // The positive case, and this test is worthless without it. "No request
    // carried statistics" is indistinguishable from "there were never any
    // statistics" unless the panel is shown to have real, large figures in
    // it — which is exactly what a leak would have had to carry.
    await expect
      .poll(
        async () => {
          const now = await figures(page);
          return Math.max(now.cacheBytes, now.networkBytes);
        },
        {
          timeout: 120_000,
          message:
            "the panel must hold real figures before their absence on the " +
            "wire can mean anything",
        },
      )
      .toBeGreaterThan(VALUE_FLOOR);

    const shown = await figures(page);
    console.log(`[diagnostics] figures on screen: ${JSON.stringify(shown)}`);

    // Go on using the application after the figures exist, including a
    // navigation away from the world. An unload beacon is the classic shape
    // for this kind of leak, and it would only ever fire here.
    await page.goto("/settings/storage");
    await expect(page.getByTestId("storage-panel")).toBeVisible({
      timeout: 60_000,
    });
    await page.waitForTimeout(3_000);

    const requests = outbound.stop();
    const values = [
      shown.cacheBytes,
      shown.networkBytes,
      shown.peerBytes,
      assetSize,
    ];

    // Evidence that the client really was talking to the server throughout,
    // so a clean result is a statement about content and not about silence.
    const afterFigures = requests.filter((r) => r.url.includes("/api/"));
    console.log(
      `[diagnostics] inspected ${requests.length} requests ` +
        `(${afterFigures.length} to the API) against ${JSON.stringify(values)}`,
    );
    expect(
      afterFigures.length,
      "the session should have been making API requests all along",
    ).toBeGreaterThan(3);

    // The detector, checked against payloads that *do* leak — otherwise a
    // clean sweep below would be equally consistent with a detector that
    // cannot see anything at all.
    expect(
      carriesStatistics(
        {
          method: "POST",
          url: "/api/telemetry",
          contentType: "application/json",
          body: JSON.stringify(shown),
        },
        values,
      ),
      "the detector must catch a payload that names the figures",
    ).not.toBeNull();
    // Smuggled under a field name that means nothing, so only the *value*
    // scan can catch it. It has to be a figure this run actually produced
    // and that clears the floor: an earlier version used `cacheBytes`, which
    // is legitimately 0 here because this scenario never serves from cache,
    // and the detector is right to ignore a 0 — searching for it would match
    // inside every uuid on the wire. The self-test was wrong, not the
    // detector, and it failed exactly as a self-test should.
    const detectable = values.find((value) => value >= VALUE_FLOOR);
    expect(
      detectable,
      "this scenario must produce at least one figure large enough to be detectable, " +
        "or the value scan is untested",
    ).toBeDefined();
    expect(
      carriesStatistics(
        {
          method: "POST",
          url: "/api/graphql",
          contentType: "application/json",
          body: JSON.stringify({
            query: "mutation($m: [Int!]!) { record(measurements: $m) { ok } }",
            variables: { m: [detectable] },
          }),
        },
        values,
      ),
      "the detector must catch figures smuggled under innocent field names",
    ).not.toBeNull();

    // SC-018.
    const leaks = requests
      .map((request) => ({ request, why: carriesStatistics(request, values) }))
      .filter((found) => found.why !== null);
    expect(
      leaks.map(
        (leak) => `${leak.request.method} ${leak.request.url}: ${leak.why}`,
      ),
      "no request may carry the client's cache statistics",
    ).toEqual([]);

    sync.stop();
  });
});
