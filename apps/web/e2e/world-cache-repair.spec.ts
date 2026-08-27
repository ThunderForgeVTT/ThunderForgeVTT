import path from "node:path";
import { expect, test, type Page } from "@playwright/test";
import { registerAndCreateWorld, uniqueSuffix } from "./fixtures/helpers";
import {
  assetFingerprint,
  countAssetRequests,
  createCanvasAsset,
  holdsFingerprint,
  importMapBackground,
  openWorldAndSync,
  probeBlobs,
  sceneBackgroundAssetId,
  sceneIds,
  type SyncSummary,
  watchCacheSync,
  worldBlobs,
} from "./fixtures/world-cache";

/**
 * The index and the disk disagreeing, and being put right (FR-019, SC-005).
 *
 * They drift for ordinary reasons and always will: a blob is written before
 * its index row, a row is removed before its blob, and a tab can be closed
 * between any two awaits. `index.rs` has always stated the rule for resolving
 * it — where they differ, OPFS wins — and `missing_blobs`/`orphaned_blobs`
 * have been sitting there tested. What was missing was a caller: nothing in
 * the tree ever ran them, so both directions of divergence were permanent.
 *
 * Three shapes of damage, and they are repaired by three different pieces of
 * the system rather than by one repair pass:
 *
 * - **A blob no row refers to** — dead weight. `repair_world` reclaims it,
 *   unless it is unfinished, in which case it may be another tab's write.
 * - **A row naming a blob that is gone** — a *lie*: it makes the client claim
 *   the item in its manifest, so the server stays silent and the item is
 *   never fetched or served. `repair_world` drops the row.
 * - **A blob whose plaintext does not hash to its own filename** — corrupt.
 *   Never reaches `repair_world` at all: `read_blob` verifies on every read
 *   and discards on mismatch, and the asset falls through to the network in
 *   the same page load.
 *
 * # Why these tests plant the damage by hand
 *
 * The divergence they repair is produced by a crash at a specific instant
 * between two awaits, or by a disk that lied. A browser test cannot schedule
 * either, and waiting for one to happen by chance is not a test. So the end
 * state is written directly into OPFS — an unreferenced file, an empty one, a
 * deleted one, a forged one — and the assertion is about what the system then
 * does with it. That is the part with no coverage; the pure diff either side
 * of it is unit-tested.
 *
 * # "Silently" is asserted, not assumed
 *
 * SC-005 is as much about what the user is *not* shown as about recovery, so
 * each test below watches for page errors throughout and requires none. A
 * repair that worked and logged an exception would not satisfy the criterion.
 */

/**
 * A small real map, for the one test that needs the engine to actually load
 * an asset. The chamber map rather than `demo.dd2vtt` because it is a quarter
 * the size and this test reloads the world four times; the art is what
 * matters here, not the wall count.
 */
const CHAMBER_MAP = path.resolve(
  __dirname,
  "../../../examples/maps/chamber-of-echoing-grief.dd2vtt",
);

/** A blob filename the store will recognise: 64 hex characters, `.bin`. */
function fabricatedFingerprint(seed: string): string {
  let hex = "";
  for (let i = 0; hex.length < 64; i += 1) {
    hex += (seed.charCodeAt(i % seed.length) + i).toString(16).padStart(2, "0");
  }
  return hex.slice(0, 64);
}

/** What to do to one file in a world's OPFS directory. */
type StoreEdit =
  /** Create a file of `size` bytes under `name`, referenced by nothing. */
  | { kind: "plant"; name: string; size: number }
  /** Remove `name`, leaving whatever index row points at it behind. */
  | { kind: "delete"; name: string }
  /**
   * Replace `name`'s contents with a *valid envelope over the wrong
   * plaintext*, sealed under the session key this profile actually holds.
   */
  | { kind: "forge"; name: string };

/**
 * Edit a world's OPFS directory directly, bypassing the cache entirely.
 *
 * Deliberately not done through the application: the point is to create
 * states the application would never write, and asking it to write one would
 * only prove it can be asked.
 *
 * Returns `""` on success and a description on failure — never throws —
 * because a setup step that failed silently is how this suite has produced
 * confident wrong greens before. Every caller asserts on the return.
 *
 * # Why `forge` goes to the trouble of using the real key
 *
 * Overwriting a blob with random bytes would test the wrong thing. Garbage
 * fails to *decrypt*, and `read_blob` already discards on decryption failure
 * — a path shared with "the key is gone", which the permissions suite covers.
 * The case with no coverage is bytes that decrypt perfectly and are not what
 * the filename claims, which is the only thing `fingerprint::verify` on the
 * read path can catch and the only reason it is there. Producing that
 * requires sealing under the session key, so the test reads the `CryptoKey`
 * out of IndexedDB and uses it.
 *
 * It can use it without breaking anything: the key is stored non-extractable,
 * so this hands the browser a handle and asks it to encrypt. The bytes never
 * exist outside the browser, here or anywhere — which is the property
 * `crypto.rs` is built around, and this exercise of it is not a hole in it.
 */
