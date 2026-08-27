import {
  expect,
  test,
  type Browser,
  type ConsoleMessage,
  type Page,
  type Response,
} from "@playwright/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";

/**
 * Spec 028 User Story 2 (T042-T044): cached content stops being readable
 * when the permission that justified caching it goes away.
 *
 * This is the disclosure half of the feature. Story 1 proves the cache is
 * fast; these prove it is not a back door. Every assertion below is about
 * one of three things:
 *
 *   1. the server refuses (FR-014 — authorization is re-established on every
 *      open, never inferred from possession),
 *   2. the bytes leave the local store (FR-015),
 *   3. whatever bytes remain are inert without the session key (FR-016,
 *      FR-016a, FR-016b).
 *
 * # Every fixture here asserts its own success
 *
 * The repeated failure mode of this feature has been a helper that swallowed
 * its error and produced a confident, wrong green: a storm with no events, a
 * "starvation" that was an inert stimulus, a cold open reporting zero
 * requests because nothing had ever uploaded. A revocation test is
 * especially vulnerable to it — "the cached bytes are gone" is
 * indistinguishable from "the bytes were never written" unless the setup
 * proved they were written. So each test asserts the *positive* state first
 * (the player could read the asset, the blob is on disk under its
 * fingerprint) and only then revokes.
 */

/** Where the engine fetches canvas assets (`ASSET_URL_PREFIX` in cached_assets.rs). */
function assetUrl(assetId: string): string {
  return `/api/canvas-assets/${assetId}.webp`;
}

/** The summary `sync_world_cache` returns, as WorldPage logs it. */
interface SyncSummary {
  status: string;
  worldId?: string;
  held?: number;
  fetch?: number;
  evicted?: number;
  blobsRemoved?: number;
  evictFailures?: number;
  prefetching?: number;
  reason?: string;
}

/**
 * Observe the cache sync from the outside.
 *
 * `WorldPage` logs the wasm summary as `console.debug("[world-cache] sync",
 * summary)`, and that is deliberately the *only* thing the TypeScript side
 * ever learns about the cache — counts, never item ids. Reading the same
 * line is therefore the honest way for a test to see what the sync did,
 * rather than reaching into internals the app itself cannot see.
 */
function watchCacheSync(page: Page) {
  const summaries: SyncSummary[] = [];
  const pending: Promise<void>[] = [];

  const onConsole = (msg: ConsoleMessage) => {
    if (!msg.text().startsWith("[world-cache] sync")) return;
    const args = msg.args();
    if (args.length < 2) return;
    pending.push(
      args[1]
        .jsonValue()
        .then((value) => {
          if (value && typeof value === "object") {
            summaries.push(value as SyncSummary);
          }
        })
        // A handle can be dead if the page navigated mid-log. Dropping it
        // undercounts, which can only make the "did a sync happen?" waits
        // below time out — never pass spuriously.
        .catch(() => {}),
    );
  };

  page.on("console", onConsole);
  return {
    /** Every summary seen so far, oldest first. */
    async all(): Promise<SyncSummary[]> {
      await Promise.all(pending);
      return [...summaries];
    },
    /** Wait for a summary logged after `since` summaries had been seen. */
    async next(since: number, timeout = 45_000): Promise<SyncSummary> {
      const deadline = Date.now() + timeout;
      for (;;) {
        const seen = await this.all();
        if (seen.length > since) return seen[seen.length - 1];
        if (Date.now() > deadline) {
          throw new Error(
            `no world-cache sync summary appeared within ${timeout}ms ` +
              `(saw ${seen.length}, wanted more than ${since})`,
          );
        }
        await page.waitForTimeout(500);
      }
    },
    count(): number {
      return summaries.length;
    },
    stop() {
      page.off("console", onConsole);
    },
  };
}

interface OpfsFile {
  path: string;
  size: number;
}

/**
 * Walk the origin private file system and list every file in it.
 *
 * Deliberately not "ask the app what it holds": SC-004a requires the store
 * be inspected directly, because testing through the application only ever
 * proves the application does not read something.
 */
async function opfsFiles(page: Page): Promise<OpfsFile[]> {
  return page.evaluate(async () => {
    interface DirLike {
      kind: string;
      entries(): AsyncIterable<[string, DirLike]>;
      getFile(): Promise<File>;
    }
    const root = (await navigator.storage.getDirectory()) as unknown as DirLike;
    const found: { path: string; size: number }[] = [];
    const walk = async (dir: DirLike, prefix: string): Promise<void> => {
      for await (const [name, handle] of dir.entries()) {
        const path = `${prefix}/${name}`;
        if (handle.kind === "directory") {
          await walk(handle, path);
        } else {
          found.push({ path, size: (await handle.getFile()).size });
        }
      }
    };
    await walk(root, "");
    return found;
  });
}

