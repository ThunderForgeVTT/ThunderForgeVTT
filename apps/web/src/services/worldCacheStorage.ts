/**
 * What the client world cache is using on this machine, and taking it back
 * (spec 028, US5, FR-025/FR-026, T064–T066).
 *
 * # Why this is TypeScript and not another wasm export
 *
 * The engine owns the cache, and R1's rule is that policy has one owner — so
 * the instinct is to add a `usage_by_world` to the Rust index and export it.
 * Two things argue against it here.
 *
 * The first is the same reason `worldCache.ts` exists: **the engine is usually
 * not loaded**. A storage screen is reached from settings, on a page that has
 * never mounted a canvas, and downloading a multi-megabyte wasm module to
 * print a number would be absurd. Sign-out already sets this precedent for
 * exactly this reason.
 *
 * The second is that a Rust `usage_by_world` would have had **no caller** —
 * and this spec has now shipped two pieces of well-tested, entirely uninvoked
 * policy (`missing_blobs`/`orphaned_blobs` until FR-019 got its caller, and
 * `limit_bytes`/`plan_eviction` until T061). Adding a third with the same
 * shape, on the same day the second was found, would be choosing the bug.
 *
 * Nothing here is *policy*. It reads sizes off a filesystem and deletes
 * directories. The rules that matter — what may be stored, what goes when
 * space runs short, what a fingerprint means — all stay in Rust, and this
 * file could not express them if it wanted to.
 *
 * # What the figures mean
 *
 * These are **bytes on disk**, read from OPFS: ciphertext, including each
 * blob's nonce and GCM tag. The budget in `budget.rs` accounts in *plaintext*
 * bytes, because that is the unit the server reports sizes in, so the two
 * differ by a small constant per item. That is the right choice for both: a
 * budget compared against server-reported sizes has to speak their unit, and
 * a user asking "how much of my disk is this using" wants the disk's answer.
 *
 * # Nothing here throws
 *
 * A storage screen that cannot read the store should say "nothing stored",
 * not break the page it is on. Every entry point resolves to a usable value —
 * an empty report, or a count of what it managed to delete.
 */

/** Mirrors `DB_NAME`/`STORE_INDEX` in `crates/thunderforge-cache-browser/src/lib.rs`. */
const DB_NAME = "thunderforge-cache";
const STORE_INDEX = "index";

/** What one world is costing on disk. */
export interface WorldUsage {
  worldId: string;
  bytes: number;
  /** Blob files, which is the count the figure is a sum over. */
  blobs: number;
}

/** Everything this profile's cache is holding for one user. */
export interface CacheUsage {
  totalBytes: number;
  worlds: WorldUsage[];
  /** OPFS is unavailable here — not the same as "nothing is stored". */
  unavailable: boolean;
}

/** One blob file found on disk, before it is grouped. */
export interface StoredBlob {
  worldId: string;
  bytes: number;
}

/**
 * The OPFS directory name for a user, mirroring `UserScope::for_user` in
 * `crates/thunderforge-opfs/src/paths.rs`: the first 32 hex characters of
 * SHA-256 over the uuid's **raw 16 bytes**, not its text.
 *
 * Hashed rather than using the uuid directly so a directory listing on a
 * shared machine does not enumerate who has signed in on it. That is
 * obfuscation and not secrecy — the encryption is what protects the content —
 * but it means this function has to agree with the Rust exactly, or the panel
 * reads an empty directory and cheerfully reports that nothing is cached.
 */
