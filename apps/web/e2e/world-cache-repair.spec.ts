import { expect, test } from "@playwright/test";
import {
  registerAndCreateWorld,
  uniqueSuffix,
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
 * The index and the disk disagreeing, and being put right (FR-019).
 *
 * They drift for ordinary reasons and always will: a blob is written before
 * its index row, a row is removed before its blob, and a tab can be closed
 * between any two awaits. `index.rs` has always stated the rule for resolving
 * it — where they differ, OPFS wins — and `missing_blobs`/`orphaned_blobs`
 * have been sitting there tested. What was missing was a caller: nothing in
 * the tree ever ran them, so both directions of divergence were permanent.
 *
 * # Why these tests plant the damage by hand
 *
 * The divergence they repair is produced by a crash at a specific instant
 * between two awaits. A browser test cannot schedule that, and waiting for it
 * to happen by chance is not a test. So the end state is written directly
 * into OPFS — an unreferenced file, and one that is empty — and the assertion
 * is about what the *repair* then does with it. That is the part with no
 * coverage; the pure diff either side of it is unit-tested.
 */

/** A blob filename the store will recognise: 64 hex characters, `.bin`. */
function fabricatedFingerprint(seed: string): string {
  let hex = "";
  for (let i = 0; hex.length < 64; i += 1) {
    hex += (seed.charCodeAt(i % seed.length) + i).toString(16).padStart(2, "0");
  }
  return hex.slice(0, 64);
}

/**
 * Write a file straight into a world's OPFS directory, bypassing the cache
 * entirely.
 *
 * Deliberately not through the application: the point is to create a state
 * the application would never write, and asking it to do so would only prove
 * it can be asked.
 */
async function plantBlob(
  page: import("@playwright/test").Page,
  worldId: string,
  fingerprint: string,
  bytes: number,
): Promise<string> {
  return page.evaluate(
    async ({ world, name, size }) => {
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
        ): Promise<{ createWritable(): Promise<WritableStream> }>;
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
      const file = await worldDir.getFileHandle(`${name}.bin`, {
        create: true,
      });
      const writable = await file.createWritable();
      if (size > 0) {
        await (writable as unknown as { write(d: Uint8Array): Promise<void> }).write(
          new Uint8Array(size).fill(7),
        );
      }
      await writable.close();
      return "";
    },
    { world: worldId, name: fingerprint, size: bytes },
  );
}

test.describe("Client world cache — repairing a divergent store (FR-019)", () => {
  test.setTimeout(180_000);

  test("a blob no index row refers to is reclaimed, and an unfinished one is not", async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Repair ${uniqueSuffix()}`,
      "e2erepair",
    );
    const scenes = await sceneIds(page, worldId);
    expect(scenes.length, "a new world should have a scene").toBeGreaterThan(0);
    const assetId = await createCanvasAsset(page, worldId, scenes[0], 3131);
    const fingerprint = await assetFingerprint(page, assetId);

    // A first open, so the world has a real cached asset and a scope
    // directory to plant things in.
    const sync = watchCacheSync(page);
    const first = await openWorldAndSync(page, worldId, sync);
    expect(first.status, JSON.stringify(first)).toBe("synced");
    await expect
      .poll(() => holdsFingerprint(page, worldId, fingerprint), {
        timeout: 60_000,
        message: "the world must be cached before its store can diverge",
      })
      .toBe(true);

    // Now the damage. One complete file nothing refers to — bytes no index
    // row can reach, which is dead weight forever. And one empty file, which
    // is indistinguishable from a write another tab is in the middle of.
    const orphan = fabricatedFingerprint("orphan");
    const unfinished = fabricatedFingerprint("unfinished");
    expect(await plantBlob(page, worldId, orphan, 4096), "planting failed").toBe(
      "",
    );
    expect(
      await plantBlob(page, worldId, unfinished, 0),
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
    const before = sync.count();
    await page.reload();
    const second = await sync.next(before);
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
  });
});