/**
 * The cached blobs for one world: `/<user-scope>/<world-id>/<fingerprint>.bin`
 * (see `BlobPath` in crates/thunderforge-cache-browser/src/opfs.rs).
 */
async function worldBlobs(page: Page, worldId: string): Promise<OpfsFile[]> {
  const files = await opfsFiles(page);
  return files.filter(
    (f) => f.path.includes(`/${worldId}/`) && f.path.endsWith(".bin"),
  );
}

/** True when a blob named for `fingerprint` is on disk for this world. */
async function holdsFingerprint(
  page: Page,
  worldId: string,
  fingerprint: string,
): Promise<boolean> {
  const blobs = await worldBlobs(page, worldId);
  return blobs.some((f) => f.path.endsWith(`/${fingerprint}.bin`));
}

/**
 * The fingerprint the cache files an asset under: SHA-256 of exactly the
 * bytes the server serves. `OpfsStore::write_blob` verifies the bytes hash to
 * the name it stores them under, so this is the blob's filename, computed
 * without the test having to know anything the client knows.
 *
 * Asserts a 200 — which doubles as the "this user really could read this
 * asset" precondition every revocation test needs.
 */
async function assetFingerprint(page: Page, assetId: string): Promise<string> {
  const probe = await page.evaluate(async (url) => {
    // `cache: "no-store"`, always. The route answers with
    // `Cache-Control: private, max-age=3600`, so an ordinary fetch would be
    // answered by the browser's own HTTP cache without the server ever
    // seeing it — and a test asking "may this session still read this?"
    // would then be reading a year-old answer. (That HTTP cache is a
    // separate store from this feature's, and it is a real, if bounded,
    // post-revocation window; see the note in the T042 test.)
    const res = await fetch(url, {
      credentials: "same-origin",
      cache: "no-store",
    });
    if (!res.ok) return { ok: false as const, status: res.status };
    const bytes = new Uint8Array(await res.arrayBuffer());
    const digest = await crypto.subtle.digest("SHA-256", bytes);
    const hex = Array.from(new Uint8Array(digest))
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");
    return { ok: true as const, status: res.status, hex, size: bytes.length };
  }, assetUrl(assetId));

  expect(
    probe.ok,
    `asset ${assetId} should be readable by this session (status ${probe.status})`,
  ).toBe(true);
  if (!probe.ok) return "";
  expect(probe.size, "asset should have real bytes").toBeGreaterThan(1_000);
  return probe.hex;
}

/**
 * The status the **server** gives this session for an asset's bytes.
 *
 * The unique query string is not decoration. `public/sw.js` serves
 * `/api/canvas-assets/*` cache-first from a Cache Storage entry keyed by the
 * whole request URL, and a service worker sits in front of `cache:
 * "no-store"` as well — so a plain fetch of a previously-loaded asset never
 * reaches the network and answers 200 no matter what the server thinks. A
 * fresh URL misses that cache and asks the origin, which is the question this
 * helper exists to ask. What the service-worker cache still holds is a
 * separate question, measured by `serviceWorkerHoldsAsset` below.
 */
async function assetStatus(page: Page, assetId: string): Promise<number> {
  return page.evaluate(
    async ({ url, nonce }) => {
      const res = await fetch(`${url}?probe=${nonce}`, {
        credentials: "same-origin",
        cache: "no-store",
      });
      return res.status;
    },
    { url: assetUrl(assetId), nonce: `${Date.now()}${Math.random()}` },
  );
}

/**
 * Whether the canvas-asset service worker still holds readable bytes for
 * this asset, read straight out of Cache Storage.
 *
 * Reported, not asserted — see the note in the T042 test. This is a
 * different store from the one spec 028 governs, and it is measured here so
 * the gap is visible rather than invisible.
 */
async function serviceWorkerHoldsAsset(
  page: Page,
  assetId: string,
): Promise<number> {
  return page.evaluate(async (url) => {
    const names = await caches.keys();
    for (const name of names) {
      const hit = await (await caches.open(name)).match(url);
      if (hit) return (await hit.arrayBuffer()).byteLength;
    }
    return 0;
  }, assetUrl(assetId));
}

interface GqlResult<T> {
  data?: T;
  errors?: { message: string; extensions?: Record<string, unknown> }[];
}

/** Scenes as the caller may see them. A GM sees hidden ones too. */
async function sceneIds(page: Page, worldId: string): Promise<string[]> {
  const res = await graphql<GqlResult<{ scenes: { sceneId: string }[] }>>(
    page,
    `
      query ($worldId: UUID!) {
        scenes(worldId: $worldId) {
          sceneId
          hidden
        }
      }
    `,
    { worldId },
  );
  expect(res.errors, `scenes query failed: ${JSON.stringify(res.errors)}`).toBe(
    undefined,
  );
  return (res.data?.scenes ?? []).map((s) => s.sceneId);
}