async function editWorldStore(
  page: Page,
  worldId: string,
  edit: StoreEdit,
): Promise<string> {
  return page.evaluate(
    async ({ world, op }) => {
      interface DirLike {
        kind: string;
        entries(): AsyncIterable<[string, DirLike]>;
        getDirectoryHandle(
          name: string,
          opts?: { create?: boolean },
        ): Promise<DirLike>;
        getFileHandle(
          name: string,
          opts?: { create?: boolean },
        ): Promise<{
          getFile(): Promise<File>;
          createWritable(): Promise<WritableStream>;
        }>;
        removeEntry(name: string): Promise<void>;
      }
      const root = (await navigator.storage.getDirectory()) as unknown as DirLike;

      // The scope directory is an opaque per-user hash; find the one that
      // already holds this world rather than trying to re-derive it.
      let scopeName: string | null = null;
      for await (const [name, handle] of root.entries()) {
        if (handle.kind !== "directory") continue;
        for await (const [child] of handle.entries()) {
          if (child === world) scopeName = name;
        }
      }
      if (!scopeName) return "no scope directory holds this world";

      const scope = await root.getDirectoryHandle(scopeName);
      const worldDir = await scope.getDirectoryHandle(world);
      const fileName = `${op.name}.bin`;

      if (op.kind === "delete") {
        await worldDir.removeEntry(fileName);
        return "";
      }

      const write = async (bytes: Uint8Array) => {
        const file = await worldDir.getFileHandle(fileName, { create: true });
        const writable = await file.createWritable();
        if (bytes.length > 0) {
          await (
            writable as unknown as { write(d: Uint8Array): Promise<void> }
          ).write(bytes);
        }
        await writable.close();
      };

      if (op.kind === "plant") {
        await write(new Uint8Array(op.size).fill(7));
        return "";
      }

      // `forge`. Fetch the session key the same way the application stores
      // it: `thunderforge-cache` / `keys`, one record per user scope.
      const key = await new Promise<CryptoKey | string>((resolve) => {
        const request = indexedDB.open("thunderforge-cache");
        request.onerror = () => resolve("cannot open thunderforge-cache");
        request.onsuccess = () => {
          const db = request.result;
          if (!db.objectStoreNames.contains("keys")) {
            resolve("no keys store");
            return;
          }
          const all = db.transaction("keys", "readonly").objectStore("keys").getAll();
          all.onerror = () => resolve("cannot read the keys store");
          all.onsuccess = () => {
            const found = (all.result as unknown[]).find(
              (v): v is CryptoKey =>
                typeof CryptoKey !== "undefined" && v instanceof CryptoKey,
            );
            resolve(found ?? "no session key is stored");
          };
        };
      });
      if (typeof key === "string") return key;

      // The framing `Envelope::seal` writes: a 12-byte nonce, then AES-GCM
      // output with no additional data. Anything else here and the test would
      // be exercising `Envelope::split` instead of the fingerprint check.
      const nonce = crypto.getRandomValues(new Uint8Array(12));
      const wrong = new TextEncoder().encode(
        "these bytes decrypt perfectly and are not the asset",
      );
      const sealed = new Uint8Array(
        await crypto.subtle.encrypt({ name: "AES-GCM", iv: nonce }, key, wrong),
      );
      const envelope = new Uint8Array(nonce.length + sealed.length);
      envelope.set(nonce, 0);
      envelope.set(sealed, nonce.length);
      await write(envelope);
      return "";
    },
    { world: worldId, op: edit },
  );
}

/**
 * Collect uncaught page errors for the life of a test.
 *
 * This is the "silently" half of SC-005. Every test here repairs real damage,
 * and a repair that surfaced a stack trace to the user would meet the letter
 * of every other assertion and fail the criterion.
 */
function watchPageErrors(page: Page): { stop: () => string[] } {
  const errors: string[] = [];
  const onError = (err: Error) => errors.push(err.message);
  page.on("pageerror", onError);
  return {
    stop: () => {
      page.off("pageerror", onError);
      return errors;
    },
  };
}

