import { expect, type ConsoleMessage, type Page, type Response } from "@playwright/test";
import { graphql, waitForEngineReady } from "./helpers";

/**
 * Shared instrumentation for spec 028's client-world-cache e2e suites.
 *
 * These started life inside `world-cache-permissions.spec.ts`, which is why
 * several of them read as arguments rather than as utilities — the reasoning
 * about *why* a probe is shaped the way it is belongs next to the probe, not
 * in the test that happens to call it. They moved here when the multi-tab
 * suite needed the same instruments, and copying them would have meant two
 * versions of "what does the cache actually hold" free to drift apart while
 * both stayed green.
 *
 * Two properties every helper here keeps:
 *
 * - **A fixture asserts its own success.** This feature's repeated failure
 *   mode has been a helper that swallowed its error and produced a confident,
 *   wrong green — a storm with no events, a cold open reporting zero requests
 *   because nothing had ever uploaded. A setup step that can fail says so.
 * - **A probe reads the store from outside the application.** Asking the app
 *   what it holds only ever demonstrates what the app chooses to read.
 */

/** Where the engine fetches canvas assets (`ASSET_URL_PREFIX` in cached_assets.rs). */
export function assetUrl(assetId: string): string {
  return `/api/canvas-assets/${assetId}.webp`;
}

/** The summary `sync_world_cache` returns, as WorldPage logs it. */
export interface SyncSummary {
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
export function watchCacheSync(page: Page) {
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

export interface OpfsFile {
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
export async function opfsFiles(page: Page): Promise<OpfsFile[]> {
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
export async function worldBlobs(page: Page, worldId: string): Promise<OpfsFile[]> {
  const files = await opfsFiles(page);
  return files.filter(
    (f) => f.path.includes(`/${worldId}/`) && f.path.endsWith(".bin"),
  );
}

/** True when a blob named for `fingerprint` is on disk for this world. */
export async function holdsFingerprint(
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
export async function assetFingerprint(page: Page, assetId: string): Promise<string> {
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
 * The unique query string is belt-and-braces rather than load-bearing now.
 * `public/sw.js` used to serve `/api/canvas-assets/*` cache-first, and a
 * service worker sits in front of `cache: "no-store"` as well, so a plain
 * fetch of a previously-loaded asset answered 200 no matter what the server
 * thought; a fresh URL missed that cache and asked the origin. T045b removed
 * that fetch handler, but the nonce stays: it keeps this helper asking the
 * origin even from a browser still running a stale worker, which is exactly
 * the population this test is about. What Cache Storage still holds is a
 * separate question, measured by `serviceWorkerHoldsAsset` below.
 */
export async function assetStatus(page: Page, assetId: string): Promise<number> {
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
 * Whether any Cache Storage entry still holds readable bytes for this asset.
 *
 * This asks about a different store from the one spec 028 governs: the
 * service worker's, which held plaintext keyed by URL and outlived
 * revocation. T045b stopped it caching and made it purge on activate, so
 * this is now asserted to be zero rather than merely reported.
 */
export async function serviceWorkerHoldsAsset(
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

export interface GqlResult<T> {
  data?: T;
  errors?: { message: string; extensions?: Record<string, unknown> }[];
}

/** Scenes as the caller may see them. A GM sees hidden ones too. */
export async function sceneIds(page: Page, worldId: string): Promise<string[]> {
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
export async function setSceneHidden(
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
export async function createScene(
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
export async function createCanvasAsset(
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

/**
 * Ask the server for a sync plan as this session sees it, claiming to hold
 * `held`.
 *
 * This is the query the client's cache is driven by, so it is also the place
 * a permission change becomes visible: an item the caller may no longer see
 * is answered in `evict`, byte-identically to one that was deleted.
 */
export async function syncPlan(
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
export function countAssetRequests(
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
export async function openWorldAndSync(
  page: Page,
  worldId: string,
  sync: ReturnType<typeof watchCacheSync>,
): Promise<SyncSummary> {
  const before = sync.count();
  await page.goto(`/world/${worldId}/play`);
  await waitForEngineReady(page);
  return sync.next(before);
}

export interface BlobProbe {
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
export async function probeBlobs(page: Page, worldId: string): Promise<BlobProbe[]> {
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
export async function sessionKeyState(page: Page): Promise<string> {
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