/**
 * Set a scene's player visibility, asserting the server agreed.
 *
 * `scenes.hidden` **defaults to true**, so a freshly created world's scene is
 * invisible to players until this is called. Forgetting that produces a test
 * where the player legitimately caches nothing and every "the content is
 * gone" assertion passes for the wrong reason — which is precisely the class
 * of false green this file is written to avoid.
 */
async function setSceneHidden(
  page: Page,
  sceneId: string,
  hidden: boolean,
): Promise<void> {
  const res = await graphql<
    GqlResult<{ updateSceneHidden: { sceneId: string; hidden: boolean } }>
  >(
    page,
    `
      mutation ($sceneId: UUID!, $hidden: Boolean!) {
        updateSceneHidden(sceneId: $sceneId, hidden: $hidden) {
          sceneId
          hidden
        }
      }
    `,
    { sceneId, hidden },
  );
  expect(
    res.errors,
    `updateSceneHidden failed: ${JSON.stringify(res.errors)}`,
  ).toBe(undefined);
  expect(res.data?.updateSceneHidden.hidden).toBe(hidden);
}

/** Create a second scene, asserting it exists. */
async function createScene(
  page: Page,
  worldId: string,
  name: string,
): Promise<string> {
  const res = await graphql<GqlResult<{ createScene: { sceneId: string } }>>(
    page,
    `
      mutation ($input: GraphQLCreateSceneInput!) {
        createScene(input: $input) {
          sceneId
        }
      }
    `,
    { input: { worldId, name } },
  );
  expect(res.errors, `createScene failed: ${JSON.stringify(res.errors)}`).toBe(
    undefined,
  );
  const sceneId = res.data?.createScene.sceneId;
  expect(sceneId, "createScene should return a scene id").toBeTruthy();
  return sceneId as string;
}

/**
 * Upload a real canvas asset to one scene, and assert it uploaded.
 *
 * Adapted from `world-cache.spec.ts`'s `createCanvasAsset` — same reasons for
 * the 512x512 noise (a trivial asset is not representative and compresses to
 * nothing) and the same insistence on failing loudly. `seed` varies the noise
 * so two assets in one world have genuinely different bytes, and therefore
 * different fingerprints and different filenames on disk. That difference is
 * what makes T043 able to say *which* blob survived rather than just how
 * many.
 */
async function createCanvasAsset(
  page: Page,
  worldId: string,
  sceneId: string,
  seed: number,
): Promise<string> {
  const result = await page.evaluate(
    async ({ world, scene, noiseSeed }) => {
      const csrf = () =>
        document.cookie
          .split(";")
          .map((p) => p.trim())
          .find((p) => p.startsWith("csrf_token="))
          ?.slice("csrf_token=".length);

      const canvas = document.createElement("canvas");
      canvas.width = 512;
      canvas.height = 512;
      const ctx = canvas.getContext("2d")!;
      const img = ctx.createImageData(512, 512);
      let state = noiseSeed;
      for (let i = 0; i < img.data.length; i += 4) {
        state = (state * 1103515245 + 12345) & 0x7fffffff;
        img.data[i] = state & 0xff;
        img.data[i + 1] = (state >> 8) & 0xff;
        img.data[i + 2] = (state >> 16) & 0xff;
        img.data[i + 3] = 255;
      }
      ctx.putImageData(img, 0, 0);
      const blob: Blob = await new Promise((resolve) =>
        canvas.toBlob((b) => resolve(b!), "image/png"),
      );
      const bin = new Uint8Array(await blob.arrayBuffer());

      const form = new FormData();
      form.append(
        "operations",
        JSON.stringify({
          query: `mutation($worldId: UUID!, $sceneId: UUID!, $file: Upload!) {
            uploadCanvasImage(worldId: $worldId, sceneId: $sceneId, kind: PASTED, file: $file) { id byteSize }
          }`,
          variables: { worldId: world, sceneId: scene, file: null },
        }),
      );
      form.append("map", JSON.stringify({ "0": ["variables.file"] }));
      form.append("0", new Blob([bin], { type: "image/png" }), "noise.png");

      const token = csrf();
      const res = await fetch("/api/graphql", {
        method: "POST",
        credentials: "same-origin",
        headers: token ? { "x-csrf-token": token } : {},
        body: form,
      });
      const json = await res.json();
      const assetId = json?.data?.uploadCanvasImage?.id;
      return assetId
        ? { ok: true as const, assetId }
        : { ok: false as const, why: JSON.stringify(json) };
    },
    { world: worldId, scene: sceneId, noiseSeed: seed },
  );

  expect(result.ok, `asset upload failed: ${result.ok ? "" : result.why}`).toBe(
    true,
  );
  return result.ok ? result.assetId : "";
}