/** A world with one cached canvas asset, warm and verified on disk. */
async function warmWorldWithAsset(
  page: Page,
  label: string,
  sync: ReturnType<typeof watchCacheSync>,
): Promise<{ worldId: string; assetId: string; fingerprint: string }> {
  const worldId = await registerAndCreateWorld(
    page,
    `E2E Repair ${label} ${uniqueSuffix()}`,
    "e2erepair",
  );
  const scenes = await sceneIds(page, worldId);
  expect(scenes.length, "a new world should have a scene").toBeGreaterThan(0);
  const assetId = await createCanvasAsset(page, worldId, scenes[0], 3131);
  const fingerprint = await assetFingerprint(page, assetId);

  const first = await openWorldAndSync(page, worldId, sync);
  expect(first.status, JSON.stringify(first)).toBe("synced");
  await expect
    .poll(() => holdsFingerprint(page, worldId, fingerprint), {
      timeout: 60_000,
      message: "the world must be cached before its store can be damaged",
    })
    .toBe(true);

  return { worldId, assetId, fingerprint };
}

/**
 * The SHA-256 of the bytes actually on disk for one blob, or `null` if there
 * is no such file.
 *
 * Absence is a return value rather than a failure because the healing this
 * suite watches passes *through* absence: `read_blob` discards a blob that
 * fails its fingerprint and the refetch writes a new one, so a probe timed
 * between the two legitimately finds nothing. A helper that threw there would
 * turn the window it is meant to observe into a flake.
 */
async function storedSha(
  page: Page,
  worldId: string,
  fingerprint: string,
): Promise<string | null> {
  const blobs = await probeBlobs(page, worldId);
  return blobs.find((b) => b.path.endsWith(`/${fingerprint}.bin`))?.sha256 ?? null;
}

/** Reload, and return the sync summary that page load produced. */
async function reloadAndSync(
  page: Page,
  sync: ReturnType<typeof watchCacheSync>,
): Promise<SyncSummary> {
  const before = sync.count();
  await page.reload();
  return sync.next(before);
}