export async function userScopeName(userId: string): Promise<string | null> {
  const hex = userId.replace(/-/g, "");
  if (!/^[0-9a-f]{32}$/i.test(hex)) return null;
  const raw = new Uint8Array(16);
  for (let i = 0; i < 16; i += 1) {
    raw[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  const digest = await crypto.subtle.digest("SHA-256", raw);
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("")
    .slice(0, 32);
}

/**
 * Group blobs into per-world totals, largest first.
 *
 * Pure, and separated from the walk for the usual reason: this is the part
 * with arithmetic in it, and it can be tested without a filesystem. Ordering
 * is by size and then by id so the list is stable — a panel whose rows
 * reorder between refreshes because two worlds tie looks broken.
 */
export function summariseUsage(
  blobs: StoredBlob[],
): Omit<CacheUsage, "unavailable"> {
  const byWorld = new Map<string, WorldUsage>();
  for (const blob of blobs) {
    const existing = byWorld.get(blob.worldId);
    if (existing) {
      existing.bytes += blob.bytes;
      existing.blobs += 1;
    } else {
      byWorld.set(blob.worldId, {
        worldId: blob.worldId,
        bytes: blob.bytes,
        blobs: 1,
      });
    }
  }
  const worlds = [...byWorld.values()].sort(
    (a, b) => b.bytes - a.bytes || a.worldId.localeCompare(b.worldId),
  );
  return {
    totalBytes: worlds.reduce((sum, world) => sum + world.bytes, 0),
    worlds,
  };
}

/** Human-readable bytes. Binary units, because storage quotas are quoted in them. */
export function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  const exponent = Math.min(
    units.length - 1,
    Math.floor(Math.log(bytes) / Math.log(1024)),
  );
  const value = bytes / 1024 ** exponent;
  // One decimal below GiB reads as noise; above it, the difference between
  // 1 GiB and 1.7 GiB is the whole point.
  const digits = exponent >= 3 ? 1 : 0;
  return `${value.toFixed(digits)} ${units[exponent]}`;
}

interface DirLike {
  kind: string;
  entries(): AsyncIterable<[string, DirLike]>;
  getDirectoryHandle(
    name: string,
    opts?: { create?: boolean },
  ): Promise<DirLike>;
  getFile(): Promise<File>;
  removeEntry(name: string, opts?: { recursive?: boolean }): Promise<void>;
}

/** The user's scope directory, or `null` if OPFS or the directory is absent. */
async function scopeDirectory(userId: string): Promise<DirLike | null> {
  const scope = await userScopeName(userId);
  if (!scope) return null;
  if (!navigator.storage?.getDirectory) return null;
  try {
    const root = (await navigator.storage.getDirectory()) as unknown as DirLike;
    return await root.getDirectoryHandle(scope);
  } catch {
    // No scope directory means nothing has been cached for this user, which
    // is a legitimate empty answer rather than a failure.
    return null;
  }
}

/**
 * Walk this user's cache and report what each world is costing (FR-025).
 *
 * World ids come from the directory names, which is what
 * `world_dir_name` writes them as. Deliberately not cross-referenced against
 * the index: the question is what is on the *disk*, and a world whose index
 * rows were lost still occupies space and still needs to be clearable.
 */
export async function readCacheUsage(userId: string): Promise<CacheUsage> {
  const scopeDir = await scopeDirectory(userId);
  if (!scopeDir) {
    return {
      totalBytes: 0,
      worlds: [],
      unavailable: !navigator.storage?.getDirectory,
    };
  }

  const blobs: StoredBlob[] = [];
  try {
    for await (const [worldId, worldDir] of scopeDir.entries()) {
      if (worldDir.kind !== "directory") continue;
      for await (const [name, file] of worldDir.entries()) {
        if (file.kind === "directory" || !name.endsWith(".bin")) continue;
        try {
          blobs.push({ worldId, bytes: (await file.getFile()).size });
        } catch {
          // A file that vanished mid-walk — another tab's eviction. It is
          // not occupying space any more, so leaving it out is correct.
        }
      }
    }
  } catch {
    return { totalBytes: 0, worlds: [], unavailable: true };
  }

  return { ...summariseUsage(blobs), unavailable: false };
}

/** Drop every index row belonging to `worldId`, or all of them when null. */
async function clearIndexRows(worldId: string | null): Promise<void> {
  const db = await new Promise<IDBDatabase | null>((resolve) => {
    // Never create the database — see the note in `worldCache.ts`. Opening a
    // version-less handle on a database that does not exist would create an
    // empty one and permanently break the Rust side's own upgrade path.
    const request = indexedDB.open(DB_NAME);
    request.onerror = () => resolve(null);
    request.onsuccess = () => resolve(request.result);
  });
  if (!db) return;
  try {
    if (!db.objectStoreNames.contains(STORE_INDEX)) return;
    await new Promise<void>((resolve) => {
      const tx = db.transaction(STORE_INDEX, "readwrite");
      const store = tx.objectStore(STORE_INDEX);
      tx.oncomplete = () => resolve();
      tx.onerror = () => resolve();
      tx.onabort = () => resolve();
      if (!worldId) {
        store.clear();
        return;
      }
      const cursor = store.openCursor();
      cursor.onsuccess = () => {
        const at = cursor.result;
        if (!at) return;
        // Rows are the JSON `IndexEntry` writes; `world_id` is serde's field
        // name for it. A row we cannot parse is left alone rather than
        // deleted — this operation is scoped to one world, and a row of
        // unknown world is not known to be that one.
        const value = at.value as unknown;
        if (typeof value === "string") {
          try {
            const parsed = JSON.parse(value) as { world_id?: string };
            if (parsed.world_id === worldId) at.delete();
          } catch {
            /* not ours to interpret */
          }
        }
        at.continue();
      };
    });
  } finally {
    db.close();
  }
}

/** What a clear actually managed to do. */
export interface ClearOutcome {
  /** Bytes that were being reported for what was cleared, before it went. */
  freedBytes: number;
  ok: boolean;
}

/**
 * Clear one world's cached content (FR-026).
 *
 * Server-side data is untouched by construction: nothing here makes a network
 * request. The world reloads on the next visit exactly as a never-cached one
 * does, which is the property US5 scenario 2 asks about — clearing a cache is
 * meant to be free of consequence beyond a slower next load.
 *
 * The blobs go first and the index rows second. That order matters: the
 * reverse leaves rows naming files that are gone, which is the *lie* FR-019's
 * repair pass exists to correct — recoverable, but it makes the client claim
 * content it cannot serve until a repair runs. Blobs without rows are merely
 * unreachable bytes, and this deletes them anyway.
 */
export async function clearWorldCache(
  userId: string,
  worldId: string,
): Promise<ClearOutcome> {
  const usage = await readCacheUsage(userId);
  const freedBytes =
    usage.worlds.find((w) => w.worldId === worldId)?.bytes ?? 0;

  const scopeDir = await scopeDirectory(userId);
  let ok = true;
  if (scopeDir) {
    try {
      await scopeDir.removeEntry(worldId, { recursive: true });
    } catch {
      // Already absent is the postcondition, so this is only a failure if
      // something is still there — which the caller's refresh will show.
      ok = false;
    }
  }
  await clearIndexRows(worldId);
  return { freedBytes, ok };
}

/**
 * Clear everything this user has cached (FR-026).
 *
 * Scoped to the signed-in user's directory rather than wiping OPFS: another
 * account on the same machine has its own scope, and its content is not this
 * user's to delete.
 */
export async function clearAllCache(userId: string): Promise<ClearOutcome> {
  const usage = await readCacheUsage(userId);
  const scope = await userScopeName(userId);
  let ok = true;

  if (scope && navigator.storage?.getDirectory) {
    try {
      const root =
        (await navigator.storage.getDirectory()) as unknown as DirLike;
      await root.removeEntry(scope, { recursive: true });
    } catch {
      ok = false;
    }
  }
  await clearIndexRows(null);
  return { freedBytes: usage.totalBytes, ok };
}