/** The player member of a world, from the GM's session. Asserts there is one. */
async function playerUserId(page: Page, worldId: string): Promise<string> {
  const res = await graphql<
    GqlResult<{ worldMembers: { userId: string; role: string }[] }>
  >(
    page,
    `
      query ($worldId: ID!) {
        worldMembers(worldId: $worldId) {
          userId
          role
        }
      }
    `,
    { worldId },
  );
  expect(res.errors, `worldMembers failed: ${JSON.stringify(res.errors)}`).toBe(
    undefined,
  );
  const players = (res.data?.worldMembers ?? []).filter(
    (m) => m.role === "Player",
  );
  expect(
    players.length,
    `expected exactly one Player member, saw ${JSON.stringify(res.data?.worldMembers)}`,
  ).toBe(1);
  return players[0].userId;
}

/**
 * Ask the server for a sync plan as this session sees it, claiming to hold
 * `held`.
 *
 * This is the query the client's cache is driven by, so it is also the place
 * a permission change becomes visible: an item the caller may no longer see
 * is answered in `evict`, byte-identically to one that was deleted.
 */
async function syncPlan(
  page: Page,
  worldId: string,
  held: { id: string; fingerprint: string }[] = [],
): Promise<
  GqlResult<{
    worldSyncPlan: {
      canonicalVersion: number;
      evict: string[];
      fetch: { id: string }[];
    };
  }>
> {
  return graphql(
    page,
    `
      query ($worldId: UUID!, $held: [HeldItemInput!]!) {
        worldSyncPlan(worldId: $worldId, held: $held) {
          canonicalVersion
          evict
          fetch {
            id
          }
        }
      }
    `,
    { worldId, held },
  );
}

/** Count canvas-asset requests for one asset until `stop()`. */
function countAssetRequests(
  page: Page,
  assetId: string,
): { stop: () => number } {
  let requests = 0;
  const onResponse = (response: Response) => {
    if (response.url().includes(`/api/canvas-assets/${assetId}`)) {
      requests += 1;
    }
  };
  page.on("response", onResponse);
  return {
    stop: () => {
      page.off("response", onResponse);
      return requests;
    },
  };
}

/**
 * Open the world for play and wait until the cache has finished one sync.
 *
 * Returns the summary, so a caller can assert on what the sync actually did
 * rather than hoping it ran at all.
 */
async function openWorldAndSync(
  page: Page,
  worldId: string,
  sync: ReturnType<typeof watchCacheSync>,
): Promise<SyncSummary> {
  const before = sync.count();
  await page.goto(`/world/${worldId}/play`);
  await waitForEngineReady(page);
  return sync.next(before);
}