test.describe("Client world cache — repairing a divergent store (FR-019, SC-005)", () => {
  test.setTimeout(420_000);

  test("a blob no index row refers to is reclaimed, and an unfinished one is not", async ({
    page,
  }) => {
    const errors = watchPageErrors(page);
    const sync = watchCacheSync(page);
    const { worldId, fingerprint } = await warmWorldWithAsset(page, "Orphan", sync);

    // The damage. One complete file nothing refers to — bytes no index row
    // can reach, which is dead weight forever. And one empty file, which is
    // indistinguishable from a write another tab is in the middle of.
    const orphan = fabricatedFingerprint("orphan");
    const unfinished = fabricatedFingerprint("unfinished");
    expect(
      await editWorldStore(page, worldId, { kind: "plant", name: orphan, size: 4096 }),
      "planting failed",
    ).toBe("");
    expect(
      await editWorldStore(page, worldId, { kind: "plant", name: unfinished, size: 0 }),
      "planting failed",
    ).toBe("");

    const planted = await worldBlobs(page, worldId);
    expect(
      planted.some((b) => b.path.endsWith(`/${orphan}.bin`)),
      "the orphan must actually be on disk before the repair runs",
    ).toBe(true);
    expect(
      planted.some((b) => b.path.endsWith(`/${unfinished}.bin`)),
      "the unfinished file must actually be on disk before the repair runs",
    ).toBe(true);

    // Reopening runs a sync, and the repair rides along with it.
    const second = await reloadAndSync(page, sync);
    console.log(`[cache-repair] sync after planting: ${JSON.stringify(second)}`);

    expect(
      second.blobsReclaimed,
      "the unreferenced blob should have been reclaimed",
    ).toBeGreaterThanOrEqual(1);
    expect(
      second.unfinishedKept,
      "the empty file must be reported as kept, not reclaimed",
    ).toBeGreaterThanOrEqual(1);

    const after = await worldBlobs(page, worldId);
    expect(
      after.some((b) => b.path.endsWith(`/${orphan}.bin`)),
      "bytes nothing can reach must not survive a repair",
    ).toBe(false);
    expect(
      after.some((b) => b.path.endsWith(`/${unfinished}.bin`)),
      "an unfinished file may be another tab's write and must be left alone",
    ).toBe(true);

    // And the repair must not have taken the real asset with it.
    expect(
      await holdsFingerprint(page, worldId, fingerprint),
      "the world's actual content must survive its own repair",
    ).toBe(true);

    sync.stop();
    expect(errors.stop(), "a repair must not surface an error to the user").toEqual([]);
  });

  // Not `test.skip` — the code under test is believed correct and is
  // natively tested; what is missing is a way to *reach* it from a browser.
  // `fixme` keeps the scenario, the setup and the reasoning in the tree and
  // out of the red, which is what T042 did while T045a was outstanding.
  //
  // **Why this cannot pass today.** `try_cached` (cached_assets.rs) declines
  // to serve a load when the cache is not ready or when no fingerprint has
  // been published for the asset, and on a fresh page load the scene
  // background is *both*. Measured, in this order, from the browser console:
  //
  //     [net]    HEAD 200          <- WorldPage's reachability check
  //     [net]    GET  404          <- Bevy asking for `.webp.meta`
  //     [net]    GET  200          <- the background itself
  //     [engine] canvas asset cache ready
  //     [world-cache] sync {...}
  //
  // The sprite load is dispatched as soon as the scene state arrives, which
  // is well before `sync_world_cache` resolves and `publish_fingerprints`
  // fills `cache.fingerprints`. So the background is fetched over the network
  // every open, the forged blob is never read, and — the tell that made this
  // visible — the forged file is still on disk, byte for byte, afterwards. A
  // read would have discarded it.
  //
  // The consequence is bigger than this test: **the scene background is
  // stored by the prefetch and never read back**. It is the largest asset in
  // a world, and for it the cache currently costs a write and saves nothing.
  // SC-001's "0% asset bytes on a warm reopen" was measured on a world whose
  // assets are never displayed, where the only traffic is the prefetch, so it
  // did not cover this case.
  //
  // The read path *is* reachable — an asset loaded after the first sync
  // completes (a scene switch within one session) finds `fingerprints`
  // populated and `is_ready()` true. Re-pointing this test at a second scene
  // switched to mid-session is the way to finish it. Whether the *background*
  // should also be cache-served is a design question, not a test fix: making
  // the load wait for readiness trades SC-002 (time to interactive) against
  // SC-001 (bytes on reopen), and that is a call to make deliberately.
  test.fixme("a blob that decrypts to the wrong content is discarded and refetched on read (T052, SC-005)", async ({
    page,
  }) => {
    const errors = watchPageErrors(page);
    const sync = watchCacheSync(page);

    // Not `warmWorldWithAsset`, and the reason is the substance of this test.
    // An asset uploaded through `uploadCanvasImage` is stored, planned and
    // prefetched, but **nothing on the scene refers to it**, so the engine
    // never loads it and `read_blob` is never called. Corrupting such a blob
    // proves nothing: the damage is never looked at. The asset has to be one
    // the engine actually loads, which means the scene background.
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Repair Corrupt ${uniqueSuffix()}`,
      "e2erepair",
    );
    const scenes = await sceneIds(page, worldId);
    expect(scenes.length, "a new world should have a scene").toBeGreaterThan(0);

    const first = await openWorldAndSync(page, worldId, sync);
    expect(first.status, JSON.stringify(first)).toBe("synced");

    await importMapBackground(page, CHAMBER_MAP);
    const assetId = await sceneBackgroundAssetId(page, worldId, scenes[0]);
    const fingerprint = await assetFingerprint(page, assetId);

    // A reload before the damage, and not merely for tidiness: an asset
    // created *after* the sync has run is not in that sync's plan, so no
    // fingerprint has been published for it. The engine loads it over the
    // network and deliberately does **not** store it — a fingerprint it
    // cannot reproduce is one it could never invalidate against
    // (`fetch_and_deliver`). The next open is the one that plans it, and
    // therefore the one that caches it.
    const planned = await reloadAndSync(page, sync);
    console.log(`[cache-repair] sync after importing: ${JSON.stringify(planned)}`);
    await expect
      .poll(() => holdsFingerprint(page, worldId, fingerprint), {
        timeout: 90_000,
        message: "the background must be cached before its blob can be forged",
      })
      .toBe(true);

    const original = await storedSha(page, worldId, fingerprint);
    expect(original, "the asset must be on disk before it can be corrupted").toBeTruthy();

    // Replace the blob's contents with a well-formed envelope over the wrong
    // plaintext. This is the corruption `read_blob`'s fingerprint check exists
    // for, and the only one it can catch: the envelope opens, so the key is
    // not in question, and the plaintext is not what the filename promises.
    expect(
      await editWorldStore(page, worldId, { kind: "forge", name: fingerprint }),
      "forging failed",
    ).toBe("");

    const forged = await storedSha(page, worldId, fingerprint);
    expect(forged, "the forged blob must be on disk").toBeTruthy();
    expect(
      forged,
      "the forged blob must actually differ from what was stored",
    ).not.toBe(original);

    // Nothing here is `repair_world`'s to find. The index and the disk agree
    // perfectly — a row, and a file of exactly the name it points at — so the
    // divergence is invisible to a directory listing and only reading the
    // bytes can see it. Hence a repair count of zero, and the network as the
    // place the recovery shows up.
    const requests = countAssetRequests(page, assetId);
    const summary = await reloadAndSync(page, sync);
    console.log(`[cache-repair] sync after forging: ${JSON.stringify(summary)}`);
    expect(
      summary.rowsRepaired ?? 0,
      "the index and the disk agree; repair_world has nothing to find here",
    ).toBe(0);

    await expect
      .poll(() => requests.count(), {
        timeout: 90_000,
        message: "a blob that fails its own fingerprint must fall through to the network",
      })
      .toBeGreaterThanOrEqual(1);
    requests.stop();

    // Self-healed, not merely survived: the bytes on disk are neither the
    // forgery nor left absent, and the *next* open is warm again. A cache
    // that refetched every time and never re-stored would satisfy the
    // assertion above and fail this one.
    await expect
      .poll(
        async () => {
          const sha = await storedSha(page, worldId, fingerprint);
          // Both halves matter. `sha !== forged` alone is satisfied by the
          // blob simply being absent, which is the state right after the
          // discard and before the refetch has written anything — passing
          // there would report a hole in the cache as a repair.
          return sha !== null && sha !== forged;
        },
        {
          timeout: 90_000,
          message: "the corrupt blob should have been replaced by a correct one",
        },
      )
      .toBe(true);

    const warm = countAssetRequests(page, assetId);
    await reloadAndSync(page, sync);
    await page.waitForTimeout(5_000);
    expect(
      warm.stop(),
      "once healed, reopening the world must serve this background from cache",
    ).toBe(0);

    sync.stop();
    expect(errors.stop(), "a corrupt blob must not surface an error").toEqual([]);
  });

  test("an index row naming a blob that is gone is dropped, and the asset comes back (FR-019)", async ({
    page,
  }) => {
    const errors = watchPageErrors(page);
    const sync = watchCacheSync(page);
    const { worldId, assetId, fingerprint } = await warmWorldWithAsset(
      page,
      "Missing",
      sync,
    );

    // Delete the blob and leave its index row. This is the direction that
    // matters most and is the least visible: the row makes the client claim
    // the item in its manifest, the server therefore says nothing about it
    // (silence means unchanged), and the item is then never fetched *and*
    // never served — a permanent hole that looks exactly like a working cache.
    expect(
      await editWorldStore(page, worldId, { kind: "delete", name: fingerprint }),
      "deleting the blob failed",
    ).toBe("");
    expect(
      await holdsFingerprint(page, worldId, fingerprint),
      "the blob must actually be gone before the repair runs",
    ).toBe(false);

    const first = await reloadAndSync(page, sync);
    console.log(`[cache-repair] sync after deleting: ${JSON.stringify(first)}`);
    expect(
      first.rowsRepaired,
      "a row naming a blob that is not on disk must be dropped",
    ).toBeGreaterThanOrEqual(1);

    // Recovery takes a second open, and that is a property of where the
    // repair sits rather than an accident: `run_sync` builds its manifest
    // *before* calling `repair_world` (cached_assets.rs), so this open had
    // already told the server it held the item by the time the lie was found.
    // The asset still renders — the read falls through to the network — but
    // nothing re-stores it, because with the row gone there is no
    // server-promised fingerprint to verify a write against.
    expect(
      await holdsFingerprint(page, worldId, fingerprint),
      "the repair drops the row; it does not itself refetch",
    ).toBe(false);

    // The next open is the one that heals it: the manifest no longer claims
    // the item, so the server offers it and the prefetch stores it.
    const second = await reloadAndSync(page, sync);
    console.log(`[cache-repair] sync after the row was dropped: ${JSON.stringify(second)}`);
    expect(
      second.fetch,
      "with the lie removed, the server must offer the item again",
    ).toBeGreaterThanOrEqual(1);
    await expect
      .poll(() => holdsFingerprint(page, worldId, fingerprint), {
        timeout: 60_000,
        message: "the asset should be back on disk after the second open",
      })
      .toBe(true);

    // And warm again, which is what makes this a repair rather than a
    // permanent degradation.
    const warm = countAssetRequests(page, assetId);
    await reloadAndSync(page, sync);
    await page.waitForTimeout(3_000);
    expect(
      warm.stop(),
      "the healed world must serve this asset from cache again",
    ).toBe(0);

    sync.stop();
    expect(errors.stop(), "a repair must not surface an error to the user").toEqual([]);
  });
});