test.describe("Client world cache — losing access (US2, T042-T044)", () => {
  // Two browser contexts, an engine start in each, a real upload and two full
  // world opens. All of it is setup for a handful of assertions.
  test.setTimeout(300_000);

  /**
   * The whole of T042's setup: a player caches a world, then loses their
   * membership, then opens it again.
   *
   * Every step asserts itself. In particular it proves the player *could*
   * read the asset and *did* end up holding it on disk before anything is
   * revoked — without that, "the bytes are gone" afterwards would be
   * indistinguishable from "the bytes were never there", which is the exact
   * false green this feature keeps producing.
   */
  async function cacheAWorldThenRevokeMembership(
    browser: Browser,
    gm: Page,
  ): Promise<{
    player: Page;
    worldId: string;
    assetId: string;
    fingerprint: string;
    sync: ReturnType<typeof watchCacheSync>;
  }> {
    const worldId = await registerAndCreateWorld(
      gm,
      `E2E Revoke ${uniqueSuffix()}`,
      "e2erevokegm",
    );

    // scenes.hidden defaults to true — without this the player is a member of
    // a world with nothing in it they may see, and every later assertion
    // would pass vacuously.
    const scenes = await sceneIds(gm, worldId);
    expect(scenes.length, "a new world should have a scene").toBeGreaterThan(0);
    await setSceneHidden(gm, scenes[0], false);
    const assetId = await createCanvasAsset(gm, worldId, scenes[0], 12345);

    const player = await inviteAndJoinAsPlayer(
      browser,
      gm,
      worldId,
      "e2erevokepl",
    );
    const sync = watchCacheSync(player);

    // Precondition, asserted: while a member, the player really can read the
    // asset's bytes — and this is the fingerprint they will be stored under.
    const fingerprint = await assetFingerprint(player, assetId);

    const first = await openWorldAndSync(player, worldId, sync);
    expect(
      first.status,
      `the player's first sync should succeed: ${JSON.stringify(first)}`,
    ).toBe("synced");
    expect(
      first.fetch,
      "a cold open should be told to fetch the world's asset",
    ).toBeGreaterThan(0);

    // Prefetch is deliberately not awaited by the sync, so poll for the blob
    // rather than assuming it is already written.
    await expect
      .poll(() => holdsFingerprint(player, worldId, fingerprint), {
        timeout: 60_000,
        message: "the player should end up holding the asset locally",
      })
      .toBe(true);
    const heldBefore = await worldBlobs(player, worldId);
    console.log(
      `[cache-perms] player holds ${heldBefore.length} blob(s) for ${worldId} before revocation`,
    );

    // --- revoke ---
    const targetUser = await playerUserId(gm, worldId);
    const removal = await graphql<GqlResult<{ removeMember: boolean }>>(
      gm,
      `
        mutation ($worldId: ID!, $userId: ID!) {
          removeMember(worldId: $worldId, userId: $userId)
        }
      `,
      { worldId, userId: targetUser },
    );
    expect(
      removal.errors,
      `removeMember failed: ${JSON.stringify(removal.errors)}`,
    ).toBe(undefined);
    expect(removal.data?.removeMember, "removeMember should report true").toBe(
      true,
    );

    // --- the next open ---
    const before = sync.count();
    await player.goto(`/world/${worldId}/play`);
    // No waitForEngineReady: a denied open is not required to reach a
    // running canvas, and insisting on one would make the test assert the
    // failure mode's cosmetics instead of its substance.
    await sync.next(before);

    return { player, worldId, assetId, fingerprint, sync };
  }

  test("a revoked member is denied on their next open (T042, SC-004)", async ({
    browser,
    page,
  }) => {
    const { player, worldId, assetId, sync } =
      await cacheAWorldThenRevokeMembership(browser, page);

    // FR-014: authorization is re-established from the database on every
    // open. The server must refuse this session outright, and its refusal
    // must not distinguish "you may not see this" from "no such world".
    const plan = await syncPlan(player, worldId);
    expect(
      plan.errors?.[0]?.message,
      `worldSyncPlan should be refused: ${JSON.stringify(plan)}`,
    ).toContain("not a member of this world");
    expect(plan.data?.worldSyncPlan ?? null).toBeNull();

    // And the origin no longer serves them the bytes.
    expect(
      await assetStatus(player, assetId),
      "a revoked member must not be able to fetch the asset from the server",
    ).not.toBe(200);

    // Reported, deliberately not asserted: `public/sw.js` keeps a cache-first
    // copy of every `/api/canvas-assets/*` response, per browser profile,
    // and clears it only on logout. A revoked member's browser therefore
    // still holds plaintext bytes for this asset, and the engine's own
    // fetches go through that worker. That is a second local store, outside
    // the one spec 028 governs and outside the files this test may touch, so
    // failing here would only make the suite red about something these tests
    // cannot fix. It is measured so it cannot be forgotten.
    const swBytes = await serviceWorkerHoldsAsset(player, assetId);
    console.log(
      `[cache-perms] service-worker cache still holds ${swBytes}B for the ` +
        `revoked asset (see public/sw.js — cleared only on logout)`,
    );

    sync.stop();
    await player.context().close();
  });

  /**
   * The other half of SC-004, and the half that does not hold yet.
   *
   * This was marked `test.fail()` when written, because nothing implemented
   * FR-015 for *whole-world* revocation: eviction was driven entirely by the
   * `evict` list of a successful plan, and a revoked member's plan request is
   * refused outright, so `run_sync` took its transport-error branch,
   * republished what it already held, and returned `degraded` — leaving the
   * blobs on disk and readable under that user's still-live session key.
   *
   * T045a added the missing path and this now passes. The marker is gone.
   *
   * The distinction that makes that path safe, and the thing to preserve if
   * this test ever needs changing: an *authorization refusal* discards, a
   * *transient failure* does not. The client keys off the server's
   * `extensions.code == "FORBIDDEN"`, not a message substring and not "any
   * error" — because discarding a user's cache every time their connection
   * blips would cost more than the bug this fixes.
   */
  test("a revoked member's cached copy is discarded, not merely unusable (T042, SC-004)", async ({
    browser,
    page,
  }) => {
    const { player, worldId, sync } = await cacheAWorldThenRevokeMembership(
      browser,
      page,
    );

    await expect
      .poll(() => worldBlobs(player, worldId).then((b) => b.length), {
        timeout: 30_000,
        message:
          "a revoked member's locally-held blobs for the world must be discarded",
      })
      .toBe(0);

    sync.stop();
    await player.context().close();
  });

  test("hiding one scene evicts only its asset; the rest still loads from cache (T043, US2 scenario 2)", async ({
    browser,
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Partial ${uniqueSuffix()}`,
      "e2epartialgm",
    );

    // Two scenes, both player-visible to start with. `scenes.hidden` is the
    // per-object visibility axis that actually applies here: ADR-050's
    // permission ladder covers actors/items/abilities/lore, not scenes or
    // canvas assets, so asset visibility rides on world membership plus this
    // GM-only flag (see authorized_current in world_sync_plan.rs).
    const existing = await sceneIds(page, worldId);
    expect(existing.length, "a new world should have a scene").toBeGreaterThan(
      0,
    );
    const keptScene = existing[0];
    const hiddenScene = await createScene(page, worldId, "To be hidden");
    await setSceneHidden(page, keptScene, false);
    await setSceneHidden(page, hiddenScene, false);

    // Different noise seeds, so the two assets are genuinely different bytes
    // and land under different filenames.
    const keptAsset = await createCanvasAsset(page, worldId, keptScene, 999);
    const doomedAsset = await createCanvasAsset(
      page,
      worldId,
      hiddenScene,
      4242,
    );

    const player = await inviteAndJoinAsPlayer(
      browser,
      page,
      worldId,
      "e2epartialpl",
    );
    const sync = watchCacheSync(player);

    // Both readable while both scenes are visible — asserted, and the
    // fingerprints are the on-disk filenames to look for.
    const keptFingerprint = await assetFingerprint(player, keptAsset);
    const doomedFingerprint = await assetFingerprint(player, doomedAsset);
    expect(
      keptFingerprint,
      "the two assets must differ, or this test cannot tell them apart",
    ).not.toBe(doomedFingerprint);

    const first = await openWorldAndSync(player, worldId, sync);
    expect(first.status, JSON.stringify(first)).toBe("synced");
    expect(
      first.fetch,
      "a cold open should be told to fetch both assets",
    ).toBeGreaterThanOrEqual(2);

    await expect
      .poll(
        async () =>
          (await holdsFingerprint(player, worldId, keptFingerprint)) &&
          (await holdsFingerprint(player, worldId, doomedFingerprint)),
        {
          timeout: 60_000,
          message: "the player should end up holding both assets locally",
        },
      )
      .toBe(true);

    // --- partial revocation: one scene becomes GM-only ---
    await setSceneHidden(page, hiddenScene, true);

    // The server's answer, first. Claiming to hold both, the player is now
    // told to evict exactly the one they may no longer see — and told
    // nothing at all about the one they may, which is what "already current"
    // looks like on this wire.
    //
    // Note what is *not* asserted here: that `GET /api/canvas-assets/<id>`
    // starts refusing. That route authorizes on world membership only (see
    // canvas_assets_serve.rs), and the player is still a member, so a URL
    // they already know keeps answering. `scenes.hidden` is enforced by the
    // queries that hand out ids — including this plan — not by the byte
    // route. Asserting otherwise would be asserting a property the system
    // does not have.
    const plan = await syncPlan(player, worldId, [
      { id: `asset:${keptAsset}`, fingerprint: keptFingerprint },
      { id: `asset:${doomedAsset}`, fingerprint: doomedFingerprint },
    ]);
    expect(
      plan.errors,
      `worldSyncPlan failed: ${JSON.stringify(plan.errors)}`,
    ).toBe(undefined);
    expect(
      plan.data?.worldSyncPlan.evict,
      "the hidden scene's asset must be evicted",
    ).toContain(`asset:${doomedAsset}`);
    expect(
      plan.data?.worldSyncPlan.evict,
      "the visible scene's asset must not be evicted",
    ).not.toContain(`asset:${keptAsset}`);
    expect(
      (plan.data?.worldSyncPlan.fetch ?? []).map((f) => f.id),
      "nothing the player may not see may appear in fetch",
    ).not.toContain(`asset:${doomedAsset}`);

    // --- reopen ---
    const requestsForKept = countAssetRequests(player, keptAsset);
    const before = sync.count();
    await player.goto(`/world/${worldId}/play`);
    await waitForEngineReady(player);
    const second = await sync.next(before);
    console.log(
      `[cache-perms] partial-revocation sync ${JSON.stringify(second)}`,
    );

    expect(second.status, JSON.stringify(second)).toBe("synced");
    expect(
      second.held,
      "the reopen should present the two blobs the player already holds",
    ).toBeGreaterThanOrEqual(2);
    expect(
      second.evicted,
      "the now-hidden scene's asset should be evicted",
    ).toBeGreaterThanOrEqual(1);
    expect(
      second.blobsRemoved,
      "eviction should actually delete bytes, not just index rows",
    ).toBeGreaterThanOrEqual(1);

    // FR-015 for the forbidden half, FR-017 for the permitted half: the
    // asset the player may no longer see is gone from disk, and the one they
    // may still see is still there.
    await expect
      .poll(() => holdsFingerprint(player, worldId, doomedFingerprint), {
        timeout: 60_000,
        message: "the hidden scene's asset must be discarded locally",
      })
      .toBe(false);
    expect(
      await holdsFingerprint(player, worldId, keptFingerprint),
      "the still-permitted asset must survive the eviction",
    ).toBe(true);

    // ...and it was served from that local copy rather than re-downloaded.
    const refetched = requestsForKept.stop();
    console.log(
      `[cache-perms] requests for the kept asset on reopen: ${refetched}`,
    );
    expect(
      refetched,
      "the still-permitted asset should load from cache, not the network",
    ).toBe(0);

    sync.stop();
    await player.context().close();
  });

  test("after sign-out the stored bytes are inert, checked in OPFS before any cleanup (T044, SC-004a)", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Signout ${uniqueSuffix()}`,
      "e2esignout",
    );
    const scenes = await sceneIds(page, worldId);
    expect(scenes.length, "a new world should have a scene").toBeGreaterThan(0);
    const assetId = await createCanvasAsset(page, worldId, scenes[0], 7777);

    const sync = watchCacheSync(page);
    const fingerprint = await assetFingerprint(page, assetId);
    const summary = await openWorldAndSync(page, worldId, sync);
    expect(summary.status, JSON.stringify(summary)).toBe("synced");

    await expect
      .poll(() => holdsFingerprint(page, worldId, fingerprint), {
        timeout: 60_000,
        message: "there must be a stored blob before sign-out to reason about",
      })
      .toBe(true);

    // Even before sign-out the file is not the asset: it does not hash to the
    // fingerprint it is named for, and carries none of the image's framing.
    // (Nonce + GCM tag is 28 bytes of overhead over the plaintext.)
    const sealedBefore = await probeBlobs(page, worldId);
    expect(sealedBefore.length).toBeGreaterThan(0);
    for (const blob of sealedBefore) {
      expect(
        blob.sha256,
        `${blob.path} must not be the plaintext it is named for`,
      ).not.toBe(blob.claimedFingerprint);
      expect(blob.looksLikeImage, `${blob.path} must not be a bare image`).toBe(
        false,
      );
    }

    const keyStateBefore = await sessionKeyState(page);

    // --- sign out, through the app's own control ---
    //
    // Through the UI, not by posting to the logout endpoint: anything the
    // app wires into sign-out (clearing the service worker's asset cache
    // already lives there) runs on this path and would be skipped by a bare
    // API call, which would quietly make this test weaker than it looks.
    await page.goto("/worlds");
    await page.getByRole("button", { name: "Menu" }).click();
    await page.getByRole("menuitem", { name: "Sign out" }).click();
    await page.waitForURL(/\/login$/, { timeout: 15_000 });
    // Asserted, not assumed: the session really is gone. Asked of the origin
    // (the `?probe=` URL bypasses the asset service worker), because the
    // whole test is meaningless if the sign-out silently failed.
    expect(
      await assetStatus(page, assetId),
      "a signed-out session must not be served world content",
    ).not.toBe(200);

    // --- immediately, before any lazy reclamation could have run ---
    const surviving = await probeBlobs(page, worldId);
    console.log(
      `[cache-perms] ${surviving.length} blob(s) survive sign-out for ${worldId}`,
    );

    // The claim is *not* "the files are gone" — FR-016b explicitly allows
    // reclamation to be lazy, and relying on deletion for confidentiality is
    // the mistake this requirement exists to prevent. The claim is that
    // whatever survives is unreadable: sealed AES-GCM, opening for nobody who
    // does not hold the key, and the key itself is non-extractable so it can
    // never be carried off with the bytes.
    for (const blob of surviving) {
      expect(
        blob.sha256,
        `${blob.path} must not have become plaintext`,
      ).not.toBe(blob.claimedFingerprint);
      expect(blob.looksLikeImage, `${blob.path} must not be a bare image`).toBe(
        false,
      );
      expect(
        blob.decryptsWithAForeignKey,
        `${blob.path} must not open under a key that is not the session key`,
      ).toBe(false);
    }

    // FR-016a: the key is discarded on sign-out, which is *why* the bytes
    // above are inert rather than merely awaiting deletion. Both readings are
    // printed, because "keys=0" only means something next to a "keys=1" taken
    // before — on its own it could equally be a probe that found no database
    // and made an empty one.
    const keyStateAfter = await sessionKeyState(page);
    console.log(
      `[cache-perms] session key before sign-out: ${keyStateBefore}; ` +
        `after: ${keyStateAfter}`,
    );
    expect(
      keyStateBefore,
      "the cache must actually have held a session key before sign-out",
    ).toContain("keys=1");
    expect(
      keyStateAfter,
      "sign-out must discard the session key (FR-016a)",
    ).toContain("keys=0");

    sync.stop();
  });
});

interface BlobProbe {
  path: string;
  size: number;
  /** SHA-256 of the bytes actually on disk. */
  sha256: string;
  /** The fingerprint the filename claims, i.e. the plaintext's hash. */
  claimedFingerprint: string;
  /** PNG/WebP/JPEG magic at offset 0. */
  looksLikeImage: boolean;
  /** Whether AES-GCM opened it under a key that is not the session key. */
  decryptsWithAForeignKey: boolean;
}

/**
 * Read this world's stored blobs straight out of OPFS and try to make sense
 * of them without the session key.
 *
 * Everything here is done from outside the application's own read path: raw
 * file handles, raw bytes, WebCrypto called by the test. That is the point of
 * SC-004a — going through the app would only demonstrate that the app chooses
 * not to read the data.
 */
async function probeBlobs(page: Page, worldId: string): Promise<BlobProbe[]> {
  return page.evaluate(async (world) => {
    interface DirLike {
      kind: string;
      entries(): AsyncIterable<[string, DirLike]>;
      getFile(): Promise<File>;
    }
    const root = (await navigator.storage.getDirectory()) as unknown as DirLike;
    const out: BlobProbeShape[] = [];
    interface BlobProbeShape {
      path: string;
      size: number;
      sha256: string;
      claimedFingerprint: string;
      looksLikeImage: boolean;
      decryptsWithAForeignKey: boolean;
    }

    const hex = (buf: ArrayBuffer) =>
      Array.from(new Uint8Array(buf))
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("");

    // A key the page just made: stands in for "anyone who does not hold the
    // session key". AES-GCM authenticates, so a wrong key cannot decrypt —
    // it throws rather than producing garbage.
    const foreign = await crypto.subtle.generateKey(
      { name: "AES-GCM", length: 256 },
      false,
      ["encrypt", "decrypt"],
    );

    const walk = async (dir: DirLike, prefix: string): Promise<void> => {
      for await (const [name, handle] of dir.entries()) {
        const path = `${prefix}/${name}`;
        if (handle.kind === "directory") {
          await walk(handle, path);
          continue;
        }
        if (!path.includes(`/${world}/`) || !name.endsWith(".bin")) continue;
        const bytes = new Uint8Array(
          await (await handle.getFile()).arrayBuffer(),
        );
        const digest = await crypto.subtle.digest("SHA-256", bytes);

        // The framing is a 12-byte nonce then the AES-GCM output.
        let opened = false;
        try {
          await crypto.subtle.decrypt(
            { name: "AES-GCM", iv: bytes.slice(0, 12) },
            foreign,
            bytes.slice(12),
          );
          opened = true;
        } catch {
          opened = false;
        }

        out.push({
          path,
          size: bytes.length,
          sha256: hex(digest),
          claimedFingerprint: name.slice(0, -".bin".length),
          looksLikeImage:
            // PNG
            (bytes[0] === 0x89 && bytes[1] === 0x50) ||
            // RIFF (WebP)
            (bytes[0] === 0x52 && bytes[1] === 0x49 && bytes[2] === 0x46) ||
            // JPEG
            (bytes[0] === 0xff && bytes[1] === 0xd8),
          decryptsWithAForeignKey: opened,
        });
      }
    };

    await walk(root, "");
    return out;
  }, worldId);
}

/**
 * What the cache's IndexedDB says about session keys: whether the database
 * exists at all, which stores it has, and how many key records are in
 * `keys`.
 *
 * Reported, not asserted — and reported in this much detail on purpose. A
 * bare count is ambiguous: opening a database that no longer exists *creates*
 * an empty one, which then honestly reports zero keys and would read as
 * "the key was discarded" when the truth might be "the probe made a new
 * database". Naming the pre-existing databases first removes that ambiguity.
 */
async function sessionKeyState(page: Page): Promise<string> {
  return page.evaluate(async () => {
    const existing = (await indexedDB.databases())
      .map((d) => d.name)
      .filter((n): n is string => Boolean(n));
    if (!existing.includes("thunderforge-cache")) {
      return "database absent";
    }
    return new Promise<string>((resolve) => {
      const open = indexedDB.open("thunderforge-cache", 1);
      open.onerror = () => resolve("open failed");
      open.onsuccess = () => {
        const db = open.result;
        const stores = Array.from(db.objectStoreNames);
        if (!stores.includes("keys")) {
          db.close();
          resolve(`stores=[${stores.join(",")}] (no keys store)`);
          return;
        }
        const req = db
          .transaction("keys", "readonly")
          .objectStore("keys")
          .count();
        req.onsuccess = () => {
          const n = req.result;
          db.close();
          resolve(`stores=[${stores.join(",")}] keys=${n}`);
        };
        req.onerror = () => {
          db.close();
          resolve("count failed");
        };
      };
    });
  });
}
